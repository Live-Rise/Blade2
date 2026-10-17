//! 内核 mux 流：`ws://host:port/api/remote.mux`，文本帧、一行一个 JSON。
//!
//! 只实现 `dsh-api-gateway` 实际用到的子集：客户端掩码、服务端不掩码、
//! 帧类型只走 text + continuation + close + ping（服务端 Ping 必须回 Pong，见 `send_pong`）。
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::kernel::Endpoint;

const PATH: &str = "/api/remote.mux";
const READ_TICK: Duration = Duration::from_millis(120);

/// 握手期（升级请求写出去 ⇒ 101 响应头收齐）的**总读预算**。
///
/// 取值出处 = 主干引导链里 mux 那一轮的单轮弃连时限，`MainWindow.xaml.cs` 逐字
/// `attemptCts.CancelAfter(TimeSpan.FromSeconds(2));`（其注释自陈「单轮 2s 弃连重试，网关挂载
/// 完成即成功；总窗口 60s 封顶」）。分叉的「弃连重试」不在本文件：那族是 `main.rs` 的
/// `arm_mux_reconnect`/`Msg::MuxReconnectTick`（`RECONNECT_INITIAL_DELAY` 500ms 翻倍封顶
/// `RECONNECT_MAX_DELAY` 30s、`RECONNECT_MAX_ATTEMPTS`），它只在**拿到 `MuxReady(Err)`** 时才被唤起。
/// ⇒ 本文件要做的就是把主干那一轮的 2s 原样搬过来，让挂死的拨号变成 `Err`，交给上面那族接住；
/// 分叉既有的 `READ_TICK`（120ms）是常驻轮询节拍、不是握手预算（`kernel.rs` 的
/// `rpc_request_timeout_matches_mainline_http_client_120s` 已把这条口径写死），故不复用。
const HANDSHAKE_BUDGET: Duration = Duration::from_secs(2);

/// 头读循环的一格判定：已读到的字节 + 已耗时 ⇒ 继续 / 成功 / 超时。
/// 抽成纯函数是为了把「有界」这条判据离线钉死（真 socket 那发见测试）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandshakeStep {
    /// 头还没读全、预算还有：继续读，`remaining` 是这一发 `read` 该设的超时。
    Keep { remaining: Duration },
    /// 头已读全（以 `CRLFCRLF` 收尾）——即便此刻已经越过死线也算成功，应答到手了。
    Done,
    /// 预算耗尽而头仍未读全：判超时。
    Expired,
}

fn handshake_step(header: &[u8], elapsed: Duration, budget: Duration) -> HandshakeStep {
    if header.ends_with(b"\r\n\r\n") {
        return HandshakeStep::Done;
    }
    if elapsed >= budget {
        return HandshakeStep::Expired;
    }
    HandshakeStep::Keep {
        remaining: budget - elapsed,
    }
}

/// 内核侧那条常驻事件流的端点名（网关按字面量记账，见 `Mux::open_events`）。
pub const EVENTS_ENDPOINT: &str = "$events";

