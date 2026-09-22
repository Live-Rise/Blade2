use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::i18n::Catalog;

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

// ============================ 会话投影（projections） ============================
//
// 规格来自主干（只读）三处：
//   * `session/list` 的 `items[i].projections = {asOfSeq?, values?}`
//     —— `MainWindow.xaml.cs:2827-2836`（整块进 `_sessionProjections` 缓存）、
//        `MainWindow.xaml.cs:2829-2830`（`values.sessionStats.turns`）、
//        `MainWindow.xaml.cs:2686-2700`（`SessionDisplayTitle` 读 `values.title`）、
//        `MainWindow.xaml.cs:14432-14437`（`asOfSeq` = 该会话 journal 游标，缺则 -1）。
//   * `session/control` 的 baseline/projection 帧（下面 `ControlState` 一节）。
//   * `MainWindow.TurnRail.cs:188-218` `ParseTurnOutline`（每轮大纲的条目形状与丢弃规则）。
//
// 原则：**主干读得到的键才出 typed 字段，其余一律原样留在 `values` 里**，
// 这样内核新增投影键（`todos`/`inbox`…）不会因为分叉没建模就被丢掉。

/// 每轮大纲的一条（内核 `dsh-session-turn-outline` 的 `wire.view` 已裁成四键）。
/// 条目缺 `turn`（或 `turn <= 0`）、缺 `seq`（或 `seq < 0`）整条丢弃；两个预览字段坏了
/// 降级成空串 —— 与主干 `ParseTurnOutline` 同策略（轮次可导航优先于预览完整）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnOutlineItem {
    pub turn: i64,
    pub seq: i64,
    pub prompt: String,
    pub response: String,
}

/// `projections.values.plan`：`{active, pending}`（主干 `ApplyPlanProjection`，
/// `MainWindow.xaml.cs:15346-15356`；值本身是 dsh-plan-mode 的 wire.view 裁剪结果）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanProjection {
    pub active: bool,
    pub pending: bool,
}

/// `projections.values.permissions.options` 的一项（主干 `ApplyPermissionsProjection`，
/// `MainWindow.xaml.cs:15358-15389`）：`description` 可缺席。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionOption {
    pub value: String,
    pub name: String,
    pub description: String,
}

/// `projections.values.permissions`：`{options[], currentValue?}`。
/// 主干只在 `options` 非空时才覆写旧表，`currentValue` 非字符串就是 `null`。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionsProjection {
    pub options: Vec<PermissionOption>,
    pub current_value: Option<String>,
}

/// 权限预设的中文可读名 —— 主干 `PermissionPresetZh`（`MainWindow.xaml.cs:16001-16009`）的
/// 同一张表。**硬约束**：内核 projection 里的 `name` 是英文 id，不能直接显示
/// （主干 15957 的注释「内核 projection 的 name 是英文 id，必须过 PermissionPresetZh」；
/// `dsh-permission-presets` 的 `optionOf()` 出的就是 `value = name = spec.name`）。
/// 所以翻译属于视图层侧的最后一道，但表放在数据层这里，`i18n.rs` 只有键没有这张映射。
/// 键 = 内核表键 + 派生的 `custom`；认不得的 id 原样回显（主干的 `_ => value`）。
pub fn permission_preset_zh(catalog: &Catalog, value: &str) -> String {
    match value {
        "read-only" => catalog.l("仅可查看"),
        "workspace-write" => catalog.l("工作区内修改"),
        "danger-full-access" => catalog.l("完全权限"),
        "auto-approve" => catalog.l("自动审批"),
        "custom" => catalog.l("自定义"),
        _ => value.to_string(),
    }
}

/// `projections.values.sessionStats`：内核 `dsh-session-stats` 的 `sessionStatsSchema`
/// 是 **`.strict()`** 的八字段表（`node_modules/@deepseek-ai/dsh-session-stats/lib/types/projection.js:27-35`），
/// 线上不可能出现第九个键（`totalTokens` 就不在其中，真内核直接拒）。
/// 主干今天只读 `turns`（左栏副标题「N 轮对话」，`MainWindow.xaml.cs:2841`），
/// #57/#58 要的 `steps`/`decodeMs`/`decodeTokens` 一并建模，省得视图层去 `values` 里捞裸 JSON。
/// 四个计数字段 schema 是 `int().nonnegative()`，四个时长/令牌字段是 `number().nonnegative()`。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SessionStats {
    pub turns: i64,
    pub steps: i64,
    pub llm_ms: f64,
    pub tool_ms: f64,
    pub ttft_ms: f64,
    pub decode_ms: f64,
    pub decode_tokens: f64,
    pub ttft_steps: i64,
}

/// strict 表里的整数字段：缺键按 0（主干 `TryGetProperty` 的同一条兜底），负数钳到 0。
fn nonneg_int(record: &Value, key: &str) -> i64 {
    record[key]
        .as_f64()
        .map(|number| number.max(0.0) as i64)
        .unwrap_or(0)
}

/// strict 表里的非负数值字段（时长/令牌数允许小数）：缺键或负数按 0。
fn nonneg_num(record: &Value, key: &str) -> f64 {
    record[key]
        .as_f64()
        .filter(|number| *number >= 0.0)
        .unwrap_or(0.0)
}

/// 一个会话的投影快照：`{asOfSeq, values}` 整块的结构化视图。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projections {
    /// 该会话 journal 的当前游标；内核没给就是 `None`（主干按 -1 处理，见 14437）。
    pub as_of_seq: Option<i64>,
    /// `values` 原样保留的整块，未建模的键全在这儿。空对象 = 内核整块缺席（旧会话）。
    pub values: Value,
    pub title: String,
    pub turn_outline: Vec<TurnOutlineItem>,
    pub plan: Option<PlanProjection>,
    pub permissions: Option<PermissionsProjection>,
    pub stats: Option<SessionStats>,
}

