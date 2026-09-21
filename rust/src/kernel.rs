use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const MARKER: &str = "dsh web: ";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(90);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Launch {
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub dsh_home: Option<PathBuf>,
    pub path_prepend: Option<PathBuf>,
    pub working_dir: Option<PathBuf>,
}

fn kernel_dir_from(exe: &Path) -> Option<PathBuf> {
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..6 {
        let candidate = dir.join("Kernel");
        if candidate.join("node.exe").is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }
    None
}

impl Launch {
    /// BLADE2_KERNEL_EXE / BLADE2_KERNEL_ARGS 用于把内核换成假实现做自测，默认对齐主线 DshKernelHost。
    pub fn from_env() -> Result<Self, String> {
        if let Ok(exe) = std::env::var("BLADE2_KERNEL_EXE") {
            let args = std::env::var("BLADE2_KERNEL_ARGS")
                .unwrap_or_default()
                .split_whitespace()
                .map(String::from)
                .collect();
            return Ok(Self {
                exe: PathBuf::from(exe),
                args,
                dsh_home: None,
                path_prepend: None,
                working_dir: None,
            });
        }
        let exe_dir = std::env::current_exe().map_err(|e| format!("无法定位自身路径: {e}"))?;
        let kernel = match std::env::var("BLADE2_KERNEL_DIR") {
            Ok(dir) => PathBuf::from(dir),
            Err(_) => kernel_dir_from(&exe_dir).ok_or("未找到 Kernel/node.exe")?,
        };
        let node = kernel.join("node.exe");
        let bin_js = kernel.join("dsh").join("lib").join("bin.js");
        if !node.is_file() || !bin_js.is_file() {
            return Err(format!(
                "内核文件缺失: {} / {}",
                node.display(),
                bin_js.display()
            ));
        }
        Ok(Self {
            exe: node,
            args: vec![
                bin_js.display().to_string(),
                "web".into(),
                "--no-open".into(),
                "--port".into(),
                "0".into(),
            ],
            dsh_home: Some(match std::env::var("BLADE2_DSH_HOME") {
                Ok(dir) => PathBuf::from(dir),
                Err(_) => std::env::var("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("Blade2"),
            }),
            path_prepend: Some(kernel.join("bin")),
            working_dir: None,
        })
    }
}

pub fn find_web_url(line: &str) -> Option<String> {
    let at = line.find(MARKER)? + MARKER.len();
    let tail = line[at..].trim_start();
    let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
    if tail[..end].starts_with("http") {
        Some(tail[..end].to_string())
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub query: Option<String>,
}

pub fn parse_url(url: &str) -> Result<Endpoint, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("仅支持 http:// 本地地址: {url}"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, ""),
    };
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| format!("地址缺少端口: {authority}"))?;
    let port: u16 = port.parse().map_err(|_| format!("端口不是数字: {port}"))?;
    let query = path.split_once('?').map(|(_, q)| q.to_string());
    Ok(Endpoint {
        host: host.to_string(),
        port,
        query,
    })
}

fn read_line<R: Read>(br: &mut BufReader<R>) -> Result<String, String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match br.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                buf.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Err(e) => return Err(format!("读取失败: {e}")),
        }
    }
    Ok(String::from_utf8_lossy(&buf)
        .trim_end_matches(['\r', '\n'])
        .to_string())
}

fn decode_chunked<R: Read>(br: &mut BufReader<R>) -> Result<String, String> {
    let mut out = Vec::new();
    loop {
        let header = read_line(br)?;
        let size = usize::from_str_radix(header.trim().split(';').next().unwrap_or("0"), 16)
            .map_err(|e| format!("分块长度无效: {e}"))?;
        if size == 0 {
            break;
        }
        let mut chunk = vec![0u8; size];
        br.read_exact(&mut chunk)
            .map_err(|e| format!("分块读取失败: {e}"))?;
        out.append(&mut chunk);
        read_line(br)?;
    }
    Ok(String::from_utf8_lossy(&out).to_string())
}

fn cookie_from(headers: &str) -> Option<String> {
    let mut pairs = Vec::new();
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        let Some(rest) = lower.strip_prefix("set-cookie:") else {
            continue;
        };
        let pair = line[line.len() - rest.len()..]
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if pair.contains('=') {
            pairs.push(pair);
        }
    }
    (!pairs.is_empty()).then(|| pairs.join("; "))
}