/// 一发 `open` 的线上形状：`{type,streamId,endpoint,payload:{args}}`，四键之外不许有第五键
/// （`parseRemoteStreamClientMessage` 用 `exactKeys` 硬校验）。
fn open_frame(stream_id: &str, endpoint: &str, args: &Value) -> Value {
    json!({
        "type": "open",
        "streamId": stream_id,
        "endpoint": endpoint,
        "payload": { "args": args },
    })
}

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
        Self::connect_with_budget(endpoint, cookie, HANDSHAKE_BUDGET)
    }

    /// `connect` 的**可注入预算**版：预算只作用在握手期那一圈，常驻档位恒为 `READ_TICK`。
    /// 留这一格是为了让「超时 ⇒ Err 且回得及时」能在 127.0.0.1 上现起的小监听上离线验掉
    /// （真连内核那条路不在本仓门禁里），而不必把单测拖到 2s。
    pub fn connect_with_budget(
        endpoint: &Endpoint,
        cookie: Option<&str>,
        budget: Duration,
    ) -> Result<Self, String> {
        let mut stream =
            TcpStream::connect((endpoint.host.as_str(), endpoint.port)).map_err(|e| {
                format!(
                    "连接 {host}:{port} 失败: {e}",
                    host = endpoint.host,
                    port = endpoint.port
                )
            })?;
        // 死线从「拿到 socket」起算，覆盖写请求 + 读应答整轮（主干那一轮的 `CancelAfter(2s)`
        // 罩的也是 `ConnectAsync` 整发，不是只罩读）。
        let started = Instant::now();
        // 握手期的读超时**必须**在进入头读循环、甚至写升级请求之前落到 socket 上。
        // 这一发就是 #194 要补的洞：原先 `set_read_timeout` 排在 101 校验之后，于是「内核那端口
        // accept 了 TCP 却迟迟不回 101」时，下面那发逐字节 `read` 永久阻塞 ⇒ 本函数既不回 Ok
        // 也不回 Err ⇒ `open_mux` 的闭包走不到任何一条 `Msg::MuxReady` ⇒ 整条引导链挂死。
        stream
            .set_read_timeout(Some(budget))
            .map_err(|e| format!("无法设置握手读超时: {e}"))?;
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
        loop {
            match handshake_step(&header, started.elapsed(), budget) {
                HandshakeStep::Done => break,
                HandshakeStep::Expired => {
                    // 定性要说清是「握手超时」而不是「连接失败」：TCP 已经通了，卡的是应答。
                    // 主干同处的口径是「弃这一轮、重试下一轮」，分叉的下一轮由 `MuxReady(Err)`
                    // 唤起 `arm_mux_reconnect` ⇒ 这一发必须是 Err，且不许往 `Ok` 那一支漏。
                    return Err(format!(
                        "mux 握手超时：{host}:{port} 已接受 TCP，但 {ms} 毫秒内没回 101 升级响应头（网关可能尚未挂载完成）",
                        host = endpoint.host,
                        port = endpoint.port,
                        ms = budget.as_millis(),
                    ));
                }
                HandshakeStep::Keep { remaining } => {
                    stream
                        .set_read_timeout(Some(remaining))
                        .map_err(|e| format!("无法设置握手读超时: {e}"))?;
                    match stream.read(&mut byte) {
                        Ok(0) => return Err("服务端在握手中途关闭连接".to_string()),
                        Ok(_) => header.push(byte[0]),
                        // 读不到字节不是「读取失败」：交回循环，由 `handshake_step` 判成 Expired。
                        // 两型都收（与 `poll` 同一口径：Windows 上「socket 读超时」的映射在
                        // `TimedOut`/`WouldBlock` 之间历来不稳，只认一型就是赌平台）。
                        Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
                            continue
                        }
                        Err(e) => return Err(format!("读取握手失败: {e}")),
                    }
                    if header.len() > 8192 {
                        return Err("握手响应头过大".to_string());
                    }
                }
            }
        }
        let text = String::from_utf8_lossy(&header);
        if !text.starts_with("HTTP/1.1 101") && !text.starts_with("HTTP/1.0 101") {
            let first = text.lines().next().unwrap_or_default().to_string();
            return Err(format!("握手被拒绝: {first}"));
        }
        // 握手成功之后才把读超时换回常驻档位 `READ_TICK`（`poll` 靠它把「这一拍没字节」
        // 当成正常空转）。两档不许互相漏：引导期档留在常驻期会让每次 `poll` 白等 2s，
        // 常驻档漏进引导期就是本次这条挂死路径。
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

    /// 拼一笔客户端帧：FIN + `opcode` + 掩码长度 + 掩码 + 掩码后 payload。
    /// 拆成纯函数（掩码由调用方给）是为了让「线上形状」能被离线钉住——opcode 与掩码位
    /// 各是一个字节，写歪了服务端只会看到「连接被关」而无从归因。
    fn masked_frame(opcode: u8, payload: &[u8], mask: [u8; 4]) -> Vec<u8> {
        let mut frame = vec![0x80 | opcode];
        match payload.len() {
            0..=125 => frame.push(0x80 | payload.len() as u8),
            126..=0xffff => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
            }
            len => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i & 3]));
        frame
    }

    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        let frame = Self::masked_frame(opcode, payload, self.next_mask());
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("发送帧失败: {e}"))?;
        self.stream
            .flush()
            .map_err(|e| format!("发送帧失败: {e}"))
    }

    fn send_text(&mut self, payload: &str) -> Result<(), String> {
        self.send_frame(0x1, payload.as_bytes())
    }

    /// 回一笔 Pong，payload 原样带回（RFC 6455 §5.5.2：收到 Ping 必须尽快回 Pong）。
    ///
    /// 这条不是可选礼貌，是**内核的存亡线**：`dsh-api-gateway` 的 `RemoteStreamMuxServer`
    /// 每 `websocketHeartbeatIntervalMs`（默认 `DEFAULT_WEBSOCKET_HEARTBEAT_INTERVAL_MS` = 2s）
    /// 给每个 socket 发一笔 Ping，只有收到 Pong 才把 `missedHeartbeats` 清零，攒到
    /// `MAX_MISSED_HEARTBEATS`（2）就 `socket.terminate()`。主干靠 .NET `ClientWebSocket`
    /// 内建的自动回 Pong 活着（`DshRpcClient.cs` 里搜不到一个 Ping/Pong 字样）；分叉的 ws
    /// 客户端是手写的，.NET 那份内建行为得自己补。0.8.3.9 之前分叉把收到的 Ping 静默丢掉，
    /// 实测每 6 秒（3 个心跳间隔）被服务端踢一次 socket，诊断里就是
    /// 「工作区流中断: 连接已被服务端关闭 ⇒ 重连中 1/12」无限循环。
    fn send_pong(&mut self, payload: &[u8]) -> Result<(), String> {
        self.send_frame(0xa, payload)
    }

    /// 打开一个业务流；`args` 是 `{"args": …}` 里的内容，主干用 `{}` 表示零参。
    pub fn open(&mut self, endpoint: &str, args: Value) -> Result<String, String> {
        self.next_id += 1;
        let id = format!("rs{}", self.next_id);
        self.send_text(open_frame(&id, endpoint, &args).to_string().as_str())?;
        Ok(id)
    }

    pub fn cancel(&mut self, stream_id: &str) -> Result<(), String> {
        self.send_text(
            json!({ "type": "cancel", "streamId": stream_id })
                .to_string()
                .as_str(),
        )
    }

    /// 打开 `$events` 那条常驻事件流（#74 的审批 / 提问帧都从这里来）。
    ///
    /// 编号口径（`Mux::open` 的 `next_id` 从 1 起自增，每个实例一套、重连即重新编起）：
    /// `open_mux` 那三发的顺序是 workspace/follow = rs1、**$events = rs2**、
    /// session/control = rs3。rs2 只是取证时好读，**分流判据一律是 open 返回的 streamId**
    /// （`DshRpcClient.cs:313` 也是把 id 记进 `_eventsStreamId`，不拿端点名比帧）。
    ///
    /// `args` 必须逐字是 `{}`：`openRemoteEvents` 对多出来的一个键都回
    /// `gateway/arguments-invalid`。流的第一元素是 `ready`，其 `clientId` 是本代次唯一
    /// 可用的回帧身份（`crate::kernel::InteractionLedger`）。
    pub fn open_events(&mut self) -> Result<String, String> {
        self.open(EVENTS_ENDPOINT, json!({}))
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
            // 服务端 Ping ⇒ 当场回 Pong（payload 原样带回）。内核每 2s 一跳、攒 2 次未回就
            // `terminate()`，不回这条的客户端在这个内核上活不过 3 跳；主干的内建行为在 .NET
            // 的 `ClientWebSocket` 里，分叉手写客户端必须自己补（见 `send_pong` 的台账）。
            // 控制帧不得分片（RFC 6455 §5.5），这里拿到的就是完整一笔。
            0x9 => {
                self.send_pong(&frame.payload)?;
                Ok(None)
            }
            // 服务端自发（未被请求）的 Pong：无动作。承载的 payload 在协议上没有约定用途。
            0xa => Ok(None),
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

    /// `$events` 的 `open` 帧形状：四键 + `args` 逐字 `{}`（多一个键网关就回
    /// `gateway/arguments-invalid`），端点名走常量而不是散落字面量。
    #[test]
    fn events_open_frame_carries_exactly_four_keys_and_empty_args() {
        let frame = open_frame("rs2", EVENTS_ENDPOINT, &json!({}));
        assert_eq!(
            frame,
            json!({
                "type": "open",
                "streamId": "rs2",
                "endpoint": "$events",
                "payload": { "args": {} },
            })
        );
        let mut keys: Vec<&str> = frame
            .as_object()
            .expect("对象")
            .keys()
            .map(|key| key.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["endpoint", "payload", "streamId", "type"]);
        // 会话控制面那发同形状：零参。
        assert_eq!(
            open_frame("rs3", "session/control", &json!({}))["payload"]["args"],
            json!({})
        );
    }

    /// 流元素的 `value` 在 mux 层**一个字都不动**：`ready`/`cancel`/`waterfall` 都得原样
    /// 交给上层按 streamId 分流（帧型解译在 `crate::kernel::parse_events_frame`）。
    #[test]
    fn events_stream_elements_pass_through_untouched() {
        for value in [
            json!({ "type": "ready", "clientId": "c-1", "host": "dsh" }),
            json!({ "type": "cancel", "eventId": "e-1" }),
            json!({
                "type": "waterfall", "event": "approval/request", "eventId": "e-2",
                "agentId": "s-1", "request": { "toolName": "bash", "reason": "escalate" }
            }),
        ] {
            let frame = json!({ "type": "item", "streamId": "rs2", "value": value }).to_string();
            assert_eq!(
                message(frame.as_str()),
                Some(MuxEvent::Item {
                    stream: "rs2".into(),
                    value: Some(value),
                })
            );
        }
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

    // ------------------------------------------------------------------ #194 B1：握手期读预算
    //
    // 三层各管一段，都不连真内核：① `handshake_step` 的三态判据（离线纯函数，方向唯一）；
    // ② 源码形状锁（`set_read_timeout` 的**位置**——正向与反向两头）；③ 真 socket 那发，
    // 在 127.0.0.1 上现起一颗「只 listen 不 accept」/「accept 后只回 404」/「回 101」的小监听，
    // 验「预算内回 Err」「不吞 Err」「回了 101 才恢复常驻档位」。`tests/ipc.rs` 那批发真连
    // `fake_dsh` 的用例走的是 `Mux::connect`（默认 2s 预算），形状不变、不受本批影响。

    use std::net::TcpListener;

    /// 一颗**不回任何字节**的本机监听：`bind` 之后不 `accept`，内核的 TCP 栈仍会把连接收进
    /// 积压队列 ⇒ 客户端 `TcpStream::connect` 成功、那发 `read` 永远等不到东西。
    /// 这正是 #194 的挂起现场（慢启动 / 被防火墙半拦 / `bin.js` 提前播报 URL 后还没 listen 完）。
    fn silent_listener() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("127.0.0.1 现起监听应能成");
        let port = listener.local_addr().expect("读回本机监听端口").port();
        (listener, port)
    }

    /// 一颗 accept 后**原样吐 `reply`** 的本机监听，之后一直读到对端关闭（提前 drop 会让
    /// 读侧看到中途 EOF，分不清是「应答被拒」还是「连接被掐」）。
    fn replying_listener(reply: &'static [u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("127.0.0.1 现起监听应能成");
        let port = listener.local_addr().expect("读回本机监听端口").port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(reply);
                let _ = stream.flush();
                let mut sink = [0u8; 1];
                while stream.read(&mut sink).unwrap_or(0) != 0 {}
            }
        });
        port
    }

    /// `replying_listener` 的反面：只回**半截头**（没有结尾空行 ⇒ 握手永远没收下），
    /// 随后**只关写方向** ⇒ 客户端那发 `read` 看到的是 FIN（`Ok(0)`），也就是「头没写完就被收走」
    /// 那一臂的形状。注：这里不能直接 `drop` —— 本地还压着没读的升级请求，drop 会发 RST，
    /// 客户端拿到的就是硬错误（`读取握手失败`）而不是 EOF，两条臂会被混成一团。
    fn half_header_then_close_listener(half: &'static [u8]) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("127.0.0.1 现起监听应能成");
        let port = listener.local_addr().expect("读回本机监听端口").port();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(half);
                let _ = stream.flush();
                let _ = stream.shutdown(std::net::Shutdown::Write);
                let mut sink = [0u8; 1];
                while stream.read(&mut sink).unwrap_or(0) != 0 {}
            }
        });
        port
    }

    /// 一颗 accept 后**先回 101、再发一笔不掩码 Ping**（服务端帧）、然后把客户端的第一帧
    /// 去掩码后交回主线程断言的假内核——形状照 `dsh-api-gateway` 的
    /// `RemoteStreamMuxServer.startHeartbeat`：Ping 由服务端发起、服务端帧不掩码。
    /// 读侧带 2s 超时：客户端不回帧 ⇒ 主侧 `recv_timeout` 判红，门禁不会被挂住。
    fn ping_then_read_listener(
        ping: &'static [u8],
    ) -> (u16, std::sync::mpsc::Receiver<([u8; 2], Vec<u8>)>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("127.0.0.1 现起监听应能成");
        let port = listener.local_addr().expect("读回本机监听端口").port();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("假内核设读超时应能成");
            let mut header = Vec::new();
            let mut byte = [0u8; 1];
            while !header.ends_with(b"\r\n\r\n") {
                match stream.read(&mut byte) {
                    Ok(0) => return,
                    Ok(_) => header.push(byte[0]),
                    Err(_) => return,
                }
            }
            let _ = stream.write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
            );
            // FIN + opcode 0x9；服务端帧不掩码（第二字节高位为 0）。payload 走短长度，
            // 前提是各测试传进来的都 ≤ 125——Ping 的 payload 本来也无语义。
            let _ = stream.write_all(&[0x89, ping.len() as u8]);
            let _ = stream.write_all(ping);
            let mut head = [0u8; 2];
            if stream.read_exact(&mut head).is_err() {
                return;
            }
            let len = (head[1] & 0x7f) as usize;
            let mut rest = vec![0u8; 4 + len];
            if stream.read_exact(&mut rest).is_err() {
                return;
            }
            let mask = [rest[0], rest[1], rest[2], rest[3]];
            let payload: Vec<u8> = rest[4..]
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ mask[i & 3])
                .collect();
            let _ = tx.send((head, payload));
            // 回帧取证完不许就走：线程一退 socket 就被 drop，客户端下一拍 `poll` 读到 EOF
            // 会当成「连接被服务端关闭」报 Err，把「回了 Pong」这条正路染成红色。
            // 与别的假监听同一形状：读到客户端关为止。
            let mut sink = [0u8; 1];
            while stream.read(&mut sink).unwrap_or(0) != 0 {}
        });
        (port, rx)
    }

    /// 带**看门狗**的拨号：把 `connect` 丢到别的线程，主侧只等 `patience`。
    /// 等不到结果 ⇒ 直接判红并点名「挂起」。这条是有意写成 panic 而不是 skip：
    /// 「永不超时」那发变异若让这里也一起挂住，整张门禁就没了信号（本仓刚因静默失效吃过亏），
    /// 所以宁可用一条有界的看门狗把「挂起」本身变成一次可观察的红。
    fn connect_watchdog(
        port: u16,
        budget: Duration,
        patience: Duration,
    ) -> (Result<Mux, String>, Duration) {
        let endpoint = Endpoint {
            host: "127.0.0.1".to_string(),
            port,
            query: None,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(Mux::connect_with_budget(&endpoint, None, budget));
        });
        let started = Instant::now();
        match rx.recv_timeout(patience) {
            Ok(outcome) => (outcome, started.elapsed()),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!(
                "握手在 {} 毫秒内既没回 Ok 也没回 Err ⇒ 引导链永久挂起（#194 的债）",
                patience.as_millis()
            ),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("拨号线程带着 panic 死了，一条出口都没交回来")
            }
        }
    }

    /// 三态判据的方向唯一：头齐 ⇒ Done（**先看头再看死线**，应答到手了就不该反判超时）、
    /// 头没齐且预算见底 ⇒ Expired、否则 Keep 且 `remaining` 恰为「预算 - 已耗时」。
    #[test]
    fn handshake_step_three_way_choice_is_unique() {
        let budget = Duration::from_millis(200);
        assert_eq!(
            handshake_step(b"", Duration::ZERO, budget),
            HandshakeStep::Keep { remaining: budget },
            "空头 + 零耗时 = 从头到尾一整格预算"
        );
        assert_eq!(
            handshake_step(b"HTTP/1.1 101 Sw", Duration::from_millis(50), budget),
            HandshakeStep::Keep {
                remaining: Duration::from_millis(150)
            },
            "半截头：继续，且这一发 read 的超时必须等于剩余预算"
        );
        assert_eq!(
            handshake_step(b"HTTP/1.1 101\r\n\r\n", Duration::from_millis(9999), budget),
            HandshakeStep::Done,
            "头读全了就算成功，哪怕已经越过死线"
        );
        assert_eq!(
            handshake_step(b"HTTP/1.1 101", budget, budget),
            HandshakeStep::Expired,
            "边界取 >=：刚好见底不许再多读一发"
        );
        assert_eq!(
            handshake_step(b"H", budget * 3, budget),
            HandshakeStep::Expired
        );
    }

    /// `\r\n\r\n` 只有落在**末尾**才算头齐：中间出现（某行自带）或只到半个 `\r\n` 都算没读完。
    #[test]
    fn handshake_step_only_reports_done_on_a_trailing_blank_line() {
        let budget = Duration::from_millis(200);
        for partial in [
            &b"HTTP/1.1 101 Switching Protocols\r\n"[..],
            &b"HTTP/1.1 101\r\n\r\nX"[..],
            &b"HTTP/1.1 101\r\n"[..],
            &b"Upgrade: websocket\r\n"[..],
        ] {
            assert_eq!(
                handshake_step(partial, Duration::from_millis(10), budget),
                HandshakeStep::Keep {
                    remaining: Duration::from_millis(190)
                },
                "头没落到 CRLFCRLF 收尾，就必须继续读而不是判成功"
            );
        }
    }

    /// 握手预算的出处锁：主干 `MainWindow.xaml.cs` 引导链 mux 那一轮逐字
    /// `attemptCts.CancelAfter(TimeSpan.FromSeconds(2));` ⇒ 分叉取同一个 2s，不自造新数。
    /// 顺手钉一条「握手预算与常驻 `READ_TICK` 是两颗不同的数」，防止哪天有人把它们并成一颗。
    #[test]
    fn handshake_budget_is_the_mainline_single_round_two_seconds() {
        assert_eq!(HANDSHAKE_BUDGET, Duration::from_secs(2));
        assert_ne!(HANDSHAKE_BUDGET, READ_TICK);
    }

    /// 源码形状锁（正向 + 反向）：**引导期档位**的 `set_read_timeout` 必须排在把升级请求写出去
    /// **之前**（这就是 PP3 K1a 给的正向锁；挪回写请求之后就还原成今日欠账 ⇒ 本条必红），
    /// 头读循环必须走 `handshake_step` 而不是裸 `while !header.ends_with(...)`，
    /// **常驻档位**必须排在 101 校验**之后**（两头的顺序都是这条债的一部分）。
    #[test]
    fn handshake_deadline_is_set_before_the_request_and_the_resident_tick_returns_after_101() {
        const SELF_SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/mux.rs"));
        let start = SELF_SRC
            .find("pub fn connect_with_budget(")
            .expect("预算版拨号入口还在");
        let body = &SELF_SRC[start..start + SELF_SRC[start..].find("fn next_mask(&mut self)").expect("下一个函数")];

        let boot_tick = body
            .find("set_read_timeout(Some(budget))")
            .expect("握手期读超时那一发不见了");
        let write = body
            .find("write_all(request.as_bytes())")
            .expect("升级请求写不出那一发不见了");
        let step = body
            .find("handshake_step(&header, started.elapsed(), budget)")
            .expect("头读循环没有走三态判据");
        let reject = body.find("\"HTTP/1.1 101\"").expect("101 校验不见了");
        let resident = body
            .find("set_read_timeout(Some(READ_TICK))")
            .expect("常驻档位那一发不见了");

        assert!(
            boot_tick < write,
            "引导期读超时必须在写升级请求之前落到 socket 上（否则慢应答那发 read 依旧无界）"
        );
        assert!(write < step, "死线要罩住写 + 读整轮：判据循环必须在写请求之后");
        assert!(step < reject, "先收齐头，再判 101");
        assert!(reject < resident, "只有握手成功才把档位换回常驻 READ_TICK");
        assert_eq!(
            body.matches("set_read_timeout(Some(budget))").count(),
            1,
            "引导期档位只许在进循环前置一次，散落第二次就是两档混用"
        );
        assert_eq!(
            body.matches("set_read_timeout(Some(READ_TICK))").count(),
            1,
            "常驻档位只在 101 之后恢复一次"
        );
        // 反向锁一：不许出现「不设超时直接 read」的裸循环（今日欠账的形状）。
        assert!(
            !body.contains("while !header.ends_with(b\"\\r\\n\\r\\n\")"),
            "头读循环又退回没有死线的裸 while 了"
        );
        assert!(
            !body.contains("set_read_timeout(None)"),
            "握手期不许出现「取消读超时」的形状"
        );
        // 反向锁二：超时那一臂必须往 Err 走，不许漏进 Ok 那一支。
        let expired = body
            .find("HandshakeStep::Expired =>")
            .expect("没有超时臂 ⇒ 永不超时");
        let after_expired = &body[expired..];
        let keep = after_expired
            .find("HandshakeStep::Keep")
            .expect("超时臂之后应还有继续读的臂");
        let arm = &after_expired[..keep];
        assert!(
            arm.contains("return Err(") && !arm.contains("Ok(Self"),
            "握手超时必须是 Err（吞成 Ok = 引导链拿到一根没有 101 的 socket）"
        );
    }

    /// 真 socket 那发（超时方向）：一颗**只 listen 不 accept** 的本地监听。
    /// 判据两条：① 必须回 **Err**，且话里说清是「握手超时」而不是「连接失败」；
    /// ② 必须在预算内回（不早于预算、也不无限拖）。全程不碰真内核。
    #[test]
    fn silent_listener_comes_back_as_a_bounded_handshake_error() {
        let (listener, port) = silent_listener();
        let budget = Duration::from_millis(250);
        let (outcome, elapsed) = connect_watchdog(port, budget, Duration::from_secs(5));
        drop(listener);

        let Err(message) = outcome else {
            panic!("不回字节的监听必须判成 Err，不得吞成 Ok（连接就此被当成握手成功交回上层）");
        };
        assert!(
            message.contains("握手超时"),
            "定性要说清是握手超时：{message}"
        );
        assert!(message.contains("101"), "要看得出卡在等 101 这一步：{message}");
        assert!(
            !message.contains("失败: "),
            "TCP 已经通了，不许报成「连接 … 失败」那一档：{message}"
        );
        assert!(
            elapsed >= budget,
            "预算还没走完就返回 ⇒ 死线被提前掐了：{elapsed:?} < {budget:?}"
        );
        assert!(
            elapsed < budget * 4 + Duration::from_millis(100),
            "超时后必须马上回 Err，不许再拖一整轮：{elapsed:?}"
        );
    }

    /// 同一条出口的**第三种**：TCP 通了、也回了字节，但**响应头没写完对端就把连接收走了**。
    /// 靶子是「中途 EOF ⇒ 必须有界地回 Err，且要说清是中途而不是超时」：预算给到 5 秒，
    /// 半截头后面立刻是 FIN，所以能在看门狗之前回来的只有那条 EOF 臂。
    /// 哪天有人把 EOF 改成 `break`（接着等下一轮 ⇒ 一路挂到预算耗干）或直接 panic，
    /// 这发要么红在措辞上、要么红在「既没回 Ok 也没回 Err」，不会像今日欠账那样无声挂死。
    #[test]
    fn a_peer_that_drops_mid_handshake_comes_back_visible() {
        // 半截响应头：没有结尾空行 ⇒ 握手未完成；随后假服务端只关写方向（FIN）。
        let port = half_header_then_close_listener(b"HTTP/1.1 101 Switching");
        let budget = Duration::from_secs(5);
        let (outcome, elapsed) = connect_watchdog(port, budget, Duration::from_millis(2_500));

        let Err(message) = outcome else {
            panic!("半截头后被收走的连接必须判成 Err，不得吞成 Ok，也不许一直等下去");
        };
        assert!(
            message.contains("中途"),
            "要看得出是「头部没写完就被关」而不是别的：{message}"
        );
        assert!(
            !message.contains("握手超时"),
            "对端已经动手关连接了，不许并成「超时」那一档（会把真因盖掉）：{message}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "中途关闭要立刻回 Err，不许把整笔预算耗干：{elapsed:?}（预算 {budget:?}）"
        );
    }

    /// 同一条出口的**反面**：端口没人听 ⇒ 那还是「连接失败」那一档，不许被握手超时并掉。
    /// 这一发证明「超时」与「连不上」是分得开的两句话（引导失败要看得见，就得看得见差别）。
    #[test]
    fn refused_port_stays_the_connection_failure_wording() {
        let port = {
            let (listener, port) = silent_listener();
            drop(listener);
            port
        };
        let (outcome, elapsed) = connect_watchdog(
            port,
            Duration::from_millis(500),
            Duration::from_secs(5),
        );
        let Err(message) = outcome else {
            panic!("一个没人听的位置不可能连上，却被交回了 Ok");
        };
        assert!(
            message.contains("失败: "),
            "连不上就该是连不上那句话：{message}"
        );
        assert!(
            !message.contains("握手超时"),
            "连不上与握手超时两码事，不得混成一句：{message}"
        );
        assert!(elapsed < Duration::from_secs(5));
    }

    /// 真 socket 那发（不吞 Err 方向）：accept 后回 **404** 而不是 101（主干注释里那格
    /// 「升级请求落在 webserver 的 404 兜底席位上」）。必须回 Err 且带着「握手被拒绝」。
    #[test]
    fn a_non_101_answer_is_rejected_not_swallowed() {
        let port = replying_listener(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        let (outcome, _) = connect_watchdog(port, Duration::from_millis(900), Duration::from_secs(5));
        let Err(message) = outcome else {
            panic!("404 应答不得当成握手成功（这就是「不许吞 Err」那一格）");
        };
        assert!(message.contains("握手被拒绝"), "要看得见被拒的那一句：{message}");
    }

    /// 真 socket 那发（档位回归方向）：回 101 ⇒ Ok，**且返回的 socket 上读超时必须是常驻
    /// `READ_TICK`**：`poll` 靠它把「这一拍没字节」当正常空转。握手档漏给常驻期 = 每次 `poll`
    /// 白等 2s，本条就是那格回归锁（`Mux::stream` 是本模块私有字段，测试在模块内可直接读回）。
    #[test]
    fn a_101_answer_returns_ok_and_hands_the_socket_back_on_the_resident_tick() {
        let port = replying_listener(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n");
        let (outcome, _) = connect_watchdog(port, Duration::from_millis(900), Duration::from_secs(5));
        let mux = match outcome {
            Ok(mux) => mux,
            Err(message) => panic!("回了 101 就必须是 Ok：{message}"),
        };
        assert_eq!(
            mux.stream.read_timeout().ok().flatten(),
            Some(READ_TICK),
            "握手成功后没把读超时恢复成常驻档位"
        );
    }

    // ---------------------------------------------------------------- #241：服务端心跳（Ping ⇒ Pong）
    //
    // `dsh-api-gateway` 的 `RemoteStreamMuxServer` 每 2s（`DEFAULT_WEBSOCKET_HEARTBEAT_INTERVAL_MS`）
    // 给每个升级上来的 socket 发一笔 Ping，`missedHeartbeats` 只在收到 Pong 时清零，攒到
    // `MAX_MISSED_HEARTBEATS`(2) 直接 `terminate()`。主干的内建回 Pong 在 .NET 的
    // `ClientWebSocket` 里；分叉手写客户端自己实现。这一族把线上形状钉住：
    // ① 纯函数一层钉 opcode/掩码位；② 真 socket 那发钉「收到 Ping 必须当场回 Pong、payload 原样」；
    // ③ 反向钉「服务端自发的 Pong 是静默丢弃、不产事件」。

    /// ① 线上形状：FIN + opcode + 掩码位，payload 按掩码逐字节异或后原样可还原。
    #[test]
    fn masked_frame_carries_fin_opcode_and_masked_payload() {
        let frame = Mux::masked_frame(0xa, b"hb-7", [1, 2, 3, 4]);
        assert_eq!(frame[0], 0x8a, "第一字节须是 FIN + Pong");
        assert_eq!(frame[1], 0x80 | 4, "第二字节须是掩码位 + 短长度");
        assert_eq!(&frame[2..6], &[1, 2, 3, 4], "掩码紧邻长度");
        let payload: Vec<u8> = frame[6..]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ [1, 2, 3, 4][i & 3])
            .collect();
        assert_eq!(payload, b"hb-7", "掩码必须可逆");
    }

    /// ② 真 socket 那发：假内核回 101 后发一笔 Ping ⇒ 客户端必须当场回一笔掩码 Pong、
    /// payload 原样带回，且不产出业务事件。不回这一笔的客户端在内核上活不过 3 个心跳间隔
    /// （实测 6 秒一循环的「工作区流中断 ⇒ 重连」就是这个缺口）。
    #[test]
    fn a_server_ping_comes_back_as_a_pong_echoing_its_payload() {
        let ping: &'static [u8] = b"hb-7";
        let (port, heard) = ping_then_read_listener(ping);
        let endpoint = Endpoint {
            host: "127.0.0.1".to_string(),
            port,
            query: None,
        };
        let mut mux = Mux::connect(&endpoint, None).expect("应能连上假内核");
        let events = mux
            .collect(Duration::from_millis(400))
            .expect("收到 Ping 不该让轮询报错");
        assert!(events.is_empty(), "Ping/Pong 都是控制帧，不该产出业务事件");
        let (head, echoed) = heard
            .recv_timeout(Duration::from_secs(3))
            .expect("假内核应收到客户端的回帧");
        assert_eq!(head[0] & 0x0f, 0xa, "回帧的 opcode 必须是 Pong");
        assert_eq!(head[0] & 0x80, 0x80, "控制帧不得分片：须带 FIN");
        assert_eq!(head[1] & 0x80, 0x80, "客户端 → 服务端的帧必须掩码");
        assert_eq!(&echoed, ping, "Pong 须原样承载 Ping 的 payload");
    }

    /// ③ 反向：服务端**自发**的 Pong（未被请求的回帧）静默丢弃，随后的业务帧照常产出——
    /// 心跳帧不许挤占事件通道，也不许让轮询报错。
    #[test]
    fn a_voluntary_server_pong_is_dropped_and_business_frames_still_flow() {
        // 101 响应头 + 一笔无 payload 的 Pong + 一笔 item。
        let item = br#"{"type":"item","streamId":"rs1","value":42}"#;
        let mut script = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n".to_vec();
        script.extend_from_slice(&[0x8a, 0x00]);
        script.push(0x81);
        script.push(item.len() as u8);
        script.extend_from_slice(item);
        let port = replying_listener(Box::leak(script.into_boxed_slice()));
        let endpoint = Endpoint {
            host: "127.0.0.1".to_string(),
            port,
            query: None,
        };
        let mut mux = Mux::connect(&endpoint, None).expect("应能连上假内核");
        let events = mux
            .collect(Duration::from_millis(600))
            .expect("Pong 之后的 item 不该让轮询报错");
        assert_eq!(events.len(), 1, "只有 item 是业务帧，Pong 必须被丢干净");
        assert!(
            matches!(events[0], MuxEvent::Item { .. }),
            "留下来的必须是 item：{:?}",
            events[0]
        );
    }
}
