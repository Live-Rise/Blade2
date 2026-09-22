//! 内核 mux 流：`ws://host:port/api/remote.mux`，文本帧、一行一个 JSON。
//!
//! 只实现 `dsh-api-gateway` 实际用到的子集：客户端掩码、服务端不掩码、
//! 帧类型只走 text + continuation + close + ping。
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::kernel::Endpoint;

const PATH: &str = "/api/remote.mux";
const READ_TICK: Duration = Duration::from_millis(120);

#[derive(Clone, Debug, PartialEq)]
pub enum MuxEvent {
    Item {
        stream: String,
        value: Option<Value>,
    },
    End {
        stream: String,
    },
    Failure {
        stream: String,
        message: String,
    },
    Cancelled {
        stream: String,
    },
}

fn now_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18 & 63) as usize] as char);
        out.push(B64[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// 服务端只要求 key 存在并能原样参与 accept 计算，不校验随机性。
fn ws_key(seed: u64) -> String {
    let mut bytes = [0u8; 16];
    let mut state = seed | 1;
    for slot in bytes.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *slot = (state >> 24) as u8;
    }
    base64(&bytes)
}

pub struct Mux {
    stream: TcpStream,
    pending: Vec<u8>,
    fragment: Vec<u8>,
    next_id: u32,
    mask: u64,
}

impl Mux {
    pub fn connect(endpoint: &Endpoint, cookie: Option<&str>) -> Result<Self, String> {
        let mut stream =
            TcpStream::connect((endpoint.host.as_str(), endpoint.port)).map_err(|e| {
                format!(
                    "连接 {host}:{port} 失败: {e}",
                    host = endpoint.host,
                    port = endpoint.port
                )
            })?;
        let request = format!(
            "GET {PATH} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n{cookie}\r\n",
            host = endpoint.host,
            port = endpoint.port,
            key = ws_key(now_nanos()),
            cookie = cookie
                .map(|c| format!("Cookie: {c}\r\n"))
                .unwrap_or_default()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("发送握手失败: {e}"))?;
        stream.flush().map_err(|e| format!("发送握手失败: {e}"))?;

        let mut header = Vec::new();
        let mut byte = [0u8; 1];
        while !header.ends_with(b"\r\n\r\n") {
            if stream
                .read(&mut byte)
                .map_err(|e| format!("读取握手失败: {e}"))?
                == 0
            {
                return Err("服务端在握手中途关闭连接".to_string());
            }
            header.push(byte[0]);
            if header.len() > 8192 {
                return Err("握手响应头过大".to_string());
            }
        }
        let text = String::from_utf8_lossy(&header);
        if !text.starts_with("HTTP/1.1 101") && !text.starts_with("HTTP/1.0 101") {
            let first = text.lines().next().unwrap_or_default().to_string();
            return Err(format!("握手被拒绝: {first}"));
        }
        stream
            .set_read_timeout(Some(READ_TICK))
            .map_err(|e| format!("无法设置读超时: {e}"))?;
        Ok(Self {
            stream,
            pending: Vec::new(),
            fragment: Vec::new(),
            next_id: 0,
            mask: now_nanos(),
        })
    }

    fn next_mask(&mut self) -> [u8; 4] {
        self.mask ^= self.mask << 13;
        self.mask ^= self.mask >> 7;
        self.mask ^= self.mask << 17;
        [
            (self.mask >> 40) as u8,
            (self.mask >> 28) as u8,
            (self.mask >> 16) as u8,
            (self.mask >> 4) as u8,
        ]
    }

    fn send_text(&mut self, payload: &str) -> Result<(), String> {
        let bytes = payload.as_bytes();
        let mut frame = vec![0x81];
        match bytes.len() {
            0..=125 => frame.push(0x80 | bytes.len() as u8),
            126..=0xffff => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            }
            len => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        let mask = self.next_mask();
        frame.extend_from_slice(&mask);
        frame.extend(bytes.iter().enumerate().map(|(i, b)| b ^ mask[i & 3]));
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("发送帧失败: {e}"))?;
        self.stream.flush().map_err(|e| format!("发送帧失败: {e}"))
    }

    /// 打开一个业务流；`args` 是 `{"args": …}` 里的内容，主干用 `{}` 表示零参。
    pub fn open(&mut self, endpoint: &str, args: Value) -> Result<String, String> {
        self.next_id += 1;
        let id = format!("rs{}", self.next_id);
        self.send_text(
            json!({
                "type": "open",
                "streamId": id,
                "endpoint": endpoint,
                "payload": { "args": args },
            })
            .to_string()
            .as_str(),
        )?;
        Ok(id)
    }

    pub fn cancel(&mut self, stream_id: &str) -> Result<(), String> {
        self.send_text(
            json!({ "type": "cancel", "streamId": stream_id })
                .to_string()
                .as_str(),
        )
    }

    /// 打开主干那条长驻「会话控制面」流（`MainWindow.xaml.cs:15201` 的
    /// `OpenRemoteStreamAsync("session/control", new { }, OnControlFrame)`）。
    ///
    /// `args` 必须是**字面量空对象** `{}`：该端点的内核描述符是 `mode="stream"` 且零参数，
    /// gateway 的 `assertExactArguments` 对多出来的键一律拒（与 `$events`、`workspace/follow`
    /// 同一条约束，也即项目记忆里「follow 帧 `args` 为 `{}` 有约束」那条）。
    ///
    /// 发起时机照主干：mux 连上、`workspace/follow` 与 `$events` 之后一次
    /// （`MainWindow.xaml.cs:2622`），切会话时幂等补开一次（`MainWindow.xaml.cs:4121`），
    /// mux 重连后重开（`MainWindow.xaml.cs:5375`、`MainWindow.ReconnectProbe.cs:35`）。
    /// 帧的落地解析在 `crate::kernel::ControlState::apply`。
    pub fn open_session_control(&mut self) -> Result<String, String> {
        self.open("session/control", json!({}))
    }

    /// 非阻塞取事件：先把可读数进缓冲，再切出所有完整帧。
    pub fn poll(&mut self) -> Result<Vec<MuxEvent>, String> {
        let mut chunk = [0u8; 4096];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err("连接已被服务端关闭".to_string()),
                Ok(n) => self.pending.extend_from_slice(&chunk[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => break,
                Err(e) => return Err(format!("读帧失败: {e}")),
            }
        }
        let mut events = Vec::new();
        while let Some(frame) = take_frame(&mut self.pending)? {
            if let Some(event) = self.consume(frame)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    fn consume(&mut self, frame: Frame) -> Result<Option<MuxEvent>, String> {
        match frame.opcode {
            0x8 => Err("服务端关闭了 mux 连接".to_string()),
            0x0 if !frame.is_last => {
                self.fragment.extend_from_slice(&frame.payload);
                Ok(None)
            }
            0x0 => {
                self.fragment.extend_from_slice(&frame.payload);
                let message = std::mem::take(&mut self.fragment);
                parse_message(&message)
            }
            0x1 if frame.is_last => parse_message(&frame.payload),
            0x1 => {
                self.fragment = frame.payload;
                Ok(None)
            }
            0x9 | 0xa => Ok(None),
            other => Err(format!("不支持的帧类型: {other:#x}")),
        }
    }

    /// 阻塞轮询直到拿到一个事件或 `timeout` 到期；`None` 表示只是超时。
    pub fn wait(&mut self, timeout: Duration) -> Result<Option<MuxEvent>, String> {
        Ok(self.collect(timeout)?.into_iter().next())
    }

    /// 后台泵：最多等 `patience`，期间攒到的事件一次性带回；空 Vec 只是超时。
    pub fn collect(&mut self, patience: Duration) -> Result<Vec<MuxEvent>, String> {
        let deadline = std::time::Instant::now() + patience;
        let mut events = Vec::new();
        loop {
            events.extend(self.poll()?);
            if !events.is_empty() || std::time::Instant::now() >= deadline {
                return Ok(events);
            }
            std::thread::sleep(Duration::from_millis(60));
        }
    }
}

#[derive(Debug)]
struct Frame {
    is_last: bool,
    opcode: u8,
    payload: Vec<u8>,
}

fn take_frame(buffer: &mut Vec<u8>) -> Result<Option<Frame>, String> {
    if buffer.len() < 2 {
        return Ok(None);
    }
    let first = buffer[0];
    let second = buffer[1];
    if second & 0x80 != 0 {
        return Err("本客户端不接受掩码的服务端帧".to_string());
    }
    let mut offset = 2usize;
    let length = match second & 0x7f {
        n @ 0..=125 => usize::from(n),
        126 => {
            if buffer.len() < offset + 2 {
                return Ok(None);
            }
            let value = u16::from_be_bytes([buffer[offset], buffer[offset + 1]]) as usize;
            offset += 2;
            value
        }
        _ => {
            if buffer.len() < offset + 8 {
                return Ok(None);
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&buffer[offset..offset + 8]);
            offset += 8;
            usize::try_from(u64::from_be_bytes(bytes))
                .map_err(|_| "帧长度超出支持范围".to_string())?
        }
    };
    if buffer.len() < offset + length {
        return Ok(None);
    }
    let payload = buffer[offset..offset + length].to_vec();
    buffer.drain(..offset + length);
    Ok(Some(Frame {
        is_last: first & 0x80 != 0,
        opcode: first & 0x0f,
        payload,
    }))
}

fn parse_message(payload: &[u8]) -> Result<Option<MuxEvent>, String> {
    let text = String::from_utf8_lossy(payload);
    let value: Value = serde_json::from_str(&text).map_err(|e| format!("帧不是 JSON: {e}"))?;
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return Ok(None);
    };
    let stream = value
        .get("streamId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let event = match kind {
        "item" => MuxEvent::Item {
            // void 结果的 `value` 键会整个缺席。
            stream,
            value: value.get("value").cloned(),
        },        "end" => MuxEvent::End { stream },
        "error" => MuxEvent::Failure {
            message: value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("未知流错误")
                .to_string(),
            stream,
        },
        "cancel" => MuxEvent::Cancelled { stream },
        _ => return Ok(None),
    };
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(text: &str) -> Option<MuxEvent> {
        parse_message(text.as_bytes()).expect("帧应能解析")
    }

    /// 流元素的四种落地：`item`（带值 / void 值键整个缺席）、`end`、`error`、`cancel`。
    /// void 那一支在 ipc 侧原本挂在 `session/control` 的一次性桩上；该桩照主干改成
    /// 长驻流之后，这条形状改由这里守住（假内核仍按线上形状发 `{type:"item",streamId}`）。
    #[test]
    fn stream_frames_map_onto_events_and_void_omits_the_value_key() {
        assert_eq!(
            message(r#"{"type":"item","streamId":"rs1","value":{"type":"baseline"}}"#),
            Some(MuxEvent::Item {
                stream: "rs1".into(),
                value: Some(json!({"type": "baseline"})),
            })
        );
        assert_eq!(
            message(r#"{"type":"item","streamId":"rs1"}"#),
            Some(MuxEvent::Item {
                stream: "rs1".into(),
                value: None,
            })
        );
        assert_eq!(
            message(r#"{"type":"end","streamId":"rs1"}"#),
            Some(MuxEvent::End { stream: "rs1".into() })
        );
        assert_eq!(
            message(r#"{"type":"cancel","streamId":"rs2"}"#),
            Some(MuxEvent::Cancelled { stream: "rs2".into() })
        );
        assert_eq!(
            message(r#"{"type":"error","streamId":"rs3","error":{"message":"未知端点"}}"#),
            Some(MuxEvent::Failure {
                stream: "rs3".into(),
                message: "未知端点".into(),
            })
        );
    }

    /// 没有 `type`、`type` 不是这四型的一律静默忽略（心跳、网关的 ready/pong 这类），
    /// 绝不能把它们当成某个流的元素塞给上层。
    #[test]
    fn frames_without_a_known_type_are_dropped() {
        assert_eq!(message(r#"{"streamId":"rs1"}"#), None);
        assert_eq!(message(r#"{"type":"pong","streamId":"rs1"}"#), None);
        assert!(parse_message(b"not json").is_err());
    }

    /// 长帧（>125 字节走 126 那一档）的分帧：`take_frame` 只在整帧齐了才吐，
    /// 半截帧留在缓冲里等下一批字节。
    #[test]
    fn split_frames_are_buffered_until_complete() {
        let mut body = json!({
            "type": "item",
            "streamId": "rs1",
            "value": { "text": "x".repeat(200) },
        })
        .to_string()
        .into_bytes();
        assert!(body.len() > 125, "这一档要落到 126 的扩展长度上");
        let mut wire = vec![0x81, 126];
        wire.extend_from_slice(&(body.len() as u16).to_be_bytes());
        wire.append(&mut body);

        let mut buffer = wire[..10].to_vec();
        assert!(
            take_frame(&mut buffer).unwrap().is_none(),
            "连长度字段都没齐，不该吐帧"
        );
        buffer.extend_from_slice(&wire[10..wire.len() - 5]);
        assert!(
            take_frame(&mut buffer).unwrap().is_none(),
            "正文还差一截，不该吐帧"
        );
        buffer.extend_from_slice(&wire[wire.len() - 5..]);
        let frame = take_frame(&mut buffer).unwrap().expect("整帧齐了就该吐");
        assert_eq!(frame.opcode, 1);
        assert!(frame.is_last);
        assert!(buffer.is_empty(), "吐完帧要把缓冲清空");
        assert!(matches!(
            parse_message(&frame.payload).unwrap(),
            Some(MuxEvent::Item { .. })
        ));
    }
}