/// 查询串转义，口径同主干的 `Uri.EscapeDataString`（未保留字节一律 %XX）。
pub fn escape_query(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn http(
    ep: &Endpoint,
    method: &str,
    path: &str,
    body: Option<&str>,
    cookie: Option<&str>,
) -> Result<(u16, String, String), String> {
    let mut stream = TcpStream::connect((ep.host.as_str(), ep.port))
        .map_err(|e| format!("连接 {}:{} 失败: {e}", ep.host, ep.port))?;
    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|e| format!("设置超时失败: {e}"))?;
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nConnection: close\r\n",
        ep.host, ep.port
    );
    if let Some(body) = body {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if let Some(cookie) = cookie {
        req.push_str(&format!("Cookie: {cookie}\r\n"));
    }
    req.push_str("\r\n");
    if let Some(body) = body {
        req.push_str(body);
    }
    stream
        .write_all(req.as_bytes())
        .map_err(|e| format!("发送失败: {e}"))?;
    stream.flush().map_err(|e| format!("刷新失败: {e}"))?;

    let mut br = BufReader::new(stream);
    let status_line = read_line(&mut br)?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("响应状态行无效: {status_line}"))?;
    let mut headers = String::new();
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    loop {
        let line = read_line(&mut br)?;
        if line.is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().ok();
        } else if lower.contains("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        }
        headers.push_str(&line);
        headers.push('\n');
    }
    let body = if chunked {
        decode_chunked(&mut br)?
    } else if let Some(len) = content_length {
        let mut buf = vec![0u8; len];
        br.read_exact(&mut buf)
            .map_err(|e| format!("正文读取失败: {e}"))?;
        String::from_utf8_lossy(&buf).to_string()
    } else {
        let mut buf = Vec::new();
        br.read_to_end(&mut buf)
            .map_err(|e| format!("正文读取失败: {e}"))?;
        String::from_utf8_lossy(&buf).to_string()
    };
    Ok((status, headers, body))
}

pub struct Kernel {
    ep: Endpoint,
    cookie: Option<String>,
    child: Option<Child>,
    next_id: u64,
    pub url: String,
    pub log: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    pub cwd: String,
    pub title: String,
    pub blank: bool,
    pub updated_at: i64,
    pub parent: Option<String>,
    /// 主干只用 `origin == "subagent"` 判定子代理行，`parentSessionId` 仅服务于会话头返回链。
    pub origin: Option<String>,
}

impl SessionInfo {
    pub fn is_subagent(&self) -> bool {
        self.origin.as_deref() == Some("subagent")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub path: String,
    pub title: String,
    pub session_ids: Vec<String>,
}

/// 主干的工作区树只能来自 `workspace/follow`（没有 workspace/list 这个 RPC）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceTree {
    pub workspaces: Vec<Workspace>,
    pub archived: Vec<String>,
}

fn string_field(record: &Value, key: &str) -> String {
    record[key].as_str().unwrap_or("").to_string()
}

fn string_list(record: &Value, key: &str) -> Vec<String> {
    record[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

impl Workspace {
    fn from_view(view: &Value) -> Option<Self> {
        let id = string_field(view, "workspaceId");
        if id.is_empty() {
            return None;
        }
        Some(Self {
            title: string_field(view, "title"),
            id,
            path: string_field(view, "path"),
            session_ids: string_list(view, "sessionIds"),
        })
    }
}

impl WorkspaceTree {
    /// 应用一帧 follow 流元素，返回是否改变了树（主干据此决定要不要重建树）。
    pub fn apply(&mut self, frame: &Value) -> bool {
        match frame["type"].as_str().unwrap_or_default() {
            "baseline" => {
                let frame = &frame["value"];
                let Some(items) = frame["items"].as_array() else {
                    return false;
                };
                let next: Vec<Workspace> = items.iter().filter_map(Workspace::from_view).collect();
                let archived = string_list(frame, "archivedSessionIds");
                let changed = next != self.workspaces || archived != self.archived;
                self.workspaces = next;
                self.archived = archived;
                changed
            }
            "upsert" => {
                let Some(workspace) = Workspace::from_view(&frame["workspace"]) else {
                    return false;
                };
                match self
                    .workspaces
                    .iter()
                    .position(|item| item.id == workspace.id)
                {
                    // 主干是就地替换；官方内核模型是插到最前，这里跟主干。
                    Some(index) => self.workspaces[index] = workspace,
                    None => self.workspaces.push(workspace),
                }
                true
            }
            "remove" => {
                let id = string_field(frame, "workspaceId");
                let before = self.workspaces.len();
                self.workspaces.retain(|item| item.id != id);
                before != self.workspaces.len()
            }
            "order" => {
                let ids = string_list(frame, "workspaceIds");
                if ids.is_empty() {
                    return false;
                }
                // 主干用 IndexOf，未知 id 得 -1 反而排到最前。
                self.workspaces.sort_by_key(|item| {
                    ids.iter()
                        .position(|id| *id == item.id)
                        .map_or(-1, |index| index as i64)
                });
                true
            }
            "archived" => {
                let archived = string_list(frame, "archivedSessionIds");
                let changed = archived != self.archived;
                self.archived = archived;
                changed
            }
            _ => false,
        }
    }

    pub fn is_archived(&self, session_id: &str) -> bool {
        self.archived.iter().any(|id| id == session_id)
    }

    /// 主干唯一的排除条件就是 archived；子代理与 blank 会话都会留下。
    pub fn visible<'a>(&self, rows: &'a [SessionInfo]) -> Vec<&'a SessionInfo> {
        rows.iter()
            .filter(|row| !self.is_archived(&row.id))
            .collect()
    }

    /// `ws-` 前缀之外的 `""` 是主干「未分组」节点的 key。
    pub fn group_sessions<'a>(
        &self,
        group_key: &str,
        rows: &[&'a SessionInfo],
    ) -> Vec<&'a SessionInfo> {
        match self.workspaces.iter().find(|item| item.id == group_key) {
            Some(workspace) => {
                let mut picked: Vec<&SessionInfo> = Vec::new();
                for id in &workspace.session_ids {
                    if let Some(row) = rows.iter().find(|row| row.id == *id) {
                        picked.push(*row);
                    }
                }
                picked
            }
            None => rows
                .iter()
                .copied()
                .filter(|row| !self.filed_ids().contains(&row.id.as_str()))
                .collect(),
        }
    }

    pub fn filed_ids(&self) -> Vec<&str> {
        self.workspaces
            .iter()
            .flat_map(|workspace| workspace.session_ids.iter().map(String::as_str))
            .collect()
    }
}