/// 主干 `ParseTurnOutline` 的分叉版：非数组一律当「没有大纲」，坏条目跳过。
pub fn parse_turn_outline(value: &Value) -> Vec<TurnOutlineItem> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut outline: Vec<TurnOutlineItem> = items
        .iter()
        .filter_map(|item| {
            let turn = item["turn"].as_i64().filter(|turn| *turn > 0)?;
            let seq = item["seq"].as_i64().filter(|seq| *seq >= 0)?;
            Some(TurnOutlineItem {
                turn,
                seq,
                prompt: string_field(item, "prompt"),
                response: string_field(item, "response"),
            })
        })
        .collect();
    // 内核 schema 保证 turn 严格递增，这里照样防御性排序（主干 216 行同一步）。
    outline.sort_by_key(|item| item.turn);
    outline
}

impl Projections {
    /// 解析一个 `projections` 块（`session/list` 的项、baseline 的 `{<sid>:…}` 值都用它）。
    pub fn from_block(block: &Value) -> Self {
        let mut this = Self {
            as_of_seq: block["asOfSeq"].as_i64(),
            values: block
                .get("values")
                .filter(|values| values.is_object())
                .cloned()
                .unwrap_or_else(|| json!({})),
            ..Default::default()
        };
        this.refresh_typed();
        this
    }

    /// 主干 `ApplyProjectionValues` 的口径：`session/control` 的 projection 增量帧
    /// 一次只给一个键，且该键的 `wire.view` 是**全量**而非补丁 ⇒ 整键替换后重算 typed 字段。
    pub fn apply_key(&mut self, key: &str, value: &Value) {
        // 增量帧对「还没见过该会话 baseline/`session/list`」的会话照样要落值：主干
        // `MainWindow.xaml.cs:15284-15306` 那一支是把 `value` 直接喂给 `ApplyPlanProjection`
        // （15347-15356 写字段），没有任何「缓存里已有这张表」的前置条件。
        // 分叉这边按会话缓存，所以 `values` 还是 `Default` 的 `Null` 时必须先把表建起来：
        // 否则 `as_object_mut()` 取不到 ⇒ 这一键被静默丢掉，`plan.pending` 这类
        // 「只有增量帧带来的键」永远读不出来（#55/#56 的「切换中…」就是这么丢的）。
        if !self.values.is_object() {
            self.values = json!({});
        }
        if let Some(values) = self.values.as_object_mut() {
            values.insert(key.to_string(), value.clone());
        }
        self.refresh_typed();
    }