/// `$events` 流里左栏唯一关心的帧：`api-session/status` 的运行布尔。
/// 主干从不读 `session/list` 带的 `running`，状态点只认这条 emit。
pub fn session_status_event(frame: &Value) -> Option<(String, bool)> {
    if frame["type"].as_str() != Some("emit")
        || frame["event"].as_str() != Some("api-session/status")
    {
        return None;
    }
    let args = frame["args"].as_array()?;
    let session_id = args.first()?.as_str()?.to_string();
    Some((session_id, args.get(1)?.as_bool()?))
}

impl Kernel {
    pub fn start(launch: &Launch) -> Result<Self, String> {
        let mut command = Command::new(&launch.exe);
        command
            .args(&launch.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &launch.working_dir {
            command.current_dir(dir);
        }
        if let Some(home) = &launch.dsh_home {
            command.env("DSH_HOME", home);
        }
        if let Some(bin) = &launch.path_prepend {
            let joined = match std::env::var("PATH") {
                Ok(path) => format!("{};{path}", bin.display()),
                Err(_) => bin.display().to_string(),
            };
            command.env("PATH", joined);
        }
        #[allow(unused_imports)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|e| format!("内核启动失败: {e}"))?;

        let (tx, rx) = mpsc::channel::<String>();
        let stdout_tx = tx.clone();
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = stdout_tx.send(line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let _ = tx.send(line);
                }
            });
        }

        let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
        let mut log = Vec::new();
        let url = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                let _ = child.kill();
                return Err("内核握手超时（90s 未见 dsh web: 行）".to_string());
            }
            match rx.recv_timeout(left) {
                Ok(line) => {
                    log.push(line.clone());
                    if let Some(url) = find_web_url(&line) {
                        break url;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = child.try_wait().ok().flatten();
                    let _ = child.kill();
                    return Err(format!("内核提前退出: {status:?}"));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    return Err("内核握手超时".to_string());
                }
            }
        };

        let ep = parse_url(&url)?;
        let path = match &ep.query {
            Some(query) => format!("/?{query}"),
            None => "/".to_string(),
        };
        let (status, headers, _) =
            http(&ep, "GET", &path, None, None).map_err(|e| format!("鉴权请求失败: {e}"))?;
        let cookie = cookie_from(&headers);
        let mut log = log;
        log.push(format!("auth HTTP {status}"));
        for line in headers.lines().filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.starts_with("set-cookie:") || lower.starts_with("location:")
        }) {
            log.push(format!("auth < {line}"));
        }
        if status >= 400 {
            return Err(format!("鉴权失败: HTTP {status}"));
        }
        Ok(Self {
            ep,
            cookie,
            child: Some(child),
            next_id: 0,
            url,
            log,
        })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.ep
    }

    pub fn cookie(&self) -> Option<&str> {
        self.cookie.as_deref()
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    pub fn call(&mut self, method: &str, args: Value) -> Result<Value, String> {
        self.next_id += 1;
        let envelope = json!({
            "type": "client-request",
            "rpcId": format!("c2-{}", self.next_id),
            "method": method,
            "payload": { "args": args },
        });
        let (status, _, body) = http(
            &self.ep,
            "POST",
            &format!("/api/{method}"),
            Some(envelope.to_string().as_str()),
            self.cookie.as_deref(),
        )?;
        if status >= 400 {
            return Err(format!(
                "{method} → HTTP {status}: {}",
                body.chars().take(200).collect::<String>()
            ));
        }
        let value: Value =
            serde_json::from_str(&body).map_err(|e| format!("{method} 响应不是 JSON: {e}"))?;
        let result = value.get("result").unwrap_or(&value);
        if result.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(result.get("value").cloned().unwrap_or(Value::Null));
        }
        let error = &result["error"];
        Err(format!(
            "{}: {}",
            error["code"].as_str().unwrap_or("rpc_error"),
            error["message"].as_str().unwrap_or("未知错误")
        ))
    }

    /// 宿主 HTTP 路由（不是 JSON-RPC）：交付物的「打开/回显」这类端点只吃查询串，
    /// 成功回 200/204 且多半没正文，所以把状态码原样交回调用方判断。
    pub fn post_route(&mut self, path: &str) -> Result<(u16, String), String> {
        http(&self.ep, "POST", path, None, self.cookie.as_deref())
            .map(|(status, _, body)| (status, body))
    }

    pub fn list_sessions(&mut self) -> Result<Vec<SessionInfo>, String> {
        let value = self.call("session/list", json!({ "_request": {} }))?;
        let rows = value["items"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|item| {
                Some(SessionInfo {
                    id: item["sessionId"].as_str()?.to_string(),
                    cwd: item["cwd"].as_str().unwrap_or("").to_string(),
                    title: item["projections"]["values"]["title"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    blank: item["blank"].as_bool().unwrap_or(false),
                    updated_at: item["updatedAt"].as_i64().unwrap_or(0),
                    parent: item["parentSessionId"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                    origin: item["origin"].as_str().map(str::to_string),
                })
            })
            .collect();
        Ok(rows)
    }

    pub fn create_session(&mut self, cwd: &str) -> Result<String, String> {
        let value = self.call("session/create", json!({ "request": { "cwd": cwd } }))?;
        value["sessionId"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "bad-response: 会话创建响应缺少 sessionId。".to_string())
    }

    pub fn shutdown(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.child = None;
    }
}

impl Drop for Kernel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_escaping_matches_escape_data_string() {
        assert_eq!(escape_query("sess-1_2.3~4"), "sess-1_2.3~4");
        assert_eq!(escape_query("a b/c?d=e&f"), "a%20b%2Fc%3Fd%3De%26f");
        assert_eq!(escape_query("会话"), "%E4%BC%9A%E8%AF%9D");
    }

    #[test]
    fn finds_handshake_url_from_prefixed_log() {
        let line = "[dsh] dsh web: http://127.0.0.1:53212/?token=abc123";
        assert_eq!(
            find_web_url(line).as_deref(),
            Some("http://127.0.0.1:53212/?token=abc123")
        );
    }

    #[test]
    fn ignores_unrelated_lines() {
        assert_eq!(find_web_url("listening on ws://x"), None);
        assert_eq!(find_web_url("dsh web: not-a-url"), None);
    }

    #[test]
    fn parses_url_with_query_and_root() {
        let ep = parse_url("http://127.0.0.1:80/?token=t").unwrap();
        assert_eq!(
            (ep.host.as_str(), ep.port, ep.query.as_deref()),
            ("127.0.0.1", 80, Some("token=t"))
        );
        let ep = parse_url("http://localhost:9").unwrap();
        assert_eq!(
            (ep.host.as_str(), ep.port, ep.query.as_deref()),
            ("localhost", 9, None)
        );
        assert!(parse_url("https://127.0.0.1:1/").is_err());
        assert!(parse_url("http://127.0.0.1/").is_err());
    }

    #[test]
    fn follow_frames_discriminate_on_type() {
        let mut tree = WorkspaceTree::default();
        let baseline = json!({
            "type": "baseline",
            "value": {
                "items": [
                    { "workspaceId": "ws-1", "path": "C:/one", "title": "one", "sessionIds": ["s-1", "s-2"] },
                    { "workspaceId": "ws-2", "path": "C:/two", "title": "two", "sessionIds": [] },
                ],
                "archivedSessionIds": ["s-9"],
            }
        });
        assert!(tree.apply(&baseline));
        assert_eq!(tree.workspaces.len(), 2);
        assert!(tree.is_archived("s-9"));
        assert!(!tree.is_archived("s-1"));
        // 判别键写成 kind 的帧必须被整体忽略，而不是当成空基线。
        assert!(!tree.apply(&json!({ "kind": "baseline", "items": [] })));
        assert_eq!(tree.workspaces.len(), 2);

        assert!(tree.apply(&json!({
            "type": "upsert",
            "workspace": { "workspaceId": "ws-3", "title": "three", "sessionIds": ["s-3"] }
        })));
        assert_eq!(
            tree.workspaces.last().map(|item| item.id.as_str()),
            Some("ws-3")
        );
        assert!(tree.apply(&json!({ "type": "order", "workspaceIds": ["ws-3", "ws-2"] })));
        // 主干用 IndexOf：不在 order 列表里的 ws-1 得到 -1，反而排到最前。
        assert_eq!(tree.workspaces[0].id, "ws-1");
        assert_eq!(
            tree.workspaces.last().map(|item| item.id.as_str()),
            Some("ws-2")
        );
        assert!(tree.apply(&json!({ "type": "remove", "workspaceId": "ws-2" })));
        assert_eq!(tree.workspaces.len(), 2);
        assert!(tree.apply(&json!({ "type": "archived", "archivedSessionIds": ["s-1"] })));
        assert!(!tree.apply(&json!({ "type": "archived", "archivedSessionIds": ["s-1"] })));
    }

    #[test]
    fn grouping_follows_session_ids_and_leaves_unfiled_in_the_pseudo_group() {
        let rows = vec![
            SessionInfo {
                id: "s-1".into(),
                cwd: "C:/one".into(),
                title: String::new(),
                blank: false,
                updated_at: 0,
                parent: None,
                origin: None,
            },
            SessionInfo {
                id: "s-ghost".into(),
                cwd: "C:/moved".into(),
                title: String::new(),
                blank: false,
                updated_at: 0,
                parent: None,
                origin: None,
            },
        ];
        let mut tree = WorkspaceTree::default();
        assert!(tree.apply(&json!({
            "type": "baseline",
            "value": {
                "items": [
                    { "workspaceId": "ws-1", "path": "C:/one", "title": "one", "sessionIds": ["s-1", "s-gone"] },
                ],
                "archivedSessionIds": [],
            }
        })));
        let visible = tree.visible(&rows);
        assert_eq!(tree.group_sessions("ws-1", &visible).len(), 1);
        // sessionIds 里的幽灵 id 只影响该组，掉队的会话进「未分组」（key = ""）。
        assert_eq!(tree.group_sessions("", &visible).len(), 1);
        assert_eq!(tree.group_sessions("", &visible)[0].id, "s-ghost");
    }

    #[test]
    fn running_flag_only_comes_from_status_emits() {
        let on = json!({ "type": "emit", "event": "api-session/status", "args": ["s-1", true] });
        assert_eq!(session_status_event(&on), Some(("s-1".to_string(), true)));
        // 参数不齐、事件不对、帧型不是 emit 一律不认。
        assert_eq!(
            session_status_event(
                &json!({ "type": "emit", "event": "api-session/status", "args": ["s-1"] })
            ),
            None
        );
        assert_eq!(
            session_status_event(
                &json!({ "type": "emit", "event": "api-session/title", "args": ["s-1", true] })
            ),
            None
        );
        assert_eq!(
            session_status_event(&json!({ "type": "ready", "clientId": "c-1", "host": "x" })),
            None
        );
        assert_eq!(
            session_status_event(&json!({ "type": "baseline", "value": { "items": [] } })),
            None
        );
    }

    #[test]
    fn collects_all_set_cookie_pairs() {
        let headers = "set-cookie: other=1; Path=/\nSet-Cookie: dsh-auth-Qh3xR2=V1.abc.sig; Path=/; HttpOnly; SameSite=Strict\n";
        assert_eq!(
            cookie_from(headers).as_deref(),
            Some("other=1; dsh-auth-Qh3xR2=V1.abc.sig")
        );
        assert_eq!(cookie_from("set-cookie: no-value-here"), None);
        assert_eq!(cookie_from("content-length: 3"), None);
    }

    #[test]
    fn decodes_chunked_body() {
        let raw = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        let mut br = BufReader::new(std::io::Cursor::new(raw.to_vec()));
        assert_eq!(decode_chunked(&mut br).unwrap(), "hello world");
    }
}