    /// 未建模键的取用入口（`todos`/`inbox`/`goal` 这类分叉还没读的投影）。
    pub fn value_of(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// 内核把「没有投影」的旧会话整个缺块（主干 2827 行的容错注释），据此判断要不要收起状态条。
    pub fn is_empty(&self) -> bool {
        self.values.as_object().is_none_or(|values| values.is_empty())
    }

    fn refresh_typed(&mut self) {
        let values = &self.values;
        self.title = string_field(values, "title");
        self.turn_outline = parse_turn_outline(&values["turnOutline"]);
        self.plan = values["plan"]
            .is_object()
            .then(|| PlanProjection {
                active: values["plan"]["active"].as_bool().unwrap_or(false),
                pending: values["plan"]["pending"].as_bool().unwrap_or(false),
            });
        self.permissions = values["permissions"]
            .is_object()
            .then(|| PermissionsProjection {
                options: values["permissions"]["options"]
                    .as_array()
                    .map(|options| {
                        options
                            .iter()
                            .filter_map(|option| {
                                let value = option["value"].as_str()?.to_string();
                                if value.is_empty() {
                                    // 主干 15377：没有 value 的选项整个不要。**真内核产不出这种条目**
                                    // （`dsh-permission-presets` 的 selectSchema 里 `value`/`name`
                                    // 都是 `string().min(1)` 必填）⇒ 这条是纯防御，别指望桩来演它。
                                    return None;
                                }
                                Some(PermissionOption {
                                    name: option["name"]
                                        .as_str()
                                        .unwrap_or(value.as_str())
                                        .to_string(),
                                    description: string_field(option, "description"),
                                    value,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                current_value: values["permissions"]["currentValue"].as_str().map(str::to_string),
            });
        self.stats = values["sessionStats"].is_object().then(|| {
            let stats = &values["sessionStats"];
            SessionStats {
                turns: nonneg_int(stats, "turns"),
                steps: nonneg_int(stats, "steps"),
                llm_ms: nonneg_num(stats, "llmMs"),
                tool_ms: nonneg_num(stats, "toolMs"),
                ttft_ms: nonneg_num(stats, "ttftMs"),
                decode_ms: nonneg_num(stats, "decodeMs"),
                decode_tokens: nonneg_num(stats, "decodeTokens"),
                ttft_steps: nonneg_int(stats, "ttftSteps"),
            }
        });
    }
}

/// `session/list` 的一行：左栏已用的那些字段（`info`）+ 整块投影（`projections`）。
/// `list_sessions()` 仍只回 `SessionInfo`（现有调用点一个字不用改），要投影的界面走这里。
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRow {
    pub info: SessionInfo,
    pub projections: Projections,
}

/// 从 `session/list` 的 `value` 解析整张台账（纯函数：ipc 用例与单测都能直接喂 JSON）。
pub fn parse_session_rows(value: &Value) -> Vec<SessionRow> {
    value["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let row = SessionRow {
                        info: SessionInfo {
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
                        },
                        projections: Projections::from_block(&item["projections"]),
                    };
                    // 主干 2819 行：`sessionId` 是 `GetProperty` 硬要求，缺 id 的行整行不要。
                    (!row.info.id.is_empty()).then_some(row)
                })
                .collect()
        })
        .unwrap_or_default()
}

// ==================== 会话控制面（`session/control` 长驻流） ====================
//
// 主干时机（`MainWindow.xaml.cs:2622`、`:4121`、`MainWindow.ReconnectProbe.cs:35`、
// `MainWindow.xaml.cs:5375`）：mux 连上、`workspace/follow` 与 `$events` 之后开一次；
// 切会话与 mux 重连后各补开一次（幂等：`_controlStreamId` 非空就跳过）。
// 发起的是 `OpenRemoteStreamAsync("session/control", new { }, …)`（15201），即零参 `{}`。
//
// 帧型（主干 15184-15190 抄来的注释，逐条对应下面的分支）：
//   baseline   {value:{queues:{sid:[item…]}, jobs:{sid:[job…]}, projections:{sid:{asOfSeq,values}}}}
//   queue      {sessionId, items:[…]}    —— 该会话队列的完整替换
//   jobs       {sessionId, jobs:[…]}     —— 该会话任务的完整替换
//   projection {sessionId, key, value, seq}
// 判别键同样是 `type`（与 follow 流那条既有约束一致），不认的型别整帧忽略。

/// 排队中的一条消息（主干 `ParseQueueItems`，`MainWindow.xaml.cs:15420-15446`）。
/// `content` 存内核原始块数组：主干「编辑只换文本块，图片/文件块原样回写」（233-241 行注释），
/// 所以占位文案（`[图片]`/`[文件 x]`）留给 UI 侧拼，数据层不掺字面量。
#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    pub item_id: String,
    /// `queued` | `steering` | `context`；内核没给按主干回落 `queued`。
    pub placement: String,
    pub content: Value,
}

impl QueueItem {
    /// 文本块的拼接（主干 `ContentText` 的 text 分支）：图片/文件块在这里只贡献占位，
    /// 分叉要显示带文案的整串时由 UI 侧读 `content` 自己补。
    pub fn text(&self) -> String {
        self.content
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| block["type"].as_str() == Some("text"))
                    .filter_map(|block| block["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    }
}

/// 运行中/已结束的任务（主干 `ParseJobs`，`MainWindow.xaml.cs:15448-15469`；
/// `kind` 是内核注册表给的作业类型 `<kind>-N`，时间是 epoch 毫秒）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Job {
    pub job_id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
    pub started_at: i64,
    pub finished_at: i64,
}

impl Job {
    /// 官方 `JobListAction` 的 `isLive`（主干 265-266 行）：运行中/停止中算存活。
    pub fn is_live(&self) -> bool {
        matches!(self.status.as_str(), "running" | "stopping")
    }
}

fn parse_queue_items(items: &Value) -> Vec<QueueItem> {
    let Some(items) = items.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let item_id = string_field(item, "id");
            if item_id.is_empty() {
                return None; // 主干 15430：没有 id 的条目整条丢
            }
            Some(QueueItem {
                placement: item["placement"]
                    .as_str()
                    .filter(|placement| !placement.is_empty())
                    .unwrap_or("queued")
                    .to_string(),
                item_id,
                // 主干 15435 的读法是 `TryGetProperty("message") && TryGetProperty("content")`：
                // 两级都取不到就 `Content = default`（`Text` 为空串），**条目照样保留**。
                // 这里曾写成 `item.get("message")?.get("content")?`，`?` 会把整条丢掉，
                // 于是内核给一条没有 message 的排队项时两端队列计数就不一致了。
                content: item
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn parse_jobs(jobs: &Value) -> Vec<Job> {
    let Some(jobs) = jobs.as_array() else {
        return Vec::new();
    };
    jobs.iter()
        .map(|job| Job {
            job_id: string_field(job, "id"),
            kind: string_field(job, "kind"),
            label: string_field(job, "label"),
            status: string_field(job, "status"),
            detail: job["detail"].as_str().map(str::to_string),
            started_at: job["startedAt"].as_i64().unwrap_or(0),
            finished_at: job["finishedAt"].as_i64().unwrap_or(0),
        })
        .collect()
}

/// 一帧 `session/control` 落了哪几块（主干据此分别刷队列面板 / 作业面板 / 状态条）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlDelta {
    pub queues: bool,
    pub jobs: bool,
    pub projections: bool,
}

impl ControlDelta {
    pub fn any(&self) -> bool {
        self.queues || self.jobs || self.projections
    }
}

/// `session/control` 的累积状态：三张表都按会话 id 分桶（主干的 `_queues`/`_jobs`/
/// `_sessionProjections` 是同一口径，只是它把投影只往当前会话上贴）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlState {
    pub queues: BTreeMap<String, Vec<QueueItem>>,
    pub jobs: BTreeMap<String, Vec<Job>>,
    pub projections: BTreeMap<String, Projections>,
}

/// 内核 `session/control` 发的那四型（主干 `OnControlFrame` 的 switch 表，
/// `MainWindow.xaml.cs:15207-15335`；帧形状见本节开头那串注释）。
/// 分流侧要靠这张表区分「内核真给的帧」与「不认的型别」：`ControlState::apply` 对两者
/// 都回空 delta，不查表就分不出「这一帧本来没内容」和「这一帧被静默丢掉」。
pub const CONTROL_FRAME_TYPES: [&str; 4] = ["baseline", "queue", "jobs", "projection"];

/// 这一帧的 `type` 是不是内核那四型之一。`type` 缺席或不是字符串按「不认」处理
/// （主干 `TryGetProperty("type")` 拿不到就走 `default: return`，MW:15336）。
pub fn control_frame_type(frame: &Value) -> Option<&'static str> {
    let kind = frame["type"].as_str()?;
    CONTROL_FRAME_TYPES
        .into_iter()
        .find(|known| *known == kind)
}

impl ControlState {
    /// 应用一帧流元素。未知 `type` 与坏帧一律不动状态（主干 15337 的兜底同口径）。
    pub fn apply(&mut self, frame: &Value) -> ControlDelta {
        match control_frame_type(frame) {
            Some("baseline") => {
                let value = &frame["value"];
                let mut delta = ControlDelta::default();
                if value.is_object() {
                    // 主干 15227：baseline 是全量替换，先清表再灌。
                    if let Some(entries) = value.get("queues").and_then(Value::as_object) {
                        self.queues.clear();
                        for (sid, items) in entries {
                            self.queues.insert(sid.clone(), parse_queue_items(items));
                        }
                        delta.queues = true;
                    }
                    if let Some(entries) = value.get("jobs").and_then(Value::as_object) {
                        self.jobs.clear();
                        for (sid, items) in entries {
                            self.jobs.insert(sid.clone(), parse_jobs(items));
                        }
                        delta.jobs = true;
                    }
                    if let Some(entries) = value.get("projections").and_then(Value::as_object) {
                        self.projections.clear();
                        for (sid, block) in entries {
                            self.projections
                                .insert(sid.clone(), Projections::from_block(block));
                        }
                        delta.projections = true;
                    }
                }
                delta
            }
            Some("queue") => {
                let sid = frame["sessionId"].as_str().unwrap_or_default().to_string();
                if sid.is_empty() {
                    return ControlDelta::default();
                }
                let items = parse_queue_items(&frame["items"]);
                self.queues.insert(sid, items);
                ControlDelta {
                    queues: true,
                    ..Default::default()
                }
            }
            Some("jobs") => {
                let sid = frame["sessionId"].as_str().unwrap_or_default().to_string();
                if sid.is_empty() {
                    return ControlDelta::default();
                }
                self.jobs.insert(sid, parse_jobs(&frame["jobs"]));
                ControlDelta {
                    jobs: true,
                    ..Default::default()
                }
            }
            Some("projection") => {
                let sid = frame["sessionId"].as_str().unwrap_or_default().to_string();
                let key = match frame["key"].as_str() {
                    Some(key) => key,
                    None => return ControlDelta::default(),
                };
                self.projections
                    .entry(sid)
                    .or_default()
                    .apply_key(key, &frame["value"]);
                ControlDelta {
                    projections: true,
                    ..Default::default()
                }
            }
            _ => ControlDelta::default(),
        }
    }

    pub fn queue_items(&self, session_id: &str) -> &[QueueItem] {
        self.queues.get(session_id).map_or(&[], Vec::as_slice)
    }

    pub fn jobs_of(&self, session_id: &str) -> &[Job] {
        self.jobs.get(session_id).map_or(&[], Vec::as_slice)
    }

    pub fn projections(&self, session_id: &str) -> Option<&Projections> {
        self.projections.get(session_id)
    }

    /// #51 轮次轨的数据源：该会话的每轮大纲（没有投影就是空表）。
    pub fn turn_outline(&self, session_id: &str) -> &[TurnOutlineItem] {
        self.projections(session_id)
            .map_or(&[], |projections| projections.turn_outline.as_slice())
    }
}

// ==================== 斜杠命令（`commands/list` / `commands/execute`） ====================
//
// 主干实际发出的 JSON（`MainWindow.xaml.cs:17326`、`:17883-17888`）：
//   commands/list     params = `{agentId}`            ← `agentId` 就是活动会话 id
//                                                  （17310 `Volatile.Read(ref _activeSessionId)`；
//                                                   17303 注释：内核 lookup "agent" = 会话 id）
//   commands/execute  params = `{agentId, line, submittedAttachments}`  ← 平铺，无 `request` 包裹
// `commands/list` 走 `CallOkAsync`（只拿 `value`），`commands/execute` 走 `CallAsync`
// （要自己看信封的 `ok`，因为内核可以给「没有 value」这个第三态）。

/// `commands/list` 的一行（主干 `CommandVm`，`MainWindow.xaml.cs:307-318` + 解析 17334-17342）：
/// `{name, description, input?:{hint?}}`；`input` 是对象就算「带参数」，`hint` 是参数提示。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandEntry {
    pub name: String,
    pub description: String,
    pub hint: String,
    pub has_input: bool,
}

impl CommandEntry {
    /// 主干 `CommandVm.SlashName`。
    pub fn slash_name(&self) -> String {
        format!("/{}", self.name)
    }
}

/// `commands/execute` 的 `submittedAttachments` 元素（主干 17871 / 17880）：
/// 图片内联 base64，文件走先上传拿到的 receipt。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmittedAttachment {
    Image {
        media_type: String,
        data: String,
        name: String,
    },
    File {
        receipt_id: String,
    },
}

impl SubmittedAttachment {
    pub fn to_value(&self) -> Value {
        match self {
            Self::Image {
                media_type,
                data,
                name,
            } => json!({ "type": "image", "mediaType": media_type, "data": data, "name": name }),
            Self::File { receipt_id } => json!({ "type": "file", "receiptId": receipt_id }),
        }
    }
}

/// `commands/execute` 的回执四态（主干 17889-17914 的四个分支）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandReply {
    /// 信封 `ok:true` 但**没有 value**（或 value 不是对象）：未识别/格式错误的行。
    Unknown,
    /// 有 value 但没有 `result`：命令已提交，内核没给即时应答。
    NoImmediateReply,
    /// `result.{kind,text}`：`kind` ∈ `success` | `error` | 其它（其它按主干原样回显 kind）。
    Result { kind: String, text: Option<String> },
}

impl CommandReply {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Result { kind, .. } if kind == "success")
    }

    /// 回执的 `kind`：`Unknown` / `NoImmediateReply` 两态没有 kind，给空串。
    /// 主干 17913 那条兜底分支就是把非 success 非 error 的 kind 原样回显出来。
    pub fn kind_of(&self) -> &str {
        match self {
            Self::Result { kind, .. } => kind,
            _ => "",
        }
    }

    /// 内核给的回执正文（`result.text`），缺席就是 `None` 由 UI 侧兜文案。
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Result { text, .. } => text.as_deref(),
            _ => None,
        }
    }
}

/// 剥一层 `ok` 之后的 `commands/execute` 结果 → 回执四态（纯函数）。
pub fn parse_command_reply(value: &Value) -> CommandReply {
    let Some(result) = value.get("result").filter(|result| result.is_object()) else {
        return if value.is_object() {
            CommandReply::NoImmediateReply
        } else {
            CommandReply::Unknown
        };
    };
    CommandReply::Result {
        kind: string_field(result, "kind"),
        text: result["text"].as_str().map(str::to_string),
    }
}

/// `commands/list` 的 value → 命令目录（纯函数；非对象的条目跳过，同主干 17330）。
pub fn parse_commands(value: &Value) -> Vec<CommandEntry> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            item.is_object().then(|| {
                let input = item.get("input").filter(|input| input.is_object());
                CommandEntry {
                    name: string_field(item, "name"),
                    description: string_field(item, "description"),
                    hint: input
                        .as_ref()
                        .and_then(|input| input["hint"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    has_input: input.is_some(),
                }
            })
        })
        .collect()
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
                    // 只有 deadline 走到头才算握手超时。Windows 的 condvar 允许假唤醒，
                    // `recv_timeout` 可能在预算远未用尽时先回一次 Timeout（rust-lang#77499 一族），
                    // 老写法在这里直接杀进程并报错 ⇒ `Kernel::start` 的调用方随机红一条，
                    // 而 `commands_execute_reports_every_reply_state` 那批「spawn 完就等首行」
                    // 的用例正好最常撞上。超时判定只认 `left`，不认单次的 Timeout。
                    continue;
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

    /// 会话台账（左栏那一列）。**返回值形状与历史逐字一致**：投影的全量解析走
    /// `list_session_rows`，这里只搬出 `info` 部分，现有依赖 `title` 的路径不受影响。
    pub fn list_sessions(&mut self) -> Result<Vec<SessionInfo>, String> {
        Ok(self
            .list_session_rows()?
            .into_iter()
            .map(|row| row.info)
            .collect())
    }

    /// 同一次 `session/list`，但把 `projections` 整块也解析出来（#51 轮次轨的快照源、
    /// 主干 `RefreshSessionStateFromListAsync`（4131-4176）补齐投影用的就是这一发）。
    pub fn list_session_rows(&mut self) -> Result<Vec<SessionRow>, String> {
        let value = self.call("session/list", json!({ "_request": {} }))?;
        Ok(parse_session_rows(&value))
    }

    /// `commands/list`：params 就一个平铺的 `agentId`（主干 17326，`agentId` = 活动会话 id）。
    /// 内核按 locale 出 `description`，失败（含 `gateway/lookup-not-found`）由调用方按 3s
    /// 负缓存处理（主干 17304-17306 的自愈窗口），这里不掺缓存。
    pub fn list_commands(&mut self, agent_id: &str) -> Result<Vec<CommandEntry>, String> {
        let value = self.call("commands/list", json!({ "agentId": agent_id }))?;
        Ok(parse_commands(&value))
    }

    /// `commands/execute`：params 平铺 `{agentId, line, submittedAttachments}`（主干 17883）。
    /// `Kernel::call` 只剥外层信封的一层 `ok`，剩下的 `value` 就是命令的业务回执；
    /// 内核「未识别的行」在 JSON 里根本没有 `value` 键，这里统一落成 `CommandReply::Unknown`。
    /// `ok:false` 走 `Err`（主干据此报「命令失败：error.message」）。
    pub fn execute_command(
        &mut self,
        agent_id: &str,
        line: &str,
        submitted_attachments: &[SubmittedAttachment],
    ) -> Result<CommandReply, String> {
        let attachments: Vec<Value> = submitted_attachments
            .iter()
            .map(SubmittedAttachment::to_value)
            .collect();
        let value = self.call(
            "commands/execute",
            json!({
                "agentId": agent_id,
                "line": line,
                "submittedAttachments": attachments,
            }),
        )?;
        Ok(parse_command_reply(&value))
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

    /// 投影全量解析：`title` 之外的既有键一个不丢（未建模的 `todos` 还在 `values` 里），
    /// 主干读得到的 `plan`/`permissions`/`sessionStats`/`turnOutline` 各出自己的字段。
    #[test]
    fn projections_keep_every_key_and_type_the_ones_mainline_reads() {
        let block = json!({
            "asOfSeq": 412,
            "values": {
                "title": "重构登录流程",
                // strict 八字段（`sessionStatsSchema`）+ 一个真内核产不出的键：`.strict()` 会直接把
                // `totalTokens` 拒掉，这里留它是为了验「没建模的键不许丢」。
                "sessionStats": {
                    "turns": 7, "steps": 12, "llmMs": 4200, "toolMs": 900,
                    "ttftMs": 310.5, "ttftSteps": 6, "decodeMs": 3300.25, "decodeTokens": 1580,
                    "totalTokens": 1234,
                },
                "plan": { "active": true, "pending": false },
                "permissions": {
                    "options": [
                        // 内核 `optionOf()` 的口径：value 与 name 同源，都是**英文 id**。
                        { "value": "workspace-write", "name": "workspace-write",
                          "description": "Write inside the workspace and permitted temporary directories; wider retries require approval." },
                        { "value": "custom", "name": "Custom" },
                        // 真内核的 selectSchema 里 value/name 都必填 ⇒ 下面这条产不出来，
                        // 纯粹喂解析器的防御分支。
                        { "name": "没有 value 的坏选项" },
                    ],
                    "currentValue": "workspace-write",
                },
                "turnOutline": [
                    { "turn": 2, "seq": 88, "prompt": "第二问", "response": "第二答" },
                    { "turn": 1, "seq": 3, "prompt": "第一问" },
                    { "turn": 0, "seq": 1 },
                    { "turn": 3 },
                ],
                "todos": { "open": 2 },
            },
        });
        let parsed = Projections::from_block(&block);
        assert_eq!(parsed.as_of_seq, Some(412));
        assert_eq!(parsed.title, "重构登录流程");
        assert_eq!(
            parsed.stats,
            Some(SessionStats {
                turns: 7,
                steps: 12,
                llm_ms: 4200.0,
                tool_ms: 900.0,
                ttft_ms: 310.5,
                decode_ms: 3300.25,
                decode_tokens: 1580.0,
                ttft_steps: 6,
            })
        );
        assert_eq!(
            parsed.plan,
            Some(PlanProjection {
                active: true,
                pending: false,
            })
        );
        let permissions = parsed.permissions.clone().expect("permissions 是对象");
        // 主干 15377：没有 `value` 的选项整个不要；有 value 没 name 的回落 value。
        assert_eq!(
            permissions.options,
            vec![
                PermissionOption {
                    value: "workspace-write".into(),
                    name: "workspace-write".into(),
                    description: "Write inside the workspace and permitted temporary directories; wider retries require approval.".into(),
                },
                // `custom` 是派生态（`optionOf` 给 name "Custom"），description 可缺席。
                PermissionOption {
                    value: "custom".into(),
                    name: "Custom".into(),
                    description: String::new(),
                },
            ]
        );
        assert_eq!(permissions.current_value.as_deref(), Some("workspace-write"));
        // 硬约束（主干 15957）：内核给的 name 就是英文 id，**必须过 permission_preset_zh**
        // 才是中文标签。桩一旦直接发中文，这条约束就被掩盖、上真机立刻露馅。
        let zh = Catalog::load("zh", None);
        let en = Catalog::load("en", None);
        assert_eq!(
            permission_preset_zh(&zh, permissions.options[0].value.as_str()),
            "工作区内修改"
        );
        assert_eq!(permission_preset_zh(&zh, "custom"), "自定义");
        assert_eq!(permission_preset_zh(&en, "workspace-write"), "Workspace write");
        assert_eq!(
            permission_preset_zh(&zh, "danger-full-access"),
            "完全权限",
            "主干表里的五个档一个都不许漏"
        );
        assert_eq!(permission_preset_zh(&zh, "read-only"), "仅可查看");
        assert_eq!(permission_preset_zh(&zh, "auto-approve"), "自动审批");
        assert_eq!(
            permission_preset_zh(&zh, "third-party-preset"),
            "third-party-preset",
            "认不得的 id 原样回显（主干的 `_ => value`）"
        );
        // 坏条目丢弃（turn<=0 / 缺 seq），预览字段缺席降级成空串，最后按 turn 升序。
        assert_eq!(
            parsed.turn_outline,
            vec![
                TurnOutlineItem {
                    turn: 1,
                    seq: 3,
                    prompt: "第一问".into(),
                    response: String::new(),
                },
                TurnOutlineItem {
                    turn: 2,
                    seq: 88,
                    prompt: "第二问".into(),
                    response: "第二答".into(),
                },
            ]
        );
        // 未建模的键：整块留着，UI 以后要读不必回炉解析。
        assert_eq!(parsed.value_of("todos"), Some(&json!({ "open": 2 })));
        assert_eq!(
            parsed.value_of("sessionStats").unwrap()["totalTokens"],
            json!(1234)
        );
        // 内核的「旧会话整块缺席」（主干 2827 的容错注释）就是空投影，不是错。
        assert!(Projections::from_block(&json!(null)).is_empty());
        assert!(Projections::from_block(&json!({ "asOfSeq": 5 })).is_empty());
        assert!(!parsed.is_empty());
    }

    /// `session/control` 的四型帧各自落在哪张表上；未知 `type` 与坏 sessionId 一律不动状态。
    #[test]
    fn control_frames_fold_into_the_state_by_type() {
        let mut state = ControlState::default();
        let baseline = json!({
            "type": "baseline",
            "value": {
                "queues": {
                    "s-1": [
                        { "id": "q-1", "placement": "steering",
                          "message": { "id": "m-1", "content": [
                              { "type": "text", "text": "插一句" },
                              { "type": "image", "mediaType": "image/png", "data": "AA" },
                          ] } },
                        { "message": { "content": [] } },
                    ],
                },
                "jobs": {
                    "s-1": [
                        { "id": "j-1", "kind": "bash-1", "label": "cargo check",
                          "status": "running", "startedAt": 100 },
                        { "id": "j-2", "kind": "subagent-2", "label": "查资料",
                          "status": "completed", "startedAt": 50, "finishedAt": 90 },
                    ],
                },
                "projections": {
                    "s-1": { "asOfSeq": 90, "values": { "title": "甲", "plan": { "active": true } } },
                    "s-2": { "asOfSeq": 3, "values": { "title": "乙" } },
                },
            },
        });
        let delta = state.apply(&baseline);
        assert!(delta.queues && delta.jobs && delta.projections);
        assert!(delta.any());
        let queued = &state.queues["s-1"];
        // 没有 id 的条目按主干 15430 整条丢掉。
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].item_id, "q-1");
        assert_eq!(queued[0].placement, "steering");
        // 数据层只拼 text 块，图片/文件的占位文案留给 UI（主干那是本地化字面量）。
        assert_eq!(queued[0].text(), "插一句");
        assert_eq!(queued[0].content[1]["type"], json!("image"));
        assert_eq!(state.jobs_of("s-1").len(), 2);
        assert!(state.jobs_of("s-1")[0].is_live());
        assert!(!state.jobs_of("s-1")[1].is_live());
        assert_eq!(state.projections("s-2").unwrap().title, "乙");
        assert_eq!(
            state.projections("s-1").unwrap().as_of_seq,
            Some(90),
            "baseline 的 projections 是按会话分桶的快照"
        );

        // queue 帧 = 该会话队列的完整替换（不影响别的会话、也不碰 jobs）。
        let delta = state.apply(&json!({ "type": "queue", "sessionId": "s-1", "items": [] }));
        assert!(delta.queues && !delta.jobs && !delta.projections);
        assert!(state.queue_items("s-1").is_empty());
        assert_eq!(state.queue_items("s-2"), &[]);

        // jobs 帧同理，detail 可缺席。
        let delta = state.apply(&json!({
            "type": "jobs", "sessionId": "s-1",
            "jobs": [{ "id": "j-3", "kind": "bash-1", "label": "git push", "status": "stopping" }],
        }));
        assert!(delta.jobs && !delta.queues);
        assert_eq!(state.jobs_of("s-1").len(), 1);
        assert_eq!(state.jobs_of("s-1")[0].detail, None);
        assert!(state.jobs_of("s-1")[0].is_live(), "stopping 也算存活");

        // projection 帧：单键整块替换，其余键与 asOfSeq 不动（主干 15316 的 turnOutline 分支）。
        let delta = state.apply(&json!({
            "type": "projection", "sessionId": "s-1", "key": "turnOutline", "seq": 91,
            "value": [{ "turn": 1, "seq": 12, "prompt": "问", "response": "答" }],
        }));
        assert!(delta.projections && !delta.queues && !delta.jobs);
        assert_eq!(
            state.turn_outline("s-1"),
            &[TurnOutlineItem {
                turn: 1,
                seq: 12,
                prompt: "问".into(),
                response: "答".into(),
            }]
        );
        let s1 = state.projections("s-1").expect("投影还在");
        assert_eq!(s1.as_of_seq, Some(90), "增量帧不许动游标");
        assert_eq!(s1.title, "甲", "增量帧不许动别的键");
        assert!(s1.plan.unwrap().active);

        // 未知型别 / 判别键写错位置的帧：状态一个字都不动。
        let before = state.clone();
        assert!(!state.apply(&json!({ "type": "heartbeat" })).any());
        assert!(!state
            .apply(&json!({ "kind": "queue", "sessionId": "s-1", "items": [] }))
            .any());
        assert!(!state.apply(&json!({ "type": "queue", "items": [] })).any());
        assert!(!state
            .apply(&json!({ "type": "projection", "sessionId": "s-1" }))
            .any());
        assert_eq!(state, before, "坏帧不许把模型带歪");
        // baseline 没带某张表时，那张表保持原样（只有带来的才整表替换）。
        let delta = state.apply(&json!({ "type": "baseline", "value": { "queues": {} } }));
        assert!(delta.queues && !delta.jobs && !delta.projections);
        assert_eq!(state.jobs_of("s-1").len(), 1);
    }

    /// 分流侧认帧型用的就是这张表（`CONTROL_FRAME_TYPES`），不是各写一份 `match`。
    /// 判不出「未识别」与「这帧本来就空」的代价 = 帧被静默丢掉（spec6 R1）。
    #[test]
    fn control_frame_table_recognizes_only_the_types_the_kernel_sends() {
        for kind in CONTROL_FRAME_TYPES {
            assert_eq!(control_frame_type(&json!({ "type": kind })), Some(kind));
        }
        // `$events` 的首帧、心跳、以及判别键写错位置的帧。
        for frame in [
            json!({ "type": "ready", "clientId": "c" }),
            json!({ "type": "emit", "event": "api-session/status", "args": ["s-1", true] }),
            json!({ "type": "snapshot", "sessionId": "s-1" }),
            json!({ "kind": "baseline" }),
            json!({ "type": 1 }),
            Value::Null,
        ] {
            assert_eq!(control_frame_type(&frame), None, "这一帧不该被认成控制帧: {frame}");
        }
        // `baseline` 这个名字在工作区流与会话控制流上**同名不同形**（前者 `value.items`、
        // 后者 `value.queues/jobs/projections`）⇒ 帧型分不开两条流，分流必须先从 streamId 认流：
        // 两条流各自的那一型落到对方手里都是「空 delta / 返回 false」，一声不响（spec6 R1）。
        let workspace_baseline = json!({ "type": "baseline", "value": { "items": [] } });
        assert_eq!(control_frame_type(&workspace_baseline), Some("baseline"));
        assert!(
            !ControlState::default().apply(&workspace_baseline).any(),
            "工作区的 baseline 在控制面手里只剩个空 delta"
        );
        let control_baseline = json!({ "type": "baseline", "value": { "queues": {} } });
        assert!(
            !WorkspaceTree::default().apply(&control_baseline),
            "反过来也一样：树把控制帧判成不认"
        );
    }

    /// 排队项的坏字段不致命：主干 `ParseQueueItems`（15420-15446）用 `TryGetProperty` 兜底，
    /// 缺 `message`/`content` 的条目**仍然保留**（`Content` = default、`Text` = ""），
    /// 只有缺 `id` 才整条丢。分叉此前用 `?` 传播，两端队列计数会不一致（「排队中 · N 条」对不上）。
    #[test]
    fn queue_items_survive_missing_message_and_content() {
        let items = parse_queue_items(&json!([
            // 整条齐备的对照组。
            { "id": "q-1", "placement": "queued",
              "message": { "id": "m-1", "content": [{ "type": "text", "text": "等这轮跑完" }] } },
            // 有 message、没有 content。
            { "id": "q-2", "placement": "steering", "message": { "id": "m-2" } },
            // 连 message 都没有（内核刚收下、内容尚未落账的形态）。
            { "id": "q-3" },
            // message 不是对象 / content 不是数组：坏字段不致命，条目留着。
            { "id": "q-4", "message": 7 },
            { "id": "q-5", "message": { "content": "不是一段字符串" } },
            // 唯一真正丢人的是缺 id（主干 15430）。
            { "message": { "content": [] } },
        ]));
        assert_eq!(
            items.iter().map(|item| item.item_id.as_str()).collect::<Vec<_>>(),
            ["q-1", "q-2", "q-3", "q-4", "q-5"],
            "只有缺 id 的条目被丢掉，缺 message/content 的必须留着"
        );
        assert_eq!(items[0].text(), "等这轮跑完");
        assert_eq!(items[0].placement, "queued");
        // 缺 content ⇒ Null（主干 `default`），拼文案得到空串而不是崩。
        assert_eq!(items[1].content, Value::Null);
        assert_eq!(items[1].text(), "");
        assert_eq!(items[1].placement, "steering");
        assert_eq!(items[2].content, Value::Null);
        // placement 缺席/空白都回落 queued（主干 15433 同口径）。
        assert_eq!(items[3].placement, "queued");
        assert_eq!(items[4].content, json!("不是一段字符串"), "内核给什么就原样留着");
        // 非数组的整块输入仍是空表（主干 ValueKind != Array 的早退）。
        assert!(parse_queue_items(&json!({ "id": "q-1" })).is_empty());
    }

    /// `commands/list` / `commands/execute` 的纯解析：目录条目照主干 17334-17342，
    /// 回执四态照主干 17889-17914（`Kernel::call` 只剥外层信封的一层 `ok`）。
    #[test]
    fn command_catalog_and_reply_states_parse_like_the_trunk() {
        let catalog = parse_commands(&json!([
            { "name": "compact", "description": "Compact older conversation history" },
            { "name": "plan", "description": "Enter or leave plan mode",
              "input": { "hint": "[off|message]", "attachments": true } },
            { "name": "broken", "input": null },
            // 内核注册表要求 `input.hint` 是非空字符串（`dsh-commands/lib/index.js:151-152`
            // 的两条 TypeError），所以 `input:{}` 线上产不出；留着只验解析器不被带歪。
            { "name": "hintless", "input": {} },
            "not-an-object",
        ]));
        assert_eq!(catalog.len(), 4, "非对象条目跳过，对象条目一律收: {catalog:?}");
        assert_eq!(catalog[0].slash_name(), "/compact");
        assert!(!catalog[0].has_input && catalog[0].hint.is_empty());
        assert!(catalog[1].has_input);
        assert_eq!(catalog[1].hint, "[off|message]");
        // `input: null` 在内核是「该命令没有输入」，主干据此把 HasInput 判成 false。
        assert!(!catalog[2].has_input);
        // 未建模的 `attachments` 键不影响读法：仍算带参数、hint 空串。
        assert!(catalog[3].has_input && catalog[3].hint.is_empty());
        assert!(parse_commands(&json!({ "items": [] })).is_empty());

        assert_eq!(parse_command_reply(&Value::Null), CommandReply::Unknown);
        assert_eq!(
            parse_command_reply(&json!("一条字符串")),
            CommandReply::Unknown,
            "value 不是对象也按未识别处理（主干 17897 同一条分支）"
        );
        assert_eq!(
            parse_command_reply(&json!({})),
            CommandReply::NoImmediateReply,
            "真内核的 value 是 undefined 或 {{commandId, result}} 二选一（dsh-commands 的 \
             commands_execute_result$schema）⇒ 有 value 就必有 result，这一态线上不可达，纯防御留在这里"
        );
        // 线上真形状：外层多一个必填的 `commandId`，解析器只认 `result`。
        assert_eq!(
            parse_command_reply(
                &json!({ "commandId": "cmd-fake-1", "result": { "kind": "success", "text": "已压缩" } })
            ),
            CommandReply::Result {
                kind: "success".into(),
                text: Some("已压缩".into()),
            }
        );
        // success 的 text 是 optional（主干 17911 的「{0} 执行完成」文案走这一支）。
        assert_eq!(
            parse_command_reply(&json!({ "result": { "kind": "success" } })),
            CommandReply::Result {
                kind: "success".into(),
                text: None,
            }
        );
        assert_eq!(
            parse_command_reply(&json!({ "result": { "kind": "success", "text": "已压缩" } })),
            CommandReply::Result {
                kind: "success".into(),
                text: Some("已压缩".into()),
            }
        );
        let error = parse_command_reply(&json!({ "result": { "kind": "error", "text": "参数不对" } }));
        assert!(!error.is_success());
        assert_eq!(error.text(), Some("参数不对"));
        // 既不是 success 也不是 error 的 kind：主干按 `{line} → {kind}` 回显，模型原样留。
        let odd = parse_command_reply(&json!({ "result": { "kind": "queued" } }));
        assert_eq!(
            odd,
            CommandReply::Result {
                kind: "queued".into(),
                text: None,
            }
        );
        assert_eq!(odd.text(), None);

        // 附件的线上形状（主干 17871 / 17880）。
        assert_eq!(
            SubmittedAttachment::Image {
                media_type: "image/png".into(),
                data: "AA==".into(),
                name: "shot.png".into(),
            }
            .to_value(),
            json!({ "type": "image", "mediaType": "image/png", "data": "AA==", "name": "shot.png" })
        );
        assert_eq!(
            SubmittedAttachment::File {
                receipt_id: "rc-1".into(),
            }
            .to_value(),
            json!({ "type": "file", "receiptId": "rc-1" })
        );
    }
}
