use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use blade2_rs::i18n::Catalog;
use blade2_rs::kernel::{
    CONTROL_FRAME_TYPES, CommandReply, ControlDelta, ControlState, DetailsKey, DetailsLedger,
    DIRECTORY_PICKER_CREATE_DIRECTORY, DIRECTORY_PICKER_LIST, DIRECTORY_PICKER_PICK,
    DirectoryEntry, DirectoryListing, Kernel,
    Launch, PermissionsProjection, PickOutcome, PlanProjection, Projections, RpcCall, SessionStats,
    SubmittedAttachment, WORKSPACE_ARCHIVE_SESSION, WORKSPACE_CREATE, WORKSPACE_DELETE,
    WORKSPACE_INSERT_BEFORE, WORKSPACE_INSERT_SESSION_BEFORE, WORKSPACE_RENAME, Workspace,
    WorkspaceCreated, WorkspaceTree, WorkspaceView, control_frame_type,
    directory_picker_create_directory, directory_picker_list, directory_picker_pick,
    page_events, parse_archived_session_ids, parse_created_directory, parse_pick,
    parse_workspace_created, parse_workspace_deleted, parse_workspace_ids, parse_workspace_value,
    permission_preset_zh, session_status_event, workspace_archive_session, workspace_create,
    workspace_delete, workspace_insert_before, workspace_insert_session_before, workspace_rename,
};
use blade2_rs::mux::{Mux, MuxEvent};
use serde_json::{Value, json};

/// 单次收帧的最长等待：假内核本就秒回，超时只可能它挂了，绝不能把测试拖住。
const WAIT_BUDGET: Duration = Duration::from_secs(5);
/// 断言「不该再有帧」用的短窗口。
const QUIET_BUDGET: Duration = Duration::from_millis(300);
/// 假内核一轮往 follow 流上推多少帧：30 条 journal 事件 + 6 个逐字增量元素。
/// 脚本加事件时改这一个常量即可（`turn_tags` 与节流出题点都按它算）。
const TURN_FRAMES: usize = 36;
/// 验节流用的帧间毫秒数（关节奏对照用）：比默认的 200ms 小一个量级，跑得起又不为零。
const PACE_MS: u64 = 40;
/// 验「中途态真存在」用的帧间毫秒数：**必须明显大于 mux 客户端的读超时 `READ_TICK`（120ms）**。
/// `Mux::poll` 一次会把「直到读超时为止」到达的帧全并进同一批，40ms 的间隔因此会被一整轮喂成
/// 一批，测试侧根本量不到「只到了一半」。150ms 才有「一帧一批」的可观测粒度。
const MID_PACE_MS: u64 = 150;
/// 补齐整轮用的宽预算：只当上限，帧一到就返回。节流轮要 36×150ms≈5.5s，复用 `WAIT_BUDGET`
/// 会把「节流确实生效」误判成超时。
const DRAIN_BUDGET: Duration = Duration::from_secs(12);

/// 测试侧一律 `--pace=0`：假内核的默认帧间节流（200ms/帧，见 `fake_dsh.rs` 的 `DEFAULT_PACE_MS`）
/// 是给 UI 抢拍「流式中」那一帧用的，而这批用例的收帧预算全按「一轮一次推完」写死——
/// 开着节奏跑只会变慢并撞穿 `QUIET_BUDGET`/`WAIT_BUDGET`，判成假失败。
fn fake_launch() -> Launch {
    Launch {
        exe: PathBuf::from(env!("CARGO_BIN_EXE_fake_dsh")),
        args: vec!["--pace=0".to_string()],
        // 假内核既不是内置第 1 级的 `node.exe + bin.js`（`Kernel::start` 走 `command.args()` 的
        // 正常列表转义即可，那颗 `--pace=0` 不含引号），也不是回退级 `cmd.exe` 的那一颗原样串
        // ⇒ `args_verbatim = false`。
        args_verbatim: false,
        // `is_bundled` 的口径是「内置那两颗 `File.Exists` 的与」，与本次实际起的是谁无关（§1.7-1）。
        // 这批用例刻意把内核换成 `fake_dsh`、绕开 `Launch::from_env` 的两级解析，因此不能谎称
        // 内置在位（真机 `Kernel/` 在不在测试目录下随构建方式而变，写 `true` 就是引入非确定性）。
        is_bundled: false,
        dsh_home: None,
        path_prepend: None,
        working_dir: None,
    }
}

/// 同上，但把帧间节流开成 `pace_ms`：只有验节流的用例走这里。
fn paced_launch(pace_ms: u64) -> Launch {
    Launch {
        args: vec![format!("--pace={pace_ms}")],
        ..fake_launch()
    }
}

/// 一轮该推上来的帧序（元素级标签）：`session_follow_streams_a_full_prompt_turn` 与
/// 节流用例共用同一份期望 ⇒ 节流只许改「什么时候到」，不许改「到什么」。
fn turn_tags() -> Vec<&'static str> {
    let mut tags = vec![
        "event:turn/start",
        "event:user/message",
        "event:request/header",
        "start",
        "chunk:text-delta",
        "chunk:text-delta",
        "chunk:text-delta",
        "chunk:reasoning-delta",
        "end",
        "event:assistant/attempt",
    ];
    for _ in 0..10 {
        tags.extend(["event:tool/call", "event:tool/result"]);
    }
    tags.extend([
        "event:system/message",
        "event:system/message",
        "event:assistant/message",
        "event:deliverables/presented",
        "event:session/title",
        "event:turn/end",
    ]);
    tags
}

fn event_stream(event: &MuxEvent) -> &str {
    match event {
        MuxEvent::Item { stream, .. }
        | MuxEvent::End { stream }
        | MuxEvent::Failure { stream, .. }
        | MuxEvent::Cancelled { stream } => stream,
    }
}

/// 有界收帧：`Mux::collect` 一次把同期到达的帧全带回，攒够 `want` 个或 `budget` 用尽就返回。
fn collect_within(mux: &mut Mux, stream: &str, want: usize, budget: Duration) -> Vec<MuxEvent> {
    let deadline = Instant::now() + budget;
    let mut events = Vec::new();
    while events.len() < want {
        let left = deadline.saturating_duration_since(Instant::now());
        events.extend(
            mux.collect(left)
                .expect("mux 读帧不应失败")
                .into_iter()
                .filter(|event| event_stream(event) == stream),
        );
        if left.is_zero() {
            break;
        }
    }
    events
}

fn collect(mux: &mut Mux, stream: &str, want: usize) -> Vec<MuxEvent> {
    collect_within(mux, stream, want, WAIT_BUDGET)
}

/// 断言是 item 并把 `value` 搬出来；void 结果原样返回 `None`。
fn expect_item(event: Option<&MuxEvent>, stream: &str) -> Option<Value> {
    match event {
        Some(MuxEvent::Item { value, .. }) => value.clone(),
        other => panic!("期望 {stream} 的 item，实际 {other:?}"),
    }
}

/// 一批 item 的 `value` 依次搬出来；混进 end/failure 或 void 就直接炸。
fn item_values(events: &[MuxEvent], stream: &str) -> Vec<Value> {
    (0..events.len())
        .map(|index| {
            expect_item(events.get(index), stream)
                .unwrap_or_else(|| panic!("{stream} 的流元素不该是 void"))
        })
        .collect()
}

/// 流元素压成一行标签：断言整轮顺序时不必把整棵 JSON 树铺进失败信息。
fn tag(frame: &Value) -> String {
    match frame["type"].as_str().unwrap_or_default() {
        "event" => format!(
            "event:{}",
            frame["event"]["type"].as_str().unwrap_or_default()
        ),
        "assistant-stream" => {
            let inner = &frame["frame"];
            match inner["type"].as_str().unwrap_or_default() {
                "chunk" => format!(
                    "chunk:{}",
                    inner["chunk"]["type"].as_str().unwrap_or_default()
                ),
                other => other.to_string(),
            }
        }
        other => other.to_string(),
    }
}

/// 主干的发送载荷：`requestId`/`sessionId`/`mode`/`content` 四项齐备。
fn prompt_args(session: &str, text: &str, mode: &str) -> Value {
    json!({
        "request": {
            "requestId": "c2-fake-1",
            "sessionId": session,
            "mode": mode,
            "content": [{ "type": "text", "text": text }],
        },
    })
}

/// 取 `tool/call` 的 arguments：字符串口径先解 JSON，对象口径直接用——主干
/// `apply_journal_event` 与分叉的 `mutation_path` 都是这两条路。
fn arguments_of(event: &Value) -> Value {
    match event["data"]["arguments"].as_str() {
        Some(text) => serde_json::from_str(text).expect("字符串口径也得是合法 JSON"),
        None => event["data"]["arguments"].clone(),
    }
}

/// 按 callId 取那条 `tool/call` 事件（本轮脚本里 callId 不重复）。
fn call_event<'v>(journal: &[&'v Value], call_id: &str) -> &'v Value {
    journal
        .iter()
        .copied()
        .find(|event| {
            event["type"].as_str() == Some("tool/call")
                && event["data"]["callId"].as_str() == Some(call_id)
        })
        .unwrap_or_else(|| panic!("脚本里该有 {call_id} 这条 tool/call"))
}

/// 按 callId 取那条 `tool/result` 的 data（撤回取证面全挂在它的 `meta` 上）。
fn result_data<'v>(journal: &[&'v Value], call_id: &str) -> &'v Value {
    journal
        .iter()
        .copied()
        .find(|event| {
            event["type"].as_str() == Some("tool/result")
                && event["data"]["message"]["source"]["callId"].as_str() == Some(call_id)
        })
        .map(|event| &event["data"])
        .unwrap_or_else(|| panic!("脚本里该有 {call_id} 这条 tool/result"))
}

/// 主干 `ToolResultText` 的镜像：正文在 `message.content[].content[]` 那一层里
/// （内核 `createToolResultMessage` 的形状），外层块上的 `text` 只是给只看 `content[0]`
/// 的壳用的同一份副本。两条读法都断言一遍，假内核少给哪一层都会在这里红。
fn result_text(data: &Value) -> String {
    let blocks = data["message"]["content"].as_array().expect("content 是数组");
    let parts: Vec<&str> = blocks
        .iter()
        .flat_map(|block| {
            block["content"]
                .as_array()
                .map(|inner| inner.iter().filter_map(|piece| piece["text"].as_str()).collect::<Vec<_>>())
                .unwrap_or_default()
        })
        .collect();
    assert!(!parts.is_empty(), "结果正文得能从内层 content[] 读到");
    assert_eq!(
        parts.join("\n"),
        blocks[0]["text"].as_str().unwrap_or_default(),
        "内层正文与外层块上的 text 必须是同一份"
    );
    parts.join("\n")
}

/// 主干 `ResultPathFromEnvelope` 的镜像：write/edit 结果信封里的 `<path>…</path>`。
fn envelope_path(text: &str) -> Option<&str> {
    let start = text.find("<path>")? + "<path>".len();
    Some(text[start..].find("</path>").map(|end| text[start..start + end].trim())?)
}

/// 主干 `ResultPathFromSentence` 的镜像：`str_replace_editor` 的整句结果里取路径。
fn sentence_path(text: &str) -> Option<&str> {
    const CREATED: &str = "New file created successfully at: ";
    const EDITED: &str = "The file ";
    if let Some(tail) = text.strip_prefix(CREATED) {
        return Some(tail.trim());
    }
    Some(text.strip_prefix(EDITED)?.split(" has been").next()?)
}

/// 结果正文给出的文件路径：两条分支（信封 / 整句）任一命中即可，与主干取路径的先后一致。
fn cited_path(text: &str) -> Option<&str> {
    envelope_path(text).or_else(|| sentence_path(text))
}

/// 一条 hunk 的形态归类（主干 `HunksFromMeta` + `RevertOneMutation` 的三分法）。
fn hunk_form(diff: &Value) -> &'static str {
    let added = diff["oldText"].is_null() || diff.get("oldText").is_none();
    if added {
        "新增"
    } else if diff["newText"].as_str().is_none_or(str::is_empty) {
        "删除"
    } else {
        "修改"
    }
}


/// 分叉 `mutation_path`（主线 `MutationPath` + `ValidEditArgs` + `EditorMutationPath`）的测试侧
/// 镜像：一等突变工具（write / edit / str_replace_editor）且参数齐备才报目标路径。
/// 它在 bin 里是私有的，ipc 测试只能拿 journal 的公开字段按同一套判定复算。
fn mutation_path<'a>(name: &str, args: &'a Value) -> Option<&'a str> {
    let path = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
    };
    let valid_edit = {
        let old = args.get("old_string").and_then(Value::as_str);
        let new = args.get("new_string").and_then(Value::as_str);
        matches!((old, new), (Some(old), Some(new)) if !old.is_empty() && old != new)
    } && args.get("replace_all").is_none_or(|flag| flag.is_boolean());
    match name {
        "write" => args
            .get("content")
            .and_then(Value::as_str)
            .and_then(|_| path("file_path")),
        "edit" if valid_edit => path("file_path"),
        "str_replace_editor"
            if args["command"].as_str() == Some("create")
                && args.get("file_text").is_some_and(Value::is_string) =>
        {
            path("path")
        }
        _ => None,
    }
}

/// 一轮 journal 事件 → 答案气泡上该有的「本轮文件改动」路径表：`tool/call` 按 callId 登记
/// （非突变/参数残缺登记空串），`tool/result` 成功且命中非空登记才计数，并截到答案自己的
/// seq——与分叉 `produced_paths`/`sync_produced` 同口径。
fn produced_paths(events: &[&Value]) -> Vec<String> {
    let answer_seq = events
        .iter()
        .find(|event| event["type"].as_str() == Some("assistant/message"))
        .and_then(|event| event["seq"].as_i64())
        .expect("脚本里得有 assistant/message");
    let mut registered: Vec<(String, String)> = Vec::new();
    let mut produced: Vec<String> = Vec::new();
    for event in events {
        let seq = event["seq"].as_i64().unwrap_or_default();
        match event["type"].as_str().unwrap_or_default() {
            "tool/call" => {
                let call_id = event["data"]["callId"].as_str().unwrap_or_default();
                if call_id.is_empty() {
                    continue;
                }
                let path = mutation_path(
                    event["data"]["name"].as_str().unwrap_or_default(),
                    &arguments_of(event),
                )
                .unwrap_or_default()
                .to_string();
                registered.retain(|(id, _)| id != call_id);
                registered.push((call_id.to_string(), path));
            }
            "tool/result" => {
                let message = &event["data"]["message"];
                let call_id = message["source"]["callId"].as_str().unwrap_or_default();
                // 官方只看 content[0] 的 isError：报错的突变一条都不算。
                let is_error = message["content"]
                    .as_array()
                    .and_then(|blocks| blocks.first())
                    .is_some_and(|block| block["isError"].as_bool().unwrap_or(false));
                if is_error || call_id.is_empty() || seq > answer_seq {
                    continue;
                }
                let hit = registered
                    .iter()
                    .find(|(id, _)| id == call_id)
                    .map(|(_, path)| path.clone())
                    .unwrap_or_default();
                if !hit.is_empty() && !produced.contains(&hit) {
                    produced.push(hit);
                }
            }
            _ => {}
        }
    }
    produced
}

/// 本轮的 `tool/call` 登记表（callId → 目标路径，空串 = 不登记），断言「谁该产出」用。
fn registered_paths(events: &[&Value]) -> Vec<(String, String)> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/call"))
        .map(|event| {
            let call_id = event["data"]["callId"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let path = mutation_path(
                event["data"]["name"].as_str().unwrap_or_default(),
                &arguments_of(event),
            )
            .unwrap_or_default()
            .to_string();
            (call_id, path)
        })
        .collect()
}

/// `session/follow` 的 open 参数（ mux 侧走 `payload.args`）。
fn follow_args(session: &str) -> Value {
    json!({
        "request": {
            "address": { "kind": "session", "sessionId": session },
            "assistantStream": true,
        },
    })
}

fn page_args(session: &str, through_seq: Option<i64>, max_messages: Option<i64>) -> Value {
    let mut request = json!({ "address": { "kind": "session", "sessionId": session } });
    if let Some(through) = through_seq {
        request["throughSeq"] = json!(through);
    }
    if let Some(max) = max_messages {
        request["maxMessages"] = json!(max);
    }
    json!({ "request": request })
}

// ==================== #78 会话历史回读：`session/page` 的读侧助手 ====================
//
// 分工照抄生产代码那两层：`records_of` 看**线上传来的信封形状**，剩下几把只吃
// `blade2_rs::kernel::page_events` 剥完信封的事件（剥壳与坏记录过滤都在产品码里，
// 测试再实现一遍就等于没测它）。
// ⇒ 所以调用点一律**两条腿各喂各的**：`records_of(&page)` 只配 `.len()` 与 `record["type"]`
//   那几把形状断言，`page_events(&page)` 才配进 `event_types`/`seqs_of`/`turn_start_seqs`/
//   `user_texts`。少剥一层，型序列读到的就全是信封自己的 `"event"`。

/// 一页 records 原样摊成数组（只看形状，不剥信封）。
fn records_of(page: &Value) -> Vec<Value> {
    page["records"].as_array().cloned().unwrap_or_default()
}

/// 事件型序列（比对回放形状用）。
fn event_types(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .map(|event| event["type"].as_str().unwrap_or_default())
        .collect()
}

/// 事件的信封 seq（主干按它定位回放锚点，分叉的气泡键也带它）。
fn seqs_of(events: &[Value]) -> Vec<i64> {
    events
        .iter()
        .map(|event| event["seq"].as_i64().unwrap_or_default())
        .collect()
}

/// 各轮 `turn/start` 的 seq —— 与 `seed_outline` 写死的那三个对得上，才说明
/// 「第 N 轮的锚点 seq == 大纲里那条 seq」是结构性的，不是两处抄数抄出来的。
fn turn_start_seqs(events: &[Value]) -> Vec<i64> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some("turn/start"))
        .map(|event| event["seq"].as_i64().unwrap_or_default())
        .collect()
}

/// 各轮提问正文（历史气泡的文本 + 轮轨 #65 的 tooltip 读的就是它）。
fn user_texts(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some("user/message"))
        .map(|event| {
            event["data"]["content"]
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
        })
        .collect()
}

/// 一轮卅条事件的骨架（live 那一路与历史回填共用：两边同形才算数）。
fn turn_skeleton() -> Vec<&'static str> {
    let mut types: Vec<&'static str> = vec![
        "turn/start",
        "user/message",
        "request/header",
        "assistant/attempt",
    ];
    for _ in 0..10 {
        types.extend(["tool/call", "tool/result"]);
    }
    types.extend([
        "system/message",
        "system/message",
        "assistant/message",
        "deliverables/presented",
        "session/title",
        "turn/end",
    ]);
    types
}

/// 握手 + 鉴权后接一根 mux，返回 (kernel, mux, 该 kernel 的 stream id)。
fn open_stream(endpoint: &str) -> (Kernel, Mux, String) {
    open_stream_with(endpoint, json!({}))
}

/// 同上，但带上主干那套 `{request:{…}}` 参数。
fn open_stream_with(endpoint: &str, args: Value) -> (Kernel, Mux, String) {
    open_stream_by(&fake_launch(), endpoint, args)
}

/// 开着帧间节流的版本（`pace_ms` 毫秒一帧）。
fn open_stream_paced(endpoint: &str, args: Value, pace_ms: u64) -> (Kernel, Mux, String) {
    open_stream_by(&paced_launch(pace_ms), endpoint, args)
}

/// 按给定的启动参数握手 + 鉴权 + 开一条流；关节奏与开节奏两条路共用同一套起手式。
fn open_stream_by(launch: &Launch, endpoint: &str, args: Value) -> (Kernel, Mux, String) {
    let kernel = Kernel::start(launch).expect("假内核应完成 dsh web: 握手");
    let mut mux = Mux::connect(kernel.endpoint(), kernel.cookie())
        .unwrap_or_else(|e| panic!("{endpoint} mux 握手失败: {e}"));
    let stream = mux
        .open(endpoint, args)
        .unwrap_or_else(|e| panic!("open {endpoint} 失败: {e}"));
    assert!(kernel.pid().is_some(), "假内核进程应在");
    (kernel, mux, stream)
}

#[test]
fn handshake_auth_and_rpc_against_fake_kernel() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    assert!(kernel.url.starts_with("http://127.0.0.1:"));
    assert!(kernel.pid().is_some());

    let rows = kernel.list_sessions().expect("session/list 应成功");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].id, "s-1001");
    assert_eq!(rows[0].title, "重构登录流程");
    assert!(!rows[0].blank);
    assert_eq!(rows[1].title, "新会话");
    assert!(rows[1].blank);
    assert_eq!(rows[2].parent.as_deref(), Some("s-1001"));

    let created = kernel
        .create_session("E:\\demo")
        .expect("session/create 应成功");
    assert_eq!(created, "s-2001");

    let error = kernel
        .call("nope/missing", json!({}))
        .expect_err("未知端点应报错");
    assert!(error.contains("not_found"), "{error}");

    kernel.shutdown();
}

/// 加载卡第 1 段的真刻度（缺口 #1 的数据源）：`plugin/installProgress` 回 `{ready,total}`，
/// ready 随墙钟推进、且**永不越过 total**。分叉的 `report_plugin_ledger` 就是按 250ms 抽这一发。
/// 默认（`--pace=0`，没给 `--plugins=`）必须是 `0/0` = 主线 KC:100 的「本轮无包要装」，
/// 否则任何一条既有 ipc 用例都会被莫名多出来的刻度带偏。
#[test]
fn plugin_install_ledger_reports_ready_over_total_and_advances() {
    // 1) 关着的样子：total 为 0 ⇒ 分叉解析成 None ⇒ 第 1 段匀速爬满（历史行为）。
    let mut quiet = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let value = quiet
        .call("plugin/installProgress", json!({}))
        .expect("plugin/installProgress 应成功");
    assert_eq!(value["total"], 0, "没给 --plugins= 就没有包要装：{value}");
    assert_eq!(value["ready"], 0);
    // 参数给空对象也认（分叉走的就是 `json!({})`）；非对象一律 bad_args。
    let error = quiet
        .call("plugin/installProgress", json!([]))
        .expect_err("args 给数组必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    quiet.shutdown();

    // 2) 开出台账：3 个包、每个 300ms ⇒ 起手 0/3，等过一格之后 ready 严格变大且 ready<=total。
    let launch = Launch {
        args: vec![
            "--pace=0".to_string(),
            "--plugins=3".to_string(),
            "--plugin-step=300".to_string(),
        ],
        ..fake_launch()
    };
    let mut kernel = Kernel::start(&launch).expect("带台账的假内核应完成握手");
    let first = kernel
        .call("plugin/installProgress", json!({}))
        .expect("第一发抽样");
    assert_eq!(first["total"], 3);
    let first_ready = first["ready"].as_i64().expect("ready 得是整数");
    assert!(
        first_ready <= 1,
        "刚握手完最多落一个包（握手本身不该吃掉一格）：{first}"
    );
    std::thread::sleep(Duration::from_millis(700));
    let later = kernel
        .call("plugin/installProgress", json!({}))
        .expect("第二发抽样");
    let ready = later["ready"].as_i64().expect("ready 得是整数");
    assert!(ready >= 2, "700ms / 每包 300ms 之后至少该有 2 个就绪：{later}");
    assert!(ready <= 3, "ready 永不越界（越界会把第 1 段的目标值推出 30 之外）：{later}");
    // 预算内一定装得齐：装齐之后分叉的抽样循环就该收手（ready >= total）。
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let value = kernel
            .call("plugin/installProgress", json!({}))
            .expect("补齐那一发");
        if value["ready"] == value["total"] {
            break;
        }
        assert!(Instant::now() < deadline, "台账该在预算内爬到 3/3：{value}");
        std::thread::sleep(Duration::from_millis(100));
    }
    kernel.shutdown();
}

#[test]
fn mainline_rpc_payload_shapes_are_required() {
    let mut kernel = Kernel::start(&fake_launch()).expect("握手");
    let error = kernel
        .call("session/list", json!({}))
        .expect_err("session/list 少了 _request 必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    let error = kernel
        .call("session/create", json!({ "cwd": "E:\\x" }))
        .expect_err("session/create 少了 request 包裹必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    kernel.shutdown();
}

#[test]
fn mux_workspace_follow_yields_baseline_then_upsert() {
    let (_kernel, mut mux, stream) = open_stream("workspace/follow");
    let events = collect(&mut mux, &stream, 2);
    assert_eq!(
        events.len(),
        2,
        "{stream} 应有 baseline + upsert 两个 item: {events:?}"
    );

    let baseline = expect_item(events.first(), &stream).expect("baseline 带 value");
    assert_eq!(baseline["type"], json!("baseline"));
    let items = baseline["value"]["items"]
        .as_array()
        .expect("baseline.value.items 是数组");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["workspaceId"], json!("ws-1"));
    assert_eq!(
        items[0]["sessionIds"]
            .as_array()
            .expect("有 2 个会话")
            .len(),
        2
    );
    assert_eq!(items[1]["workspaceId"], json!("ws-2"));
    assert_eq!(items[1]["sessionIds"].as_array().expect("空数组").len(), 0);
    assert_eq!(baseline["value"]["archivedSessionIds"], json!(["s-9"]));

    let upsert = expect_item(events.get(1), &stream).expect("upsert 带 value");
    assert_eq!(upsert["type"], json!("upsert"));
    assert_eq!(upsert["workspace"]["workspaceId"], json!("ws-3"));
    assert_eq!(upsert["workspace"]["path"], json!("C:/repo/three"));

    // 长流不该收尾：再等一小段也不应出现 end/failure。
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "workspace/follow 不该发 end"
    );
}

#[test]
fn mux_events_stream_sends_ready_then_emit() {
    let (_kernel, mut mux, stream) = open_stream("$events");
    let events = collect(&mut mux, &stream, 2);
    assert_eq!(events.len(), 2, "$events 应有 ready + emit: {events:?}");
    let ready = expect_item(events.first(), &stream).expect("ready 带 value");
    assert_eq!(ready["type"], json!("ready"));
    assert_eq!(ready["clientId"], json!("fake-client"));
    let emit = expect_item(events.get(1), &stream).expect("emit 带 value");
    assert_eq!(emit["type"], json!("emit"));
    assert_eq!(emit["event"], json!("api-session/status"));
    assert_eq!(emit["args"], json!(["s-1", true]));
}

/// `session/control` 的起手式：走 `Mux::open_session_control()` 那个零参发起口，
/// 也就是主干 `OpenRemoteStreamAsync("session/control", new { }, …)`
/// （`MainWindow.xaml.cs:15201`）在分叉侧的等价物 ——  args 逐字是 `{}`。
fn open_control_stream() -> (Kernel, Mux, String) {
    let kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let mut mux = Mux::connect(kernel.endpoint(), kernel.cookie()).expect("mux 握手失败");
    let stream = mux
        .open_session_control()
        .expect("open session/control 失败");
    (kernel, mux, stream)
}

/// 一条会话的投影块（`session/list` 与 `session/control` 共用形状），供多处断言复用。
fn projection_of<'a>(state: &'a ControlState, id: &str) -> &'a Projections {
    state
        .projections(id)
        .unwrap_or_else(|| panic!("baseline 里该有 {id} 的投影"))
}

#[test]
fn mux_session_control_leads_with_baseline_then_stays_open() {
    let (_kernel, mut mux, stream) = open_control_stream();
    let events = collect(&mut mux, &stream, 4);
    assert_eq!(
        events.len(),
        4,
        "{stream} 应有 baseline + queue + jobs + projection 四帧: {events:?}"
    );
    let frames = item_values(&events, &stream);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        vec!["baseline", "queue", "jobs", "projection"],
        "新流的帧序：全量 baseline 在前，增量在后（内核就是 this order）"
    );

    // ---- baseline：三张表全量灌进模型（主干 15227「先清表再灌」）----
    let mut state = ControlState::default();
    assert_eq!(
        state.apply(&frames[0]),
        ControlDelta {
            queues: true,
            jobs: true,
            projections: true,
        },
        "baseline 三张表都齐"
    );
    assert_eq!(state.queue_items("s-1001").len(), 1);
    assert_eq!(
        state.queue_items("s-1001")[0].text(),
        "等这轮跑完再说",
        "排队项正文取 text 块"
    );
    assert_eq!(state.queue_items("s-1002").len(), 0, "没有队列的会话不必出现在表里");
    assert_eq!(state.jobs_of("s-1001").len(), 1);
    assert!(state.jobs_of("s-1001")[0].is_live(), "running 算存活");
    assert_eq!(projection_of(&state, "s-1001").title, "重构登录流程");
    assert_eq!(projection_of(&state, "s-1001").turn_outline.len(), 3);
    assert_eq!(
        projection_of(&state, "s-1002").title,
        "新会话",
        "空白会话也在 baseline 里（只有 title）"
    );

    // ---- queue：整表替换一个会话（不是追加）----
    assert_eq!(
        state.apply(&frames[1]),
        ControlDelta {
            queues: true,
            ..ControlDelta::default()
        }
    );
    let queued = state.queue_items("s-1001");
    assert_eq!(queued.len(), 2, "queue 帧整表替换：baseline 那条不再作数");
    assert_eq!(queued[0].item_id, "q-2");
    assert_eq!(queued[0].placement, "steering");
    assert_eq!(queued[1].placement, "context");
    assert_eq!(
        queued[1].text(),
        "带上这份日志",
        "text 拼接只认 text 块，图片块留给 UI 侧占位"
    );
    assert_eq!(
        queued[1].content
            .as_array()
            .expect("content 是原始块数组")
            .len(),
        2,
        "图片块必须原样留着（主干「编辑只换文本块」）"
    );

    // ---- jobs：同上，且顺序就是内核给的顺序 ----
    assert!(state.apply(&frames[2]).jobs);
    let jobs = state.jobs_of("s-1001");
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].job_id, "j-2");
    assert!(!jobs[0].is_live(), "completed 不算存活");
    assert_eq!(jobs[1].kind, "bash-1");
    assert_eq!(jobs[1].label, "cargo check");
    assert!(jobs[1].is_live());

    // ---- projection：单键全量 ----
    assert!(state.apply(&frames[3]).projections);
    assert_eq!(state.turn_outline("s-1001").len(), 3);
    assert_eq!(state.turn_outline("s-1001")[2].seq, 88);
    assert_eq!(
        state
            .turn_outline("s-1001")
            .iter()
            .map(|item| item.turn)
            .collect::<Vec<_>>(),
        vec![1, 2, 3],
        "轮次严格递增"
    );

    // 长驻流不收尾：主干那条流跟到关窗，桩不许自己发 end。
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "session/control 是长驻流，不该发 end"
    );

    // 无参端点的把关：args 多一个键就撞上网关的 `assertExactArguments`。
    let noisy = mux
        .open("session/control", json!({ "_request": {} }))
        .expect("open 只把帧写出去，回错走流上");
    match collect(&mut mux, &noisy, 1).first() {
        Some(MuxEvent::Failure { stream: id, .. }) => assert_eq!(id, &noisy),
        other => panic!("非空 args 的 session/control 应被判失败，实际 {other:?}"),
    }
}

/// `Shell::open_mux` 那三发一起开（`workspace/follow` → `$events` → `session/control`），
/// 一根 socket 上混着到的帧按 streamId 分流。#60 接通的就是这一段，而 spec6 R1 说的
/// 「只开流不分流 = 静默丢帧」只有拿真线上帧才验得出来：两条流的 `baseline` **同名不同形**。
#[test]
fn mux_three_streams_demix_by_stream_id() {
    let kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let mut mux = Mux::connect(kernel.endpoint(), kernel.cookie()).expect("mux 握手失败");
    let workspace = mux
        .open("workspace/follow", json!({}))
        .expect("开工作区流失败");
    let events = mux.open("$events", json!({})).expect("开 $events 流失败");
    let control = mux.open_session_control().expect("开 session/control 失败");
    assert_ne!(workspace, control, "同一根 mux 上每条流的 id 必须互不相同");
    assert_ne!(workspace, events);
    assert_ne!(events, control);

    let mut tree = WorkspaceTree::default();
    let mut state = ControlState::default();
    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut unknown = 0usize;
    let mut workspace_frames = 0usize;
    let mut workspace_accepted = 0usize;
    let mut status_frames = 0usize;
    let mut stolen_by_tree: Vec<Value> = Vec::new();
    let deadline = Instant::now() + WAIT_BUDGET;
    while workspace_frames < 2
        || kinds.values().sum::<usize>() < CONTROL_FRAME_TYPES.len()
        || status_frames < 1
    {
        if Instant::now() >= deadline {
            panic!(
                "三条流的帧没收齐: 工作区 {workspace_frames} 帧 / 控制面 {kinds:?}（未识别 {unknown}）\
                 / 运行态 {status_frames} 帧"
            );
        }
        for event in mux
            .collect(Duration::from_millis(200))
            .expect("读帧不应失败")
        {
            let MuxEvent::Item {
                value: Some(frame),
                stream,
            } = event
            else {
                panic!("三条长驻流都只该发带值的 item: {event:?}");
            };
            if stream == control {
                // 分流臂的第一判据是 streamId，不是帧型：`baseline` 这个名字两边都在用。
                match control_frame_type(&frame) {
                    Some(kind) => *kinds.entry(kind).or_default() += 1,
                    None => unknown += 1,
                }
                // R1 的反证：这一路的帧要是漏进了兜底，`WorkspaceTree::apply` 一律判「不认」
                // 并返回 false —— 三张表永不落地，且一条错误都不报。
                if tree.apply(&frame) {
                    stolen_by_tree.push(frame.clone());
                }
                state.apply(&frame);
            } else if stream == workspace {
                workspace_frames += 1;
                workspace_accepted += usize::from(tree.apply(&frame));
            } else {
                assert_eq!(stream, events, "只剩 $events 这一条流没认出来: {stream}");
                match session_status_event(&frame) {
                    Some((id, running)) => {
                        status_frames += 1;
                        assert_eq!((id.as_str(), running), ("s-1", true));
                    }
                    None => assert_eq!(frame["type"], json!("ready"), "$events 只该有 ready + emit"),
                }
            }
        }
    }
    assert!(
        stolen_by_tree.is_empty(),
        "控制面的帧被 WorkspaceTree 认下了，分流判据失效: {stolen_by_tree:?}"
    );
    assert_eq!(unknown, 0, "桩发的控制帧每一帧都该在 CONTROL_FRAME_TYPES 里");
    for kind in CONTROL_FRAME_TYPES {
        assert_eq!(
            kinds.get(kind).copied().unwrap_or_default(),
            1,
            "开流即推的那一串里每型各一帧，实际 {kinds:?}"
        );
    }
    assert_eq!(
        (workspace_frames, workspace_accepted),
        (2, 2),
        "工作区流的 baseline + upsert 都得被树认下"
    );
    assert_eq!(tree.workspaces.len(), 3, "树只吃自己那条流的东西");
    assert!(tree.is_archived("s-9"));
    assert_eq!(status_frames, 1, "$events 的 emit 走运行态那一支");

    // 三张表真落了东西（不是空 delta 蒙过去的）。
    assert_eq!(state.queue_items("s-1001").len(), 2, "queue 帧整表替换 baseline 那一份");
    assert_eq!(state.jobs_of("s-1001").len(), 2);
    assert!(state.jobs_of("s-1001")[1].is_live());
    assert_eq!(projection_of(&state, "s-1001").title, "重构登录流程");
    assert_eq!(state.turn_outline("s-1001").len(), 3, "projection 帧落的 turnOutline");
    assert!(state.projections("s-1002").is_some(), "baseline 的投影是按会话分桶的");

    // 兜底那条路上今天会发生什么：四型全被树判 false ⇒ 光开流不分流就是零报错丢帧。
    for frame in [
        json!({ "type": "queue", "sessionId": "s-1001", "items": [] }),
        json!({ "type": "jobs", "sessionId": "s-1001", "jobs": [] }),
        json!({ "type": "projection", "sessionId": "s-1001", "key": "title", "value": "乙", "seq": 1 }),
    ] {
        assert!(!tree.apply(&frame), "{frame} 本就不该进树");
        assert!(control_frame_type(&frame).is_some());
    }
}

#[test]
fn session_control_projection_frames_are_full_tables_not_patches() {
    let (mut kernel, mut mux, stream) = open_control_stream();
    let mut state = ControlState::default();
    for frame in &item_values(&collect(&mut mux, &stream, 4), &stream) {
        state.apply(frame);
    }
    // 种子会话只跑过一轮（主干的 rail 门槛是「>= 2 条刻度」）。
    assert_eq!(state.turn_outline("s-1003").len(), 1);

    kernel
        .call("session/prompt", prompt_args("s-1003", "再补一轮", "queue"))
        .expect("prompt 应被接受");
    let events = collect(&mut mux, &stream, 1);
    let frame = expect_item(events.first(), &stream).expect("projection 带 value");
    assert_eq!(frame["type"], json!("projection"));
    assert_eq!(frame["sessionId"], json!("s-1003"));
    assert_eq!(frame["key"], json!("turnOutline"));
    assert_eq!(
        frame["value"]
            .as_array()
            .expect("projection 的 value 是整表")
            .len(),
        2,
        "wire.view 是全量：旧条目必须跟着一起来，主干整键替换"
    );
    assert!(state.apply(&frame).projections);
    assert_eq!(
        state
            .turn_outline("s-1003")
            .iter()
            .map(|item| item.turn)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(state.turn_outline("s-1003")[1].prompt, "再补一轮");

    // 同一张内核投影表的两个读数口必须一致（主干两处读的是同一个值）。
    let row = kernel
        .list_session_rows()
        .expect("session/list 应成功")
        .into_iter()
        .find(|row| row.info.id == "s-1003")
        .expect("台账里有 s-1003");
    assert_eq!(row.projections.turn_outline, state.turn_outline("s-1003").to_vec());
    // F9：typed 读数就是内核 strict 表那 8 个键的全量（桩按 `turns` 派生 `steps`/`ttftSteps`）。
    assert_eq!(
        row.projections.stats,
        Some(SessionStats {
            turns: 2,
            steps: 4,
            llm_ms: 4200.0,
            tool_ms: 900.0,
            ttft_ms: 310.0,
            decode_ms: 3300.0,
            decode_tokens: 1580.0,
            ttft_steps: 2,
        })
    );
}

#[test]
fn commands_list_round_trips_the_kernel_catalog() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let commands = kernel.list_commands("s-1001").expect("commands/list 应成功");
    assert_eq!(commands.len(), 9, "目录里九条命令");
    assert_eq!(commands[0].name, "compact");
    assert_eq!(commands[0].slash_name(), "/compact");
    assert_eq!(
        commands[0].description,
        "Compact older conversation history",
        "内置项的描述逐字照内核词典（主干据此才做本地化替换）"
    );
    assert!(!commands[0].has_input, "没有 input 键 = 无参命令");
    let feedback = commands
        .iter()
        .find(|command| command.name == "feedback")
        .expect("有 feedback");
    assert!(feedback.has_input);
    assert_eq!(feedback.hint, "<text>");
    let usage = commands
        .iter()
        .find(|command| command.name == "usage")
        .expect("有 usage");
    // F1：`input` 一旦出现，内核的 `normalizeDefinition` 就要求 `hint` 是非空字符串
    // （缺了或空白直接 TypeError）⇒ 真内核**产不出** `input:{}`，旧桩那条 `usage` 是假状态。
    // 无参数命令的正确表达是整个键缺席（主干 17341 的 `TryGetProperty("hint")` 兜底在真机不走）。
    assert!(
        !usage.has_input && usage.hint.is_empty(),
        "无参数命令 = 没有 input 键，而不是有 input 没 hint"
    );
    // 未建模键 `input.attachments`：主干 17341 只读 hint，分叉解析必须容忍它多出来。
    let goal = commands
        .iter()
        .find(|command| command.name == "goal")
        .expect("有 goal");
    assert!(goal.has_input);
    assert_eq!(goal.hint, "[<objective>|clear|edit <objective>|pause|resume]");
    assert_eq!(
        commands
            .iter()
            .filter(|command| command.has_input)
            .count(),
        5,
        "五条带 input 的命令，hint 全非空"
    );
    assert!(
        commands
            .iter()
            .all(|command| !command.has_input || !command.hint.is_empty()),
        "F1：内核侧不可能出现「有 input 却空 hint」的条目"
    );

    // params 平铺就一个 `agentId`（主干 17326）：少它、多键都被 wire 把关。
    let error = kernel
        .call("commands/list", json!({}))
        .expect_err("缺 agentId 必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    let error = kernel
        .call("commands/list", json!({ "agentId": "s-1001", "cwd": "x" }))
        .expect_err("多未知键必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    // agent 查不到是内核的 lookup 失败，主干据此走 3s 负缓存（17304-17306）。
    let error = kernel
        .list_commands("s-4004")
        .expect_err("没有这个 agent");
    assert!(error.contains("gateway/lookup-not-found"), "{error}");
}

#[test]
fn commands_execute_reports_every_reply_state() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    // 1) `result.kind = success`：正文就是 `result.text`。
    let reply = kernel
        .execute_command("s-1001", "/compact", &[])
        .expect("envelope 应 ok");
    assert!(reply.is_success());
    assert_eq!(reply.text(), Some("Compacted older conversation history."));
    // 2) `result.kind = error`。
    let reply = kernel
        .execute_command("s-1001", "/feedback", &[])
        .expect("参数不足也是正常回执");
    assert!(!reply.is_success());
    assert_eq!(reply.kind_of(), "error");
    assert_eq!(reply.text(), Some("Usage: /feedback <text>"));
    // 3) success 但 text 缺席（F4：`result.text` 在 success 支是 optional）——
    //    主干 17911 的「{0} 执行完成」那一支今天终于有样本了。
    let reply = kernel
        .execute_command("s-1001", "/deploy prod", &[])
        .expect("deploy 回执");
    assert_eq!(
        reply,
        CommandReply::Result {
            kind: "success".to_string(),
            text: None,
        }
    );
    assert!(reply.is_success(), "kind=success 且无 text 仍是成功支");
    assert_eq!(reply.text(), None, "text 缺席不得被填成空串");
    // 4) 线上没有 value 这个键（未识别/不是命令的行）。
    assert_eq!(
        kernel
            .execute_command("s-1001", "/nosuchcmd", &[])
            .expect("内核 ok、没有 value"),
        CommandReply::Unknown
    );
    assert_eq!(
        kernel
            .execute_command("s-1001", "普通聊天文本", &[])
            .expect("内核 ok、没有 value"),
        CommandReply::Unknown
    );
    // F5：内核的 value 是 `undefined | {commandId, result}` 二选一 ⇒「有 value 没 result」
    // 那一态线上产不出，桩不再演它（`CommandReply::NoImmediateReply` 只剩防御支，
    // 由 kernel.rs 的合成帧单测覆盖）。这里反向钉住：桩给的每一发 value 都带 result。
    let value = kernel
        .call(
            "commands/execute",
            json!({
                "agentId": "s-1001",
                "line": "/usage",
                "submittedAttachments": [],
            }),
        )
        .expect("usage 的 value");
    assert_eq!(
        value["result"]["kind"],
        json!("success"),
        "旧桩那条「有 value 无 result」是假状态"
    );
    assert!(
        value["commandId"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        "F2：`commandId` 是必填键（`cmd-<实例 token>-<递增号>`），主干不读它但形状必须合法"
    );
    assert_eq!(value.as_object().expect("value 是对象").len(), 2);
    // 附件把关（内核 `dsh-commands/lib/index.js:349-356`）：compact 没声明
    // `input.attachments` ⇒ 带附件直接 settle 成 error，handler 根本不跑。
    // 这条同时也是「submittedAttachments 真到了内核」的证据。
    let reply = kernel
        .execute_command(
            "s-1001",
            "/compact",
            &[SubmittedAttachment::File {
                receipt_id: "r-1".to_string(),
            }],
        )
        .expect("带附件的 /compact 是正常回执，不是 RPC 失败");
    assert_eq!(reply.kind_of(), "error");
    assert_eq!(reply.text(), Some("/compact does not accept attachments"));
    // 声明了 attachments 的命令带附件照常跑（两型都过网关把关）。
    let reply = kernel
        .execute_command(
            "s-1001",
            "/goal 把测试补完",
            &[
                SubmittedAttachment::Image {
                    media_type: "image/png".to_string(),
                    data: "AA==".to_string(),
                    name: "shot.png".to_string(),
                },
                SubmittedAttachment::File {
                    receipt_id: "r-1".to_string(),
                },
            ],
        )
        .expect("goal 收附件");
    assert_eq!(reply.text(), Some("Goal set: 把测试补完"));
    // F11 + F7：`/permission` 的三条分支逐字照 `dsh-permission-presets/lib/index.js:161-181`。
    // 空参数是 **success**（旧桩回 error，行为与文案双不一致）。
    let reply = kernel
        .execute_command("s-1001", "/permission", &[])
        .expect("无参数的 /permission 是 success");
    assert!(reply.is_success(), "{reply:?}");
    assert_eq!(
        reply.text(),
        Some("current preset workspace-write (available: workspace-write, danger-full-access)")
    );
    // 表外名字（旧桩那套 `default`/`accept-edits`/`read-only` 都不在真预设表里）⇒ error。
    let reply = kernel
        .execute_command("s-1001", "/permission read-only", &[])
        .expect("表外预设是正常回执");
    assert_eq!(reply.kind_of(), "error");
    assert_eq!(
        reply.text(),
        Some("unknown preset \"read-only\" (available: workspace-write, danger-full-access)")
    );
    let reply = kernel
        .execute_command("s-1001", "/permission default", &[])
        .expect("旧桩的假 id 同样被真表拒");
    assert_eq!(reply.kind_of(), "error");
    // 真预设名切换成功 ⇒ success + `preset <name>`，并且立刻反映到投影里。
    let reply = kernel
        .execute_command("s-1001", "/permission danger-full-access", &[])
        .expect("切到真预设应成功");
    assert_eq!(reply.text(), Some("preset danger-full-access"));
    let rows = kernel.list_session_rows().expect("session/list 应成功");
    let switched = rows
        .iter()
        .find(|row| row.info.id == "s-1001")
        .expect("台账里有 s-1001");
    assert_eq!(
        switched
            .projections
            .permissions
            .as_ref()
            .expect("权限投影在")
            .current_value
            .as_deref(),
        Some("danger-full-access"),
        "F7：currentValue 是英文预设 id，中文只属于视图层那张 PermissionPresetZh"
    );

    // F6：`mediaType` 是**闭 union**（png/jpeg/webp/gif）。旧桩只查非空字符串，
    // 于是 `image/svg+xml` 这种真内核必拒的形状桩会收下来，分叉把拒因误当「内核 bug」。
    let error = kernel
        .call(
            "commands/execute",
            json!({
                "agentId": "s-1001",
                "line": "/goal x",
                "submittedAttachments": [{
                    "type": "image",
                    "mediaType": "image/svg+xml",
                    "data": "AA==",
                }],
            }),
        )
        .expect_err("表外 mediaType 必须被网关拒");
    assert!(error.contains("bad_args"), "{error}");

    // 平铺三键缺一不可（主干 17883-17888）。
    let error = kernel
        .call(
            "commands/execute",
            json!({ "agentId": "s-1001", "line": "/compact" }),
        )
        .expect_err("少了 submittedAttachments 必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    let error = kernel
        .call(
            "commands/execute",
            json!({
                "agentId": "s-1001",
                "line": "/compact",
                "submittedAttachments": [{ "type": "zip" }],
            }),
        )
        .expect_err("附件元素只认 image/file 两型");
    assert!(error.contains("bad_args"), "{error}");
    let error = kernel
        .execute_command("s-4004", "/compact", &[])
        .expect_err("agent 不存在");
    assert!(error.contains("gateway/lookup-not-found"), "{error}");
}

#[test]
fn session_list_rows_carry_every_projection_key_the_trunk_reads() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let rows = kernel.list_session_rows().expect("session/list 应成功");
    assert_eq!(rows.len(), 3);
    let first = &rows[0];
    assert_eq!(first.info.id, "s-1001");
    // 向后兼容：整块解析没动过 title 那条读法。
    assert_eq!(first.info.title, "重构登录流程");
    assert_eq!(first.projections.title, first.info.title);
    assert_eq!(
        kernel.list_sessions().expect("旧读法仍在")[0].title,
        first.info.title
    );
    // SS1：这一格原来是 `Some(88)`，判据写「游标取大纲末条的 seq」——那正是被钉住的 bug。
    // 内核的 `asOfSeq` 是**折叠水位**（末条事件的 seq），不是末轮 `turn/start` 的号：
    // `dsh-session-projection/lib/index.js:153` 的 `cursorBefore(session.seq)` +
    // 同文件 `:23-25`（`offset === 0 ? -1 : offset - 1`）+ `dsh-session/lib/index.js:1129-1131`
    // （`session.seq` = 下一条事件的号 = 日志长度）⇒ 种子 s-1001 的真水位是 117。
    // 同一份 117 桩自己的 `session/page` 早就在认（`select_model_echoes_and_page_backfills_
    // the_journal` 里那两条 `末条 seq == 游标` 与 `is past cursor 117`）——
    // 两块投影各报一个游标才是事故。
    assert_eq!(
        first.projections.as_of_seq,
        Some(117),
        "游标 = 该会话 journal 的末条序号，不是大纲末条的 turn/start 号"
    );
    assert_eq!(
        first
            .projections
            .turn_outline
            .iter()
            .map(|item| item.turn)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        first.projections.stats,
        Some(SessionStats {
            turns: 3,
            steps: 6,
            llm_ms: 4200.0,
            tool_ms: 900.0,
            ttft_ms: 310.0,
            decode_ms: 3300.0,
            decode_tokens: 1580.0,
            ttft_steps: 3,
        })
    );
    assert_eq!(first.projections.plan, Some(PlanProjection::default()));
    let permissions = first
        .projections
        .permissions
        .clone()
        .expect("权限投影在");
    // F7 + F8：`options` 的形状就是真内核 `selectFor()`（`lib/index.js:230-236`）那一句
    // 「表项按声明顺序 + `...currentValue === "custom" ? [optionOf("custom")] : []`」——
    // **默认态（当前档命中表）只有两项**，`custom` 是派生态、只有派生出来才追加（桩此前恒发
    // 三项，那是桩的形状不是内核的形状；完整两头见
    // `permission_custom_option_only_arrives_when_the_knobs_leave_the_table`）。
    // `optionOf()` 出的 `value` 与 `name` 同源且都是**英文**，`description` 两项都在
    // （selectSchema 里 value/name 是 min(1) 必填，「没有 value 的坏选项」线上产不出
    // ⇒ 主干 15377 那条丢弃逻辑是纯防御，桩不演它）。
    assert_eq!(
        permissions
            .options
            .iter()
            .map(|option| option.value.as_str())
            .collect::<Vec<_>>()
            .join(","),
        "workspace-write,danger-full-access",
        "默认表两项，命中表的那一档不许追加派生态"
    );
    assert!(
        permissions
            .options
            .iter()
            .all(|option| option.value != "custom"),
        "`custom` 是派生态，不许恒在选项里: {:?}",
        permissions.options
    );
    assert!(
        permissions
            .options
            .iter()
            .all(|option| !option.name.is_empty()
                && option.name.chars().all(|c| c.is_ascii())
                && !option.description.is_empty()
                && option.name == option.value),
        "桩发的 name 必须是英文预设 id（中文是视图层 PermissionPresetZh 的活）: {:?}",
        permissions.options
    );
    assert_eq!(permissions.current_value.as_deref(), Some("workspace-write"));
    assert_eq!(
        first.projections.value_of("todos"),
        Some(&json!({ "open": 0, "done": 3 })),
        "未建模的投影键必须原样留在 values 里"
    );

    // 空白会话：内核投影还没长起来 ⇒ values 只有 title，其余 typed 字段全 None。
    let blank = &rows[1];
    assert_eq!(blank.projections.title, "新会话");
    assert!(blank.projections.turn_outline.is_empty());
    assert!(blank.projections.plan.is_none());
    assert!(blank.projections.permissions.is_none());
    assert!(blank.projections.stats.is_none());
    assert!(!blank.projections.is_empty());

    // 命令改的就是这张投影表：下一发 `session/list` 立刻是新值。
    // F7：能切换的只有真预设表里的名字，`read-only`/`default` 那些是旧桩的假 id（真内核对
    // 它们回 error，见 `commands_execute_reports_every_reply_state`）。
    let child_projections = |kernel: &mut Kernel| -> Projections {
        kernel
            .list_session_rows()
            .expect("session/list 应成功")
            .into_iter()
            .find(|row| row.info.id == "s-1003")
            .expect("台账里有 s-1003")
            .projections
    };
    // 换档之前先各读一次：s-1001 走合成默认旋钮 ⇒ 命中默认表第一项（本用例开头那次
    // `session/list` 钉过），s-1003 的种子 journal 记的却是一条**表外** `sandbox/mode` 覆盖
    // ⇒ `derive()`（`lib/index.js:213-223`）落到派生态 `custom`。两条会话读的是各自的旋钮、
    // 各自 fold 出来的 state，不是全局一张嘴；`custom` 那一项的进出见
    // `permission_custom_option_only_arrives_when_the_knobs_leave_the_table`。
    assert_eq!(
        child_projections(&mut kernel)
            .permissions
            .expect("权限投影在")
            .current_value
            .as_deref(),
        Some("custom"),
        "旋钮撞不到表的 seeded 会话派生成 custom，而同一次 list 里的 s-1001 仍在表内"
    );
    kernel
        .execute_command("s-1003", "/plan on", &[])
        .expect("/plan 应成功");
    kernel
        .execute_command("s-1003", "/permission danger-full-access", &[])
        .expect("/permission 应成功");
    let child = child_projections(&mut kernel);
    assert_eq!(
        child.plan,
        Some(PlanProjection {
            active: true,
            pending: false,
        })
    );
    // `/permission <preset>` 是唯一写入口（内核没有 permission RPC，主干 15906 的注释）：
    // handler 记完预设就推整块 select 视图，而视图的 `currentValue` 取的正是刚选的那一档
    // （`dsh-permission-presets/lib/index.js:161-176` 的 handler ⇒ `apply()` 280-286 ⇒
    // `derive()`/`selectFor()` 213-236：命中的表项原样回名字）。旧桩那套 `read-only` 是
    // 表外假 id，真内核直接回 error（见上面 `commands_execute_reports_every_reply_state`），
    // 所以切完档的当前档绝不可能是 `read-only`。
    assert_eq!(
        child
            .permissions
            .expect("权限投影在")
            .current_value
            .as_deref(),
        Some("danger-full-access")
    );

    // 整块缺席的旧会话（主干 2827 的容错）：投影是空表，不凭空造数据。
    let gone = Projections::from_block(&json!({ "sessionId": "s-9" }));
    assert!(gone.is_empty());
    assert!(gone.title.is_empty());
}

/// F9：内核的 `sessionStatsSchema` 是 **`.strict()`** 的八键表
/// （`dsh-session-stats/lib/types/projection.js:27-35`）—— 多一个键整块被拒，
/// 少一个键就是 #57/#58 拿不到数据。旧桩发的 `{turns, totalTokens}` 两头都踩：
/// `totalTokens` 真内核产不出，`steps`/`decodeMs` 又没有。这里钉线上那一发。
#[test]
fn stub_session_stats_are_the_strict_eight_keys_with_no_invented_total_tokens() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let value = kernel
        .call("session/list", json!({ "_request": {} }))
        .expect("session/list 应成功");
    let stats = &value["items"][0]["projections"]["values"]["sessionStats"];
    let mut keys: Vec<&str> = stats
        .as_object()
        .expect("sessionStats 是对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "decodeMs",
            "decodeTokens",
            "llmMs",
            "steps",
            "toolMs",
            "ttftMs",
            "ttftSteps",
            "turns"
        ],
        "strict 表的键集一个不多一个不少"
    );
    assert_eq!(stats.get("totalTokens"), None, "旧桩那条 invented 键必须彻底消失");
    // 少的那两半今天齐了：#57/#58 要的步数与解码时长在 typed 读数里，而不是靠 UI 猜。
    let rows = kernel.list_session_rows().expect("session/list 应成功");
    let stats = rows[0].projections.stats.expect("typed 读数在");
    assert_eq!((stats.turns, stats.steps, stats.ttft_steps), (3, 6, 3));
    assert!(stats.decode_ms > 0.0 && stats.decode_tokens > 0.0, "{stats:?}");
    assert!(stats.llm_ms > 0.0 && stats.tool_ms > 0.0 && stats.ttft_ms > 0.0);
    // 显示源仍是 journal（审计 §2 的口径），但线上那 8 个键必须能在 values 里交叉核对。
    assert_eq!(
        rows[0].projections.value_of("sessionStats"),
        Some(&stats_raw()),
        "values 里保留的就是原样的八键块"
    );
}

/// 桩给的 `sessionStats` 常量（与 `fake_dsh.rs::session_stats(3)` 同值）：
/// 用它来证明 typed 读数和线上块是同一份数据，而不是各算一遍。
fn stats_raw() -> Value {
    json!({
        "turns": 3,
        "steps": 6,
        "llmMs": 4200,
        "toolMs": 900,
        "ttftMs": 310,
        "ttftSteps": 3,
        "decodeMs": 3300,
        "decodeTokens": 1580,
    })
}

/// F7：桩发的权限条目是**英文预设 id**，中文只属于视图层那张 `PermissionPresetZh`
/// （主干 15957 的硬约束）。分叉要是照「wire 上就是中文」实现，上真机立刻漏英文。
#[test]
fn stub_permission_options_arrive_as_english_ids_the_view_has_to_translate() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let rows = kernel.list_session_rows().expect("session/list 应成功");
    let permissions = rows[0]
        .projections
        .permissions
        .clone()
        .expect("权限投影在");
    let catalog = Catalog::load("zh", None);
    assert!(
        permissions
            .options
            .iter()
            .all(|option| option.name.is_ascii() && option.value.is_ascii()),
        "wire 上不许出现中文 name: {:?}",
        permissions.options
    );
    // 视图侧过表才变中文；表里认不得的 id 原样回显（主干的 `_ => value`）。
    let labels: Vec<String> = permissions
        .options
        .iter()
        .map(|option| permission_preset_zh(&catalog, &option.value))
        .collect();
    assert_eq!(labels, vec!["工作区内修改", "完全权限"]);
    assert!(
        labels
            .iter()
            .zip(permissions.options.iter())
            .all(|(label, option)| label != &option.name),
        "直显 name 就是漏翻译：翻译后的串必须与 wire 串不同"
    );
    assert_eq!(
        permission_preset_zh(&catalog, "read-only"),
        "仅可查看",
        "表是照主干整表抄的，含 deployment 才有的 read-only/auto-approve"
    );
    assert_eq!(
        permission_preset_zh(&catalog, "some-future-preset"),
        "some-future-preset"
    );
}

/// 那第三项 `custom` 到底什么时候上线：真内核 `selectFor()`
/// （`Kernel/dsh/node_modules/@deepseek-ai/dsh-permission-presets/lib/index.js:230-236`）
/// 是「表项按声明顺序 + `...currentValue === "custom" ? [optionOf(CUSTOM_PRESET)] : []`」，
/// 也就是**只有 `derive()`（同文件 213-223）撞不到表项、派生出 `custom` 时才追加**。
/// 桩此前恒发三项（分叉照桩写就会以为选项是定长三档），这里两头都钉：
/// 默认态不许出现 `custom`；切到自定义上下文（会话自己的旋钮在表外）才出现；一切回表内立刻消失。
#[test]
fn permission_custom_option_only_arrives_when_the_knobs_leave_the_table() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let permissions_of = |kernel: &mut Kernel, id: &str| -> PermissionsProjection {
        kernel
            .list_session_rows()
            .expect("session/list 应成功")
            .into_iter()
            .find(|row| row.info.id == id)
            .expect("台账里有这条会话")
            .projections
            .permissions
            .clone()
            .expect("权限投影在")
    };
    // 嵌套 fn（而不是闭包）：只为省掉那个 `&'_ str` 的生命周期标注。
    fn values(permissions: &PermissionsProjection) -> Vec<&str> {
        permissions
            .options
            .iter()
            .map(|option| option.value.as_str())
            .collect()
    }
    let catalog = Catalog::load("zh", None);

    // ① 默认态：s-1001 的旋钮就是合成默认那一束，命中表 ⇒ 两项，没有派生态。
    let on_table = permissions_of(&mut kernel, "s-1001");
    assert_eq!(
        values(&on_table),
        ["workspace-write", "danger-full-access"],
        "命中默认表时不许追加 `custom`"
    );
    assert_eq!(on_table.current_value.as_deref(), Some("workspace-write"));

    // ② 切到自定义上下文：s-1003 的 journal 里那条 `sandbox/mode` 是 `read-only` ——
    // `SANDBOX_MODES` 的合法值（`dsh-sandbox-policy/lib/index.js:31-35`）、
    // `permissionStateSchema` 那个 union 的一支（`index.js:27-31`），却不在任何预设束里，
    // 于是 `derive()` 回 `custom`（桩侧 `seed_knobs()` 演这条 seeded 覆盖：`pinInitialPermission()`
    // 对 seeded 会话是「preserve their effective knob values」，派生是 custom 时**不**补预设，
    // `index.js:287-311`）。第三项这才追加进来，位置在表项之后、名字是 `optionOf("custom")`
    // 的 "Custom"（`index.js:255-267`）。
    let custom = permissions_of(&mut kernel, "s-1003");
    assert_eq!(
        values(&custom),
        ["workspace-write", "danger-full-access", "custom"]
    );
    assert_eq!(custom.current_value.as_deref(), Some("custom"));
    assert_eq!(
        custom.options[2].name, "Custom",
        "唯一一处 name 与 value 不同源的项"
    );
    assert_eq!(
        permission_preset_zh(&catalog, custom.options[2].value.as_str()),
        "自定义",
        "视图层过表才是中文"
    );
    // 报现状可以报 `custom`，切过去却不行：它不是 `names`（= 表键，`index.js:167,186-188`）里的一项，
    // 内核构造函数甚至禁止表项占用这个名字（`index.js:108`）。
    let reply = kernel
        .execute_command("s-1003", "/permission", &[])
        .expect("空参数是 success");
    assert!(reply.is_success(), "{reply:?}");
    assert_eq!(
        reply.text(),
        Some("current preset custom (available: workspace-write, danger-full-access)")
    );
    let reply = kernel
        .execute_command("s-1003", "/permission custom", &[])
        .expect("`custom` 走的是普通 error 回执");
    assert_eq!(reply.kind_of(), "error");
    assert_eq!(
        reply.text(),
        Some("unknown preset \"custom\" (available: workspace-write, danger-full-access)")
    );
    assert_eq!(
        values(&permissions_of(&mut kernel, "s-1003")),
        values(&custom),
        "error 分支不落账，投影不许动"
    );

    // ③ 一切回表内：`apply()`（`index.js:280-286`）把选择与整束旋钮一起落账，
    // 派生重新命中 ⇒ 第三项随派生态一起消失，`currentValue` 换成刚选的那一档。
    kernel
        .execute_command("s-1003", "/permission workspace-write", &[])
        .expect("切回真预设应成功");
    let back = permissions_of(&mut kernel, "s-1003");
    assert_eq!(back.current_value.as_deref(), Some("workspace-write"));
    assert_eq!(
        values(&back),
        ["workspace-write", "danger-full-access"],
        "回到表内 ⇒ 派生态消失"
    );
    // 另一条会话没被带跑：旋钮与派生都是按会话来的。
    assert_eq!(
        values(&permissions_of(&mut kernel, "s-1001")),
        ["workspace-write", "danger-full-access"]
    );
    assert_eq!(
        permissions_of(&mut kernel, "s-1001")
            .current_value
            .as_deref(),
        Some("workspace-write")
    );
}

/// 数据层缺陷（审计 §4 末）：`parse_queue_items` 过去写 `item.get("message")?.get("content")?`，
/// 两级缺任一层就**整条丢**；主干 15435 用 `TryGetProperty` 兜底后条目仍然保留
/// （`Content = default`、`Text = ""`）。后果是内核给一条没有 message 的排队项时，
/// 分叉的「排队中 · N 条」比主干小。真内核的 schema 里 message 是必填 ⇒ 桩不演这一态，
/// 这里拿合成帧喂解析器。
#[test]
fn queue_entries_missing_message_or_content_survive_instead_of_being_dropped() {
    let mut state = ControlState::default();
    let frame = json!({
        "type": "queue",
        "sessionId": "s-1001",
        "items": [
            { "id": "q-ok", "placement": "queued",
              "message": { "id": "m-1", "content": [{ "type": "text", "text": "正常一条" }] } },
            { "id": "q-no-message", "placement": "steering" },
            { "id": "q-message-without-content", "placement": "context", "message": { "id": "m-3" } },
            { "id": "q-empty-content", "message": { "id": "m-4", "content": [] } },
            { "id": "q-non-object-message", "message": "not-an-object" },
            { "placement": "queued", "message": { "id": "m-6", "content": [] } },
        ],
    });
    assert!(state.apply(&frame).queues);
    let items = state.queue_items("s-1001");
    assert_eq!(
        items.iter().map(|item| item.item_id.as_str()).collect::<Vec<_>>(),
        vec![
            "q-ok",
            "q-no-message",
            "q-message-without-content",
            "q-empty-content",
            "q-non-object-message"
        ],
        "只有主干 15430 那条「没有 id」才整条丢；缺 message/content 的必须留着"
    );
    assert_eq!(items[0].text(), "正常一条");
    assert_eq!(items[1].content, Value::Null, "缺 message 的条目 content 落成 Null");
    assert_eq!(items[1].text(), "", "文本拼接对缺块就是空串，不 panic");
    assert_eq!(items[2].content, Value::Null);
    assert_eq!(items[3].content, json!([]), "空数组是真的空数组，不是缺席");
    assert_eq!(items[4].content, Value::Null, "message 不是对象也当缺席");
    // placement 缺席回落 queued（主干 15434：`TryGetProperty` 取不到、或取到但不是字符串，
    // 都落到 `"queued"` 那一支），留下的 5 条里三条没给 placement ⇒ 计数是 3 不是 2。
    assert_eq!(
        items
            .iter()
            .map(|item| item.placement.as_str())
            .collect::<Vec<_>>(),
        vec!["queued", "steering", "context", "queued", "queued"],
        "q-ok 显式 queued，q-empty-content 与 q-non-object-message 没带 placement ⇒ 回落 queued"
    );
    assert_eq!(
        items.iter().filter(|item| item.placement == "queued").count(),
        3,
        "与主干同口径：回落算进「排队中 · N 条」"
    );
    assert_eq!(items[1].placement, "steering");
    assert_eq!(items[2].placement, "context");
}

/// F10：`plan.pending` 是真的会亮的布尔（轮次开着时 `/plan` 只登记意图，
/// 下一个 in-turn pre-step 才落账），主干 15950 据此显「计划模式（切换中…）」。
/// 桩侧那条可达路径（pacer 队列里真有没推完的轮）由 `fake_dsh.rs` 的进程内用例钉死；
/// 这里钉解析侧：一帧 plan 增量必须把两个布尔都收进 typed 投影，不许把 pending 演丢。
#[test]
fn plan_projection_keeps_the_switching_state_readable() {
    fn apply_plan(state: &mut ControlState, value: Value) -> bool {
        state
            .apply(&json!({
                "type": "projection", "sessionId": "s-1003", "key": "plan", "value": value,
            }))
            .projections
    }

    let mut state = ControlState::default();
    assert!(apply_plan(&mut state, json!({ "active": false, "pending": true })));
    assert_eq!(
        state.projections("s-1003").expect("投影在").plan,
        Some(PlanProjection {
            active: false,
            pending: true,
        }),
        "#55/#56 的「切换中…」全靠这一位"
    );
    apply_plan(&mut state, json!({ "active": true, "pending": false }));
    assert_eq!(
        state.projections("s-1003").expect("投影在").plan,
        Some(PlanProjection {
            active: true,
            pending: false,
        })
    );
    apply_plan(&mut state, json!({ "active": true }));
    assert_eq!(
        state.projections("s-1003").expect("投影在").plan,
        Some(PlanProjection {
            active: true,
            pending: false
        }),
        "pending 缺席按 false：内核 `get()` 在无意图时就把 pending 裁成 false"
    );
}

#[test]
fn mux_unknown_endpoint_becomes_failure() {
    let (_kernel, mut mux, stream) = open_stream("nope/missing");
    let events = collect(&mut mux, &stream, 1);
    assert_eq!(events.len(), 1, "未知端点应回一个 error 帧: {events:?}");
    match &events[0] {
        MuxEvent::Failure {
            stream: id,
            message,
        } => {
            assert_eq!(id, &stream);
            assert_eq!(message, "未知端点");
        }
        other => panic!("期望 Failure，实际 {other:?}"),
    }
}

#[test]
fn session_prompt_validates_mainline_request_shape() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // 这条会话一条 follow 流都没开：主干允许先发提示再补跟随，内核也照收。
    let value = kernel
        .call(
            "session/prompt",
            prompt_args("s-1001", "继续写测试", "queue"),
        )
        .expect("形状对的 prompt 应被接受");
    assert_eq!(value, json!({ "accepted": true }));

    let mut cases: Vec<(&str, Value)> = Vec::new();
    cases.push((
        "扁平传",
        json!({
            "requestId": "c2-fake-1",
            "sessionId": "s-1001",
            "mode": "steer",
            "content": [{ "type": "text", "text": "x" }],
        }),
    ));
    let mut no_mode = prompt_args("s-1001", "x", "queue");
    no_mode["request"]
        .as_object_mut()
        .expect("request 是对象")
        .remove("mode");
    cases.push(("缺 mode", no_mode));
    cases.push(("mode 不是 queue/steer", prompt_args("s-1001", "x", "turbo")));
    let mut no_content = prompt_args("s-1001", "x", "queue");
    no_content["request"]
        .as_object_mut()
        .expect("request 是对象")
        .remove("content");
    cases.push(("缺 content", no_content));
    let mut empty_content = prompt_args("s-1001", "x", "queue");
    empty_content["request"]["content"] = json!([]);
    cases.push(("content 空数组", empty_content));
    let mut odd_block = prompt_args("s-1001", "x", "queue");
    odd_block["request"]["content"] = json!([{ "type": "audio", "data": "aaa" }]);
    cases.push(("content 元素不认识", odd_block));

    for (case, args) in cases {
        let error = kernel
            .call("session/prompt", args)
            .expect_err("{case} 必须被拒");
        assert!(
            error.contains("bad_args"),
            "{case} 应回 bad_args，实际 {error}"
        );
    }
    kernel.shutdown();
}

#[test]
fn session_follow_streams_a_full_prompt_turn() {
    let (mut kernel, mut mux, follow) = open_stream_with("session/follow", follow_args("s-9001"));

    // 开流先落一个空快照，主干据此建会话头。
    let snapshot = item_values(&collect(&mut mux, &follow, 1), &follow)
        .into_iter()
        .next()
        .expect("快照先到");
    assert_eq!(snapshot["type"], json!("snapshot"));
    assert_eq!(snapshot["header"]["id"], json!("s-9001"));
    assert_eq!(snapshot["header"]["cwd"], json!("C:/repo/one"));
    assert_eq!(snapshot["cursor"], json!(0));
    assert_eq!(snapshot["hasMore"], json!(false));
    assert_eq!(
        snapshot["records"]
            .as_array()
            .expect("records 是数组")
            .len(),
        0
    );
    assert_eq!(snapshot["projections"]["asOfSeq"], json!(0));

    let prompt = "继续写测试";
    let accepted = kernel
        .call("session/prompt", prompt_args("s-9001", prompt, "queue"))
        .expect("session/prompt 应被接受");
    assert_eq!(accepted["accepted"], json!(true));

    // 一轮 30 条 journal 事件 + 6 个逐字增量元素。
    let frames = item_values(&collect(&mut mux, &follow, TURN_FRAMES), &follow);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "一轮的元素顺序与条数"
    );
    assert_eq!(
        frames[1]["event"]["data"]["content"][0]["text"],
        json!(prompt),
        "user/message 要原样带回提示正文"
    );
    assert_eq!(
        frames
            .iter()
            .filter(|frame| tag(frame) == "chunk:text-delta")
            .count(),
        3,
        "逐字增量至少两片"
    );
    let deltas: String = frames
        .iter()
        .filter(|frame| tag(frame) == "chunk:text-delta")
        .flat_map(|frame| {
            frame["frame"]["chunk"]["text"]
                .as_str()
                .unwrap_or_default()
                .chars()
        })
        .collect();
    // 下标 32 才是 settled 的 assistant/message：突变/结果成对铺在它之前，chips 才截得到。
    let settled = &frames[32]["event"];
    assert_eq!(settled["data"]["message"]["id"], json!("msg-1-27"));
    let settled_text = settled["data"]["message"]["content"][0]["text"]
        .as_str()
        .expect("assistant/message 有正文");
    assert_eq!(deltas, settled_text, "增量拼起来必须正好是 settled 正文");
    assert_eq!(settled_text, format!("已收到: {prompt}"));

    // journal 游标：一轮卅条事件连号，end 帧的 seq 指向刚落地的 assistant/message。
    assert_eq!(frames[0]["event"]["seq"], json!(1));
    assert_eq!(frames[1]["event"]["seq"], json!(2));
    assert_eq!(frames[2]["event"]["seq"], json!(3));
    assert_eq!(settled["seq"], json!(27));
    assert_eq!(frames[35]["event"]["seq"], json!(30));
    assert_eq!(frames[0]["event"]["data"]["turn"], json!(1));
    assert_eq!(frames[3]["frame"]["startedAfterSeq"], json!(2));
    assert_eq!(frames[8]["frame"]["outcome"]["kind"], json!("committed"));
    assert_eq!(
        frames[8]["frame"]["outcome"]["eventType"],
        json!("assistant/message")
    );
    assert_eq!(frames[8]["frame"]["outcome"]["seq"], settled["seq"]);

    // 整轮的 journal 事件（去掉逐字增量）：下面所有数据面断言都在这条有序序列上做。
    let journal: Vec<&Value> = frames
        .iter()
        .filter(|frame| tag(frame).starts_with("event:"))
        .map(|frame| &frame["event"])
        .collect();

    // 操作行的数据面：型号/消息 id/推理块/用量/工具行/交付物，缺一项就有一段整列收起。
    assert_eq!(
        frames[2]["event"]["data"]["header"]["config"]["model"],
        json!("dsh-test-model"),
        "本轮型号由 request/header 报"
    );
    let message = &settled["data"]["message"];
    assert_eq!(message["id"], json!("msg-1-27"), "id 按 msg-<turn>-<seq>");
    assert_eq!(message["content"][1]["type"], json!("reasoning"));
    assert!(
        message["content"][1]["text"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "reasoning 块要有正文，主干才出得了折叠那行"
    );
    assert_eq!(
        settled["data"]["usage"]["totalTokens"],
        json!(1234),
        "奇数轮给 totalTokens"
    );
    // 每条事件各带自己的信封时间，且轮尾比轮头晚 4~9 秒：共用一个 now 会让「用时」永远是 0。
    let started = frames[0]["event"]["time"]
        .as_i64()
        .expect("turn/start 有信封时间");
    let ended = frames[35]["event"]["time"]
        .as_i64()
        .expect("turn/end 有信封时间");
    assert!(
        (4000..=9000).contains(&(ended - started)),
        "本轮用时 {ended} - {started}"
    );
    let clocks: Vec<i64> = journal
        .iter()
        .map(|event| event["time"].as_i64().unwrap_or_default())
        .collect();
    assert_eq!(
        clocks
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        clocks.len(),
        "journal 事件的时间要逐个不同，否则整屏时钟挤成同一个 HH:mm"
    );
    // 工具行：arguments 的两种发法在这一轮里都出现，摘要键各命中一种变体。
    let names: Vec<&str> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/call"))
        .map(|event| event["data"]["name"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        names,
        vec![
            "bash",
            "read",
            "grep",
            "write",
            "write",
            "edit",
            "str_replace_editor",
            "Bash",
            "edit",
            "edit",
        ]
    );
    let calls: Vec<&str> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/call"))
        .map(|event| event["data"]["callId"].as_str().unwrap_or_default())
        .collect();
    let results: Vec<&str> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/result"))
        .map(|event| {
            event["data"]["message"]["source"]["callId"]
                .as_str()
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        calls,
        vec![
            "call-1", "call-2", "call-3", "call-4", "call-5", "call-6", "call-7", "call-8",
            "call-9", "call-10"
        ],
        "每条 tool/call 都带 callId"
    );
    assert_eq!(
        results, calls,
        "tool/result 要与 tool/call 的 callId 一一对应、同序到达"
    );
    // 内核两种发法都得覆盖：字符串口径 call-1/3/6/8，对象口径 call-2/4/5/7/9/10。
    let as_text: Vec<&str> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/call"))
        .filter(|event| event["data"]["arguments"].is_string())
        .map(|event| event["data"]["callId"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(as_text, vec!["call-1", "call-3", "call-6", "call-8"]);
    // 摘要变体：bash 取 description、read 兜底 file_path、search 取 queries[0]、write 取 path。
    assert_eq!(
        arguments_of(call_event(&journal, "call-1"))["description"],
        json!("跑一遍假内核自测")
    );
    assert_eq!(
        arguments_of(call_event(&journal, "call-2"))["file_path"],
        json!("E:\\demo\\alpha\\src\\main.rs")
    );
    assert_eq!(
        arguments_of(call_event(&journal, "call-3"))["queries"][0],
        json!("操作行在哪个分支落地")
    );
    assert_eq!(
        arguments_of(call_event(&journal, "call-4"))["path"],
        json!("E:\\demo\\alpha\\out\\report.md")
    );

    // 「本轮文件改动」的数据面：登记只有一等突变工具给路径，非突变与参数残缺一律空串。
    let expected: Vec<(String, String)> = [
        ("call-1", ""), // bash：非突变
        ("call-2", ""), // read：非突变
        ("call-3", ""), // grep：非突变
        ("call-4", ""), // write 但只给 path，没有 file_path ⇒ 形状不合
        ("call-5", "E:\\demo\\alpha\\src\\lib.rs"),
        ("call-6", "E:\\demo\\alpha\\src\\app.rs"), // 登记了，但结果 isError ⇒ 不算产出
        ("call-7", "E:\\demo\\alpha\\out\\notes.md"),
        ("call-8", ""), // Bash：非突变（且名字没进 TOOL_TABLE）
        ("call-9", ""), // edit 缺 new_string ⇒ 参数残缺
        ("call-10", "E:\\demo\\alpha\\src\\lib.rs"), // edit 把整段删空 ⇒ 与 call-5 同一路径
    ]
    .iter()
    .map(|(call_id, path)| (call_id.to_string(), path.to_string()))
    .collect();
    assert_eq!(
        registered_paths(&journal),
        expected,
        "突变登记：非突变与参数残缺都该登记空串"
    );
    assert_eq!(
        produced_paths(&journal),
        vec![
            "E:\\demo\\alpha\\src\\lib.rs".to_string(),
            "E:\\demo\\alpha\\out\\notes.md".to_string(),
        ],
        "只有两条路径产出 chip：call-10 与 call-5 同路径要去重，isError 的 call-6、\
         残缺的 call-9、非突变的其余五条都不产出"
    );
    // 那一翻 isError 就落在 call-6 上，整轮只此一条，别把成功分支也带歪。
    let errored: Vec<&str> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/result"))
        .filter(|event| {
            event["data"]["message"]["content"]
                .as_array()
                .and_then(|blocks| blocks.first())
                .is_some_and(|block| block["isError"].as_bool().unwrap_or(false))
        })
        .map(|event| {
            event["data"]["message"]["source"]["callId"]
                .as_str()
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(errored, vec!["call-6"], "整轮只该有一条 isError 结果");

    // 失败收尾的尝试：分叉据此汇一条 `⚠ code: message` 助手卡，字段名照主线 assistant/attempt。
    let attempt = journal
        .iter()
        .find(|event| event["type"].as_str() == Some("assistant/attempt"))
        .expect("脚本里得有 assistant/attempt");
    let reason = &attempt["data"]["stream"][0]["chunk"]["reason"];
    assert_eq!(
        attempt["data"]["stream"][0]["chunk"]["type"],
        json!("finish")
    );
    assert_eq!(reason["kind"], json!("error"));
    assert_eq!(reason["failure"]["code"], json!("provider_rate_limited"));
    assert!(
        reason["failure"]["message"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "失败文案要非空，否则分叉只能回落兜底串"
    );
    assert_eq!(attempt["data"]["turn"], json!(1));

    // 系统提示词：同一会话的第二条起分叉改说「系统提示词更新」，两条都得落在本轮。
    let systems: Vec<i64> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("system/message"))
        .map(|event| event["data"]["turn"].as_i64().unwrap_or_default())
        .collect();
    assert_eq!(systems, vec![1, 1], "一轮里两条 system/message");

    // 内核命名链：侧栏就地改名那条，标题必须非空。
    let titled = journal
        .iter()
        .find(|event| event["type"].as_str() == Some("session/title"))
        .expect("脚本里得有 session/title");
    assert_eq!(titled["data"]["title"], json!("对齐内核 journal"));
    assert_eq!(titled["data"]["turn"], json!(1));

    // 交付物两条：主干用「、」拼接，只有一条文件永远验不到分隔符。
    let presented = journal
        .iter()
        .find(|event| event["type"].as_str() == Some("deliverables/presented"))
        .expect("脚本里得有 deliverables/presented");
    let files = presented["data"]["files"].as_array().expect("files 是数组");
    assert_eq!(files.len(), 2);
    assert!(
        files
            .iter()
            .all(|file| file["path"].as_str().is_some_and(|path| !path.is_empty())),
        "每条交付物都要带 path"
    );
    assert_eq!(presented["data"]["turn"], json!(1));

    // 长活流：说完一轮也不能 end。
    assert!(
        collect_within(&mut mux, &follow, 1, QUIET_BUDGET).is_empty(),
        "session/follow 不该发 end"
    );

    kernel
        .call("session/prompt", prompt_args("s-9001", "第二句", "steer"))
        .expect("同一会话的第二轮也应被接受");
    let second = item_values(&collect(&mut mux, &follow, TURN_FRAMES), &follow);
    assert_eq!(second[0]["event"]["type"], json!("turn/start"));
    assert_eq!(
        second[0]["event"]["seq"],
        json!(31),
        "seq 要接着上一轮往后走"
    );
    assert_eq!(second[0]["event"]["data"]["turn"], json!(2));
    assert_eq!(
        second[1]["event"]["data"]["content"][0]["text"],
        json!("第二句")
    );
    // 偶数轮换成四桶口径：totalTokens 缺席，主干把四桶相加。
    let usage = &second[32]["event"]["data"]["usage"];
    assert!(
        usage.get("totalTokens").is_none(),
        "第二轮不该再带 totalTokens"
    );
    assert_eq!(usage["inputTokens"], json!(1200));
    assert_eq!(
        second[32]["event"]["seq"],
        json!(57),
        "第二轮的 settled 也跟着整轮往后挪"
    );
    assert_eq!(
        second[32]["event"]["data"]["message"]["id"],
        json!("msg-2-57"),
        "第二轮的消息 id 与 seq 都要往后挪"
    );
    let second_journal: Vec<&Value> = second
        .iter()
        .filter(|frame| tag(frame).starts_with("event:"))
        .map(|frame| &frame["event"])
        .collect();
    assert_eq!(
        produced_paths(&second_journal).len(),
        2,
        "第二轮的突变产出与第一轮同形（登记表按 callId 复用，turn/start 要清空）"
    );
    assert!(
        collect_within(&mut mux, &follow, 1, QUIET_BUDGET).is_empty(),
        "第二轮之后 follow 仍不该 end"
    );
}

#[test]
fn mux_cancel_stops_the_follow_stream() {
    let (mut kernel, mut mux, follow) = open_stream_with("session/follow", follow_args("s-9002"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");

    // 地址不合的 follow 直接回 error 帧，不能悄悄挂一条空流。
    let bogus_stream = mux
        .open(
            "session/follow",
            json!({ "request": { "address": { "kind": "workspace", "sessionId": "s-9002" } } }),
        )
        .expect("open 坏参数的流");
    match &collect(&mut mux, &bogus_stream, 1)[..] {
        [MuxEvent::Failure { stream, .. }] => assert_eq!(stream, &bogus_stream),
        other => panic!("坏参数的 session/follow 应回 error 帧，实际 {other:?}"),
    }

    mux.cancel(&follow).expect("cancel 帧应发得出");
    // 读循环按序处理帧：新流的快照到了，说明 cancel 已经被读到。
    let kept = mux
        .open("session/follow", follow_args("s-9003"))
        .expect("cancel 之后还要能开流");
    assert_eq!(
        collect(&mut mux, &kept, 1).len(),
        1,
        "cancel 不能把整根 socket 带坏"
    );

    kernel
        .call(
            "session/prompt",
            prompt_args("s-9002", "没人听的话", "queue"),
        )
        .expect("没有跟随流的会话也要收下");
    assert!(
        collect_within(&mut mux, &follow, 1, QUIET_BUDGET).is_empty(),
        "被 cancel 的流不该再出帧"
    );
    assert!(
        collect_within(&mut mux, &kept, 1, QUIET_BUDGET).is_empty(),
        "s-9002 的轮不该串到 s-9003 的流上"
    );
    kernel.shutdown();
}

#[test]
fn events_stream_reports_running_during_a_turn() {
    let (mut kernel, mut mux, events) = open_stream("$events");
    assert_eq!(
        collect(&mut mux, &events, 2).len(),
        2,
        "ready + 初始 emit 打底"
    );
    let follow = mux
        .open("session/follow", follow_args("s-9004"))
        .expect("同一条 socket 上再开 follow");
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "快照先到");

    kernel
        .call("session/prompt", prompt_args("s-9004", "跑一轮", "queue"))
        .expect("session/prompt 应被接受");
    let flags: Vec<(String, bool)> =
        item_values(&collect_within(&mut mux, &events, 2, WAIT_BUDGET), &events)
            .iter()
            .filter_map(session_status_event)
            .collect();
    assert_eq!(
        flags,
        vec![("s-9004".to_string(), true), ("s-9004".to_string(), false)],
        "一轮里运行态要先亮后灭"
    );
}

#[test]
fn mux_two_streams_share_one_socket() {
    let (_kernel, mut mux, workspaces) = open_stream("workspace/follow");
    let baseline = item_values(&collect(&mut mux, &workspaces, 2), &workspaces);
    assert_eq!(baseline[0]["type"], json!("baseline"));

    // 主干的真实顺序：workspace/follow 建表之后才订 `$events`，两条流共用一根 socket。
    let events = mux.open("$events", json!({})).expect("open $events");
    let frames = item_values(&collect(&mut mux, &events, 2), &events);
    assert_eq!(frames[0]["type"], json!("ready"));
    assert_eq!(frames[0]["clientId"], json!("fake-client"));
    assert_eq!(frames[1]["event"], json!("api-session/status"));
    assert_eq!(frames[1]["args"], json!(["s-1", true]));
    assert!(
        collect_within(&mut mux, &workspaces, 1, QUIET_BUDGET).is_empty(),
        "第二次的 open 不该把工作区流顶掉"
    );
}

/// 缺口 #67 的**协议层**证据：一根 socket 死掉之后重连，三条长驻流都得能在**新连接**上重开，
/// 而且两条 `baseline` 是整表重灌而不是差分 —— 这正是分叉 `Shell::reseed_after_reconnect`
/// 「先把旧真相清空、再等新 baseline」所依赖的前提（清空了还能变回原样，才敢清）。
///
/// 这里驱动的是 `Mux` + `WorkspaceTree` + `ControlState`，也就是分叉 `apply_mux_events` 喂的
/// 那三件；`Shell` 本身在 bin 里、要真窗口，离线测不到。模型层的接线（重连必须复用
/// `open_mux`、回填必须先对代次）由 `main.rs` 的 `mux_retry_tests` 在源码上守，
/// 退避序列与上限由那里的另外两条直接算。
#[test]
fn mux_reconnect_reopens_all_three_streams_and_reseeds_from_baseline() {
    /// 三条长驻流一次开完：与分叉 `open_mux` 同一发次、同一顺序（follow 先建表 → 再订
    /// `$events` → 最后控制面），主干 `RestoreBusinessStreamsAsync` 重开的也是这三条。
    fn open_three(mux: &mut Mux) -> [String; 3] {
        let workspace = mux
            .open("workspace/follow", json!({}))
            .expect("开工作区流失败");
        let events = mux.open("$events", json!({})).expect("开 $events 流失败");
        let control = mux.open_session_control().expect("开 session/control 失败");
        [workspace, events, control]
    }

    /// 一个跨流统一收帧的泵（**不能**逐条流各 `collect` 一次：那样第一路的 `collect`
    /// 会顺手把另两路已到的事件读掉丢掉，后面两路就永远等不到）。灌进两张全新的表，
    /// 回 (工作区数, 投影会话数, 各流到达的帧型标签)。
    fn drain(mux: &mut Mux, ids: &[String; 3]) -> (usize, usize, Vec<String>) {
        let [workspace, events, control] = ids;
        let mut tree = WorkspaceTree::default();
        let mut state = ControlState::default();
        let mut tags: Vec<String> = Vec::new();
        let (mut ws, mut ev, mut ct) = (0usize, 0usize, 0usize);
        let deadline = Instant::now() + WAIT_BUDGET;
        while ws < 2 || ev < 2 || ct < CONTROL_FRAME_TYPES.len() {
            assert!(Instant::now() < deadline, "三条流的头一批帧没收齐: {tags:?}");
            for event in mux
                .collect(Duration::from_millis(200))
                .expect("读帧不应失败")
            {
                let MuxEvent::Item {
                    value: Some(frame),
                    stream,
                } = event
                else {
                    continue;
                };
                let kind = frame["type"].as_str().unwrap_or_default().to_string();
                if stream == *workspace {
                    ws += 1;
                    tags.push(format!("ws/{kind}"));
                    tree.apply(&frame);
                } else if stream == *control {
                    ct += 1;
                    tags.push(format!("ct/{kind}"));
                    state.apply(&frame);
                } else {
                    assert_eq!(&stream, events, "只剩 $events 这一条流没认出来: {stream}");
                    ev += 1;
                    tags.push(format!("ev/{kind}"));
                }
            }
        }
        (tree.workspaces.len(), state.projections.len(), tags)
    }

    /// 只看某一条流的帧型序列（三条流的到达顺序跨流不必一致，流**内**必须一致）。
    fn tags_of<'a>(prefix: &str, tags: &'a [String]) -> Vec<&'a str> {
        tags.iter()
            .filter_map(|tag| tag.strip_prefix(prefix))
            .collect()
    }

    let kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let mut first = Mux::connect(kernel.endpoint(), kernel.cookie()).expect("首连握手失败");
    let ids = open_three(&mut first);
    assert_eq!(
        ids[0], "rs1",
        "首连的流编号口径变了 ⇒ 下面那条「新连接会串台」的论证不成立"
    );
    let before = drain(&mut first, &ids);
    assert!(before.0 > 0, "假内核该有工作区 baseline");
    assert!(before.1 > 0, "假内核的 baseline 该带会话投影");
    assert_eq!(tags_of("ws/", &before.2), vec!["baseline", "upsert"]);
    assert_eq!(
        tags_of("ct/", &before.2),
        vec!["baseline", "queue", "jobs", "projection"]
    );

    // ---- 断流：丢掉客户端实例 == socket 关闭（真内核那侧就是 `os error 10054` 那一类）----
    drop(first);

    // ---- 重连：新 socket 上三条流全部重开，两张表**全新的**（= `reseed_after_reconnect`）----
    let mut second = Mux::connect(kernel.endpoint(), kernel.cookie()).expect("重连握手失败");
    let ids = open_three(&mut second);
    assert_eq!(
        ids[0], "rs1",
        "新连接的 streamId 又从头编起 ⇒ 这正是分叉必须把 `control_stream` / `follows` 两张标记\
         随实例一起作废的原因：留着旧的就会把帧串到别的流上"
    );
    let after = drain(&mut second, &ids);
    assert_eq!(after.0, before.0, "重连后工作区 baseline 必须是全量，不是差分");
    assert_eq!(after.1, before.1, "重连后投影 baseline 必须是全量，不是差分");
    assert_eq!(
        tags_of("ws/", &after.2),
        tags_of("ws/", &before.2),
        "工作区流重开后到达的帧型序列必须一模一样"
    );
    assert_eq!(
        tags_of("ct/", &after.2),
        tags_of("ct/", &before.2),
        "控制面重开后同样以全量 baseline 开头"
    );
    assert!(
        tags_of("ev/", &after.2).first().is_some_and(|kind| *kind == "ready"),
        "$events 重开的第一帧必须是 ready（分叉靠它认这条流）: {:?}",
        tags_of("ev/", &after.2)
    );
}

#[test]
fn select_model_echoes_and_page_backfills_the_journal() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let value = kernel
        .call(
            "session/selectModel",
            json!({ "request": {
                "sessionId": "s-1001", "provider": "deepseek", "model": "dsk-reason",
                "reasoningEffort": "high",
            } }),
        )
        .expect("selectModel 应回显选择");
    assert_eq!(
        value["selected"],
        json!({ "provider": "deepseek", "model": "dsk-reason", "reasoningEffort": "high" })
    );

    let value = kernel
        .call(
            "session/selectModel",
            json!({ "request": { "sessionId": "s-1001", "provider": "p", "model": "m" } }),
        )
        .expect("不带推理档也要能选");
    assert_eq!(value["selected"], json!({ "provider": "p", "model": "m" }));
    assert!(
        value["selected"].get("reasoningEffort").is_none(),
        "没发过的键不能冒出来"
    );

    let error = kernel
        .call(
            "session/selectModel",
            json!({ "sessionId": "s-1001", "provider": "p", "model": "m" }),
        )
        .expect_err("扁平传必须被拒");
    assert!(error.contains("bad_args"), "{error}");

    // #78：`session/page` 从「桩里挂着但分叉根本不读」升格成分叉进会话第一发读的端点，
    // 于是桩这边也给了真数据 —— 种子会话 s-1001 起手就有三轮历史（`seed_journal` 由
    // `seed_outline` 派生）。「没发过提示的会话是一页空的」那条前提搬到 s-1002。
    let blank = kernel
        .call("session/page", page_args("s-1002", None, None))
        .expect("空白会话也该回一页记录");
    assert!(records_of(&blank).is_empty(), "空白会话没有 journal: {blank}");
    assert!(
        page_events(&blank).is_empty(),
        "空页剥信封也得是空数组（不能冒出一条 None 变体）"
    );
    assert_eq!(blank["hasMore"], json!(false));

    let seeded = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("种子会话该带着三轮历史回来");
    let seeded_records = records_of(&seeded);
    let seeded_events = page_events(&seeded);
    assert_eq!(
        seeded_records.len(),
        90,
        "三轮 × 卅条 = 90 条，一发拉完"
    );
    assert_eq!(
        seeded_events.len(),
        90,
        "剥信封不丢条：90 条记录都是带 event 子对象的好形状"
    );
    assert_eq!(
        turn_start_seqs(&seeded_events),
        vec![3, 41, 88],
        "轮锚 seq 就是大纲里写死的那三个（轮轨 #65 的跳转目标按它定位）"
    );
    assert_eq!(
        user_texts(&seeded_events),
        ["把登录改成走内核", "补上失败分支", "再补一轮测试"],
        "提问正文按时间序回来"
    );
    assert_eq!(
        event_types(&seeded_events[..30]),
        turn_skeleton(),
        "回填的第一轮与 follow 流推过的那卅条逐字同形"
    );
    assert_eq!(
        seeded_events.last().and_then(|event| event["seq"].as_i64()),
        Some(117),
        "一页拉完时末条 seq == 游标"
    );

    // 游标探测那一发（`kernel::JOURNAL_PROBE_SEQ = 1<<30`）：真内核越界回
    // `gateway/bad-request` + "session page through seq <n> is past cursor <sourceCursor>"
    // （`dsh-api-session-controller/lib/index.js:1379`），桩逐字同形 —— 不然
    // `parse_past_cursor_seq` 与回退重试这条路径在自测里永远走不到。
    let probe = kernel
        .call("session/page", page_args("s-1001", Some(1 << 30), None))
        .expect_err("越界 throughSeq 必须被拒");
    assert_eq!(
        probe,
        "gateway/bad-request: session page through seq 1073741824 is past cursor 117"
    );
    // 空 journal 的游标是 -1（真内核 `sourceLog.at(-1)?.seq ?? -1`）：一发就到底。
    let probe_blank = kernel
        .call("session/page", page_args("s-1002", Some(1 << 30), None))
        .expect_err("空白会话的探测同样越界");
    assert!(probe_blank.ends_with("is past cursor -1"), "{probe_blank}");

    kernel
        .call("session/prompt", prompt_args("s-1001", "回填我", "queue"))
        .expect("session/prompt 应被接受");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("一轮之后 journal 能回填");
    let records = records_of(&page);
    let events = page_events(&page);
    assert_eq!(records.len(), 120, "90 条种子历史 + 本轮卅条");
    // 回填的 journal 要与 follow 流推过的那批同形：卅条事件，突变/结果成对铺在答案之前。
    assert_eq!(
        event_types(&events[90..]),
        turn_skeleton(),
        "回填的 journal 要与 follow 流推过的那批同形"
    );
    for record in &records {
        assert_eq!(record["type"], json!("event"));
    }
    assert_eq!(
        events[90 + 26]["data"]["message"]["content"][0]["text"],
        json!("已收到: 回填我")
    );
    // 主干按信封 seq 定位回放锚点：本轮的 seq 也得连号，且接着种子游标往下走。
    assert_eq!(
        seqs_of(&events[90..]),
        (118..=147).collect::<Vec<i64>>(),
        "本轮首 seq = 种子游标 117 + 1"
    );

    // 主干翻历史的两把刀：throughSeq 截断、maxMessages 取尾部。
    let capped = kernel
        .call("session/page", page_args("s-1001", Some(35), None))
        .expect("throughSeq");
    let capped_records = records_of(&capped);
    let capped_events = page_events(&capped);
    assert_eq!(capped_records.len(), 30, "只留 seq<=35 = 第一轮那卅条");
    assert_eq!(turn_start_seqs(&capped_events), vec![3], "截断只到第一轮");
    assert_eq!(
        seqs_of(&capped_events),
        (3..=32).collect::<Vec<i64>>()
    );
    let tail = kernel
        .call("session/page", page_args("s-1001", None, Some(2)))
        .expect("maxMessages");
    let tail_records = records_of(&tail);
    let tail_events = page_events(&tail);
    assert_eq!(tail_records.len(), 2, "maxMessages=2 ⇒ 尾部两条记录");
    assert_eq!(
        event_types(&tail_events),
        vec!["session/title", "turn/end"]
    );
    assert_eq!(
        tail["hasMore"],
        json!(true),
        "被 maxMessages 截断 ⇒ hasMore 翻真，主干据此继续往前翻"
    );

    let error = kernel
        .call(
            "session/page",
            json!({ "request": { "address": { "kind": "workspace", "sessionId": "s-1001" } } }),
        )
        .expect_err("address.kind 只能是 session");
    assert!(error.contains("bad_args"), "{error}");

    // 停止键语义那条 unary：形状与 selectModel 同族。
    let value = kernel
        .call(
            "session/cancel",
            json!({ "request": { "sessionId": "s-1001" } }),
        )
        .expect("session/cancel 应被接受");
    assert_eq!(value, json!({ "accepted": true }));
    let error = kernel
        .call("session/cancel", json!({ "sessionId": "s-1001" }))
        .expect_err("扁平传必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    kernel.shutdown();
}

/// 交付物「打开/回显」是宿主路由而非 RPC（分叉走 `Kernel::post_route`，真 HTTP）：
/// 内核按 (sessionId, seq, index) 回查 journal 定位文件，命中 204 无正文。
#[test]
fn present_open_route_resolves_the_journal_coordinates() {
    let (mut kernel, mut mux, follow) = open_stream_with("session/follow", follow_args("s-9005"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call(
            "session/prompt",
            prompt_args("s-9005", "打开交付物", "queue"),
        )
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect(&mut mux, &follow, TURN_FRAMES), &follow);
    // 回查坐标取交付物那条事件自己的信封 seq：写死会在脚本加事件时错位。
    let presented = frames
        .iter()
        .map(|frame| &frame["event"])
        .find(|event| event["type"].as_str() == Some("deliverables/presented"))
        .expect("本轮该有 deliverables/presented");
    let seq = presented["seq"].as_i64().expect("交付物事件有 seq");
    assert_eq!(
        presented["data"]["files"]
            .as_array()
            .expect("files 是数组")
            .len(),
        2
    );

    // 命中：204 且真没正文（主干把 200/204 都当「系统已接手」，静默无提示）。
    for (index, action) in [(0usize, "open"), (1usize, "reveal")] {
        let (status, body) = kernel
            .post_route(&format!(
                "/api/present.open?sessionId=s-9005&seq={seq}&index={index}&action={action}"
            ))
            .expect("宿主路由该走真 HTTP 通到假内核");
        assert_eq!(status, 204, "{action} 第 {index} 条该静默接手: {body}");
        assert_eq!(body, "", "204 不该带正文");
    }

    // 回查不到：主干据此提示「找不到该交付物」，绝不能被当成成功。
    let misses = [
        format!("/api/present.open?sessionId=s-9005&seq={seq}&index=9&action=open"),
        format!("/api/present.open?sessionId=s-9005&seq=400000&index=0&action=open"),
        "/api/present.open?sessionId=s-nope&seq=1&index=0&action=open".to_string(),
    ];
    for case in misses {
        let (status, _) = kernel.post_route(&case).expect("路由本身可达");
        assert_eq!(status, 404, "{case} 该回 404");
    }

    // 坐标缺失/动作不认识：400——参数问题不伪装成「找不到」，也不伪装成 204。
    for case in [
        "/api/present.open",
        "/api/present.open?sessionId=s-9005",
        "/api/present.open?sessionId=&seq=1&index=0&action=open",
        "/api/present.open?sessionId=s-9005&seq=x&index=0&action=open",
        "/api/present.open?sessionId=s-9005&seq=1&action=open",
        "/api/present.open?sessionId=s-9005&seq=1&index=0&action=launch",
    ] {
        let (status, _) = kernel.post_route(case).expect("路由本身可达");
        assert_eq!(status, 400, "{case} 该回 400");
    }
    kernel.shutdown();
}

/// 轮尾 chip 点击那条 RPC（分叉 `open_path` → `session/openWorkspacePath`）：
/// 成功回信封，路径为空要失败，两条分支都得在自测里走得通。
#[test]
fn open_workspace_path_rpc_covers_both_click_branches() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let value = kernel
        .call(
            "session/openWorkspacePath",
            json!({ "request": { "path": "E:\\demo\\alpha\\src\\lib.rs" } }),
        )
        .expect("有效路径应回成功信封");
    assert_eq!(value, json!({ "opened": true }), "主干只看 opened 布尔");
    // 侧栏那处调用还带 action=reveal，同族参数一并收下。
    kernel
        .call(
            "session/openWorkspacePath",
            json!({ "request": { "path": "C:/repo/one", "action": "reveal" } }),
        )
        .expect("带 action 也要能开");

    let error = kernel
        .call(
            "session/openWorkspacePath",
            json!({ "request": { "path": "" } }),
        )
        .expect_err("空路径必须 ok:false");
    assert!(error.contains("bad_args"), "{error}");
    let error = kernel
        .call(
            "session/openWorkspacePath",
            json!({ "path": "C:/repo/one" }),
        )
        .expect_err("扁平传必须被拒");
    assert!(error.contains("bad_args"), "{error}");
    kernel.shutdown();
}

/// 台账 #77 卡 P4-a 第一颗：`session/canOpenWorkspacePath`（主干 `CanOpenWorkspacePathAsync`，
/// `MainWindow.xaml.cs:4181-4202`）此前四层里 **④ 全零** —— ②有调用点、③有桩臂，但从没人在真
/// socket 上过一遍。它演的是「本部署能不能在内核主机开路径」那道**开关**，回执是**裸 boolean**
/// （`dsh-api-session-controller/lib/index.js:2570` + `dsh-api-remotes/lib/client.js:7497`），
/// 一旦被包成对象，主干那声 `GetBoolean()` 就读成 false ⇒ 整条「在文件夹中打开」静默消失。
#[test]
fn can_open_workspace_path_answers_a_bare_boolean_and_takes_no_arguments() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let opened = kernel
        .call("session/canOpenWorkspacePath", json!({}))
        .expect("无参探测该被臂接住");
    assert!(opened.is_boolean(), "result 是裸 `boolean()`，实际 {opened}");
    assert_eq!(opened, json!(true), "本部署（真内核）回 true，桩跟着回 true");
    assert!(
        raw_response_body(&kernel, "session/canOpenWorkspacePath", "r-canopen", json!({}))
            .contains(r#""value":true"#),
        "线上信封得是 `\"value\":true`，不是 `\"value\":{{...}}`"
    );

    // 描述符 `parameters: []` ⇒ wire 上多一颗键就该被 assertExactArguments 拒掉。
    let err = kernel
        .call("session/canOpenWorkspacePath", json!({ "path": "C:/repo/one" }))
        .expect_err("这颗端点不收参数");
    assert!(err.starts_with("bad_args: "), "{err}");
    assert!(err.contains(r#"unexpected "path""#), "{err}");
    kernel.shutdown();
}

/// 台账 #77 卡 P4-a 第二颗：`session/modelCatalog`（主干 `RunKernelBootCoreAsync`）此前
/// 同样 **④ 全零**。断言的是**逐字键名**：旧桩那一份 `{items:[{id,label}]}` 是凭空造的键，
/// 真内核从来不发 ⇒ 分叉按 id 查档位会查空而不报错（`fake_dsh.rs:5047` 那条自陈）。
#[test]
fn model_catalog_replies_the_schema_shape_and_not_an_invented_items_list() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let catalog = kernel
        .call("session/modelCatalog", json!({}))
        .expect("session/modelCatalog 该被臂接住（不在臂上会回 not_found）");
    assert_eq!(
        sorted_keys(&catalog),
        ["default", "groups", "routableProviders"],
        "目录三根键（`session_modelCatalog_result$schema`）"
    );
    assert!(
        catalog.get("items").is_none(),
        "旧桩那凭空造的 `items` 键回来了 ⇒ 分叉的行卡会静默变空态"
    );
    assert_eq!(
        sorted_keys(&catalog["default"]),
        ["model", "provider", "reasoningEffort"],
        "`default` 恰三键"
    );
    let group = &catalog["groups"][0];
    assert_eq!(
        sorted_keys(group),
        ["id", "models", "name"],
        "分组恰三键，模型住在 `models` 而不是 `items`"
    );
    let model = &group["models"][0];
    assert_eq!(
        sorted_keys(model),
        ["description", "id", "name", "reasoning"],
        "目录条目四键（`description`/`reasoning` 都可缺，但这台部署给全）"
    );
    assert_eq!(
        sorted_keys(&model["reasoning"]),
        ["defaultEffort", "efforts"],
        "`reasoning` 两键"
    );
    assert_eq!(
        sorted_keys(&model["reasoning"]["efforts"][0]),
        ["id", "name"],
        "档位条目两键"
    );

    // 真接线判据：分叉是**按 id 查档**的（`fake_dsh.rs:5047` 那条自陈点名的就是这件事）
    // ⇒ `default` 那三根字面值必须在目录里查得到，查空是静默的。
    assert_eq!(
        model["id"], catalog["default"]["model"],
        "default.model 得指到目录里那一枚"
    );
    assert_eq!(
        model["reasoning"]["efforts"][0]["id"],
        catalog["default"]["reasoningEffort"],
        "default.reasoningEffort 得指到 efforts 里那一档"
    );
    let providers = catalog["routableProviders"].as_array().expect("数组");
    assert!(
        providers
            .iter()
            .any(|row| row.as_str() == Some("dsh-test")),
        "routableProviders 得含本部署那一颗：{}",
        catalog["routableProviders"]
    );
    assert!(
        providers.iter().all(|row| row.is_string()),
        "routableProviders 是字符串数组，不是对象数组"
    );
    kernel.shutdown();
}

/// 设置页·模型提供方那一叠只读端点：目录 / 已注册路由 / 设置文档快照 / 凭据状态 / 拉模型。
/// 断言的是**逐字的键名与形状**——主干 `LoadProviderRowsAsync` 就是照这几个键取值的，
/// 假内核给错一个键名，分叉的行卡会静默变空态而不是报错。
#[test]
fn settings_rpc_lists_providers_and_credential_states() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let catalog = kernel
        .call("llm/listConfigurableProviders", json!({}))
        .expect("提供方目录应非空，模型页才有行卡可渲染");
    assert_eq!(
        catalog,
        json!([
            {
                "provider": "deepseek-official",
                "displayName": "DeepSeek",
                "settingsNs": "llm-deepseek",
                "settingsPath": [],
            },
            {
                "provider": "my-gateway",
                "displayName": "自建网关",
                "settingsNs": "llm-pi-ai",
                "settingsPath": ["providers", "my-gateway"],
                "declared": true,
            },
        ]),
        "适配器注册的根没有 declared 键；手声明的路由才有（主干据此画「自定义」标记）"
    );

    let routes = kernel
        .call("llm/listProviders", json!({}))
        .expect("已注册路由清单");
    assert_eq!(
        routes,
        json!([
            { "id": "deepseek-official", "name": "DeepSeek" },
            { "id": "my-gateway", "name": "自建网关" },
        ]),
        "这条只喂设置页那句说明文案：键是 id/name，与目录的 provider/displayName 不同源"
    );

    let described = kernel
        .call("settings/describe", json!({}))
        .expect("一次回全部命名空间");
    assert_eq!(described["writable"], json!(true));
    assert_eq!(described["hasDocument"], json!(true));
    let namespaces = described["namespaces"]
        .as_array()
        .expect("缺 namespaces 主干就当整次失败");
    // #128（SK3）改判：原先钉 `== 3`，而「台账只有 3 支」正是本卡要消灭的缺口，
    // 不是被我用例改坏的目标。台账现在 1:1 覆盖主干 `MainWindow.xaml.cs:10625-10630`
    // `SectionNamespaces` 四分区列出的 13 支 ns。下面 `[0]`/`[1]`/`[2]` 的下标断言
    // 仍然成立——扩容是**追加**在既有三支之后，顺序没动。
    assert_eq!(namespaces.len(), 13);
    let official = &namespaces[0];
    assert_eq!(official["ns"], json!("llm-deepseek"));
    assert_eq!(official["user"], json!({}), "没写过用户层就是空对象（value 全继承 base）");
    assert_eq!(
        official["value"]["models"]
            .as_array()
            .expect("models 是数组")
            .len(),
        2,
        "主干 InheritedModelsOf 读的就是这一串，作为编辑卡的继承模型目录"
    );
    let pi = &namespaces[1];
    assert_eq!(pi["ns"], json!("llm-pi-ai"));
    assert_eq!(pi["revision"].as_f64(), Some(0.0), "revision 从 0 起（真内核 dsh-settings/lib/index.js:428）");
    assert_eq!(pi["applies"], json!("live"));
    assert_eq!(
        pi["user"]["providers"]["my-gateway"]["baseURL"],
        json!("https://gateway.invalid/v1")
    );
    assert!(
        pi["base"]["providers"]["my-gateway"].is_null(),
        "user 命中而 base 不命中 → 主干判 Removable，删除钮才出现"
    );
    assert_eq!(
        pi["schema"]["dict"]["providers"]["inner"]["dict"]["api"]["list"]
            .as_array()
            .expect("api 是 union")
            .len(),
        3,
        "ProtocolChoices 走的就是 providers.\\0probe.api 这把 schema 路径"
    );

    let states = kernel
        .call(
            "credentials/describe",
            json!({ "refs": ["DEEPSEEK_OFFICIAL_API_KEY", "MY_GATEWAY_API_KEY", "NO_SUCH_KEY"] }),
        )
        .expect("批量点名凭据");
    assert_eq!(
        states,
        json!({
            "DEEPSEEK_OFFICIAL_API_KEY": { "configured": true, "writable": true, "source": "credential-store" },
            "MY_GATEWAY_API_KEY": { "configured": true, "writable": true, "source": "credential-store" },
            "NO_SUCH_KEY": { "configured": false, "writable": true },
        }),
        "Record 的键就是请求点名的 ref；configured 决定状态点，writable 决定删除时带不带 credentials/unset"
    );

    let models = kernel
        .call(
            "llm/discoverModels",
            json!({ "settingsNs": "llm-pi-ai", "request": {
                "provider": "my-gateway", "baseURL": "https://gateway.invalid/v1",
                "api": "openai-completions", "apiKey": "sk-fake",
            } }),
        )
        .expect("探针拉模型");
    assert_eq!(
        models,
        json!([
            { "id": "router-mini", "name": "Router Mini", "contextWindow": 131072, "maxTokens": 8192 },
            { "id": "router-pro" },
        ]),
        "name/contextWindow/maxTokens 都是可选键，缺省就不冒出来"
    );

    for (label, method, args, code) in [
        (
            "无参端点多给一个键",
            "llm/listConfigurableProviders",
            json!({ "_request": {} }),
            "bad_args",
        ),
        (
            "llm/listProviders 不吃参数",
            "llm/listProviders",
            json!({ "request": {} }),
            "bad_args",
        ),
        (
            "settings/describe 不吃 ns",
            "settings/describe",
            json!({ "ns": "llm-pi-ai" }),
            "bad_args",
        ),
        (
            "discoverModels 缺 request 这根 wire 字段",
            "llm/discoverModels",
            json!({ "settingsNs": "llm-pi-ai" }),
            "bad_args",
        ),
        (
            "discoverModels 的 request 不是对象",
            "llm/discoverModels",
            json!({ "settingsNs": "llm-pi-ai", "request": "my-gateway" }),
            "bad_args",
        ),
        (
            "credentials/describe 的键名是 refs 不是 ref",
            "credentials/describe",
            json!({ "ref": "X" }),
            "bad_args",
        ),
        (
            "credentials/describe 的 refs 只能是字符串数组",
            "credentials/describe",
            json!({ "refs": [1] }),
            "bad_args",
        ),
        (
            "discoverModels 的 provider/baseURL 全空",
            "llm/discoverModels",
            json!({ "settingsNs": "llm-pi-ai", "request": {} }),
            "llm/model-discovery-rejected",
        ),
        (
            "discoverModels 的 ns 没注册 discovery",
            "llm/discoverModels",
            json!({ "settingsNs": "llm-nope", "request": { "provider": "p" } }),
            "llm/model-discovery-rejected",
        ),
    ] {
        let error = kernel
            .call(method, args)
            .expect_err("参数不合 wire 描述符 / 探针被内核拒绝");
        assert!(error.contains(code), "{label} 该回 {code}，实际 {error}");
    }
    kernel.shutdown();
}

/// 设置页的三条写端点 + 凭据库 + 「打开设置文档」。主干的闭环是「写一次 → 重新 describe →
/// 重渲染」，所以假内核必须真的把写入落进台账：按真内核「raw 变了才 +1」（rs1 §1）、陈旧写回
/// `settings/conflict`、被拒的写不入账、`z.void()` 那两条线上根本没有 `value` 键。
#[test]
fn settings_write_endpoints_round_trip_and_track_revisions() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // describe 先把三个命名空间各起一行账：revision 从 0 起（真内核 `:428` 的 `? 0`）。
    let described = kernel
        .call("settings/describe", json!({}))
        .expect("一次回全部命名空间");
    assert_eq!(
        described["namespaces"][1]["revision"].as_f64(),
        Some(0.0),
        "写之前先读到的 revision 就是乐观锁的初值"
    );

    let mutate_args = |ops: Value, expected: Option<f64>, ns: &str| -> Value {
        let mut args = json!({ "ns": ns, "ops": ops });
        if let Some(revision) = expected {
            args["expectedRevision"] = json!(revision);
        }
        args
    };

    // settings/mutate：path 寻址的最小改动；回的就是新那一段视图（与 describe 的元素同形）。
    let mutated = kernel
        .call(
            "settings/mutate",
            mutate_args(
                json!([
                    { "op": "set", "path": ["providers", "my-gateway", "baseURL"], "value": "https://alt.invalid/v1" }
                ]),
                Some(0.0),
                "llm-pi-ai",
            ),
        )
        .expect("带正确 expectedRevision 的写应提交");
    assert_eq!(mutated["ns"], json!("llm-pi-ai"));
    assert_eq!(
        mutated["revision"].as_f64(),
        Some(1.0),
        "提交一次 revision +1（起点 0 ⇒ 1）"
    );
    assert_eq!(
        mutated["user"]["providers"]["my-gateway"]["baseURL"],
        json!("https://alt.invalid/v1"),
        "user 层才是「页面改过什么」的真相，主干算删除清单时看它"
    );
    assert_eq!(
        mutated["value"]["providers"]["my-gateway"]["baseURL"],
        json!("https://alt.invalid/v1"),
        "value 是 base+user 的合并层，行卡渲染读的就是它"
    );

    // 拿旧 revision 再写：内核回 settings/conflict（主干为这个码留了专属文案）。
    let error = kernel
        .call(
            "settings/mutate",
            mutate_args(
                json!([
                    { "op": "set", "path": ["providers", "my-gateway", "baseURL"], "value": "https://stale.invalid/v1" }
                ]),
                Some(0.0),
                "llm-pi-ai",
            ),
        )
        .expect_err("乐观锁必须挡住陈旧写");
    assert!(error.contains("settings/conflict"), "{error}");
    assert!(
        error.contains("expectedRevision 0") && error.contains("实际 1"),
        "details 里得铺出 expected/actual，主干才有的可展示：{error}"
    );

    // 冲突不入账：按真实 revision 接着写还是能过；unset 整段路由后行卡就该消失。
    let removed = kernel
        .call(
            "settings/mutate",
            mutate_args(
                json!([{ "op": "unset", "path": ["providers", "my-gateway"] }]),
                Some(1.0),
                "llm-pi-ai",
            ),
        )
        .expect("陈旧写之后真实 revision 仍可用");
    assert_eq!(removed["revision"].as_f64(), Some(2.0));
    assert_eq!(removed["user"]["providers"], json!({}));
    assert!(
        removed["value"]["providers"]["my-gateway"].is_null(),
        "删掉整段路由后 value 里就没这条了——主干据此撤下卡片"
    );
    assert_eq!(
        removed["secrets"][0]["set"], json!(false),
        "secrets 跟着 user 层走：主干删除时据此决定要不要顺带 credentials/unset"
    );

    // settings/replace：整段替换用户层（浅合并是 update 的活），空 section 就是「恢复默认」。
    let replaced = kernel
        .call(
            "settings/replace",
            json!({ "ns": "llm-pi-ai", "section": { "providers": { "my-gateway": { "displayName": "重建的网关" } } } }),
        )
        .expect("不带 expectedRevision 就是无锁写");
    assert_eq!(replaced["revision"].as_f64(), Some(3.0));
    assert_eq!(
        replaced["user"]["providers"]["my-gateway"]["displayName"],
        json!("重建的网关")
    );
    assert!(
        replaced["user"]["providers"]["my-gateway"]
            .get("baseURL")
            .is_none(),
        "section 是整段替换：上一轮的字段不会漏进这一轮"
    );

    // settings/update：顶层字段浅合并，主干「设为默认」走的就是这一条。
    let updated = kernel
        .call(
            "settings/update",
            json!({ "ns": "agent-presets", "patch": { "default": "my-reviewer" } }),
        )
        .expect("patch 提交");
    assert_eq!(updated["ns"], json!("agent-presets"));
    assert_eq!(updated["user"], json!({ "default": "my-reviewer" }));
    assert_eq!(updated["value"]["default"], json!("my-reviewer"));
    assert_eq!(
        updated["revision"].as_f64(),
        Some(1.0),
        "每个命名空间各记各的账，互不牵连（起点 0）"
    );

    // credentials/set|unset 是 z.void()：线上没有 value 键，Kernel::call 只回 Value::Null。
    assert_eq!(
        kernel
            .call(
                "credentials/set",
                json!({ "ref": "MY_GATEWAY_API_KEY", "value": "sk-new" })
            )
            .expect("存凭据"),
        Value::Null,
        "void 端点别给它造一个 value"
    );
    let states = kernel
        .call(
            "credentials/describe",
            json!({ "refs": ["MY_GATEWAY_API_KEY", "NO_SUCH_KEY"] }),
        )
        .expect("复查凭据");
    assert_eq!(
        states["MY_GATEWAY_API_KEY"],
        json!({ "configured": true, "writable": true, "source": "credential-store" })
    );
    assert_eq!(
        states["NO_SUCH_KEY"],
        json!({ "configured": false, "writable": true }),
        "没配过的条目不冒 source 键"
    );
    assert_eq!(
        kernel
            .call("credentials/unset", json!({ "ref": "MY_GATEWAY_API_KEY" }))
            .expect("删凭据"),
        Value::Null
    );
    assert_eq!(
        kernel
            .call("credentials/describe", json!({ "refs": ["MY_GATEWAY_API_KEY"] }))
            .expect("再点一次")["MY_GATEWAY_API_KEY"]["configured"],
        json!(false),
        "状态点要跟着灭：主干保存前会重算这一批 ref"
    );
    assert_eq!(
        kernel
            .call("credentials/describe", json!({ "refs": [] }))
            .expect("空数组合法"),
        json!({}),
        "主干在没有引用时压根不发这一帧，发了也得是空 Record"
    );

    let opened = kernel
        .call("settings/openSettingsDocument", json!({}))
        .expect("打开设置文档");
    assert_eq!(opened, json!({ "opened": true }));

    for (label, method, args, code) in [
        (
            "mutate 少了 ops 这根 wire 字段",
            "settings/mutate",
            json!({ "ns": "llm-pi-ai" }),
            "bad_args",
        ),
        (
            "mutate 的 ops 不能是空数组",
            "settings/mutate",
            json!({ "ns": "llm-pi-ai", "ops": [] }),
            "bad_args",
        ),
        (
            "set 这个 op 缺 value",
            "settings/mutate",
            json!({ "ns": "llm-pi-ai", "ops": [{ "op": "set", "path": ["providers"] }] }),
            "bad_args",
        ),
        (
            "path 里只能是字符串（内核的 settings path 没有下标）",
            "settings/mutate",
            json!({ "ns": "llm-pi-ai", "ops": [{ "op": "set", "path": [0], "value": 1 }] }),
            "bad_args",
        ),
        (
            "op 只能是 set/unset",
            "settings/mutate",
            json!({ "ns": "llm-pi-ai", "ops": [{ "op": "merge", "path": ["a"], "value": 1 }] }),
            "bad_args",
        ),
        (
            "expectedRevision 显式 null：strict codec 判非法，得整个键都不发",
            "settings/update",
            json!({ "ns": "llm-pi-ai", "patch": {}, "expectedRevision": null }),
            "bad_args",
        ),
        (
            "update 的 patch 不是对象",
            "settings/update",
            json!({ "ns": "llm-pi-ai", "patch": "my-reviewer" }),
            "bad_args",
        ),
        (
            "replace 扁平传（少了 section 这根 wire 字段）",
            "settings/replace",
            json!({ "ns": "llm-pi-ai" }),
            "bad_args",
        ),
        (
            "replace 多给一个未知键",
            "settings/replace",
            json!({ "ns": "llm-pi-ai", "section": {}, "force": true }),
            "bad_args",
        ),
        (
            "ns 空字符串",
            "settings/mutate",
            json!({ "ns": "", "ops": [{ "op": "unset", "path": ["a"] }] }),
            "bad_args",
        ),
        (
            "假内核没这个命名空间",
            "settings/update",
            json!({ "ns": "llm-nope", "patch": {} }),
            "settings/rejected",
        ),
        (
            "openSettingsDocument 不吃参数",
            "settings/openSettingsDocument",
            json!({ "path": "C:/x" }),
            "bad_args",
        ),
        (
            "credentials/set 的 value 必须是字符串",
            "credentials/set",
            json!({ "ref": "X", "value": 1 }),
            "bad_args",
        ),
        (
            "credentials/set 少了 value",
            "credentials/set",
            json!({ "ref": "X" }),
            "bad_args",
        ),
        (
            "credentials/unset 的 ref 不能是空串",
            "credentials/unset",
            json!({ "ref": "" }),
            "bad_args",
        ),
    ] {
        let error = kernel.call(method, args).expect_err("参数该被拒");
        assert!(error.contains(code), "{label} 该回 {code}，实际 {error}");
    }

    // 被拒的写一律不入账：三个命名空间的账仍停在最后一次成功之后（起点是 0，见 rs1-report §1）。
    let after = kernel
        .call("settings/describe", json!({}))
        .expect("复查快照");
    assert_eq!(
        after["namespaces"][0]["revision"].as_f64(),
        Some(0.0),
        "没写过的命名空间也占一行，revision 停在起点 0"
    );
    assert_eq!(after["namespaces"][1]["revision"].as_f64(), Some(3.0));
    assert_eq!(after["namespaces"][2]["revision"].as_f64(), Some(1.0));
    kernel.shutdown();
}

/// 复查一次预设花名册（`agentPresets/list` 的 `presets` 段）：写入之后主干设置页就是这么重拉的
/// （`MW:13068` 整发 + `MW:13096` 取 `presets`）。
/// ⚠ 这里**只**取 `presets`，不给 `items` 留后路：真内核的 `remoteExportList` 从来不发 `items`
/// （取证 `rust/tmp/pk1-report.md` §1），这条 helper 若写成「先试 presets 再试 items」就等于把
/// 主干选择器那家缺陷扶正成契约。缺键时直接 panic，让 `agent_presets_list_wire_shape_…` 那条
/// 形状用例去解释为什么。
fn preset_rows(kernel: &mut Kernel) -> Vec<Value> {
    kernel
        .call("agentPresets/list", json!({}))
        .expect("花名册")["presets"]
        .as_array()
        .expect("presets 是数组（真内核的 AgentPresetRoster 只有 presets+authorable 两枚键，没有 items）")
        .clone()
}

/// 设置页·Agent 预设那一叠：花名册 / 正文 / 副本为新增 / 用于当前会话 / 设为默认 /
/// 打开目录 / 删除，外加 `settings/canOpenAgentPresetDirectory` 那枚能力门控。
/// 主干按 `code` 分支的四条业务拒绝（not-found / invalid / read-only / locked）都得能撞上。
#[test]
fn agent_preset_rpc_covers_copy_use_and_delete_loop() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let listed = kernel
        .call("agentPresets/list", json!({}))
        .expect("花名册");
    assert_eq!(
        listed["authorable"],
        json!(true),
        "false 时主干整段「新增/复制」都不画"
    );
    assert_eq!(
        listed["presets"],
        json!([
            {
                "id": "standard", "trust": "system", "isDefault": true, "name": "标准模式",
                "description": "功能完整的编码 Agent，支持文件编辑、Shell、文件与网页检索、Skills、计划、目标、子代理和工作流。",
            },
            // ⚠ 第二行**没有 `description` 键**，而不是 `"description": null`：真内核那三行
            // `...preset.description === void 0 ? {} : { description }`
            // （`dsh-agent-presets/lib/index.js:1362-1364`）+ `z.string().optional()` 的值域里没有
            // null ⇒ 内核发不出 null。老桩发的是 null，且这一格被本用例写死成了契约（pk1 §1.4-a）。
            {
                "id": "my-reviewer", "trust": "user", "isDefault": false, "name": "我的审阅",
            },
        ]),
        "trust 分内置/自定义、isDefault 画那枚标记；可选键是「有才发」的那一型"
    );
    assert!(
        listed["presets"][1].get("description").is_none(),
        "自定义档那一行必须缺 description 键：{:?}",
        listed["presets"][1]
    );

    // 能力门控：值是**裸布尔**（主干比的是 ValueKind == True），包一层对象就永远灰着。
    assert_eq!(
        kernel
            .call("settings/canOpenAgentPresetDirectory", json!({}))
            .expect("门控只读，不该失败"),
        json!(true)
    );

    let read = kernel
        .call("agentPresets/read", json!({ "agentPreset": "my-reviewer" }))
        .expect("读正文");
    assert_eq!(read["agentPreset"], json!("my-reviewer"));
    assert_eq!(read["trust"], json!("user"), "主干按 trust 决定是否给「删除/打开目录」");
    assert_eq!(read["name"], json!("我的审阅"));
    assert!(
        read["content"]
            .as_str()
            .is_some_and(|text| text.contains("我的审阅")),
        "正文得是字符串 markdown：{read}"
    );

    // 副本为新增：void；新条目必然是 trust:"user"，且沿用来源的 description。
    assert_eq!(
        kernel
            .call(
                "agentPresets/copy",
                json!({ "from": "standard", "id": "review-copy", "name": "审阅副本" })
            )
            .expect("复制为新"),
        Value::Null
    );
    let presets = preset_rows(&mut kernel);
    assert_eq!(presets.len(), 3);
    assert_eq!(presets[2]["id"], json!("review-copy"));
    assert_eq!(presets[2]["trust"], json!("user"));
    assert_eq!(presets[2]["isDefault"], json!(false));
    assert_eq!(
        presets[2]["description"], presets[0]["description"],
        "没给 name 之外的字段可继承，description 是内核从来源抄过来的"
    );
    assert!(
        presets[2]["description"].is_string() && presets[0]["description"].is_string(),
        "上面那条相等判据在「两枚都缺键」时也会成立，所以这里单独钉一次真有值：\
         内核 copyComposition（`dsh-agent-presets/lib/index.js:543`）在 `:563` 那一行\
         「来源的 description 是 `void 0` 就不给这一枚键」，来源没有就不该凭空长出来"
    );

    // 设为默认：写 agent-presets 的 default，花名册的 isDefault 跟着翻（只此一枚）。
    kernel
        .call(
            "settings/update",
            json!({ "ns": "agent-presets", "patch": { "default": "review-copy" } }),
        )
        .expect("设为默认");
    let presets = preset_rows(&mut kernel);
    assert_eq!(presets[2]["isDefault"], json!(true));
    assert_eq!(
        presets[0]["isDefault"],
        json!(false),
        "落选的内置预设要自动摘掉标记，否则页面上会同时亮两枚"
    );

    // 打开目录：走的是 settings/ 这一族，且同样吃 preset id。
    assert_eq!(
        kernel
            .call(
                "settings/openAgentPresetDirectory",
                json!({ "agentPreset": "review-copy" })
            )
            .expect("打开目录"),
        json!({ "opened": true })
    );

    // 用于当前会话：回的是**生效的预设 id 字符串**（主干 applied.GetString()），不是对象。
    assert_eq!(
        kernel
            .call(
                "agentPresets/select",
                json!({ "agentId": "s-7777", "agentPreset": "review-copy" })
            )
            .expect("用于当前会话"),
        json!("review-copy")
    );
    // 会话开跑过一轮，预设就被钉死（内核的 agentId 是会话 lookup）。
    kernel
        .call(
            "session/prompt",
            prompt_args("s-7777", "先跑一轮", "queue"),
        )
        .expect("session/prompt 应被接受");
    let error = kernel
        .call(
            "agentPresets/select",
            json!({ "agentId": "s-7777", "agentPreset": "standard" }),
        )
        .expect_err("跑过的会话改不动预设");
    assert!(error.contains("agent-preset/locked"), "{error}");

    // 删除：内置的只读，用户自建的能删，删完花名册就短一条。
    let error = kernel
        .call("agentPresets/deletePreset", json!({ "id": "standard" }))
        .expect_err("内置预设不可删");
    assert!(error.contains("agent-preset/read-only"), "{error}");
    assert_eq!(
        kernel
            .call("agentPresets/deletePreset", json!({ "id": "my-reviewer" }))
            .expect("删自定义预设"),
        Value::Null
    );
    let presets = preset_rows(&mut kernel);
    assert_eq!(presets.len(), 2);
    assert!(
        presets
            .iter()
            .all(|preset| preset["id"].as_str() != Some("my-reviewer")),
        "{presets:?}"
    );

    for (label, method, args, code) in [
        (
            "门控端点不吃参数",
            "settings/canOpenAgentPresetDirectory",
            json!({ "agentPreset": "standard" }),
            "bad_args",
        ),
        (
            "list 不吃参数",
            "agentPresets/list",
            json!({ "request": {} }),
            "bad_args",
        ),
        (
            "read 的 wire 字段叫 agentPreset",
            "agentPresets/read",
            json!({ "id": "standard" }),
            "bad_args",
        ),
        (
            "read 的 agentPreset 不能是空串",
            "agentPresets/read",
            json!({ "agentPreset": "" }),
            "bad_args",
        ),
        (
            "没这个预设：read",
            "agentPresets/read",
            json!({ "agentPreset": "nope" }),
            "agent-preset/not-found",
        ),
        (
            "没这个预设：打开目录",
            "settings/openAgentPresetDirectory",
            json!({ "agentPreset": "nope" }),
            "agent-preset/not-found",
        ),
        (
            "打开目录少了 agentPreset",
            "settings/openAgentPresetDirectory",
            json!({}),
            "bad_args",
        ),
        (
            "copy 少 name 也合法，但 from/id 必填",
            "agentPresets/copy",
            json!({ "from": "standard" }),
            "bad_args",
        ),
        (
            "copy 的 id 撞了已有预设",
            "agentPresets/copy",
            json!({ "from": "standard", "id": "standard" }),
            "agent-preset/invalid",
        ),
        (
            "copy 的来源不存在",
            "agentPresets/copy",
            json!({ "from": "nope", "id": "whatever" }),
            "agent-preset/not-found",
        ),
        (
            "select 的 wire 字段叫 agentId/agentPreset",
            "agentPresets/select",
            json!({ "sessionId": "s-8888", "agentPreset": "standard" }),
            "bad_args",
        ),
        (
            "select 不存在的预设（会话没跑过，先撞上 not-found）",
            "agentPresets/select",
            json!({ "agentId": "s-8888", "agentPreset": "nope" }),
            "agent-preset/not-found",
        ),
        (
            "deletePreset 少了 id",
            "agentPresets/deletePreset",
            json!({}),
            "bad_args",
        ),
        (
            "删掉的预设再点名就是不存在",
            "agentPresets/deletePreset",
            json!({ "id": "my-reviewer" }),
            "agent-preset/not-found",
        ),
    ] {
        let error = kernel.call(method, args).expect_err("参数或业务态该被拒");
        assert!(error.contains(code), "{label} 该回 {code}，实际 {error}");
    }
    kernel.shutdown();
}

/// `agentPresets/list` 的**回帧形状**用例：钉「桩发的形状 == 真内核的形状」这一格，
/// 外加一条反向判据钉「`items` 那枚别名只在主干选择器的 fallback 里存在，桩不许发、产品侧不许依赖」。
///
/// 真内核取证（逐字原文与行号见 `rust/tmp/pk1-report.md` §1）：
/// · 生产者 `dsh-agent-presets/lib/index.js:1355-1368` 的 `remoteExportList`（`:1170` `Remote("list")`）；
/// · 线上契约 `dsh-api-remotes/lib/client.js:4315-4325` 那份 `mode:"strict"` 的 result schema：
///   `object({ presets: array(object({id, trust, isDefault, name?, description?, broken?})), authorable })`；
/// · 内核自家 UI 的空表回落 `dsh-client-ui-agent-preset/lib/client.js:420-423` = `{presets:[], authorable:false}`；
/// · 全树反查 `items` 在 `dsh-agent-presets/` **零命中** ⇒ 真内核既不发 `items`、也不发裸数组。
///
/// 这条用例因此是**双向**的：正向按内核的键集与值型判，反向按「主干那家读不到内核形状的读者」判。
#[test]
fn agent_presets_list_wire_shape_is_the_kernel_roster_and_no_items_alias_exists() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    /// `AgentPresetRoster` 的两枚必有键。
    const ROSTER_KEYS: [&str; 2] = ["authorable", "presets"];
    /// `AgentPresetRow` 的三枚必有键（`name` 在 schema 里是 `.optional()`，故不入必有集）。
    const ROW_REQUIRED: [&str; 3] = ["id", "isDefault", "trust"];
    /// `AgentPresetRow` 允许的全部键：多一枚就是内核值域外的形状。
    const ROW_ALLOWED: [&str; 6] = ["broken", "description", "id", "isDefault", "name", "trust"];

    /// 整棵回执里不许出现 `null`：真内核的可选键走「缺键」那一型，值域内根本没有 null。
    fn first_null(value: &Value, path: &str) -> Option<String> {
        match value {
            Value::Null => Some(path.to_string()),
            Value::Array(rows) => rows
                .iter()
                .enumerate()
                .find_map(|(index, row)| first_null(row, &format!("{path}[{index}]"))),
            Value::Object(map) => {
                map.iter().find_map(|(key, child)| first_null(child, &format!("{path}.{key}")))
            }
            _ => None,
        }
    }

    let listed = kernel.call("agentPresets/list", json!({})).expect("花名册");

    // ---- 正向：顶层就是 `AgentPresetRoster`，不多不少两枚键 ----
    assert!(
        !listed.is_array(),
        "真内核从不发「整回执是数组」那一型；桩一发裸数组就等于替主干选择器补它缺的那一格：{listed}"
    );
    assert_eq!(
        sorted_keys(&listed),
        ROSTER_KEYS,
        "`AgentPresetRoster` 只有 presets+authorable；多出来的键（尤其 `items`）内核发不出"
    );
    assert!(listed["authorable"].is_boolean(), "authorable: z.boolean()，不是字符串也不是 0/1");
    // 反向判据的正身：别名一枚都不许存在，顶层与行内都不许。
    assert!(
        listed.get("items").is_none(),
        "桩发了 `items` ⇒ 主干那家只认 items 的读者会在离线轮变绿、真内核轮变红，这是自测造假绿"
    );

    let rows = listed["presets"].as_array().expect("presets 是数组").clone();
    assert_eq!(rows.len(), 2, "现测初值条数（1 内置 + 1 自定义）");
    let mut ids: Vec<&str> = Vec::new();
    for row in &rows {
        let keys = sorted_keys(row);
        for key in &keys {
            assert!(
                ROW_ALLOWED.contains(key),
                "{key} 不在内核 `AgentPresetRow` 的键集（允许：{}）里：{row}",
                ROW_ALLOWED.join(" ")
            );
        }
        for required in ROW_REQUIRED {
            assert!(keys.contains(&required), "缺了内核 schema 的必有键 {required}：{row}");
        }
        assert!(
            row.get("items").is_none(),
            "`items` 只活在主干选择器那条 fallback 里，行内同样不许出现：{row}"
        );
        assert!(row["id"].is_string(), "id: z.string()");
        let id = row["id"].as_str().expect("id 是字符串");
        assert!(!id.is_empty(), "id 是目录名，内核 `PRESET_ID` 拒空串");
        assert!(!ids.contains(&id), "discoverPresets 是 first-root-wins 的 Map 去重，id 不该重复");
        ids.push(id);
        assert!(
            row["trust"].as_str().is_some_and(|trust| matches!(trust, "system" | "user")),
            "trust: z.union([literal(\"system\"), literal(\"user\")]) ⇒ {}",
            row["trust"]
        );
        assert!(row["isDefault"].is_boolean(), "isDefault: z.boolean()");
        for optional in ["name", "description", "broken"] {
            if let Some(value) = row.get(optional) {
                assert!(value.is_string(), "{optional} 有则必是字符串，不能是 null：{value}");
            }
        }
    }
    assert_eq!(
        rows.iter().filter(|row| row["isDefault"] == json!(true)).count(),
        1,
        "内核的 isDefault 是 `preset.id === defaultId`（单一默认档），全表至多一枚 true"
    );
    assert!(first_null(&listed, "roster").is_none(), "可选键必须缺键而不是 null");

    // ---- 反向：主干「选择器那一家读者」的读法喂这条真回帧，一行都拿不到 ----
    // `MW:4848-4851` 只认（整回执是数组 | `items`）。真内核两型都不发 ⇒ 这一段在**真内核下同样是死代码**，
    // 自定义档不出不是假内核的偏差，是主干自身的缺陷。桩若在这里替内核补 `items`，本判据立刻红。
    let rows_for_selector_reader: Vec<Value> = match listed.as_array() {
        Some(rows) => rows.clone(),
        None => listed.get("items").and_then(Value::as_array).cloned().unwrap_or_default(),
    };
    assert!(
        rows_for_selector_reader.is_empty(),
        "选择器那家读者本该读不到东西（离线轮与真内核轮同值）：{rows_for_selector_reader:?}"
    );
    // 而设置页那一家读者（`MW:13069/13096`）读同一发，条数就是内核的条数 ⇒ 两家的分叉在**读者**不在桩。
    assert_eq!(
        listed["presets"].as_array().expect("presets").len(),
        2,
        "同一条回执，设置页读者拿 2 行、选择器读者拿 0 行"
    );

    // ---- 形状判据得在花名册的整个生命周期上成立，不只是初值 ----
    kernel
        .call("agentPresets/copy", json!({ "from": "my-reviewer", "id": "pk1-shape-copy" }))
        .expect("复制一份无 description 的自定义档");
    let after_copy = preset_rows(&mut kernel);
    assert_eq!(after_copy.len(), 3, "现测：copy 之后 3 条");
    let copied = after_copy.iter().find(|row| row["id"] == json!("pk1-shape-copy")).expect("新行");
    assert_eq!(copied["trust"], json!("user"), "复制出来必然落在 user 根");
    assert!(
        copied.get("description").is_none(),
        "来源 `my-reviewer` 没有 description ⇒ 内核 `copyComposition`（`index.js:543`）在 `:563` \
         那句展开里同样不给这一枚键：{copied}"
    );
    assert_eq!(sorted_keys(copied), ["id", "isDefault", "name", "trust"]);

    kernel
        .call(
            "settings/update",
            json!({ "ns": "agent-presets", "patch": { "default": "pk1-shape-copy" } }),
        )
        .expect("换默认档");
    let after_default = preset_rows(&mut kernel);
    assert_eq!(
        after_default.iter().filter(|row| row["isDefault"] == json!(true)).count(),
        1,
        "换默认档后仍只能有一枚 true，否则形状用例与设置页的标记都会双亮"
    );

    kernel
        .call("agentPresets/deletePreset", json!({ "id": "pk1-shape-copy" }))
        .expect("删掉派生档");
    let after_delete = preset_rows(&mut kernel);
    assert_eq!(after_delete.len(), 2, "现测：删完回到 2 条");
    for row in &after_delete {
        let keys = sorted_keys(row);
        for key in &keys {
            assert!(ROW_ALLOWED.contains(key), "删完之后冒出内核键集外的 {key}：{row}");
        }
        assert!(
            keys.contains(&"id") && keys.contains(&"trust") && keys.contains(&"isDefault"),
            "删完不该把必有键也删掉：{row}"
        );
    }
    assert!(first_null(&json!({ "presets": after_delete }), "roster").is_none());

    kernel.shutdown();
}

/// 反馈族第二层 ok 的拆解：`Kernel::call` 只剥外层信封（`{result:{ok,value}}`），剩下的
/// `value` 本身又是内核方法的返回体 `{ok:true,value}` / `{ok:false,error}`。
/// 分叉的 `feedback_call` 就是这个动作的镜像，测试用它才和壳看到的完全一致。
fn feedback_value(body: Value) -> Value {
    assert_eq!(
        body["ok"],
        json!(true),
        "内层 ok 必须是 true；外层已经是 true 了还返到这里，说明业务失败被当成了成功"
    );
    body.get("value").cloned().unwrap_or(Value::Null)
}

fn feedback_error(body: Value) -> Value {
    assert_eq!(
        body["ok"],
        json!(false),
        "这条断言不能少：业务失败必须走内层 ok:false，而不是外层信封失败"
    );
    body.get("error").cloned().unwrap_or(Value::Null)
}

/// 一个对象排序后的键名表：用来把「内核只发这几个键」钉死（多一个自创键就红）。
fn object_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("元素必须是对象")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// 跑一轮提示并从 `session/page` 的 journal 里取答案的 `message.id`。
/// 两件事都是反馈族的硬前提：`messageFeedback` 只认日志里真实存在的那条 `assistant/message`
/// （假内核按 journal 判 target-not-found），而分叉的唯一门槛也是这个 id。
fn prompt_one_turn(kernel: &mut Kernel, session: &str, text: &str) -> String {
    kernel
        .call("session/prompt", prompt_args(session, text, "queue"))
        .expect("提示该被收下（假内核没有 follow 流也照记 journal）");
    let page = kernel
        .call("session/page", page_args(session, None, None))
        .expect("journal 回填该成功");
    answer_of(&page["records"])
}

/// 从 `session/page` 的 records 里取答案的 `message.id`。
/// #78 之后种子会话一开工就有三轮历史 ⇒ 必须取**最后**一条（= 刚 prompt 那轮的答案），
/// 取第一条会捡到种子轮的 `msg-1-*`，分叉锚点就错位。
fn answer_of(records: &Value) -> String {
    records
        .as_array()
        .expect("records 是数组")
        .iter()
        .rev()
        .find(|record| record["event"]["type"].as_str() == Some("assistant/message"))
        .and_then(|record| record["event"]["data"]["message"]["id"].as_str())
        .expect("答案气泡得有 message.id，否则主干/分叉都不给反馈入口")
        .to_string()
}

/// 答案气泡自己的事件 seq（= 分叉 `Msg::Branch(seq)` 递给 `session/fork` 的 `atSeq`）。
fn answer_seq(kernel: &mut Kernel, session: &str) -> i64 {
    let message_id = kernel
        .call("session/page", page_args(session, None, None))
        .and_then(|page| {
            page["records"]
                .as_array()
                .and_then(|records| {
                    records.iter().rev().find_map(|record| {
                        (record["event"]["type"].as_str() == Some("assistant/message"))
                            .then(|| {
                                record["event"]["data"]["message"]["id"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string()
                            })
                    })
                })
                .map(Ok)
                .unwrap_or_else(|| Err("没有答案".to_string()))
        })
        .expect("journal 里该有答案");
    // 假内核的 message.id 形如 `msg-<turn>-<seq>`，最后一段就是答案自己那条事件的 seq。
    message_id
        .rsplit('-')
        .next()
        .and_then(|tail| tail.parse::<i64>().ok())
        .unwrap_or_else(|| panic!("{message_id} 的 seq 段解不出来"))
}

/// 一条 `session/list` 记录里读出的字段（走 `Kernel::list_sessions`，与分叉同一份解析）。
fn row_of(rows: &[blade2_rs::kernel::SessionInfo], id: &str) -> blade2_rs::kernel::SessionInfo {
    rows.iter()
        .find(|row| row.id == id)
        .unwrap_or_else(|| panic!("台账里该有 {id}"))
        .clone()
}

#[test]
fn session_fork_rename_then_list_closes_the_branch_loop() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // 先跑一轮：真内核只允许从**已完成轮**分叉（journal 里没有 turn/end 就 fork-unavailable）。
    let message_id = prompt_one_turn(&mut kernel, "s-1001", "分支一下");
    // #78 之后 s-1001 起手就带三轮种子历史 ⇒ 这一发 prompt 是第 4 轮，答案仍是轮内第 27 条
    // 事件（该轮首 seq 118 + 26）。id 的格式不变：`msg-<turn>-<seq>`，atSeq 靠尾段算。
    assert_eq!(message_id, "msg-4-144", "答案 id 的格式是 msg-<turn>-<seq>，atSeq 靠它算");
    let at_seq = answer_seq(&mut kernel, "s-1001");
    let source_page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("源会话的回填");

    let before = kernel.list_sessions().expect("fork 前的台账");
    assert_eq!(before.len(), 3, "只有三行种子台账");

    let value = kernel
        .call(
            "session/fork",
            json!({ "request": { "sessionId": "s-1001", "atSeq": at_seq } }),
        )
        .expect("session/fork 该成功");
    // 逐字键名：真内核的 SessionForkValue 只有 `sessionId`（子会话 id），既不是 childSessionId
    // 也不是 id —— 分叉 `fork_at` 读的就是 `value["sessionId"]`，读错就是「没回 sessionId」。
    assert_eq!(object_keys(&value), ["sessionId"]);
    let child = value["sessionId"].as_str().expect("子会话 id 是字符串");
    assert!(
        child.starts_with("session-"),
        "真内核的子会话 id 是 `session-<uuid>`，前缀不许变（{child}）"
    );
    assert_ne!(child, "s-1001");

    // 新会话必须出现在后续 session/list 里：主干 fork 完先 RefreshSessions 再改名，
    // 左栏那一行就是从这张表长出来的；桩里少这一步，分叉的分支流程在自测里永远走不通。
    let rows = kernel.list_sessions().expect("fork 后的台账");
    assert_eq!(rows.len(), 4, "fork 出来的子会话要在台账里追加一行");
    let child_row = row_of(&rows, child);
    assert_eq!(
        child_row.title, "重构登录流程",
        "子会话继承源会话的标题投影（真内核是整段日志复制过去的）"
    );
    assert_eq!(child_row.cwd, "E:\\demo\\alpha", "cwd 照 header 继承");
    assert_eq!(child_row.parent.as_deref(), Some("s-1001"), "parentSessionId 指向源会话");
    assert!(!child_row.blank, "带着整轮历史的子会话不是空白会话");
    assert!(
        !child_row.is_subagent(),
        "分叉出来的子会话 origin 缺席，别和子代理（origin=subagent）混为一谈"
    );

    // 改名：分叉的分支流程拿源标题做递增（`标题 (1)`），再发 session/rename。
    let renamed = kernel
        .call(
            "session/rename",
            json!({ "request": { "sessionId": child, "title": "重构登录流程 (1)" } }),
        )
        .expect("session/rename 该成功");
    // SessionRenameValue 是 `{title, seq}`：title 是归一化后的值、seq 是那条事件的位置。
    assert_eq!(object_keys(&renamed), ["seq", "title"]);
    assert_eq!(renamed["title"], json!("重构登录流程 (1)"));
    assert!(renamed["seq"].as_i64().is_some_and(|seq| seq > 0), "{renamed}");

    // 标题要用改名后的值出现在后续 list 里（自测那条 `(1)` 结尾的行就靠这一步）。
    let rows = kernel.list_sessions().expect("rename 后的台账");
    assert_eq!(rows.len(), 4, "改名不新增行");
    assert_eq!(row_of(&rows, child).title, "重构登录流程 (1)");
    assert_eq!(
        row_of(&rows, "s-1001").title, "重构登录流程",
        "改子会话不许顺手把源会话的标题也带跑"
    );

    // 全角 `（N）` 只是壳侧的正则口径，内核存的是原样字符串：再改一次、再分一层都得以最新值继承。
    kernel
        .call(
            "session/rename",
            json!({ "request": { "sessionId": child, "title": "重构登录流程（2）" } }),
        )
        .expect("全角括号标题同样能改");
    let grand = kernel
        .call("session/fork", json!({ "request": { "sessionId": child } }))
        .expect("不带 atSeq 也能分叉（取最后一个已完成轮）");
    let grand_id = grand["sessionId"].as_str().expect("第二个子会话 id");
    assert_ne!(grand_id, child);
    let rows = kernel.list_sessions().expect("再 fork 后的台账");
    assert_eq!(rows.len(), 5);
    assert_eq!(
        row_of(&rows, grand_id).title, "重构登录流程（2）",
        "继承的是**当前**标题（rename 覆写表优先），不是初始种子值"
    );
    assert_eq!(row_of(&rows, grand_id).parent.as_deref(), Some(child));

    // 子会话的 journal = 源会话到边界为止的前缀：切过去 `session/page` 才有内容可回填，
    // 否则分支后的画面是一片白（真内核靠 seed 继承，桩也得给）。
    let child_page = kernel
        .call("session/page", page_args(child, None, None))
        .expect("子会话的回填");
    assert_eq!(
        child_page["records"], source_page["records"],
        "atSeq 落在本轮答案上 ⇒ 整轮都该被带进子会话"
    );
    let last = child_page["records"]
        .as_array()
        .expect("records")
        .last()
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(last["event"]["type"], json!("turn/end"), "前缀必须停在轮尾");

    // 轮次游标也随日志继承：s-1001 分叉前已经是第 4 轮（三轮种子历史 + 刚问的那轮），
    // 子会话下一轮因此是 turn 5，答案 id 不能退回 msg-1-*（气泡 key 会撞）。
    prompt_one_turn(&mut kernel, child, "分支后接着问");
    let records = kernel
        .call("session/page", page_args(child, None, None))
        .expect("子会话第二轮的回填")["records"]
        .clone();
    let answers: Vec<String> = records
        .as_array()
        .expect("records")
        .iter()
        .filter(|record| record["event"]["type"].as_str() == Some("assistant/message"))
        .map(|record| {
            record["event"]["data"]["message"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(
        answers.len(),
        5,
        "继承的前缀含三轮种子历史 + 分叉那条（第 4 轮），分支后又问出一轮 ⇒ 五条答案，\
         多一条就是重复回填：{answers:?}"
    );
    assert_eq!(
        answers[..4],
        ["msg-1-29", "msg-2-67", "msg-3-114", message_id.as_str()],
        "继承下来的四条答案原样在子会话里，末条就是分叉锚点那条"
    );
    assert!(
        answers[4].starts_with("msg-5-"),
        "子会话的下一轮 turn 必须续号，实际 {}",
        answers[4]
    );

    kernel.shutdown();
}

#[test]
fn session_fork_and_rename_reject_off_shape_arguments() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    prompt_one_turn(&mut kernel, "s-1001", "先攒一轮可分叉的历史");

    for (label, args) in [
        (
            "少了 request 这根 wire 字段（扁平传）",
            json!({ "sessionId": "s-1001" }),
        ),
        (
            "request 不是对象",
            json!({ "request": "s-1001" }),
        ),
        (
            "request.sessionId 是空串",
            json!({ "request": { "sessionId": "" } }),
        ),
        (
            "request 里多一个未知键",
            json!({ "request": { "sessionId": "s-1001", "atSequence": 3 } }),
        ),
    ] {
        let error = kernel
            .call("session/fork", args)
            .expect_err("非法参数必须被拒");
        assert!(error.contains("bad_args"), "{label}：{error}");
    }

    // atSeq 出现就必须是非负整数（真码 gateway/bad-request，落进分叉的「分叉失败: …」文案）。
    for (label, at_seq) in [("负数", json!(-1)), ("字符串", json!("12")), ("小数", json!(1.5))] {
        let error = kernel
            .call("session/fork", json!({ "request": { "sessionId": "s-1001", "atSeq": at_seq } }))
            .expect_err("非法 atSeq 必须被拒");
        assert!(error.contains("gateway/bad-request"), "{label}：{error}");
    }

    let error = kernel
        .call("session/fork", json!({ "request": { "sessionId": "nope-404" } }))
        .expect_err("源会话不存在");
    assert!(error.contains("session/not-found"), "{error}");
    // 存在的会话≠可分叉的会话：s-1002 一行日志都没有，真内核回 fork-unavailable。
    let error = kernel
        .call("session/fork", json!({ "request": { "sessionId": "s-1002" } }))
        .expect_err("没有已完成轮的会话不能分叉");
    assert!(error.contains("session/fork-unavailable"), "{error}");
    assert_eq!(
        kernel.list_sessions().expect("上面几次失败不该改台账").len(),
        3,
        "被拒的 fork 一条都不能留下子会话，否则左栏冒出点不动的空行"
    );

    for (label, args) in [
        (
            "少了 title",
            json!({ "request": { "sessionId": "s-1001" } }),
        ),
        (
            "title 不是字符串",
            json!({ "request": { "sessionId": "s-1001", "title": 7 } }),
        ),
        (
            "多一个未知键",
            json!({ "request": { "sessionId": "s-1001", "title": "x", "name": "x" } }),
        ),
    ] {
        let error = kernel
            .call("session/rename", args)
            .expect_err("非法参数必须被拒");
        assert!(error.contains("bad_args"), "{label}：{error}");
    }
    // 归一化后没有可见字符 → session/title-invalid（主干整段 catch、静默，分叉同样只丢标题）。
    let error = kernel
        .call(
            "session/rename",
            json!({ "request": { "sessionId": "s-1001", "title": "   \t " } }),
        )
        .expect_err("空白标题必须被拒");
    assert!(error.contains("session/title-invalid"), "{error}");
    let error = kernel
        .call(
            "session/rename",
            json!({ "request": { "sessionId": "nope-404", "title": "随便" } }),
        )
        .expect_err("给不存在的会话改名");
    assert!(error.contains("session/not-found"), "{error}");

    kernel.shutdown();
}

/// `messageFeedback/put|list|delete` 的正路：写入 → 读回 → 撤销 → 读空，
/// 外加「双层 ok」这条最容易踩的形状（外层信封恒 ok:true，业务成败在内层）。
#[test]
fn message_feedback_put_list_delete_round_trip() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let message = prompt_one_turn(&mut kernel, "s-1001", "这条要点赞");

    let body = kernel
        .call(
            "messageFeedback/put",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message,
                "rating": "positive", "ifVersion": null,
            } }),
        )
        // 这一条不能改成 expect_err：业务成功时外层与内层都是 ok:true，
        // 而**业务失败时外层同样是 ok:true**（`call` 返 Ok，得自己读内层），
        // 所以「call 成功」从来不代表反馈写进去了。
        .expect("传输层该成功");
    let item = feedback_value(body);
    assert_eq!(
        object_keys(&item),
        ["createdAt", "messageId", "rating", "updatedAt", "version"],
        "MessageFeedbackItem 只有这几个键；note/category 是「有才有」，多余键一律不该冒出来"
    );
    assert_eq!(item["messageId"], json!(message));
    assert_eq!(item["rating"], json!("positive"));
    assert!(item.get("note").is_none(), "没发说明就得整个键缺席（内核是 optional 不是 nullable）");
    assert!(item.get("category").is_none());
    assert!(
        item["version"].as_str().is_some_and(|version| !version.is_empty()),
        "version 是内核生成的乐观并发凭据，壳只回传不许自造：{item}"
    );
    assert!(item["createdAt"].as_i64().is_some_and(|v| v > 0));
    assert!(item["updatedAt"].as_i64().is_some_and(|v| v >= item["createdAt"].as_i64().unwrap()));
    let version = item["version"].as_str().expect("version 是字符串").to_string();

    // list 拉回同一条（主干只在切会话流程末尾拉这一次，不是轮询）。
    let body = kernel
        .call(
            "messageFeedback/list",
            json!({ "request": { "sessionId": "s-1001" } }),
        )
        .expect("list 传输层该成功");
    let listed = feedback_value(body);
    // 逐字：ListValue 是 `{items:[…]}`，分叉读的就是 `value["items"]`。
    assert_eq!(object_keys(&listed), ["items"]);
    let items = listed["items"].as_array().expect("items 是数组");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0], item, "list 回读必须与 put 回的条目逐字相同");

    // 带说明 + 分类的再写：put 是**整体替换**，所以主干改判时会把旧 note 一起带上。
    let body = kernel
        .call(
            "messageFeedback/put",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message,
                "rating": "negative", "note": "结尾跑题了", "category": "task-result",
                "ifVersion": version,
            } }),
        )
        .expect("带版本改写该成功");
    let updated = feedback_value(body);
    assert_eq!(updated["rating"], json!("negative"));
    assert_eq!(updated["note"], json!("结尾跑题了"));
    assert_eq!(updated["category"], json!("task-result"));
    assert_ne!(
        updated["version"], item["version"],
        "真改了内容就得换 version（内核每次落事件都 randomUUID），否则下一次并发比对形同虚设"
    );
    assert_eq!(
        updated["createdAt"], item["createdAt"],
        "createdAt 锚在首次落库，updatedAt 才跟着走（内核 itemSchema 还要 updatedAt >= createdAt）"
    );
    let version = updated["version"].as_str().expect("新版本").to_string();

    // 内容完全一致的重复 put 是 no-op：内核保留原 version、一条事件都不追加。
    let body = kernel
        .call(
            "messageFeedback/put",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message,
                "rating": "negative", "note": "结尾跑题了", "category": "task-result",
                "ifVersion": version,
            } }),
        )
        .expect("重复写同样的内容该成功");
    assert_eq!(
        feedback_value(body)["version"],
        json!(version),
        "no-op 不许换版本号，否则连点两次必把下一次自己顶成 version-conflict"
    );

    // delete：成功回的是 `{absent:true}`（不是被删的条目），分叉据此本地撤销。
    let body = kernel
        .call(
            "messageFeedback/delete",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message, "ifVersion": version,
            } }),
        )
        .expect("delete 传输层该成功");
    assert_eq!(feedback_value(body), json!({ "absent": true }), "delete 的 value 逐字是 {{absent:true}}");
    let body = kernel
        .call(
            "messageFeedback/list",
            json!({ "request": { "sessionId": "s-1001" } }),
        )
        .expect("list 该成功");
    assert_eq!(
        feedback_value(body)["items"].as_array().expect("items").len(),
        0,
        "删完再 list 必须空表，否则切回会话会看到幽灵赞踩"
    );

    // 幂等：内核侧本来没有这条，delete 同样成功（主干据此本地撤销且不碰 Status 文案）。
    let body = kernel
        .call(
            "messageFeedback/delete",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message, "ifVersion": version,
            } }),
        )
        .expect("重复 delete 该幂等成功");
    assert_eq!(feedback_value(body), json!({ "absent": true }));

    kernel.shutdown();
}

/// `messageFeedback` 的失败面：每条都得是**内层 ok:false + 内核原样错误体**，
/// 分叉才有分支可测（version-conflict 采纳 `current` 重试、其余查 `FeedbackErrorText` 码表）。
#[test]
fn message_feedback_business_failures_carry_kernel_error_bodies() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let message = prompt_one_turn(&mut kernel, "s-1001", "并发冲突演习");
    let put = |kernel: &mut Kernel, if_version: Value| {
        kernel.call(
            "messageFeedback/put",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message,
                "rating": "positive", "ifVersion": if_version,
            } }),
        )
    };

    // 没观察过版本（ifVersion:null）而内核已有条目 ⇒ 冲突，且 `current` 是内核权威值。
    let stored = feedback_value(put(&mut kernel, Value::Null).expect("首次写入该成功"));
    let body = put(&mut kernel, json!("stale-version")).expect(
        "传输层失败与业务失败是两回事：版本冲突必须照 200 + 内层 ok:false 回来，不能升成 RPC 异常",
    );
    let error = feedback_error(body);
    assert_eq!(error["code"], json!("version-conflict"));
    assert!(
        error.get("message").is_none(),
        "内核的错误体只有 code + 定位键；桩多造一个 message 就会掩盖分叉查表兜底的缺口"
    );
    // 分叉 `AdoptFeedbackConflict` 采纳的就是这一坨（为 null 即本地删除）。
    assert_eq!(error["current"], stored, "current 得是内核那条 item，分叉据此重试一次");
    let retry_version = error["current"]["version"]
        .as_str()
        .expect("current.version")
        .to_string();
    assert_eq!(
        feedback_value(put(&mut kernel, json!(retry_version)).expect("采纳后重试该成功"))["version"],
        stored["version"],
        "同内容重试命中内核的 no-op 分支 ⇒ 版本不变"
    );

    // 内核侧已被撤销 ⇒ current 是 null，分叉据此只做本地撤销、不报错。
    // 先把那条真删掉（delete 只认字符串版本），再拿悬空版本去写才测得出这个分支。
    let body = kernel
        .call(
            "messageFeedback/delete",
            json!({ "request": {
                "sessionId": "s-1001", "messageId": message, "ifVersion": retry_version,
            } }),
        )
        .expect("delete 该成功");
    assert_eq!(feedback_value(body), json!({ "absent": true }));
    let body = put(&mut kernel, json!("never-existed")).expect("悬空版本必须冲突");
    let error = feedback_error(body);
    assert_eq!(error["code"], json!("version-conflict"));
    assert!(
        error["current"].is_null(),
        "current:null = 内核侧本就没有这条，分叉据此判本地撤销而不是报错：{error}"
    );

    // 会话不存在 / 消息不在日志里：两条定位键都齐，主干据此分别出两种文案。
    let error = feedback_error(
        kernel
            .call(
                "messageFeedback/list",
                json!({ "request": { "sessionId": "nope-404" } }),
            )
            .expect("未知会话也是内层失败，不是信封失败"),
    );
    assert_eq!(error, json!({ "code": "session-not-found", "sessionId": "nope-404" }));
    let error = feedback_error(
        kernel
            .call(
                "messageFeedback/put",
                json!({ "request": {
                    "sessionId": "s-1001", "messageId": "msg-9-999",
                    "rating": "positive", "ifVersion": null,
                } }),
            )
            .expect("目标消息不存在也是内层失败"),
    );
    assert_eq!(
        error,
        json!({ "code": "target-not-found", "sessionId": "s-1001", "messageId": "msg-9-999" }),
        "「该消息不在本会话日志里（子代理或已裁剪）」整句都靠这两个键定位"
    );

    // note 那两道关抢在会话/消息存在性之前（内核 `put()` 先 resolveNote）：
    // 用不存在的会话 + 空白说明才测得出这个先后。
    let error = feedback_error(
        kernel
            .call(
                "messageFeedback/put",
                json!({ "request": {
                    "sessionId": "nope-404", "messageId": "msg-1-1",
                    "rating": "positive", "note": "   ", "ifVersion": null,
                } }),
            )
            .expect("空白说明该被内层拒"),
    );
    assert_eq!(error, json!({ "code": "note-blank" }), "note-blank 必须抢在 session-not-found 之前");
    // 上限是内核配置 maxNoteBytes: 8192（dsh-web-app/cordis.patch.yml:56）。
    let long_note = "a".repeat(8193);
    let error = feedback_error(
        kernel
            .call(
                "messageFeedback/put",
                json!({ "request": {
                    "sessionId": "s-1001", "messageId": message,
                    "rating": "positive", "note": long_note, "ifVersion": null,
                } }),
            )
            .expect("超长说明该被内层拒"),
    );
    assert_eq!(
        error,
        json!({ "code": "note-too-large", "maxBytes": 8192, "actualBytes": 8193 }),
        "主干 FeedbackErrorText 那句「上限 N 字节」读的就是这里的 maxBytes"
    );

    // wire 层的把关（真码 gateway/input-invalid，本套桩统一报 bad_args）：
    // 这些都得是**外层失败**，好让分叉落进 `FeedbackFailure::Transport` 而不是查码表。
    for (label, method, args) in [
        (
            "put 少了 request 包裹",
            "messageFeedback/put",
            json!({ "sessionId": "s-1001", "messageId": message, "rating": "positive", "ifVersion": null }),
        ),
        (
            "put 少了 ifVersion 键",
            "messageFeedback/put",
            json!({ "request": { "sessionId": "s-1001", "messageId": message, "rating": "positive" } }),
        ),
        (
            "put 的 rating 不在枚举里",
            "messageFeedback/put",
            json!({ "request": { "sessionId": "s-1001", "messageId": message, "rating": "meh", "ifVersion": null } }),
        ),
        (
            "put 的 note 给成 null（内核是 optional 不是 nullable）",
            "messageFeedback/put",
            json!({ "request": { "sessionId": "s-1001", "messageId": message, "rating": "positive", "note": null, "ifVersion": null } }),
        ),
        (
            "put 的 category 不在 7 个字面量里",
            "messageFeedback/put",
            json!({ "request": { "sessionId": "s-1001", "messageId": message, "rating": "positive", "category": "vibes", "ifVersion": null } }),
        ),
        (
            // 契约陷阱：delete 的 ifVersion 是 required string，传 null 会被内核拒 ⇒
            // 本地没观察到版本时主干必须先 list 对齐，而不是发 null 删。
            "delete 的 ifVersion 给成 null",
            "messageFeedback/delete",
            json!({ "request": { "sessionId": "s-1001", "messageId": message, "ifVersion": null } }),
        ),
        (
            "delete 少了 ifVersion",
            "messageFeedback/delete",
            json!({ "request": { "sessionId": "s-1001", "messageId": message } }),
        ),
        (
            "list 少了 request",
            "messageFeedback/list",
            json!({ "sessionId": "s-1001" }),
        ),
    ] {
        let error = kernel.call(method, args).expect_err("非法参数必须被拒");
        assert!(error.contains("bad_args"), "{label}：{error}");
    }

    kernel.shutdown();
}

/// 技能面板与 `/` 菜单读的 `skills/list`。注意：设置页的技能**增删改没有 RPC**
/// （主干 `MainWindow.Skills.cs` 走本机目录 + SKILL.md frontmatter），这里只有清单一条。
#[test]
fn skills_list_returns_the_catalog_the_menu_renders() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let listed = kernel
        .call(
            "skills/list",
            json!({ "request": { "sessionId": "s-1001" } }),
        )
        .expect("技能清单");
    let skills = listed["skills"]
        .as_array()
        .expect("缺 skills 或不是数组，主干当整次失败弹错误框");
    assert_eq!(skills.len(), 2, "空清单面板只会写「没有可用技能」，自测看不出错");

    // 逐字的键名：name/description/modelInvocable 必有，whenToUse 是「有才有」的可选键。
    // 闭包签名没法把返回引用的生命周期绑到入参上（E0621），所以写成嵌套 fn。
    fn keys_of(skill: &Value) -> Vec<&str> {
        let mut keys: Vec<&str> = skill
            .as_object()
            .expect("元素是对象")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        keys
    }
    assert_eq!(
        keys_of(&skills[0]),
        ["description", "modelInvocable", "name", "whenToUse"]
    );
    assert_eq!(keys_of(&skills[1]), ["description", "modelInvocable", "name"]);
    assert_eq!(skills[0]["modelInvocable"], json!(true));
    assert_eq!(
        skills[1]["modelInvocable"],
        json!(false),
        "frontmatter 里 disable-model-invocation 的条目就靠这个键从模型侧摘掉"
    );

    // 名字必须是小写 kebab：带空格或斜杠的条目会被 ShowCapabilitySkillsAsync 静默跳过。
    for skill in skills {
        let name = skill["name"].as_str().expect("name 是字符串");
        assert!(
            !name.is_empty() && !name.starts_with('/') && !name.chars().any(|c| c.is_whitespace()),
            "{name} 会被主干静默丢掉"
        );
        assert!(
            !skill["description"].as_str().unwrap_or_default().is_empty(),
            "{name} 的说明是面板按钮的第二行"
        );
    }

    for (label, args) in [
        (
            "少了 request 这根 wire 字段",
            json!({ "sessionId": "s-1001" }),
        ),
        (
            "request 不是对象",
            json!({ "request": "s-1001" }),
        ),
        (
            "request.sessionId 是空串",
            json!({ "request": { "sessionId": "" } }),
        ),
        (
            "多给一个未知顶层键",
            json!({ "request": { "sessionId": "s-1001" }, "cwd": "E:\\demo" }),
        ),
    ] {
        let error = kernel
            .call("skills/list", args)
            .unwrap_err();
        assert!(
            error.contains("bad_args"),
            "{label} 该被 wire 把关拦下，实际 {error}"
        );
    }
    kernel.shutdown();
}

// ================================================================ 目标 A：流式帧的真实节流
// 以前假内核零延迟推帧：一轮在几十毫秒内灌完，UI 侧「流式中」那一帧压根不存在，
// qa3 的 `qa3-3-stream.png` 与 `qa3-4-turn.png` 才会一模一样。现在 `prompt` 把投递交给
// pacer 线程排队推，帧间睡 `--pace=<ms>`（或 `FAKE_DSH_PACE_MS`，默认 200ms，0=关）。

/// 关掉节奏（`fake_launch` 给所有既有用例传的 `--pace=0`）时，一轮必须一次到位：
/// 那批用例的收帧预算全按这个写死。谁把默认值改成非零，这条会立刻红。
#[test]
fn pacing_off_delivers_the_whole_turn_in_one_burst() {
    let (mut kernel, mut mux, follow) = open_stream_with("session/follow", follow_args("s-9101"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");

    let started = Instant::now();
    kernel
        .call("session/prompt", prompt_args("s-9101", "关节奏", "queue"))
        .expect("session/prompt 应被接受");
    let frames = collect_within(&mut mux, &follow, TURN_FRAMES, Duration::from_millis(900));
    assert_eq!(
        frames.len(),
        TURN_FRAMES,
        "关节奏时整轮 {TURN_FRAMES} 帧该在 900ms 内一次到位（实际 {:?}）",
        started.elapsed()
    );
    assert_eq!(
        item_values(&frames, &follow)
            .iter()
            .map(tag)
            .collect::<Vec<_>>(),
        turn_tags(),
        "关节奏的帧序就是这轮的基准形状"
    );
    kernel.shutdown();
}

/// 开着节奏（`MID_PACE_MS`，见其注释的一帧一批条件）：① RPC 不被推帧拖住、② 头几帧真花了
/// 时间、③ 半个轮次的预算里整轮**没**推完（= 中途态存在，UI 抢得到拍）、④ 补齐之后帧序与
/// 关节奏时一字不差（节流只许改「什么时候到」，不许改「到什么」）。
#[test]
fn paced_turn_leaves_a_midway_window_for_ui_snapshots() {
    let (mut kernel, mut mux, follow) =
        open_stream_paced("session/follow", follow_args("s-9102"), MID_PACE_MS);
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "快照是现成的，不该被节奏拖住");

    let started = Instant::now();
    kernel
        .call("session/prompt", prompt_args("s-9102", "节流我", "queue"))
        .expect("session/prompt 应被接受");
    assert!(
        started.elapsed() < Duration::from_millis(MID_PACE_MS * 3),
        "推帧在后台线程，RPC 不该等整轮: {:?}",
        started.elapsed()
    );

    let head = collect(&mut mux, &follow, 6);
    assert_eq!(
        head.len(),
        6,
        "只想要六帧就该只到六帧（帧间隔大于 READ_TICK 才谈得上中途）"
    );
    assert_eq!(
        item_values(&head, &follow).iter().map(tag).collect::<Vec<_>>(),
        turn_tags()[..6],
        "头六帧的形状与关节奏时相同，节流不许改内容"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(MID_PACE_MS * 5 - 20),
        "六帧之间该睡出五个间隔，实际 {:?}",
        started.elapsed()
    );

    // 中途：只给四个间隔的预算，剩下的 30 帧必然还没推完 —— 这段窗口就是 UI 的抢拍时机。
    let missing = TURN_FRAMES - 6;
    let mid_budget = Duration::from_millis(MID_PACE_MS * 4);
    let partial = collect_within(&mut mux, &follow, missing, mid_budget);
    assert!(!partial.is_empty(), "帧该还在陆续到，而不是掐然不动");
    assert!(
        partial.len() < missing,
        "节流没生效：{mid_budget:?} 里就补完了剩下的 {missing} 帧，UI 无从抢拍"
    );

    let mut frames = item_values(&head, &follow);
    frames.extend(item_values(&partial, &follow));
    let want = missing - partial.len();
    let tail = collect_within(&mut mux, &follow, want, DRAIN_BUDGET);
    assert_eq!(tail.len(), want, "剩下的帧最终都得补齐");
    frames.extend(item_values(&tail, &follow));
    assert_eq!(frames.len(), TURN_FRAMES, "整轮帧数不因节流而变");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "节流过的帧序必须与关节奏时逐字相同"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(MID_PACE_MS * TURN_FRAMES as u64 / 2),
        "整轮被拖长的工夫才是 UI 的抢拍窗口，实际 {:?}",
        started.elapsed()
    );
    kernel.shutdown();
}

/// 轮尾那条 running=false 只能在最后一帧之后到：分叉的「思考中」占位、左栏那个点全靠它撑着，
/// 提前灭掉就等于把整段流式窗口从 UI 上抹掉（qa3 的 `pending-placeholder-gone` 会误判）。
/// 这里用 40ms 的便宜节奏：帧序与状态帧同出一条 socket、同一个写线程，FIFO 即投递序，
/// 「false 排在 36 帧之后」由批内顺序 + 总耗时两头的断言一起兜住。
#[test]
fn running_flag_clears_only_after_the_paced_turn_ends() {
    let (mut kernel, mut mux, events) = open_stream_paced("$events", json!({}), PACE_MS);
    let follow = mux
        .open("session/follow", follow_args("s-9103"))
        .expect("同一条 socket 上再开 follow");
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "快照先到");

    let started = Instant::now();
    kernel
        .call("session/prompt", prompt_args("s-9103", "节流收尾", "queue"))
        .expect("session/prompt 应被接受");
    let mut on_follow = 0usize;
    let mut flags: Vec<bool> = Vec::new();
    while started.elapsed() < WAIT_BUDGET && flags.iter().all(|flag| *flag) {
        for event in mux.collect(Duration::from_millis(60)).expect("mux 读帧不应失败") {
            let MuxEvent::Item {
                stream,
                value: Some(value),
            } = &event
            else {
                continue;
            };
            if stream == &follow {
                on_follow += 1;
                continue;
            }
            if stream == &events {
                if let Some((session, running)) = session_status_event(value) {
                    if &session == "s-9103" {
                        flags.push(running);
                    }
                }
            }
        }
    }
    assert_eq!(flags, vec![true, false], "运行态要先亮后灭");
    assert_eq!(
        on_follow, TURN_FRAMES,
        "running=false 必须在整轮 {TURN_FRAMES} 帧都推完之后才到（那时只收到 {on_follow} 帧）"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(PACE_MS * TURN_FRAMES as u64 / 2),
        "整轮被拖长的工夫才是 UI 的抢拍窗口，实际 {:?}",
        started.elapsed()
    );
    kernel.shutdown();
}

// ================================================================ 目标 B：撤回取证的 meta.diffs
/// `tool/result` 的 `data.meta.diffs` 是「撤回本轮修改」唯一的取证面，三种 hunk 形态各得有一条：
/// `oldText:null` = 纯新增、两头都非空 = 修改、`newText:""` = 纯删除（主干 `HunksFromMeta` +
/// `RevertOneMutation` 的三分法）。路径以结果正文报出的为准（`<path>` 信封 / 整句两种口径）。
#[test]
fn tool_result_meta_carries_every_diff_shape_revert_needs() {
    let (mut kernel, mut mux, follow) = open_stream_with("session/follow", follow_args("s-9301"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args("s-9301", "撤回我", "queue"))
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect(&mut mux, &follow, TURN_FRAMES), &follow);
    let journal: Vec<&Value> = frames
        .iter()
        .filter(|frame| tag(frame).starts_with("event:"))
        .map(|frame| &frame["event"])
        .collect();

    // 只有成功的文件突变带 meta：跑命令/读文件/检索（call-1/2/3/8）、失败的编辑（call-6）与
    // 参数残缺那条（call-9）整块缺席 ⇒ UI 侧「拿不到 hunk 就不给撤回」有负项可判。
    let with_meta: Vec<&str> = journal
        .iter()
        .filter(|event| event["type"].as_str() == Some("tool/result"))
        .filter(|event| event["data"].get("meta").is_some())
        .map(|event| {
            event["data"]["message"]["source"]["callId"]
                .as_str()
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        with_meta,
        vec!["call-4", "call-5", "call-7", "call-10"],
        "带 meta.diffs 的就这四条，且都排在答案 seq 之前"
    );

    // 结构：每条 hunk 三键齐备，`newText` 恒为字符串（主干认不出就整条作废），
    // `oldText` 只能是字符串或 null；hunk.path 与结果正文报出的路径同源。
    let mut forms: Vec<&str> = Vec::new();
    for call_id in &with_meta {
        let data = result_data(&journal, call_id);
        let text = result_text(data);
        let diffs = data["meta"]["diffs"]
            .as_array()
            .unwrap_or_else(|| panic!("{call_id} 的 meta.diffs 该是数组"));
        for diff in diffs {
            assert_eq!(
                object_keys(diff),
                vec!["newText", "oldText", "path"],
                "{call_id} 的 hunk 只该带这三个键"
            );
            assert!(
                diff["newText"].is_string(),
                "{call_id} 的 newText 必须是字符串（纯删除给空串）"
            );
            assert!(
                diff["oldText"].is_string() || diff["oldText"].is_null(),
                "{call_id} 的 oldText 只能是字符串或 null"
            );
            let cited = cited_path(&text).unwrap_or_else(|| panic!("{call_id} 的正文该报出路径"));
            assert_eq!(
                cited,
                diff["path"].as_str().expect("hunk 得带 path"),
                "{call_id} 的正文路径要与 hunk.path 一致，撤回才不会改错文件"
            );
            forms.push(hunk_form(diff));
        }
    }
    for want in ["新增", "修改", "删除"] {
        assert!(
            forms.iter().any(|got| *got == want),
            "三种形态各至少一条，实际只有 {forms:?}"
        );
    }

    // 新建（call-4）：内核口径是 `before===null ⇒ diffs 空数组`，所以「新建」的唯一正据是正文
    // 那句 `Created file`；空 diffs 单独看还可能只是内容没变，两种情形靠这句文案分开。
    let created = result_data(&journal, "call-4");
    assert_eq!(
        created["meta"]["diffs"].as_array().expect("数组").len(),
        0,
        "新建的 diffs 是空数组"
    );
    assert!(
        result_text(created).contains("Created file"),
        "新建只能靠那句文案判定"
    );

    // 新增/修改两条：hunk 的 newText 必须正好等于 tool/call 参数里那份新内容。
    for (call_id, key) in [("call-5", "content"), ("call-7", "file_text")] {
        let args = arguments_of(call_event(&journal, call_id));
        let diffs = result_data(&journal, call_id)["meta"]["diffs"]
            .as_array()
            .expect("数组");
        assert_eq!(diffs.len(), 1, "{call_id} 该是一条 hunk");
        assert_eq!(
            diffs[0]["newText"],
            args[key],
            "{call_id} 的 newText 要等于写进去的正文"
        );
    }
    // 纯删除（call-10）：删掉的正是 call-5 刚写进去的那一行 ⇒ newText 空串。
    let removed = &result_data(&journal, "call-10")["meta"]["diffs"][0];
    assert_eq!(removed["newText"], json!(""), "纯删除 hunk 的 newText 是空串");
    assert_eq!(
        removed["oldText"],
        json!("pub fn produced() {}\n"),
        "删掉的该是 call-5 写进去那一行"
    );

    // `session/page` 回填的 journal 带着同一份 meta：历史会话翻回来也得能撤回。
    let page = kernel
        .call("session/page", page_args("s-9301", None, None))
        .expect("journal 回填该成功");
    let backfilled = page["records"]
        .as_array()
        .expect("数组")
        .iter()
        .find(|record| {
            record["event"]["type"].as_str() == Some("tool/result")
                && record["event"]["data"]["message"]["source"]["callId"].as_str()
                    == Some("call-5")
        })
        .expect("回填里该有 call-5");
    assert_eq!(
        backfilled["event"]["data"]["meta"],
        result_data(&journal, "call-5")["meta"],
        "回填的 meta 与流上推的那份一致"
    );
    kernel.shutdown();
}

// ================================================================ #93 注入 / 中继 / 召回帧
// 主干对 `user/message` 有三型「不是真人提问」的分支：① 中继
// （`MainWindow.MessageDetails.cs:242-260`，`source.kind=="agent-message" && form=="relay" &&
// senderSessionId`）、② 跨会话召回（`:261-301`，`kind=="session-reference" && form=="recall" &&
// references[]`）、③ 其余上下文注入（`:303-325`，如 `kind=="file"` + `path=="AGENTS.md"`）。
// 分叉的判据本体：渲染层 `src/main.rs:863-905`（`injection_of`）+ 冻结采集层
// `src/kernel.rs:5502-5636`（`DetailsLedger::note_user_message`）。真机上这三型只能靠真人
// + 真内核手工触发 ⇒ 自测脚本演不到。下面这批由假内核按 `--inject=1` 发帧（`fake_dsh.rs` 的
// `injection_entries`），**只跑假内核**，一帧都不碰 `Kernel/` 下的 node。
//
// 采集层那把 oracle 是公开入口 `DetailsLedger::note_event`（`injection_of` 在 bin 里、私有，
// 集成测试读不到 ⇒ 字段级判据在下面的 `assert` 里逐条复述，语义级判据交给 note_event）。

/// 一轮 `--inject=1` 多出来的帧数（`fake_dsh.rs::injection_entries` 的五条）。
const INJECTED_FRAMES: usize = 5;

/// 带开关的启动参数（默认仍压着 `--pace=0`，只有验节流的最后一条自己给节奏）。
fn launch_with(args: &[&str]) -> Launch {
    Launch {
        args: args.iter().map(|text| text.to_string()).collect(),
        ..fake_launch()
    }
}

/// 开档该见的帧序：关档的 `turn_tags()` 在第 3~7 格插入五帧 `event:user/message`
/// （注入发生在真人提问之后、本轮 `request/header` 之前）。
fn injected_turn_tags() -> Vec<String> {
    let mut tags: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    tags.splice(
        2..2,
        std::iter::repeat("event:user/message".to_string()).take(INJECTED_FRAMES),
    );
    tags
}

/// 一批流元素里的 journal 事件本体（`{type:"event", event:{…}}` 剥一层，与 `page_events` 同口径）。
fn journal_of(frames: &[Value]) -> Vec<Value> {
    frames
        .iter()
        .filter_map(|frame| frame.get("event"))
        .cloned()
        .collect()
}

/// 事件数组里所有 `user/message` 的 data。
fn user_frames(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some("user/message"))
        .map(|event| event["data"].clone())
        .collect()
}

/// 按 `source.kind` 找回那一枚 journal **事件**（要的是连信封 seq/time 的那份，采集层吃它）。
fn user_event(events: &[Value], kind: &str) -> Value {
    events
        .iter()
        .find(|event| {
            event["type"].as_str() == Some("user/message")
                && event["data"]["source"]["kind"].as_str() == Some(kind)
        })
        .cloned()
        .unwrap_or_else(|| panic!("脚本里该有一枚 source.kind == {kind} 的 user/message"))
}

/// `data.content[]` 里 text 块拼起来的正文（采集层 `join_text_blocks` 的同一口径）。
fn body_of(data: &Value) -> String {
    data["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block["type"].as_str() == Some("text"))
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// 注入行按分叉的落表方式取键：`src/main.rs:5718-5726` 先画 `role="tool"` 那一行、
/// 再把 `(tool, turn, seq)` 喂给采集层 ⇒ 用例照同一格算。
fn details_row_key(event: &Value, role: &str) -> DetailsKey {
    DetailsKey::bubble(
        role,
        event["data"]["turn"].as_i64().unwrap_or_default(),
        event["seq"].as_i64().unwrap_or_default(),
    )
}

/// 「这一档没演注入」的三连判据：kind 只能是空/`user`、不许有 form/`references`、正文不许带
/// `dsh-session:`。缺省档一帧都不冒出来，才谈得上「默认关」不漂。
fn assert_no_injection(frames: &[Value]) {
    for data in frames {
        let kind = data["source"]["kind"].as_str().unwrap_or_default();
        assert!(
            kind.is_empty() || kind == "user",
            "默认档不许冒出非用户来源的 user/message: {data}"
        );
        assert!(
            data["source"].get("form").is_none()
                && data["source"].get("references").is_none()
                && data["source"].get("senderSessionId").is_none(),
            "默认档的提问帧不许带注入侧的任何键: {}",
            data["source"]
        );
        assert!(
            !body_of(data).contains("dsh-session:"),
            "默认档不许带会话提及: {}",
            body_of(data)
        );
    }
}

/// **钉「默认关」不漂**：qa3 那 17 项基线与本文件既有的 `turn_tags()` 全按默认档写死，
/// 谁把 `injection_entries` 改成无条件发帧、或把开关默认值改成 1，这条立刻红。
/// live（follow 流）与历史回读（`session/page`）两路一起查 —— 两边共用 `turn_elements`。
#[test]
fn the_default_profile_delivers_no_injection_frame_at_all() {
    let (mut kernel, mut mux, follow) = open_stream_with("session/follow", follow_args("s-9401"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args("s-9401", "别演注入", "queue"))
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect(&mut mux, &follow, TURN_FRAMES), &follow);
    assert_eq!(frames.len(), TURN_FRAMES, "默认档一轮还是 {TURN_FRAMES} 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "默认档的帧序不许多出任何一行"
    );
    let users = user_frames(&journal_of(&frames));
    assert_eq!(users.len(), 1, "默认档一轮只有一条真人提问");
    assert_no_injection(&users);

    // 历史回读那一路共用同一枚开关：种子会话 s-1001 的三轮 journal 一帧注入都不许有。
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("种子会话该带着三轮历史回来");
    let seeded = page_events(&page);
    assert_eq!(seeded.len(), 90, "默认档三轮 = 30 帧 x3");
    let seeded_users = user_frames(&seeded);
    assert_eq!(seeded_users.len(), 3, "每轮一条真人提问");
    assert_no_injection(&seeded_users);
    kernel.shutdown();
}

/// 开档：三型帧（中继 / 召回 / 其余注入）**全部到达客户端**，且逐字段过两处判据。
/// 语义那一层不只查 JSON：把每帧原样喂进冻结的采集层 `DetailsLedger::note_event`，
/// 要求它真的写了那一格、写出来的就是主干那三案的同一份值。
#[test]
fn the_injection_switch_delivers_all_three_shapes_to_the_client() {
    let launch = launch_with(&["--pace=0", "--inject=1"]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args("s-9402"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args("s-9402", "演一把注入", "queue"))
        .expect("session/prompt 应被接受");
    let want = TURN_FRAMES + INJECTED_FRAMES;
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    assert_eq!(frames.len(), want, "开档一轮该有 {want} 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        injected_turn_tags(),
        "注入帧该插在真人提问之后、request/header 之前"
    );
    let events = journal_of(&frames);
    let injected: Vec<Value> = user_frames(&events)
        .into_iter()
        .filter(|data| {
            let kind = data["source"]["kind"].as_str().unwrap_or_default();
            !kind.is_empty() && kind != "user"
        })
        .collect();
    assert_eq!(
        injected
            .iter()
            .map(|data| data["source"]["kind"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["file", "instructions", "agent-message", "session-reference"],
        "三型各至少一条（其余注入给两型：文件 / 插件）"
    );
    for data in &injected {
        // `injection_of`（src/main.rs:865-875）与 `note_user_message`（src/kernel.rs:5504-5509）
        // 共用的那三条前置判据，逐帧复述一遍。
        assert!(data["content"].is_array(), "判据①：data.content 得是数组");
        assert!(data["source"].is_object(), "判据②：data.source 得是对象");
        assert!(!body_of(data).is_empty(), "行正文读得到: {data}");
    }

    let mut ledger = DetailsLedger::new();

    // ① 中继：kind + form + senderSessionId ⇒ relay_session_id + 一条「跨会话中继」明细。
    let relay = user_event(&events, "agent-message");
    assert_eq!(relay["data"]["source"]["form"].as_str(), Some("relay"));
    let key = details_row_key(&relay, "tool");
    assert!(
        ledger.note_event(&relay, Some(&key)).wrote.is_some(),
        "中继帧没被采集层认下"
    );
    let row = ledger.get(&key).expect("中继该落在自己那一格");
    assert_eq!(row.relay_session_id.as_deref(), Some("s-1001"));
    assert_eq!(
        row.context_entries,
        vec![(
            "跨会话中继".to_string(),
            "另一条会话问：登录改造后的鉴权测试补上了吗？".to_string()
        )],
        "主干 :257 那一档：整行只有这一条明细"
    );

    // ② 召回：references[] 三元素 ⇒ 两条登记（第三元素两头空 ⇒ 整条不登记）。
    let recall = user_event(&events, "session-reference");
    assert_eq!(recall["data"]["source"]["form"].as_str(), Some("recall"));
    let key = details_row_key(&recall, "tool");
    assert!(ledger.note_event(&recall, Some(&key)).wrote.is_some());
    let row = ledger.get(&key).expect("召回那一格");
    assert_eq!(
        row.references,
        vec![
            ("s-1001".to_string(), "重构登录流程".to_string()),
            ("s-1003".to_string(), "s-1003".to_string()),
        ],
        "label 空的元素回落到 sessionId；两头都空的整条丢弃"
    );
    assert_eq!(
        row.context_entries,
        vec![
            (
                "跨会话召回 · 重构登录流程".to_string(),
                "保留 12 条 · 省略 30 条 · 已截断".to_string()
            ),
            (
                "跨会话召回 · s-1003".to_string(),
                "保留 2 条 · 省略 0 条".to_string()
            ),
        ]
    );
    assert_eq!(row.recalls.len(), 2);
    assert!(row.recalls[0].truncated && !row.recalls[1].truncated);

    // ③ 其余注入·文件型：标签链没有 plugin ⇒ 走到 path（屏幕文案「上下文注入 · AGENTS.md」）。
    let file = user_event(&events, "file");
    assert_eq!(file["data"]["source"]["path"].as_str(), Some("AGENTS.md"));
    let key = details_row_key(&file, "tool");
    assert!(ledger.note_event(&file, Some(&key)).wrote.is_some());
    let row = ledger.get(&key).expect("注入那一格");
    assert_eq!(row.context_entries.len(), 1, "纯文件注入没有附加表: {:?}", row.context_entries);
    assert_eq!(row.context_entries[0].0, "AGENTS.md");
    assert!(row.context_entries[0].1.starts_with("# AGENTS.md"));

    // ③′ 其余注入·插件型：plugin 抢在 path/label 之前，外加 changes/entries/sections 三张表。
    let plugin = user_event(&events, "instructions");
    assert_eq!(plugin["data"]["source"]["plugin"].as_str(), Some("dsh-memory"));
    let key = details_row_key(&plugin, "tool");
    assert!(ledger.note_event(&plugin, Some(&key)).wrote.is_some());
    let row = ledger.get(&key).expect("插件注入那一格");
    assert_eq!(row.context_entries[0].0, "dsh-memory", "标签链 plugin 优先");
    assert!(
        row.context_entries
            .contains(&("AGENTS.md".to_string(), "已载入".to_string())),
        "baseline:true 且非 remove ⇒ 已载入: {:?}",
        row.context_entries
    );
    assert!(
        row.context_entries
            .contains(&("memory/stale.md".to_string(), "已移除".to_string())),
        "remove 那一档不套 baseline: {:?}",
        row.context_entries
    );
    assert_eq!(
        row.instruction_changes,
        vec![
            ("AGENTS.md".to_string(), "loaded".to_string()),
            ("memory/stale.md".to_string(), "remove".to_string())
        ],
        "空 path 的 change 整条丢弃"
    );
    assert!(
        row.context_entries
            .contains(&("记忆卡 1".to_string(), "第 1 条记忆卡的说明".to_string())),
        "catalog 前 8 条登记"
    );
    assert!(
        row.context_entries
            .contains(&("…还有 {0} 条".to_string(), "…还有 1 条".to_string())),
        "第 9 条逼出「…还有 N 条」那一行（title 是模板 = 主干原样）"
    );
    assert_eq!(
        row.context_entries
            .iter()
            .filter(|(title, _)| title.is_empty())
            .count(),
        1,
        "snapshot 先来一行空标题哨兵"
    );
    assert!(
        row.context_entries
            .contains(&("当前工作区".to_string(), r"E:\demo\alpha".to_string())),
        "sections 的 (name, text): {:?}",
        row.context_entries
    );
    assert_eq!(row.relay_session_id, None, "注入帧不许写中继位");

    // 真人提问 + 两枚 `dsh-session:` 提及：kind=="user" ⇒ 不算注入，但正文那两枚 URI 要能被
    // 采集层解出 sessionId（一枚裸 base64、一枚 `{"sessionId":…}` 载荷）。
    let mention = events
        .iter()
        .find(|event| {
            event["type"].as_str() == Some("user/message")
                && body_of(&event["data"]).contains("dsh-session:")
        })
        .cloned()
        .expect("开档该有一条带会话提及的真人提问");
    assert_eq!(mention["data"]["source"]["kind"].as_str(), Some("user"));
    let key = details_row_key(&mention, "user");
    assert!(
        ledger.note_event(&mention, Some(&key)).wrote.is_some(),
        "@ 提及该在气泡行落地时补登"
    );
    let row = ledger.get(&key).expect("提问那一格");
    assert_eq!(
        row.references,
        vec![
            ("s-1001".to_string(), "重构登录流程".to_string()),
            ("s-1003".to_string(), "补历史回读".to_string()),
        ],
        "base64 解码两条分支都得命中（URI 原样进表 = 解不出来才回落）"
    );
    assert!(row.context_entries.is_empty() && row.relay_session_id.is_none());
    assert_eq!(ledger.pending_references().len(), 0, "补登完桶该清空");
    kernel.shutdown();
}

/// 注入帧走的也是**同一台 journal 机器** ⇒ 历史回读（进会话第一发 `session/page`）必须把它们
/// 一并带回，否则切会话翻回去就只有真人当场那一次演得到。
#[test]
fn the_injection_frames_come_back_in_history_replay_too() {
    let launch = launch_with(&["--pace=0", "--inject=1"]);
    let mut kernel = Kernel::start(&launch).expect("带开关的假内核应完成握手");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("种子会话该带着三轮历史回来");
    let events = page_events(&page);
    assert_eq!(
        events.len(),
        90 + 3 * INJECTED_FRAMES,
        "三轮各多五帧"
    );
    assert_eq!(
        turn_start_seqs(&events),
        vec![3, 41, 88],
        "注入帧不许挪动轮锚点（分叉的气泡键与轮轨都吃它）"
    );
    let mut seqs: Vec<i64> = seqs_of(&events);
    seqs.sort_unstable();
    let uniq = seqs.len();
    seqs.dedup();
    assert_eq!(seqs.len(), uniq, "整页 seq 不许自撞（keyed_children 撞键是崩）");

    let users = user_frames(&events);
    assert_eq!(users.len(), 3 * (1 + INJECTED_FRAMES));
    for kind in ["file", "instructions", "agent-message", "session-reference"] {
        assert_eq!(
            users
                .iter()
                .filter(|data| data["source"]["kind"].as_str() == Some(kind))
                .count(),
            3,
            "每型该在三轮里各出现一次: {kind}"
        );
    }
    assert_eq!(
        users
            .iter()
            .filter(|data| body_of(data).contains("dsh-session:"))
            .count(),
        3,
        "每轮一枚带提及的真人提问"
    );
    // 每帧都挂在自己那一轮的 turn 上（分叉 `apply_journal_event` 按 data.turn 定位那一轮）。
    for data in &users {
        assert!(matches!(data["turn"], Value::Number(_)), "注入帧也得带 turn: {data}");
    }
    kernel.shutdown();
}

/// 开着帧间节流的注入轮：① 注入行落在**流式窗口里**（UI 抢得到拍），② 帧与帧的间隔确实
/// 大于 mux 的读超时（`MID_PACE_MS > READ_TICK = 120ms`，见其注释），③ 整轮最终补齐、
/// 帧序与关节奏时逐字相同。等帧全按事件等 + 带超时上限，不靠 sleep 撞运气。
#[test]
fn the_paced_injected_turn_arrives_frame_by_frame() {
    let pace_arg = format!("--pace={MID_PACE_MS}");
    let launch = launch_with(&[pace_arg.as_str(), "--inject=1"]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args("s-9404"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "快照是现成的，不该被节奏拖住");

    let started = Instant::now();
    kernel
        .call("session/prompt", prompt_args("s-9404", "节流 + 注入", "queue"))
        .expect("session/prompt 应被接受");
    // 头六帧 = turn/start + 真人提问 + 四型注入 ⇒ 注入行在流式窗口里就到齐了。
    let head = collect(&mut mux, &follow, 6);
    assert_eq!(head.len(), 6, "只想要六帧就该只到六帧");
    assert!(
        started.elapsed() >= Duration::from_millis(MID_PACE_MS * 5 - 20),
        "六帧之间该睡出五个间隔（否则 mux 会把它们并成一批，拍不到中途）: {:?}",
        started.elapsed()
    );
    let expect = injected_turn_tags();
    assert_eq!(
        item_values(&head, &follow)
            .iter()
            .map(tag)
            .collect::<Vec<_>>(),
        &expect[..6],
        "中途那一格里就该看到注入行，而不是整轮灌完才有"
    );

    let want = TURN_FRAMES + INJECTED_FRAMES - 6;
    let rest = collect_within(&mut mux, &follow, want, DRAIN_BUDGET);
    assert_eq!(rest.len(), want, "剩下的帧最终都得补齐");
    let mut all = item_values(&head, &follow);
    all.extend(item_values(&rest, &follow));
    assert_eq!(
        all.iter().map(tag).collect::<Vec<_>>(),
        expect,
        "节流只许改「什么时候到」，不许改「到什么」"
    );
    kernel.shutdown();
}

// ==================== #75 工作区 CRUD + 内核目录选择器（九发）====================
//
// 判据的权威源**不是**主干 C# 的读法，而是内核生成的 typert 描述符 + 网关那道
// `assertExactArguments`（`Kernel/dsh/node_modules/@deepseek-ai/dsh-api-workspace-controller/
// lib/typert.host.js:120-370`、`dsh-api-gateway/lib/index.js:1040-1052`）：外层 args 的键集合
// 要逐个对上 `wire` 名，少必填 ⇒ `missing "x"`、多未知键 ⇒ `unexpected "y"`。
// 所以「整棵 JSON 树对比」在这里不是洁癖，它就是「这一发能不能过网关那道关」的等价判据。
// 三类覆盖各占一处：① `workspace_rpc_args_…`（形状）② `…parsers_…`（成功/缺字段两路）
// ③ `…rejects_…` / `…refuses_…`（桩拒得起来、错误码传到客户端）。

/// 内核 `WorkspaceView`：回执与 follow 流共用的那六键对象（`updatedAt` 是假内核
/// `workspace_view()` 里写死的那一颗）。
fn view_json(id: &str, path: &str, title: &str, created: &str, sessions: &[&str]) -> Value {
    json!({
        "workspaceId": id,
        "path": path,
        "title": title,
        "sessionIds": sessions,
        "createdAt": created,
        "updatedAt": "2026-09-20T00:00:00.000Z",
    })
}

/// 与假内核 `seed_workspaces()` 同源的那张种子表（数组序 = 显示序）。
fn seed_view(id: &str) -> Value {
    match id {
        "ws-1" => view_json("ws-1", "C:/repo/one", "one", "2026-09-01T00:00:00.000Z", &["s-1", "s-2"]),
        "ws-2" => view_json("ws-2", "C:/repo/two", "two", "2026-09-02T00:00:00.000Z", &[]),
        "ws-3" => view_json("ws-3", "C:/repo/three", "three", "2026-09-03T00:00:00.000Z", &["s-3"]),
        other => panic!("种子表里没有 {other}"),
    }
}

/// ① 九发出去的 `method` + 逐字段 `args`（整棵树比，不是「有没有某个键」）。
/// 纯函数，不起内核 ⇒ 这一发红了就是构造层写错，与假内核无关。
#[test]
fn workspace_rpc_args_match_the_kernel_descriptor_key_for_key() {
    let cases: Vec<(RpcCall, &str, Value)> = vec![
        // 六发 workspace 写操作：描述符的 parameters 恒为 `[{wire:'request'}]` ⇒ 必裹一层。
        (
            workspace_rename("ws-1", "新名字"),
            WORKSPACE_RENAME,
            json!({ "request": { "workspaceId": "ws-1", "title": "新名字" } }),
        ),
        // 构造层不 trim：主干是 `box.Text.Trim()` 之后才传，空白标题该由内核拒。
        (
            workspace_rename("ws-1", "  留着  "),
            WORKSPACE_RENAME,
            json!({ "request": { "workspaceId": "ws-1", "title": "  留着  " } }),
        ),
        (
            workspace_delete("ws-2"),
            WORKSPACE_DELETE,
            json!({ "request": { "workspaceId": "ws-2" } }),
        ),
        (
            workspace_create("E:\\repos\\one"),
            WORKSPACE_CREATE,
            json!({ "request": { "path": "E:\\repos\\one" } }),
        ),
        (
            workspace_insert_before("ws-2", Some("ws-1")),
            WORKSPACE_INSERT_BEFORE,
            json!({ "request": { "workspaceId": "ws-2", "beforeWorkspaceId": "ws-1" } }),
        ),
        // 无锚点（主干下移到末位那一档）⇒ `beforeWorkspaceId` 整颗键**不存在**，不是 null。
        (
            workspace_insert_before("ws-2", None),
            WORKSPACE_INSERT_BEFORE,
            json!({ "request": { "workspaceId": "ws-2" } }),
        ),
        // 空白锚点视同未选（同 `session_create_location` 对空 workspaceId 的口径）。
        (
            workspace_insert_before("ws-2", Some("   ")),
            WORKSPACE_INSERT_BEFORE,
            json!({ "request": { "workspaceId": "ws-2" } }),
        ),
        (
            workspace_archive_session("s-1001"),
            WORKSPACE_ARCHIVE_SESSION,
            json!({ "request": { "sessionId": "s-1001" } }),
        ),
        (
            workspace_insert_session_before("ws-1", "s-1001", None),
            WORKSPACE_INSERT_SESSION_BEFORE,
            json!({ "request": { "workspaceId": "ws-1", "sessionId": "s-1001" } }),
        ),
        (
            workspace_insert_session_before("ws-1", "s-1001", Some("s-1")),
            WORKSPACE_INSERT_SESSION_BEFORE,
            json!({
                "request": { "workspaceId": "ws-1", "sessionId": "s-1001", "beforeSessionId": "s-1" }
            }),
        ),
        // 三发 directoryPicker：pick 是**零颗**、list 只一颗可选 path、createDirectory 平铺两颗。
        (directory_picker_pick(), DIRECTORY_PICKER_PICK, json!({})),
        (directory_picker_list(None), DIRECTORY_PICKER_LIST, json!({})),
        (
            directory_picker_list(Some("C:/fake-home/docs")),
            DIRECTORY_PICKER_LIST,
            json!({ "path": "C:/fake-home/docs" }),
        ),
        // 空串路径 ⇒ 回落到「内核默认起点」，绝不能发一个 `{path:""}` 或 `{path:null}` 出去。
        (directory_picker_list(Some("")), DIRECTORY_PICKER_LIST, json!({})),
        (
            directory_picker_create_directory("C:/fake-home", "new-folder"),
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            json!({ "path": "C:/fake-home", "name": "new-folder" }),
        ),
    ];
    for (index, (call, method, args)) in cases.into_iter().enumerate() {
        assert_eq!(call.method, method, "第 {index} 发的 method 不对");
        assert_eq!(call.args, args, "第 {index} 发的 args 整棵树不对");
        let (parts_method, parts_args) = call.clone().into_parts();
        assert_eq!((parts_method, parts_args), (method, args), "into_parts 与字段不一致");
    }
    // `createDirectory` 平铺是本刀唯一「不能裹 request」的一发：把两族形状比一比对，
    // 谁哪天顺手给它加了一层包裹，这条就会立刻红。
    assert!(
        directory_picker_create_directory("C:/x", "y").args.get("request").is_none(),
        "createDirectory 是两颗平铺 wire 字段，裹 request 会被网关拒成 unexpected \"request\""
    );
    assert!(
        directory_picker_pick().args.as_object().is_some_and(|map| map.is_empty()),
        "pick 的 parameters 是空数组：args 必须恰好是空对象"
    );
}

/// ② 回执解析的成功路：内核描述符那六键 / 五键全给齐时必须逐颗折进结构体。
#[test]
fn workspace_rpc_parsers_read_every_field_the_kernel_sends() {
    let view = view_json("ws-1", "C:/repo/one", "one", "2026-09-01T00:00:00.000Z", &["s-1", "s-2"]);

    // rename / insertSessionBefore 共用的 `{workspace}`。
    let parsed = parse_workspace_value(&json!({ "workspace": view })).expect("六颗必填都在");
    assert_eq!(
        parsed,
        WorkspaceView {
            id: "ws-1".to_string(),
            path: "C:/repo/one".to_string(),
            title: "one".to_string(),
            session_ids: vec!["s-1".to_string(), "s-2".to_string()],
            created_at: "2026-09-01T00:00:00.000Z".to_string(),
            updated_at: "2026-09-20T00:00:00.000Z".to_string(),
        }
    );
    // 能折回左栏那张表（`WorkspaceTree` 的元素形状 = 四颗，时间戳不入表）。
    assert_eq!(
        parsed.to_workspace(),
        Workspace {
            id: "ws-1".to_string(),
            path: "C:/repo/one".to_string(),
            title: "one".to_string(),
            session_ids: vec!["s-1".to_string(), "s-2".to_string()],
        }
    );

    // create 的 `{workspace, created}`：created=false 是「路径早就注册过」，不是失败。
    let created =
        parse_workspace_created(&json!({ "workspace": view, "created": false })).expect("两颗都在");
    assert_eq!(
        created,
        WorkspaceCreated { view: parsed, created: false }
    );

    assert_eq!(parse_workspace_deleted(&json!({ "deleted": true })), Ok(true));
    assert_eq!(
        parse_workspace_ids(&json!({ "workspaceIds": ["ws-2", "ws-1"] })).expect("全序"),
        vec!["ws-2".to_string(), "ws-1".to_string()]
    );
    assert_eq!(
        parse_archived_session_ids(&json!({ "archivedSessionIds": [] })).expect("空归档集"),
        Vec::<String>::new()
    );

    // pick：字符串 = 选中，null / 空串 = 取消（主干 :3921 同一判据）。
    assert_eq!(parse_pick(&json!("C:/picked")), PickOutcome::Picked("C:/picked".to_string()));
    assert_eq!(parse_pick(&Value::Null), PickOutcome::Cancelled);
    assert_eq!(parse_pick(&json!("")), PickOutcome::Cancelled);
    assert_eq!(parse_pick(&json!({})), PickOutcome::Cancelled, "形状不对也不能 panic");

    // createDirectory：value 是**裸字符串**，不是对象。
    assert_eq!(
        parse_created_directory(&json!("C:/fake-home/new-folder")),
        Ok("C:/fake-home/new-folder".to_string())
    );

    // list：五颗全必填，crumbs 与 entries 同形。
    let listing = DirectoryListing::parse(&json!({
        "path": "C:/fake-home/docs",
        "home": "C:/fake-home",
        "crumbs": [{ "name": "fake-home", "path": "C:/fake-home", "hidden": false },
                    { "name": "docs", "path": "C:/fake-home/docs", "hidden": false }],
        "entries": [{ "name": "notes", "path": "C:/fake-home/docs/notes", "hidden": false }],
        "truncated": true,
    }))
    .expect("主干 :4008 之后读的就是这几颗");
    assert_eq!(listing.home, "C:/fake-home");
    assert!(listing.truncated);
    assert_eq!(
        listing.entries,
        vec![DirectoryEntry {
            name: "notes".to_string(),
            path: "C:/fake-home/docs/notes".to_string(),
            hidden: false,
        }]
    );
    assert_eq!(listing.visible_crumbs().len(), 2);
    assert_eq!(listing.visible_entries().len(), 1);
}

/// ② 的另一路：内核**没给**（键缺席）与**形状不对**（键在、类型错）要分得开，且都不 panic。
#[test]
fn workspace_rpc_parsers_report_what_the_kernel_did_not_send() {
    let mut partial = view_json("ws-1", "C:/p", "t", "2026-01-01T00:00:00.000Z", &["s-1"]);
    // 六键少一颗 ⇒ 判词点名那颗键（这里挑一棵最容易被内核版本差异打掉的时间戳）。
    partial.as_object_mut().unwrap().remove("createdAt");
    let error = WorkspaceView::parse(&partial).expect_err("createdAt 必填");
    assert!(error.contains("createdAt"), "{error}");
    assert!(error.contains("没有"), "缺键要说成「没给」：{error}");

    // 键在但类型错 ⇒ 说成「不是预期的类型」，不与缺键混为一谈。
    let mut off_type = view_json("ws-1", "C:/p", "t", "c", &["s-1"]);
    off_type["sessionIds"] = json!("s-1");
    let error = WorkspaceView::parse(&off_type).expect_err("sessionIds 得是数组");
    assert!(error.contains("不是预期的类型"), "{error}");

    // `{workspace}` 外层少包裹。
    let error = parse_workspace_value(&json!({ "workspaces": [] })).expect_err("没有 workspace 键");
    assert!(error.contains("workspace"), "{error}");
    // create 少 `created`（视图本身是齐的）。
    let error = parse_workspace_created(&json!({ "workspace": seed_view("ws-1") }))
        .expect_err("created 必填");
    assert!(error.contains("created"), "{error}");
    // delete 的 `deleted` 描述符是字面量 true，给个数字就是形状不对。
    let error = parse_workspace_deleted(&json!({ "deleted": 1 })).expect_err("deleted 得是布尔");
    assert!(error.contains("deleted"), "{error}");
    assert_eq!(parse_workspace_deleted(&Value::Null).expect_err("整个回执不是对象").contains("deleted"), true);
    assert!(parse_workspace_ids(&json!({ "ids": [] })).expect_err("键名不对").contains("workspaceIds"));
    assert!(parse_archived_session_ids(&json!({}))
        .expect_err("归档集不能缺")
        .contains("archivedSessionIds"));
    assert!(parse_created_directory(&json!({ "path": "C:/x" })).expect_err("裸串才对").contains("字符串"));

    // list：五颗逐颗缺一遍 + 元素少一颗。
    let full = json!({
        "path": "C:/fake-home", "home": "C:/fake-home",
        "crumbs": [], "entries": [{ "name": "docs", "path": "C:/fake-home/docs", "hidden": false }],
        "truncated": false,
    });
    DirectoryListing::parse(&full).expect("齐的时候得过");
    for field in ["path", "home", "crumbs", "entries", "truncated"] {
        let mut shorn = full.clone();
        shorn.as_object_mut().unwrap().remove(field);
        let error = DirectoryListing::parse(&shorn).expect_err("{field} 是必填");
        assert!(error.contains(field), "{field}：{error}");
    }
    let mut element = full.clone();
    element["entries"][0].as_object_mut().unwrap().remove("hidden");
    let error = DirectoryListing::parse(&element).expect_err("元素的 hidden 也必填");
    assert!(error.contains("hidden"), "{error}");
}

/// ③+成功路：默认（native）档下，构造层发出去的六发 workspace 写操作桩**全部收下**，
/// 回执逐颗对上内核描述符；并且**同一份 args** 换个包裹方式就被拒（抄错一层的代价）。
#[test]
fn fake_kernel_accepts_every_workspace_rpc_the_constructor_emits() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // rename：回执是改完之后的整颗视图。
    let (method, args) = workspace_rename("ws-2", "改过的名字").into_parts();
    let value = kernel.call(method, args).expect("rename 该成功");
    assert_eq!(value["workspace"]["title"], json!("改过的名字"));
    assert_eq!(
        parse_workspace_value(&value).expect("回执是 {workspace}").title,
        "改过的名字"
    );

    // create：已注册过的路径 ⇒ created:false（内核 resolveByPath 在前），整棵树可比。
    let (method, args) = workspace_create("C:/repo/one").into_parts();
    let value = kernel.call(method, args).expect("已存在的路径要能解析出来");
    assert_eq!(value, json!({ "workspace": seed_view("ws-1"), "created": false }));
    // 真目录 ⇒ created:true，新 id 可预期、标题取末段、会话表为空。
    let here = std::env::temp_dir().to_string_lossy().to_string();
    let value = kernel
        .call(WORKSPACE_CREATE, workspace_create(&here).args)
        .expect("当前目录必然存在");
    let created = parse_workspace_created(&value).expect("{workspace,created}");
    assert!(created.created, "新路径该真的建出来：{value}");
    assert_eq!(created.view.id, "ws-new-1");
    assert_eq!(created.view.path, here);
    assert!(created.view.session_ids.is_empty());
    assert!(!created.view.title.is_empty());

    // archiveSession：回执是归档集全量（种子 s-9 在前，新归档追加在后）。
    let value = kernel
        .call(
            WORKSPACE_ARCHIVE_SESSION,
            workspace_archive_session("s-1001").args,
        )
        .expect("s-1001 是台账里的会话");
    assert_eq!(value, json!({ "archivedSessionIds": ["s-9", "s-1001"] }));
    // 重复归档不产生第二条（内核那句 push-if-absent 的等价物）。
    let value = kernel
        .call(
            WORKSPACE_ARCHIVE_SESSION,
            workspace_archive_session("s-1001").args,
        )
        .expect("再归档一次也不该报错");
    assert_eq!(value, json!({ "archivedSessionIds": ["s-9", "s-1001"] }));

    // insertSessionBefore：把 s-1002 从 ws-1 挪进 ws-2 ⇒ 回执是目标工作区的新视图。
    let value = kernel
        .call(
            WORKSPACE_INSERT_SESSION_BEFORE,
            workspace_insert_session_before("ws-2", "s-1002", None).args,
        )
        .expect("主干 :4266 就是这一发（不带 beforeSessionId）");
    assert_eq!(
        parse_workspace_value(&value).expect("{workspace}").session_ids,
        vec!["s-1002".to_string()]
    );

    // insertBefore：把 ws-3 提到 ws-1 之前 ⇒ 全序换形；无锚点那一发追加到尾部。
    // 判据里带上 ws-new-1 是故意的：上面那发真目录 create 已经把它挂到台账尾部，
    // 这两条因此同时证明「只挪目标那一颗，其余相对次序不动」。
    let value = kernel
        .call(
            WORKSPACE_INSERT_BEFORE,
            workspace_insert_before("ws-3", Some("ws-1")).args,
        )
        .expect("上移一档");
    assert_eq!(
        parse_workspace_ids(&value).expect("{workspaceIds}"),
        ["ws-3", "ws-1", "ws-2", "ws-new-1"].map(str::to_string).to_vec()
    );
    let value = kernel
        .call(
            WORKSPACE_INSERT_BEFORE,
            workspace_insert_before("ws-3", None).args,
        )
        .expect("末位那一档主干给的是 null ⇒ 键不存在");
    assert_eq!(
        parse_workspace_ids(&value).expect("全序"),
        ["ws-1", "ws-2", "ws-new-1", "ws-3"].map(str::to_string).to_vec()
    );

    // delete：回执只有 `{deleted:true}`（描述符是字面量，不是布尔）。
    let value = kernel
        .call(WORKSPACE_DELETE, workspace_delete("ws-2").args)
        .expect("删掉登记")
        ;
    assert_eq!(value, json!({ "deleted": true }));
    assert_eq!(parse_workspace_deleted(&value), Ok(true));
    // 删过的再删 ⇒ not-found（不会被当成幂等成功，主干据此报「删除失败」）。
    let error = kernel
        .call(WORKSPACE_DELETE, workspace_delete("ws-2").args)
        .expect_err("重复删除必须报错");
    assert!(error.contains("workspace/not-found"), "{error}");

    kernel.shutdown();
}

/// ③：形状错的两条路都得拒得起来 —— 外层包裹错（抄错一层）与内层键集合错。
#[test]
fn fake_kernel_rejects_off_shape_workspace_and_picker_arguments() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // 把 `request` 摘掉 = 平铺传：网关 assertExactArguments 的 `missing "request"`。
    for (method, request) in [
        (WORKSPACE_RENAME, json!({ "workspaceId": "ws-1", "title": "x" })),
        (WORKSPACE_DELETE, json!({ "workspaceId": "ws-1" })),
        (WORKSPACE_CREATE, json!({ "path": "C:/repo/one" })),
        (WORKSPACE_INSERT_BEFORE, json!({ "workspaceId": "ws-1" })),
        (WORKSPACE_ARCHIVE_SESSION, json!({ "sessionId": "s-1001" })),
        (WORKSPACE_INSERT_SESSION_BEFORE, json!({ "workspaceId": "ws-1", "sessionId": "s-1001" })),
    ] {
        let error = kernel
            .call(method, request)
            .expect_err("{method} 的平铺传必须被网关拒");
        assert!(error.contains("bad_args"), "{method}：{error}");
    }

    // 反过来：directoryPicker 那三发**不该**裹 request（裹了就红）。
    for (method, args) in [
        (DIRECTORY_PICKER_PICK, json!({ "request": {} })),
        (DIRECTORY_PICKER_LIST, json!({ "request": { "path": "C:/x" } })),
        (
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            json!({ "request": { "path": "C:/fake-home", "name": "x" } }),
        ),
    ] {
        let error = kernel
            .call(method, args)
            .expect_err("{method} 多包一层 request 必须被拒");
        assert!(error.contains("bad_args"), "{method}：{error}");
        assert!(error.contains("request"), "判词要说出那颗未知键：{error}");
    }

    // 内层 request 多一颗未知键 / 少一颗必填。
    for (label, method, args) in [
        (
            "rename 少 title",
            WORKSPACE_RENAME,
            json!({ "request": { "workspaceId": "ws-1" } }),
        ),
        (
            "rename 多未知键",
            WORKSPACE_RENAME,
            json!({ "request": { "workspaceId": "ws-1", "title": "x", "name": "x" } }),
        ),
        (
            "insertBefore 的锚点是空串",
            WORKSPACE_INSERT_BEFORE,
            json!({ "request": { "workspaceId": "ws-1", "beforeWorkspaceId": "" } }),
        ),
        (
            // 主干 :4228 那三行拼的是 beforeWorkspaceId；抄成 atSequence 这类别名就落在这里。
            "insertBefore 的锚点键名拼错",
            WORKSPACE_INSERT_BEFORE,
            json!({ "request": { "workspaceId": "ws-1", "before": "ws-2" } }),
        ),
        (
            "insertSessionBefore 少 sessionId",
            WORKSPACE_INSERT_SESSION_BEFORE,
            json!({ "request": { "workspaceId": "ws-1" } }),
        ),
        (
            "create 的 path 不是字符串",
            WORKSPACE_CREATE,
            json!({ "request": { "path": 7 } }),
        ),
        ("request 不是对象", WORKSPACE_DELETE, json!({ "request": "ws-1" })),
        ("pick 多一颗键", DIRECTORY_PICKER_PICK, json!({ "signal": true })),
        ("list 发 null 路径", DIRECTORY_PICKER_LIST, json!({ "path": Value::Null })),
        (
            "createDirectory 少 name",
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            json!({ "path": "C:/fake-home" }),
        ),
    ] {
        let error = kernel.call(method, args).expect_err(label);
        assert!(error.contains("bad_args"), "{label}：{error}");
    }

    kernel.shutdown();
}

/// ③：内核会拒的**业务**错，桩也照真码拒 ⇒ 分叉那几条失败文案才拿得到 code。
#[test]
fn fake_kernel_refuses_workspace_rpcs_with_the_kernel_error_codes() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // rename：不存在的工作区 ⇒ workspace/not-found（主干 :3834 整段 catch 后报「重命名失败」）。
    let error = kernel
        .call(WORKSPACE_RENAME, workspace_rename("ws-nope", "随便").args)
        .expect_err("不存在的工作区");
    assert!(error.contains("workspace/not-found"), "{error}");
    assert!(error.contains("ws-nope"), "消息要带上那颗 id：{error}");
    // 空白标题（trim 后为空）⇒ gateway/bad-request，**不是** bad_args。
    let error = kernel
        .call(WORKSPACE_RENAME, workspace_rename("ws-1", "  \t ").args)
        .expect_err("空白标题");
    assert!(error.contains("gateway/bad-request"), "{error}");
    // 与别的工作区重名 ⇒ workspace/name-conflict（主干据此也只能弹通用失败）。
    let error = kernel
        .call(WORKSPACE_RENAME, workspace_rename("ws-1", "two").args)
        .expect_err("标题撞上 ws-2");
    assert!(error.contains("workspace/name-conflict"), "{error}");
    // 改成自己当前的标题 ⇒ 内核跳过重名检查，幂等成功。
    kernel
        .call(WORKSPACE_RENAME, workspace_rename("ws-1", "one").args)
        .expect("同名于自己不是冲突");

    // create：路径不是目录 ⇒ workspace/invalid-path（内核 stat 那一关）。
    let error = kernel
        .call(
            WORKSPACE_CREATE,
            workspace_create("Z:/definitely/not/here-ws1").args,
        )
        .expect_err("不存在的路径");
    assert!(error.contains("workspace/invalid-path"), "{error}");

    // insertBefore：任一颗 id 不在序里都回 not-found（内核把 OrderInvalid 折成那颗 id）。
    let error = kernel
        .call(WORKSPACE_INSERT_BEFORE, workspace_insert_before("ws-nope", Some("ws-1")).args)
        .expect_err("被挪的那颗不存在");
    assert!(error.contains("workspace/not-found"), "{error}");
    let error = kernel
        .call(WORKSPACE_INSERT_BEFORE, workspace_insert_before("ws-1", Some("ws-nope")).args)
        .expect_err("锚点那颗不存在");
    assert!(error.contains("workspace/not-found"), "{error}");
    assert!(error.contains("ws-nope"), "报错要点名锚点那颗：{error}");

    // archiveSession：会话不认识 ⇒ session/not-found。
    let error = kernel
        .call(WORKSPACE_ARCHIVE_SESSION, workspace_archive_session("s-nope").args)
        .expect_err("没有这个会话");
    assert!(error.contains("session/not-found"), "{error}");

    // insertSessionBefore：工作区不存在先报 not-found；存在但会话无账 ⇒ workspace/move-invalid。
    let error = kernel
        .call(
            WORKSPACE_INSERT_SESSION_BEFORE,
            workspace_insert_session_before("ws-nope", "s-1001", None).args,
        )
        .expect_err("目标工作区不存在");
    assert!(error.contains("workspace/not-found"), "{error}");
    let error = kernel
        .call(
            WORKSPACE_INSERT_SESSION_BEFORE,
            workspace_insert_session_before("ws-1", "s-nope", None).args,
        )
        .expect_err("会话无账");
    assert!(error.contains("workspace/move-invalid"), "{error}");
    // 锚点不在目标工作区里 ⇒ 同一条 move-invalid（内核 WorkspaceMoveInvalidError）。
    let error = kernel
        .call(
            WORKSPACE_INSERT_SESSION_BEFORE,
            workspace_insert_session_before("ws-2", "s-1001", Some("s-1")).args,
        )
        .expect_err("锚点不在目标工作区");
    assert!(error.contains("workspace/move-invalid"), "{error}");

    kernel.shutdown();
}

/// 默认档 = native：`pick` 可用、`list`/`createDirectory` 回 `directory-picker/unavailable`。
/// 主干 :3990 就是 `Contains("directory-picker/unavailable")` 才收起浏览器区的，
/// 这条串在分叉侧必须是**码**而不是文案的一部分才会红。
#[test]
fn directory_picker_default_serves_native_pick_and_refuses_browsing() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let value = kernel
        .call(DIRECTORY_PICKER_PICK, directory_picker_pick().args)
        .expect("native 档 pick 可用");
    assert_eq!(
        parse_pick(&value),
        PickOutcome::Picked("C:/fake-picked/notes".to_string()),
        "主干 :3921 读的是裸字符串"
    );

    for (method, args) in [
        (DIRECTORY_PICKER_LIST, directory_picker_list(None).args),
        (DIRECTORY_PICKER_LIST, directory_picker_list(Some("C:/fake-home")).args),
        (
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            directory_picker_create_directory("C:/fake-home", "x").args,
        ),
    ] {
        let error = kernel.call(method, args).expect_err("native 档不服务 browse 两颗");
        assert!(error.contains("directory-picker/unavailable"), "{method}：{error}");
    }

    kernel.shutdown();
}

/// `--picker=1` 翻成 browse 档：`pick` 改口拒、`list` 两种入参形状都可用（含整棵回执树对比）、
/// `createDirectory` 的成功/撞名/父目录读不到/name 不是单段四路都演到。
#[test]
fn directory_picker_browse_mode_lists_both_argument_shapes_and_creates() {
    let mut kernel =
        Kernel::start(&launch_with(&["--pace=0", "--picker=1"])).expect("browse 档握手");

    // 反面对照：browse 档不服务原生对话框，但能力门的判据与上一条完全同形。
    let error = kernel
        .call(DIRECTORY_PICKER_PICK, directory_picker_pick().args)
        .expect_err("browse 档没有原生选择器");
    assert!(error.contains("directory-picker/unavailable"), "{error}");

    // 空参那一发（主干 :3986）：整棵回执树比。
    let value = kernel
        .call(DIRECTORY_PICKER_LIST, directory_picker_list(None).args)
        .expect("无 path = 内核默认起点");
    assert_eq!(
        value,
        json!({
            "path": "C:/fake-home",
            "home": "C:/fake-home",
            "crumbs": [],
            "entries": [
                { "name": ".cache", "path": "C:/fake-home/.cache", "hidden": true },
                { "name": "docs", "path": "C:/fake-home/docs", "hidden": false },
                { "name": "src", "path": "C:/fake-home/src", "hidden": false },
            ],
            "truncated": false,
        }),
        "五颗必填逐颗对上 DirectoryListing"
    );
    let listing = DirectoryListing::parse(&value).expect("上面那棵树就该解得开");
    assert_eq!(listing.visible_entries().len(), 3);

    // 带 path 那一发（主干 :4102）：面包屑从 home 起，hidden 跟着段名走。
    let value = kernel
        .call(
            DIRECTORY_PICKER_LIST,
            directory_picker_list(Some("C:/fake-home/docs")).args,
        )
        .expect("带 path");
    assert_eq!(
        value,
        json!({
            "path": "C:/fake-home/docs",
            "home": "C:/fake-home",
            "crumbs": [{ "name": "docs", "path": "C:/fake-home/docs", "hidden": false }],
            "entries": [{ "name": "notes", "path": "C:/fake-home/docs/notes", "hidden": false }],
            "truncated": false,
        })
    );

    // createDirectory 成功：value 是裸字符串；下一发 list 就该在 entries 里看到它。
    let value = kernel
        .call(
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            directory_picker_create_directory("C:/fake-home/src", "新目录").args,
        )
        .expect("建得出来");
    assert_eq!(
        parse_created_directory(&value),
        Ok("C:/fake-home/src/新目录".to_string())
    );
    let listing = DirectoryListing::parse(
        &kernel
            .call(
                DIRECTORY_PICKER_LIST,
                directory_picker_list(Some("C:/fake-home/src")).args,
            )
            .expect("刚建完就该列得出来"),
    )
    .expect("形状齐");
    assert_eq!(
        listing.entries,
        vec![DirectoryEntry {
            name: "新目录".to_string(),
            path: "C:/fake-home/src/新目录".to_string(),
            hidden: false,
        }]
    );

    // 三条被拒路：撞名 / 父目录读不到 / name 不是单一段（后两条真内核都有独立码）。
    let error = kernel
        .call(
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            directory_picker_create_directory("C:/fake-home/src", "新目录").args,
        )
        .expect_err("同名必撞");
    assert!(error.contains("directory-picker/exists"), "{error}");
    let error = kernel
        .call(
            DIRECTORY_PICKER_CREATE_DIRECTORY,
            directory_picker_create_directory("C:/nowhere", "x").args,
        )
        .expect_err("父目录读不到");
    assert!(error.contains("directory-picker/unreadable"), "{error}");
    for label in ["a/b", "..", "   ", "."] {
        let error = kernel
            .call(
                DIRECTORY_PICKER_CREATE_DIRECTORY,
                directory_picker_create_directory("C:/fake-home", label).args,
            )
            .expect_err("name 必须是单个非空路径段");
        assert!(error.contains("gateway/bad-request"), "{label}：{error}");
    }
    let error = kernel
        .call(DIRECTORY_PICKER_LIST, directory_picker_list(Some("C:/nowhere")).args)
        .expect_err("读不到的目录");
    assert!(error.contains("directory-picker/unreadable"), "{error}");

    kernel.shutdown();
}

/// 回执要喂回的那张表：`WorkspaceView::to_workspace()` 折出来的东西必须能直接进
/// `WorkspaceTree::apply` 认的那一型帧 —— 这是下一个 main.rs 写者唯一需要的「状态侧接口」，
/// 所以在这里钉死（种子表 = follow 的 baseline = 写操作回执，三边同一批字面值）。
#[test]
fn workspace_writes_feed_the_same_tree_the_follow_stream_builds() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    // follow baseline 的第 0 项与 rename 回执里的视图必须折成同一颗 `Workspace`。
    let from_follow = view_json("ws-1", "C:/repo/one", "one", "2026-09-01T00:00:00.000Z", &["s-1", "s-2"]);
    let from_reply = kernel
        .call(WORKSPACE_RENAME, workspace_rename("ws-1", "one").args)
        .expect("改成自己当前的标题 = 幂等成功");
    let tree_view = WorkspaceView::parse(&from_follow).expect("baseline 的 items[i] 与回执同形");
    let reply_view = parse_workspace_value(&from_reply).expect("回执是 {workspace}");
    assert_eq!(tree_view, reply_view);
    assert_eq!(
        reply_view.to_workspace(),
        Workspace {
            id: "ws-1".to_string(),
            path: "C:/repo/one".to_string(),
            title: "one".to_string(),
            session_ids: vec!["s-1".to_string(), "s-2".to_string()],
        }
    );

    // 归档回执直接喂 `WorkspaceTree` 的 `archived` 帧（主干 :4250 之后整段换掉归档集）。
    let archived = parse_archived_session_ids(
        &kernel
            .call(
                WORKSPACE_ARCHIVE_SESSION,
                workspace_archive_session("s-1001").args,
            )
            .expect("归档一条"),
    )
    .expect("回执是 {archivedSessionIds}");
    let mut tree = WorkspaceTree::default();
    let changed = tree.apply(&json!({ "type": "archived", "archivedSessionIds": archived }));
    assert!(changed, "归档集变了就要重画（主干据此 RefreshWorkspaces）");
    assert_eq!(tree.archived, vec!["s-9".to_string(), "s-1001".to_string()]);
    assert!(tree.is_archived("s-1001"));

    // insertSessionBefore 的回执喂 `upsert` 帧：目标工作区换了会话表（左栏那一列的新顺序）。
    let moved = kernel
        .call(
            WORKSPACE_INSERT_SESSION_BEFORE,
            workspace_insert_session_before("ws-2", "s-1001", None).args,
        )
        .expect("把 s-1001 挪进 ws-2");
    assert_eq!(
        parse_workspace_value(&moved)
            .expect("回执是 {workspace}")
            .session_ids,
        vec!["s-1001".to_string()]
    );
    let mut tree = WorkspaceTree::default();
    tree.apply(&json!({
        "type": "baseline",
        "value": { "items": [tree_view_serialized()], "archivedSessionIds": [] },
    }));
    // `value.workspace` 那一层直接就是 follow 的 upsert 载荷 —— 两族共用同一颗视图对象。
    assert!(tree.apply(&json!({ "type": "upsert", "workspace": moved["workspace"] })));
    assert_eq!(
        tree.workspaces
            .iter()
            .find(|item| item.id == "ws-2")
            .map(|item| item.session_ids.clone())
            .unwrap_or_default(),
        vec!["s-1001".to_string()],
        "回执喂回树之后，ws-2 那一组就该多出这条会话"
    );

    kernel.shutdown();
}

/// 上一把测试要的那颗 baseline 料（与假内核 `seed_workspaces()` 的 ws-1 逐字同形）。
fn tree_view_serialized() -> Value {
    view_json("ws-1", "C:/repo/one", "one", "2026-09-01T00:00:00.000Z", &["s-1", "s-2"])
}

// ==================== #104 `_maxTokensBubbles`：走真解码器的采集链自证 ====================
// 这一组**不开假内核**：`fake_dsh.rs` 的默认档一轮压根不发截断形状（它的 `turn/end` 是
// `json!({"turn": turn})`，`src/bin/fake_dsh.rs:3554`），而本片可写面不含那支桩 ⇒ 桩规格已
// 写进 `tmp/ct1-report.md` 的「越界请求」。这里改锁**分叉自己的解码→折叠接缝**：帧必须先活着
// 穿过产品码里的公开信封剥壳 `page_events`（`src/kernel.rs:2212`），再进 `note_event`，
// 才谈得上画不画那一行 —— 剥壳丢字段、按条过滤误伤，这条链上任何一环坏了用例就红。

/// 主干 `AppendMaxTokensRow`（`MainWindow.MessageDetails.cs:534-552`）的整链判据：
/// 两型喂帧各自认得准、同轮只出一行、`chunk` 嵌套那一档不算（主干两臂路径本就不对称）、
/// 且全程**不脏** `_messageDetails`（主干那两案对它是 no-op，`MessageDetails.cs:148-153`）。
#[test]
fn max_tokens_frames_survive_page_decoding_and_mark_exactly_one_row_per_turn() {
    let page = json!({
        "records": [
            {"event": {"seq": 11, "time": 900, "type": "assistant/attempt",
                "data": {"turn": 7, "stream": [{"type": "finish", "reason": {"kind": "max-tokens"}}]}}},
            {"event": {"seq": 12, "time": 910, "type": "turn/end",
                "data": {"turn": 7, "reason": {"kind": "max-tokens"}}}},
            {"event": {"seq": 13, "time": 920, "type": "turn/end",
                "data": {"turn": 8, "reason": {"kind": "end_turn"}}}},
            {"event": {"seq": 14, "time": 930, "type": "assistant/attempt",
                "data": {"turn": 8, "stream": [{"chunk": {"type": "finish",
                    "reason": {"kind": "max-tokens"}}}]}}},
            {"event": {"seq": 15, "time": 940, "type": "turn/end",
                "data": {"turn": 9, "reason": {"kind": "max-tokens"}}}},
            {"event": {"seq": 17, "time": 945, "type": "assistant/attempt",
                "data": {"turn": 10, "stream": [{"type": "finish", "reason": {"kind": "max-tokens"}}]}}},
            {"event": {"seq": 16, "time": 950, "type": "system/message",
                "data": {"turn": 9, "message": {"content": [{"type": "text", "text": "提示词"}]}}}},
            {"note": "坏记录：没有 event 那一层，page_events 按条跳过"},
        ]
    });
    let events = page_events(&page);
    assert_eq!(events.len(), 7, "剥壳该把 7 条事件原样交出来（坏记录不算）");

    let mut ledger = DetailsLedger::new();
    for event in &events {
        ledger.note_event(event, Some(&DetailsKey::frame(event["seq"].as_i64().unwrap_or(0),
            event["data"]["turn"].as_i64().unwrap_or(0))));
    }
    assert_eq!(
        ledger.max_tokens_turns(),
        vec![7, 9, 10],
        "7 轮两型帧只出一行（主干 `:539` 的门）、8 轮两发都不出（`end_turn` + `chunk` 嵌套档）、\
         9 轮只由 turn/end 出、10 轮只由 assistant/attempt 出 \
         —— 后两格是必要的：`note_event` 的分派臂少任何一案都会红在这里"
    );
    assert!(
        ledger.has_max_tokens(7) && !ledger.has_max_tokens(8) && ledger.has_max_tokens(10),
        "逐轮判据不成立：{:?}",
        ledger.max_tokens_turns()
    );

    // 第二张表不许串写第一张：`_messageDetails` 只认 `system/message` 那一案。
    assert_eq!(
        ledger.keys(),
        vec![DetailsKey::frame(16, 9)],
        "只有 `system/message` 该落进 _messageDetails（主干 `MessageDetails.cs:148-153` 两案是 no-op）"
    );

    // 清会话：截断账与表行同发作废（主干 `:124-128` 同一次 `ResetMessageDomainState`）。
    ledger.reset();
    assert!(ledger.is_empty() && ledger.max_tokens_turns().is_empty());
}

/// 文案通路：那三串在主干 `ShellEnglish` 里 0 命中 ⇒ 走 `DetText` 的双义字面量，
/// 分叉等价 `Catalog::dt`。`zh` 出原样、`en` 出**主干第二参逐字**（含那颗半角引号包着的
/// `continue`），任何一档露出另一种语言都是红。
#[test]
fn max_tokens_card_texts_follow_the_dettext_bilingual_path_not_the_shell_dictionary() {
    let zh = Catalog::load("zh", None);
    let en = Catalog::load("en", None);
    let cases: [(&str, &str); 3] = [
        ("已达到输出 token 上限", "Output token limit reached"),
        (
            "回答被截断，已有输出保留在对话中。发送“继续”可让模型接着输出。",
            "The reply was cut off; earlier output is preserved in the conversation. Send \"continue\" to let the model resume.",
        ),
        ("发送继续", "Send continue"),
    ];
    for (chinese, english) in cases {
        assert_eq!(zh.dt(chinese, english), chinese, "zh 档必须原样出中文");
        assert_eq!(en.dt(chinese, english), english, "en 档必须出主干第二参");
    }
    // 主干 `SendContinuePrompt` 填框走 `DetText("继续","continue")`：**第二参是小写**，
    // 但主干 `ShellEnglish` 有 `["继续"] = "Continue"` ⇒ 通道 A 先赢，**生效英文是大写**。
    // 所以产品侧那一发必须是 `bt`（先过表）而不是 `dt`（只认内联）—— 见 MR9 刀 A。
    assert_eq!(zh.dt("继续", "continue"), "继续");
    assert_eq!(en.dt("继续", "continue"), "continue");
    assert_eq!(
        en.bt("继续", "continue"),
        "Continue",
        "`bt` 让登记表道先赢 ⇒ 这才是主干生效英文；分叉不许用 `dt` 把它压回小写"
    );
    assert_eq!(zh.bt("继续", "continue"), "继续");
}

// ==================== #112：两枚默认关旋钮的「真过 socket / 真走 dispatch 臂」证据 ====================
// FS1 §9-1/2 挂账的两类证据：六型 projection 帧只验到桩内纯函数层的帧堆，`goals/get` 的单测
// 直接调函数体、绕过了 `handle_http` 上那条臂（`fake_dsh.rs:1618`）。下面三条补的就是这一段。

/// 带档位的启动助手。**不动 `fake_launch()` 的默认语义**（它只传 `--pace=0` ⇒ 两枚新旋钮恒关，
/// 接手时那 53 条既有用例的输入字节流一个都不变）；档位只在这条路上按显式 argv 传进去
/// （argv 压过 env，走桩既有 `knob()`，`fake_dsh.rs:147`）。
fn launch_with_args(args: &[&str]) -> Launch {
    let mut argv = vec!["--pace=0".to_string()];
    argv.extend(args.iter().map(|arg| arg.to_string()));
    Launch {
        args: argv,
        ..fake_launch()
    }
}

/// `open_control_stream()` 的带档位版本：同一套起手式（握手 + 鉴权 + `open_session_control()`）。
fn open_control_stream_by(launch: &Launch) -> (Kernel, Mux, String) {
    let kernel = Kernel::start(launch).expect("假内核应完成 dsh web: 握手");
    let mut mux = Mux::connect(kernel.endpoint(), kernel.cookie()).expect("mux 握手失败");
    let stream = mux
        .open_session_control()
        .expect("open session/control 失败");
    (kernel, mux, stream)
}

/// 对象键集按字母序取出：与内核 `hasExactKeys` 同口径的「恰好这些键」判据（多一键都是形状不合）。
fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("该值得是对象")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

/// 真 HTTP/JSON-RPC 层的**原始响应体**：`Kernel::call` 把「没有 `value` 键」与「`value:null`」
/// 两种形态都吞成 `Value::Null`，要证 FS1 §3.2 那条「整串里没有 value 键」只能自己读一次 socket。
/// 走的是 `kernel.rs` 那个私有 `http()` 的同一条路：`POST /api/<method>` + 握手拿到的 Cookie。
fn raw_response_body(kernel: &Kernel, method: &str, rpc_id: &str, args: Value) -> String {
    let ep = kernel.endpoint();
    let mut stream = TcpStream::connect((ep.host.as_str(), ep.port))
        .unwrap_or_else(|e| panic!("连接 {}:{}/api/{method} 失败: {e}", ep.host, ep.port));
    stream
        .set_read_timeout(Some(WAIT_BUDGET))
        .expect("设读超时");
    let cookie = kernel.cookie().expect("握手该拿到鉴权 Cookie");
    let body = json!({
        "type": "client-request",
        "rpcId": rpc_id,
        "method": method,
        "payload": { "args": args },
    })
    .to_string();
    let request = format!(
        "POST /api/{method} HTTP/1.1\r\nHost: {}:{}\r\nAccept: application/json\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCookie: {cookie}\r\n\r\n{body}",
        ep.host,
        ep.port,
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("发请求");
    stream.flush().expect("刷新");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("读响应");
    raw.split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("响应得有头体分隔: {raw}"))
        .1
        .to_string()
}

/// **用例 1**：`--projections=1` 的六型 projection 帧真的过了一遍 socket。
/// 钉住三件事：① 首批那四帧（`tests/ipc.rs:685` 逐字钉着的 baseline/queue/jobs/projection）**字节不变**；
/// ② 追加的六发 key 序 = 主干 `ApplyProjectionValues`（`MainWindow.xaml.cs:16045`）的分派序、
/// `seq` 89..=94、`sessionId` 恒 `s-1001`、外壳五键；③ 每型 value 的**键集**过完 socket 还在全在，
/// 并且分叉自己的 `ControlState::apply` 吃得下（typed 的 plan/permissions 落值、未建模键整块留着）。
#[test]
fn projections_knob_pushes_six_live_frames_over_the_socket_after_the_pinned_four() {
    // 关档对照流：同一份种子、同一套起手式，取「开档前四帧应当长什么样」的逐字字节。
    let (mut quiet, mut quiet_mux, quiet_stream) = open_control_stream_by(&fake_launch());
    let quiet_events = collect(&mut quiet_mux, &quiet_stream, 4);
    let quiet_frames = item_values(&quiet_events, &quiet_stream);
    assert_eq!(
        quiet_frames.iter().map(tag).collect::<Vec<_>>(),
        vec!["baseline", "queue", "jobs", "projection"],
        "对照流本身得是那四条既有帧，否则下面的字节比对没有意义: {quiet_frames:?}"
    );
    quiet.shutdown();
    drop(quiet_mux);

    let (mut kernel, mut mux, stream) =
        open_control_stream_by(&launch_with_args(&["--projections=1"]));
    let events = collect(&mut mux, &stream, 10);
    assert_eq!(events.len(), 10, "--projections=1 该是 4 + 6 帧: {events:?}");
    let frames = item_values(&events, &stream);
    assert_eq!(
        frames[..4].iter().map(Value::to_string).collect::<Vec<_>>(),
        quiet_frames.iter().map(Value::to_string).collect::<Vec<_>>(),
        "开档只许追加：首批四帧的字节一个都不许变（否则就是顶掉了 :685/:2234 那两条断言）"
    );

    let appended = &frames[4..];
    assert_eq!(
        appended.iter().map(tag).collect::<Vec<_>>(),
        vec!["projection"; 6],
        "追加的六发全是 projection 帧"
    );
    assert_eq!(
        appended
            .iter()
            .map(|frame| frame["key"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["plan", "permissions", "schedule", "goal", "modelSelection", "todos"],
        "key 序 = 主干的分派序"
    );
    assert_eq!(
        appended
            .iter()
            .map(|frame| frame["seq"].as_i64().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![89, 90, 91, 92, 93, 94],
        "seq 从种子 turnOutline(88) 之后接力"
    );
    for frame in appended {
        assert_eq!(
            sorted_keys(frame),
            ["key", "seq", "sessionId", "type", "value"],
            "外壳五键（桩的 `projection_frame`）过完 socket 一个不少"
        );
        assert_eq!(frame["sessionId"], json!("s-1001"), "桩的控制流只服务那条种子会话");
    }
    let values = appended.iter().map(|frame| &frame["value"]).collect::<Vec<_>>();

    assert_eq!(sorted_keys(values[0]), ["active", "pending"]);
    assert_eq!(
        values[0],
        &json!({ "active": false, "pending": false }),
        "种子会话没进计划模式"
    );

    assert_eq!(sorted_keys(values[1]), ["currentValue", "options"]);
    assert_eq!(values[1]["currentValue"], json!("workspace-write"));
    let options = values[1]["options"].as_array().expect("options 是数组");
    assert_eq!(options.len(), 2, "内核默认表两行；没派生成 custom 就不追加第三项");
    for option in options {
        assert_eq!(sorted_keys(option), ["description", "name", "value"]);
        assert!(!option["value"].as_str().unwrap_or_default().is_empty());
    }

    let schedule = values[2].as_array().expect("wire.view 是数组，不是那块 state 对象");
    assert_eq!(schedule.len(), 2);
    assert_eq!(sorted_keys(&schedule[0]), ["id", "kind", "prompt", "scheduledAt"]);
    assert_eq!(
        sorted_keys(&schedule[1]),
        ["everySeconds", "id", "kind", "prompt", "scheduledAt"],
        "`every` 型比 `at` 型只多 `everySeconds` 一根键（内核按 kind 精确 hasExactKeys）"
    );
    assert!(schedule[1]["everySeconds"].as_i64().unwrap_or_default() >= 300);
    for record in schedule {
        assert_eq!(
            record["scheduledAt"].as_str().unwrap_or_default().len(),
            24,
            "规范 RFC 3339 UTC（四位年 + 三位小数 + Z）"
        );
    }

    assert_eq!(
        sorted_keys(values[3]),
        ["createdAt", "goal", "roundsStarted", "updatedAt"]
    );
    assert_eq!(
        sorted_keys(&values[3]["goal"]),
        ["id", "maxGoalRounds", "objective", "phase", "revision"],
        "投影增量的内层 goal 刻意没有 activation"
    );

    assert_eq!(sorted_keys(values[4]), ["lastUsed", "next"]);
    assert_eq!(
        sorted_keys(&values[4]["lastUsed"]),
        ["model", "provider", "reasoningEffort"]
    );

    let todos = values[5].as_array().expect("todos 的权威 wire 形状是数组");
    assert_eq!(todos.len(), 3);
    for todo in todos {
        assert_eq!(sorted_keys(todo), ["content", "status"]);
        assert!(
            ["pending", "in_progress", "completed"]
                .contains(&todo["status"].as_str().unwrap_or_default()),
            "status 只能是内核那三个 literals: {todo}"
        );
    }

    // 过了 socket 还得进得了分叉自己的模型，才算「这条流真能用」。
    let mut state = ControlState::default();
    for frame in &frames {
        state.apply(frame);
    }
    let projections = projection_of(&state, "s-1001");
    assert_eq!(projections.plan, Some(PlanProjection { active: false, pending: false }));
    let permissions = projections
        .permissions
        .as_ref()
        .expect("permissions 增量得落进 typed 表");
    assert_eq!(permissions.current_value.as_deref(), Some("workspace-write"));
    assert_eq!(permissions.options.len(), 2);
    for key in ["schedule", "goal", "modelSelection", "todos"] {
        assert!(
            projections.value_of(key).is_some(),
            "分叉还没建模的 {key} 得整块留在 values 里，不许因「没字段」丢掉"
        );
    }
    assert_eq!(
        projections.turn_outline.len(),
        3,
        "六发增量各换各的键，种子大纲不受影响"
    );
    drop(mux);
    kernel.shutdown();
}

/// **用例 1 的第二档**：`--projections=2` 末两发负形过 socket 之后 `value` 仍是 `null` 本体
/// （不是缺键、不是空对象），并且分叉侧的整键替换语义真的把前一轮的成功形状盖掉。
#[test]
fn projections_knob_level_two_pushes_the_two_null_shapes_last_over_the_socket() {
    let (mut kernel, mut mux, stream) =
        open_control_stream_by(&launch_with_args(&["--projections=2"]));
    let events = collect(&mut mux, &stream, 12);
    assert_eq!(events.len(), 12, "2 档 = 4 + 6 + 2 发负形: {events:?}");
    let frames = item_values(&events, &stream);
    let nulls = &frames[10..];
    assert_eq!(
        nulls
            .iter()
            .map(|frame| frame["key"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["goal", "todos"],
        "两发负形排在最后，顺序就是内核 schema 里带 null 分支的那两键"
    );
    assert!(
        nulls.iter().all(|frame| frame["value"].is_null()),
        "负形的 value 得是 null 本体: {nulls:?}"
    );
    assert_eq!(
        nulls
            .iter()
            .map(|frame| frame["seq"].as_i64().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![95, 96],
        "seq 一路接力到 96（FS1 §10-1 纠过的就是这两个数）"
    );
    for frame in nulls {
        assert_eq!(
            sorted_keys(frame),
            ["key", "seq", "sessionId", "type", "value"],
            "null 也不许多一键少一键（value 键在、只是值为 null）"
        );
    }

    let mut state = ControlState::default();
    for frame in &frames {
        state.apply(frame);
    }
    let projections = projection_of(&state, "s-1001");
    assert_eq!(projections.value_of("goal"), Some(&Value::Null), "goal 被负形整键盖掉");
    assert_eq!(
        projections.value_of("todos"),
        Some(&Value::Null),
        "todos 的负形盖过 baseline 那份 {{open,done}} 假形状"
    );
    assert!(
        projections.permissions.is_some(),
        "同一批里的成功键不受影响：permissions 还在"
    );
    drop(mux);
    kernel.shutdown();
}

/// **用例 2**：`goals/get` 真的被 HTTP/JSON-RPC 层路由到了（`fake_dsh.rs:1618` 那条臂）。
/// 三档回法全走 `Kernel::call`，没有一条直调 `goals_get()`：臂不在 ⇒ 假内核回 `not_found`，
/// 下面每一条 `expect` 都会红。无目标那一发另走一次原始 socket，才量得到「整串没有 value 键」。
#[test]
fn goals_get_is_routed_by_the_http_layer_and_replies_three_shapes() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // ---- 1) 有目标会话：九键全量 GoalView，activation 在 ----
    let goal = kernel
        .call("goals/get", json!({ "agentId": "s-1001" }))
        .expect("goals/get 该被 dispatch 臂接住（不在臂上会回 not_found）");
    assert_eq!(
        sorted_keys(&goal),
        [
            "activation",
            "createdAt",
            "id",
            "maxGoalRounds",
            "objective",
            "phase",
            "revision",
            "roundsStarted",
            "updatedAt"
        ],
        "GoalView 九键（内核描述符 goals_get_result$schema）得整根过完这一发"
    );
    assert_eq!(goal["activation"], json!("armed"));
    assert_eq!(goal["phase"], json!("active"));
    assert_eq!(goal["revision"], json!(3));
    assert_eq!(goal["maxGoalRounds"], json!(8));
    assert_eq!(goal["roundsStarted"], json!(2));
    assert!(goal.get("blockedReason").is_none(), "0 档不该冒出 blockedReason");
    assert!(
        raw_response_body(&kernel, "goals/get", "r-full", json!({ "agentId": "s-1001" }))
            .contains(r#""value":{"#,),
        "有目标那一发的原始信封里当然得有 value 键（与下面那发对照）"
    );

    // ---- 2) 已知会话但没挂目标：`Kernel::call` 回 Null，且原始信封**没有** value 键 ----
    let goalless = kernel
        .call("goals/get", json!({ "agentId": "s-1002" }))
        .expect("有 agent、无目标是成功回法，不是错误");
    assert_eq!(goalless, Value::Null);
    assert_eq!(
        raw_response_body(&kernel, "goals/get", "r-void", json!({ "agentId": "s-1002" })),
        r#"{"result":{"ok":true},"rpcId":"r-void","type":"client-response"}"#,
        "线上形状就是 `{{ok:true}}` 而已；多出 `\"value\":null` 即红（serde_json 无 preserve_order ⇒ 字母序）"
    );

    // ---- 3) 未知 agent：内核式业务拒绝，HTTP 200 + result.ok=false ----
    assert_eq!(
        kernel
            .call("goals/get", json!({ "agentId": "s-9999" }))
            .expect_err("台账外的 agent 必须被拒"),
        r#"GOAL_AGENT_NOT_LIVE: agent "s-9999" is not live in this registry"#,
        "错误码与 message 都得是内核 assertLive 那句原文"
    );

    // ---- 4) 参数把关：wire 上必须恰好 `{agentId}` ----
    assert_eq!(
        kernel
            .call("goals/get", json!({}))
            .expect_err("少 agentId 得被 assertExactArguments 拒掉"),
        r#"bad_args: args fields do not match the descriptor: missing "agentId""#
    );
    assert_eq!(
        kernel
            .call("goals/get", json!({ "agentId": "s-1001", "revision": 3 }))
            .expect_err("多一键同样得拒"),
        r#"bad_args: args fields do not match the descriptor: unexpected "revision""#
    );
    kernel.shutdown();
}

/// **用例 2 的旋钮档**：`--goal=1` / `--goal=2` 这两型也在真臂上发得出来
/// （SB1 §5-9 那条不确定项 ⇒ 桩两型都不许拍死）。1 档是**整根键缺席**，不是 `null`。
#[test]
fn goal_knob_switches_the_activation_and_blocked_shapes_on_the_wire() {
    let mut disarmed = Kernel::start(&launch_with_args(&["--goal=1"])).expect("握手");
    let bare = disarmed
        .call("goals/get", json!({ "agentId": "s-1001" }))
        .expect("--goal=1 也得走通那条臂");
    assert_eq!(
        sorted_keys(&bare),
        [
            "createdAt",
            "id",
            "maxGoalRounds",
            "objective",
            "phase",
            "revision",
            "roundsStarted",
            "updatedAt"
        ],
        "1 档 = 投影增量那一型的键集"
    );
    assert_eq!(
        bare,
        json!({
            "id": "goal-1",
            "revision": 3,
            "objective": "把 session/control 的六型投影桩补齐",
            "phase": "active",
            "maxGoalRounds": 8,
            "roundsStarted": 2,
            "createdAt": 1_700_000_000_000_i64,
            "updatedAt": 1_700_000_600_000_i64,
        }),
        "整根 activation 缺席 ⇒ 剩下的字节必须与 0 档同源"
    );
    disarmed.shutdown();

    let mut blocked = Kernel::start(&launch_with_args(&["--goal=2"])).expect("握手");
    let view = blocked
        .call("goals/get", json!({ "agentId": "s-1001" }))
        .expect("--goal=2 也得走通那条臂");
    assert_eq!(view["phase"], json!("blocked"));
    assert_eq!(view["activation"], json!("disarmed"));
    assert_eq!(view["roundsStarted"], json!(8));
    assert_eq!(
        sorted_keys(&view["blockedReason"]),
        ["code", "message"],
        "blockedReason 恰两键（内核 decodeSnapshot 的 expectedKeys）"
    );
    assert_eq!(view["blockedReason"]["code"], json!("round-budget-exhausted"));
    blocked.shutdown();
}

// ==================== 批 B3（goals 族第④层）：六枚变更臂走真 socket ====================
// 判据来源：`tmp/s3-report.md` §5（桩侧交付）+ `tmp/m2b-spec.md` §1.2/§1.3（真内核线出口逐枚复验）。
// ⚠ 码面一律 `GOAL_STALE_REVISION`（`dsh-goal/lib/index.js:763`）—— 主代理派单里写的
// `GOAL_REVISION_CONFLICT` 在真内核**零命中**（S3 纠正①），照抄就是一条钉在假码上的用例。

/// 六枚变更 + 一枚回读全走真 socket：回执形状逐枚照 `@Remote` 出口，且**每枚之后立刻 `goals/get`**
/// （主干 `MainWindow.Capabilities.cs:248→:249` 那条「变更即回读」链）—— 桩不落态就红。
/// 顺序有讲究：种子那枚是 `active`/`armed`/`revision 3` ⇒ 先 `edit`（不吃迁移闸）、再 `pause`/`resume`、
/// 然后 `complete`（此后才允许 `create`，见 `apply_goal` 的 `phase != "complete"` 那拍）、最后 `clear`。
#[test]
fn six_goal_mutations_route_through_the_http_layer_and_survive_the_readback() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // ---- 0) 先回读种子，拿它的 id/revision 当 CAS 凭据（不写死 ⇒ 种子搬家不假红）----
    let seed = kernel
        .call("goals/get", json!({ "agentId": "s-1001" }))
        .expect("goals/get 在架");
    let (sid, srev) = (seed["id"].clone(), seed["revision"].clone());
    assert_eq!(srev, json!(3), "种子的 revision 是这条链的起点，动了它下面全红");

    // ---- 1) edit：`{agentId, ref, request}` ⇒ view；revision +1，其余三根**不许漂** ----
    let edited = kernel
        .call(
            "goals/edit",
            json!({
                "agentId": "s-1001",
                "ref": { "id": sid, "revision": srev },
                "request": { "objective": "  接通 goals 六发  " }
            }),
        )
        .expect("goals/edit 该被 dispatch 臂接住");
    assert_eq!(
        sorted_keys(&edited),
        [
            "activation",
            "createdAt",
            "id",
            "maxGoalRounds",
            "objective",
            "phase",
            "revision",
            "roundsStarted",
            "updatedAt"
        ],
        "变更臂回的是全量 GoalView 九键，不是 `{{ref}}` 也不是墓碑"
    );
    assert_eq!(edited["revision"], json!(4));
    assert_eq!(edited["objective"], json!("接通 goals 六发"), "落账前要 trim（内核 resolveObjective）");
    assert_eq!(edited["phase"], json!("active"), "edit 不换 phase（S3 纠正③）");
    assert_eq!(edited["activation"], json!("armed"), "edit 不换 activation");
    assert_eq!(edited["roundsStarted"], json!(2), "edit 不吃轮次");
    assert_eq!(edited["createdAt"], seed["createdAt"], "createdAt 是出生戳，不许被 bump");
    assert_ne!(edited["updatedAt"], seed["updatedAt"], "updatedAt 必须走");

    // ---- 2) 拿旧 revision 重放 ⇒ `GOAL_STALE_REVISION`（真内核的 CAS 闸，不是自创码）----
    let stale = kernel
        .call(
            "goals/edit",
            json!({
                "agentId": "s-1001",
                "ref": { "id": sid, "revision": srev },
                "request": { "objective": "重放这一发" }
            }),
        )
        .expect_err("旧 revision 的 ref 必须被 expectCurrent 拒掉");
    assert!(
        stale.starts_with("GOAL_STALE_REVISION: "),
        "码面错了：{stale}"
    );
    assert_eq!(
        kernel
            .call("goals/get", json!({ "agentId": "s-1001" }))
            .expect("回读在架")["revision"],
        json!(4),
        "被拒的那一发不许偷偷落态"
    );

    // ---- 3) pause / resume / complete：两键 `{agentId, ref}`，逐枚验迁移 + 回读逐字相等 ----
    let mut rev = 4i64;
    for (verb, phase, activation) in [
        ("pause", "paused", "disarmed"),
        ("resume", "active", "armed"),
        ("complete", "complete", "disarmed"),
    ] {
        rev += 1;
        let moved = kernel
            .call(
                &format!("goals/{verb}"),
                json!({ "agentId": "s-1001", "ref": { "id": sid, "revision": rev - 1 } }),
            )
            .unwrap_or_else(|err| panic!("goals/{verb} 该被接住：{err}"));
        assert_eq!(moved["phase"], json!(phase), "{verb} 的 phase 档");
        assert_eq!(moved["activation"], json!(activation), "{verb} 的 activation 档");
        assert_eq!(moved["revision"], json!(rev), "{verb} 要 bump revision");
        let back = kernel
            .call("goals/get", json!({ "agentId": "s-1001" }))
            .expect("回读在架");
        assert_eq!(back, moved, "{verb} 之后桩落的那一态必须就是回读那一态");
    }

    // ---- 4) resume 的迁移闸（不是只演 happy path）：已 active+armed 再 resume ⇒ 拒 ----
    let again = kernel
        .call(
            "goals/resume",
            json!({ "agentId": "s-1001", "ref": { "id": sid, "revision": rev } }),
        )
        .expect_err("已经 complete 的目标不可 resume（complete 不在 resumable 三档里）");
    assert!(
        again.starts_with("GOAL_INVALID_TRANSITION: "),
        "迁移闸的码面错了：{again}"
    );

    // ---- 5) create：`complete` 之后才允许；回执**只吐 `{ref:{id,revision}}**`（不是全量 view）----
    let created = kernel
        .call(
            "goals/create",
            json!({
                "agentId": "s-1001",
                "request": { "objective": "第二枚目标", "maxGoalRounds": 4 }
            }),
        )
        .expect("goals/create 该被 dispatch 臂接住");
    assert_eq!(
        sorted_keys(&created),
        ["ref"],
        "create 的线出口是 `return {{ ref: {{ id, revision }} }}`（`index.js:896-901`），回全量 view 即假绿"
    );
    assert_eq!(created["ref"]["revision"], json!(1), "新目标从 revision 1 起");
    let nid = created["ref"]["id"].clone();
    assert!(
        nid.as_str().unwrap_or_default().starts_with("goal-"),
        "id 形如 `goal-<seq>`，实际 {nid}"
    );
    let fresh = kernel
        .call("goals/get", json!({ "agentId": "s-1001" }))
        .expect("回读在架");
    assert_eq!(fresh["id"], nid, "回读看到的得是新那枚");
    assert_eq!(fresh["objective"], json!("第二枚目标"));
    assert_eq!(fresh["maxGoalRounds"], json!(4), "显式 maxGoalRounds 要盖过默认 8");
    assert_eq!(fresh["roundsStarted"], json!(0));
    assert_eq!(fresh["phase"], json!("active"));
    assert_eq!(fresh["activation"], json!("armed"));

    // ---- 6) clear：回**墓碑** `{id, revision: 旧+1}`（S3 纠正④：不是 void 信封）----
    let tomb = kernel
        .call(
            "goals/clear",
            json!({ "agentId": "s-1001", "ref": { "id": nid, "revision": 1 } }),
        )
        .expect("goals/clear 该被接住");
    assert_eq!(sorted_keys(&tomb), ["id", "revision"], "墓碑恰两键");
    assert_eq!(tomb["id"], nid);
    assert_eq!(tomb["revision"], json!(2));

    // ---- 7) clear 之后：`goals/get` 的整串信封**没有** `value` 键（与「从没挂过」同形）----
    assert_eq!(
        kernel
            .call("goals/get", json!({ "agentId": "s-1001" }))
            .expect("无目标是成功回法，不是错误"),
        Value::Null
    );
    assert!(
        !raw_response_body(&kernel, "goals/get", "r-cleared", json!({ "agentId": "s-1001" }))
            .contains(r#""value""#),
        "clear 之后还吐 value 键 ⇒ 与真内核 `view()` 回 undefined 不符"
    );
    let gone = kernel
        .call(
            "goals/pause",
            json!({ "agentId": "s-1001", "ref": { "id": nid, "revision": 2 } }),
        )
        .expect_err("没有当前目标时变更臂要撞 expectCurrent 第一拍");
    assert!(
        gone.starts_with("GOAL_NOT_FOUND: "),
        "码面错了：{gone}"
    );
    kernel.shutdown();
}

/// 六枚的**把关档**：`assertExactArguments` 的三型键集漂移、活会话无目标、台账外 agent、
/// 以及一条**反多做**哨兵 —— 真内核的 goals 远端只有七枚，第八枚（`goals/block`）不在架上。
#[test]
fn goal_mutation_wire_gates_reject_before_touching_state() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let seed = kernel
        .call("goals/get", json!({ "agentId": "s-1001" }))
        .expect("种子在架");
    let (sid, srev) = (seed["id"].clone(), seed["revision"].clone());

    // ---- 1) 描述符键集：多一键 / 少一键 / 平铺，全是 `bad_args` 而不是业务码 ----
    for (label, method, args, want) in [
        (
            "create 多带 ref",
            "goals/create",
            json!({
                "agentId": "s-1001",
                "request": { "objective": "多一颗 ref" },
                "ref": { "id": sid, "revision": srev }
            }),
            r#"unexpected "ref""#,
        ),
        (
            "pause 缺 ref",
            "goals/pause",
            json!({ "agentId": "s-1001" }),
            r#"missing "ref""#,
        ),
        (
            "edit 把 objective 平铺到顶层",
            "goals/edit",
            json!({
                "agentId": "s-1001",
                "ref": { "id": sid, "revision": srev },
                "objective": "平铺"
            }),
            r#"unexpected "objective""#,
        ),
        (
            "任何一枚少 agentId",
            "goals/resume",
            json!({ "ref": { "id": sid, "revision": srev } }),
            r#"missing "agentId""#,
        ),
    ] {
        let err = kernel
            .call(method, args)
            .expect_err(&format!("{label} 该被 assertExactArguments 拒掉"));
        assert!(
            err.starts_with("bad_args: ") && err.contains(want),
            "{label}：要 `{want}`，实际 {err}"
        );
    }

    // ---- 2) 业务校验档（三枚各一型，全在真臂上）----
    assert!(kernel
        .call(
            "goals/edit",
            json!({
                "agentId": "s-1001",
                "ref": { "id": sid, "revision": srev },
                "request": { "maxGoalRounds": 0 }
            }),
        )
        .expect_err("轮数要正整数")
        .starts_with("GOAL_INVALID_MAX_ROUNDS: "));
    assert!(kernel
        .call(
            "goals/edit",
            json!({
                "agentId": "s-1001",
                "ref": { "id": sid, "revision": srev },
                "request": {}
            }),
        )
        .expect_err("两根都缺席 = 不合法编辑")
        .starts_with("GOAL_INVALID_EDIT: "));
    assert!(kernel
        .call(
            "goals/create",
            json!({
                "agentId": "s-1001",
                "request": { "objective": "种子还 active，这里就该撞已存在" }
            }),
        )
        .expect_err("active 目标之上不能再 create")
        .starts_with("GOAL_ALREADY_EXISTS: "));

    // ---- 3) agent 档：台账外 ⇒ `GOAL_AGENT_NOT_LIVE`（HTTP 仍 200），活而无目标 ⇒ `GOAL_NOT_FOUND` ----
    for (method, args) in [
        (
            "goals/create",
            json!({ "agentId": "s-9999", "request": { "objective": "x" } }),
        ),
        (
            "goals/edit",
            json!({ "agentId": "s-9999", "ref": { "id": "goal-1", "revision": 1 }, "request": {} }),
        ),
        (
            "goals/pause",
            json!({ "agentId": "s-9999", "ref": { "id": "goal-1", "revision": 1 } }),
        ),
        (
            "goals/resume",
            json!({ "agentId": "s-9999", "ref": { "id": "goal-1", "revision": 1 } }),
        ),
        (
            "goals/complete",
            json!({ "agentId": "s-9999", "ref": { "id": "goal-1", "revision": 1 } }),
        ),
        (
            "goals/clear",
            json!({ "agentId": "s-9999", "ref": { "id": "goal-1", "revision": 1 } }),
        ),
    ] {
        let err = kernel
            .call(method, args)
            .expect_err(&format!("{method} 的 assertLive 那一拍"));
        assert_eq!(
            err,
            r#"GOAL_AGENT_NOT_LIVE: agent "s-9999" is not live in this registry"#,
            "{method}：错误码与 message 都得是内核 assertLive 那句原文"
        );
    }
    assert!(kernel
        .call(
            "goals/clear",
            json!({ "agentId": "s-1002", "ref": { "id": "goal-1", "revision": 1 } }),
        )
        .expect_err("活会话但没挂目标")
        .starts_with("GOAL_NOT_FOUND: "));

    // ---- 4) 反多做：真内核的 goals 远端**只有七枚**（get + create/edit/pause/resume/complete/clear）
    //         ⇒ 第八枚（`goals/block` 之类）哪一层都不许有臂。这一发要的就是 `not_found`。
    let extra = kernel
        .call("goals/block", json!({ "agentId": "s-1001" }))
        .expect_err("goals 族没有第八枚远端");
    assert!(
        extra.contains("not_found"),
        "多造一枚远端 = 主干没有的东西被画出来了：{extra}"
    );

    // ---- 5) 上面那些**全被拒**的一串之后，种子那一态必须一格没动 ----
    assert_eq!(
        kernel
            .call("goals/get", json!({ "agentId": "s-1001" }))
            .expect("回读在架"),
        seed,
        "被拒的发数再多人家也不许改账"
    );
    kernel.shutdown();
}

// ==================== #117：`--trunc=` 走真 socket 的集成用例（SK1 §7 R-2） ====================
// SK1 只在桩内单测里演过五档（`cargo test --bin fake_dsh` 38 条），真过一遍 mux/HTTP 的那一路
// 还没有用例。下面这批开真 `fake_dsh.exe`（stdio/HTTP 桩，不起窗口）、发一条真 prompt、
// 从 follow 流收整轮 journal，逐档验「帧数 / 字节 / 键集」。形状与切点全照 `tmp/sk1-report.md`
// §2/§3 抄，但**期望以真 socket 实测为准**：内核那一轮 live prompt 的 turn 是 1（见
// `session_follow_streams_a_full_prompt_turn` 的 `frames[0].event.data.turn == 1`），
// 不是桩内单测手填的 7 —— 这是 R-2 与 sk1 §5 的一处口径差，见报告 §3 纠偏。

/// 收真 socket 的一整轮：返回（follow 流帧序列 供查帧序、剥壳后的 journal 事件 供查数据面）。
/// `want` 是收帧预算——档 2/3/4 一轮 37 帧（多一发 attempt），必须显式抬预算，
/// 否则 `collect_within` 会卡在 36 上等到 `WAIT_BUDGET` 超时（SK1 §8-3 点名的坑）。
fn trunc_turn(session: &str, level: u64, want: usize) -> (Vec<Value>, Vec<Value>) {
    let trunc_arg = format!("--trunc={level}");
    let launch = launch_with(&["--pace=0", &trunc_arg]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args(session, "被截断的这一轮", "queue"))
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 开档 2/3/4 该见的帧序：关档 `turn_tags()` 在 `event:assistant/message` **之前**插一格
/// `event:assistant/attempt`（切点算出来的，不写死下标，与 `fake_dsh.rs` 的 splice 同法）。
fn trunc_frame_tags(extra_attempt: bool) -> Vec<String> {
    let mut tags: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    if extra_attempt {
        let at = tags
            .iter()
            .position(|tag| tag == "event:assistant/message")
            .expect("关档帧序里得有 assistant/message");
        tags.insert(at, "event:assistant/attempt".to_string());
    }
    tags
}

/// 找 journal 里的 `assistant/attempt`（关档 1 发、档 2/3/4 变 2 发，新增那发 seq 最大）。
fn attempt_events(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some("assistant/attempt"))
        .cloned()
        .collect()
}

fn single_event(events: &[Value], kind: &str) -> Value {
    events
        .iter()
        .find(|event| event["type"].as_str() == Some(kind))
        .cloned()
        .unwrap_or_else(|| panic!("journal 里该有一发 {kind}"))
}

/// 剥掉信封 `time`（墙钟值，两次起桩必不等）后的 journal：只留 seq/type/data 逐字节可比。
fn json_without_time(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .map(|event| json!({ "seq": event["seq"], "type": event["type"], "data": event["data"] }))
        .collect()
}

/// **档 1**：只在既有那发 `turn/end` 的 data 上补一键 ⇒ 帧序/帧数与关档逐字相等，
/// 且那枚 `reason` 恰一键 `{kind:"max-tokens"}`。字节级：serde_json 无 preserve_order ⇒
/// 键按字母序，`reason` 抢在 `turn` 前，与 SK1 §5 的关档对照串同形（仅 turn 值不同）。
#[test]
fn truncation_level_one_adds_a_key_to_turn_end_without_adding_a_frame() {
    let (frames, events) = trunc_turn("s-9501", 1, TURN_FRAMES);
    assert_eq!(events.len(), 30, "档 1 不许加 journal 帧");
    assert_eq!(frames.len(), TURN_FRAMES, "档 1 一轮仍 36 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "档 1 的帧序与关档逐格相等"
    );
    let turn_end = single_event(&events, "turn/end");
    assert_eq!(
        serde_json::to_string(&turn_end["data"]).unwrap(),
        r#"{"reason":{"kind":"max-tokens"},"turn":1}"#,
        "档 1 补键后的 turn/end.data 逐字节"
    );
    assert_eq!(sorted_keys(&turn_end["data"]), ["reason", "turn"]);
    assert_eq!(
        sorted_keys(&turn_end["data"]["reason"]),
        ["kind"],
        "reason 恰一键：可选键集为空由内核校验器锁死（sk1 §1）"
    );
    assert_eq!(turn_end["data"]["reason"]["kind"], json!("max-tokens"));
    assert_eq!(turn_end["data"]["turn"], json!(1), "live prompt 的轮号是 1");
    assert_eq!(attempt_events(&events).len(), 1, "档 1 不许多出 attempt 帧");
}

/// **档 2（flat）**：多一发 `assistant/attempt`，流片是 `{"type":"finish","reason":{…}}`——
/// 读者 B 吃的那一型（`chunk` 必须不存在）。一轮变 37 帧，新增那发 seq 严格小于 settled。
#[test]
fn truncation_level_two_adds_a_flat_finish_attempt_over_the_socket() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = trunc_turn("s-9502", 2, want);
    assert_eq!(frames.len(), want, "档 2 一轮 37 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        trunc_frame_tags(true),
        "新增 attempt 该插在 assistant/message 之前"
    );
    let attempts = attempt_events(&events);
    assert_eq!(attempts.len(), 2, "关档 1 发 + 档 2 追加 1 发 = 2 发");
    let added = &attempts[attempts.len() - 1];
    assert_eq!(sorted_keys(&added["data"]), ["step", "stream", "turn"]);
    assert_eq!(added["data"]["turn"], json!(1), "追加那发挂在同一轮 turn=1");
    let stream = added["data"]["stream"].as_array().expect("stream 得是数组");
    assert_eq!(stream.len(), 1, "只补一发收尾片");
    let piece = &stream[0];
    assert_eq!(
        sorted_keys(piece),
        ["reason", "type"],
        "flat 片恰两键，chunk 不许存在"
    );
    assert!(piece.get("chunk").is_none(), "flat 型不许带 chunk（差一层 chunk = 读者 B 的判据）");
    assert_eq!(piece["type"], json!("finish"));
    assert_eq!(sorted_keys(&piece["reason"]), ["kind"]);
    assert_eq!(piece["reason"]["kind"], json!("max-tokens"));
    let settled = single_event(&events, "assistant/message");
    assert!(
        added["seq"].as_i64().unwrap() < settled["seq"].as_i64().unwrap(),
        "新增 attempt 的 seq 必须严格小于 settled，否则时间单调性/切点都塌了"
    );
}

/// **档 3（nested）**：追加的流片是内核真发形状 `{type:"chunk", time, chunk:{type:"finish",…}}`
/// ⇒ 正对读者 B 的负形：`piece["type"] != "finish"`、`piece` 顶层**没有** `reason`，flat 读者读不到。
#[test]
fn truncation_level_three_adds_the_nested_kernel_shape_the_flat_reader_cannot_read() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = trunc_turn("s-9503", 3, want);
    assert_eq!(frames.len(), want, "档 3 一轮 37 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        trunc_frame_tags(true)
    );
    let attempts = attempt_events(&events);
    assert_eq!(attempts.len(), 2, "档 3 同样追加一发 attempt");
    let added = &attempts[attempts.len() - 1];
    let piece = &added["data"]["stream"][0];
    assert_eq!(
        sorted_keys(piece),
        ["chunk", "time", "type"],
        "nested 片恰三键：type/time/chunk"
    );
    assert_eq!(piece["type"], json!("chunk"), "外层 type 是 chunk 不是 finish");
    assert_ne!(piece["type"], json!("finish"), "负形：flat 读者判 `type==finish` 落空");
    assert!(
        piece.get("reason").is_none(),
        "负形：顶层无 reason，flat 读者判 `piece.reason` 是对象也落空"
    );
    assert!(
        piece["time"].as_i64().is_some_and(|time| time != 0),
        "内层 time 取信封时间（now+STEP），不是 0"
    );
    let inner = &piece["chunk"];
    assert_eq!(sorted_keys(inner), ["reason", "type"]);
    assert_eq!(inner["type"], json!("finish"));
    assert_eq!(inner["reason"]["kind"], json!("max-tokens"));
    let turn_end = single_event(&events, "turn/end");
    assert!(
        turn_end["data"].get("reason").is_none(),
        "档 3 只加 attempt，不许动 turn/end（补 reason 是档 1/4 的活）"
    );
}

/// **档 4（1+2 同轮）**：turn/end 带 reason **且**多一发 flat attempt，两臂的 `data.turn`
/// 全是 1 ⇒ 给主干「同轮只出一条」那道门（分叉 `kernel.rs:5567`）真实输入。
#[test]
fn truncation_level_four_fires_both_arms_on_the_same_turn_over_the_socket() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = trunc_turn("s-9504", 4, want);
    assert_eq!(frames.len(), want, "档 4 一轮 37 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        trunc_frame_tags(true)
    );
    let turn_end = single_event(&events, "turn/end");
    assert_eq!(
        turn_end["data"]["reason"]["kind"],
        json!("max-tokens"),
        "档 4 的 A 臂：turn/end 带 reason"
    );
    let attempts = attempt_events(&events);
    assert_eq!(attempts.len(), 2, "档 4 的 B 臂：多一发 attempt");
    let piece = &attempts[attempts.len() - 1]["data"]["stream"][0];
    assert_eq!(
        sorted_keys(piece),
        ["reason", "type"],
        "档 4 追加的是 flat 片（与档 2 同型）"
    );
    assert_eq!(piece["type"], json!("finish"));
    assert_eq!(piece["reason"]["kind"], json!("max-tokens"));
    assert_eq!(turn_end["data"]["turn"], json!(1));
    assert_eq!(attempts[attempts.len() - 1]["data"]["turn"], json!(1));
}

/// **未定义档回落**：`--trunc=5` 与 `--trunc=9` 都该与关档 `--trunc=0` 同形——帧数 36、帧序逐字等、
/// turn/end 不带 reason、attempt 仍只默认那一发。`time` 是墙钟值（两次起桩必不等），故比对
/// 剥掉信封 `time` 后逐字节相等（其余 seq/type/data 都得一致）。
#[test]
fn an_undefined_truncation_level_falls_back_to_the_off_journal() {
    let (_, off) = trunc_turn("s-9505", 0, TURN_FRAMES);
    for level in [5u64, 9] {
        let session = format!("s-95{}", level);
        let (frames, events) = trunc_turn(&session, level, TURN_FRAMES);
        assert_eq!(
            frames.iter().map(tag).collect::<Vec<_>>(),
            turn_tags(),
            "未定义档 {level} 的帧序该与关档逐格相等"
        );
        assert_eq!(
            json_without_time(&events),
            json_without_time(&off),
            "未定义档 {level} 剥掉墙钟 time 后该与关档 journal 逐字节相等"
        );
        assert!(
            single_event(&events, "turn/end")["data"]
                .get("reason")
                .is_none(),
            "未定义档 {level} 不许给 turn/end 补 reason"
        );
        assert_eq!(attempt_events(&events).len(), 1, "未定义档 {level} 不许追加 attempt 帧");
    }
}

/// **回读同形（SK1 §8-4 的账）**：`seed_journal` 与 live 取同一档 ⇒ 历史回读那一路必须演同一型。
/// 档 1：三轮 turn/end 全带 reason、帧数不动（仍 90）、attempt 仍每轮只默认那一发。
/// 档 2：每轮多一发 flat attempt（90→93）、turn/end **不带** reason（补 reason 是档 1/4 的活）。
#[test]
fn the_truncation_frames_come_back_in_history_replay_matching_the_live_shape() {
    // 档 1 回读：补键、不加帧。
    let mut kernel = Kernel::start(&launch_with(&["--pace=0", "--trunc=1"])).expect("握手");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("档 1 种子会话该带回历史");
    let events = page_events(&page);
    assert_eq!(events.len(), 90, "档 1 回读一轮 30 帧 ×3，不加帧");
    let turn_ends: Vec<Value> = events
        .iter()
        .filter(|event| event["type"].as_str() == Some("turn/end"))
        .cloned()
        .collect();
    assert_eq!(turn_ends.len(), 3);
    for turn_end in &turn_ends {
        assert_eq!(
            turn_end["data"]["reason"]["kind"],
            json!("max-tokens"),
            "回读 turn/end 与 live 同形：带 reason"
        );
    }
    assert_eq!(attempt_events(&events).len(), 3, "档 1 回读不许多出 attempt");
    kernel.shutdown();

    // 档 2 回读：多一发 flat attempt/轮，turn/end 不动。
    let mut kernel = Kernel::start(&launch_with(&["--pace=0", "--trunc=2"])).expect("握手");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("档 2 种子会话该带回历史");
    let events = page_events(&page);
    assert_eq!(events.len(), 93, "档 2 回读每轮多一发 attempt（30+1）×3");
    let added = attempt_events(&events)
        .into_iter()
        .filter(|attempt| {
            attempt["data"]["stream"][0]["type"].as_str() == Some("finish")
                && attempt["data"]["stream"][0]["reason"]["kind"].as_str() == Some("max-tokens")
        })
        .collect::<Vec<_>>();
    assert_eq!(added.len(), 3, "三轮各多一发 flat max-tokens attempt");
    for attempt in &added {
        assert!(
            attempt["data"]["stream"][0].get("chunk").is_none(),
            "回读的追加片也是 flat（无 chunk），与 live 同形"
        );
    }
    for turn_end in events
        .iter()
        .filter(|event| event["type"].as_str() == Some("turn/end"))
    {
        assert!(
            turn_end["data"].get("reason").is_none(),
            "档 2 回读不许给 turn/end 补 reason（那是档 1/4）"
        );
    }
    kernel.shutdown();
}

// ==================== #123：`--retry=` 六档走真 socket 的集成用例（SK2 §7 R-1） ====================
// SK2 只在桩内单测里演过六档（`cargo test --bin fake_dsh` 48 条），真过一遍 mux/HTTP 那一路
// 还没有用例。下面这批开真 `fake_dsh.exe`（stdio/HTTP 桩，不起窗口）、发一条真 prompt、
// 从 follow 流收整轮 journal，逐档验「帧数 / 两发配对 / 键集的集合等式」。形状照
// `tmp/sk2-report.md` §2 那张权威表，但**期望全按本片真 socket 现测写**（现测记录在报告 §1/§2）：
// · live 一轮的元素数 = 该档 journal 事件数 + 那 6 枚与档无关的 `assistant-stream` 逐字增量
//   ⇒ 实测 36 / 38 / 37 / 38 / 37 / 37（档 0→5）⇒ `collect_within` 的预算必须逐档抬；
// · live prompt 的轮号是 1（不是桩内单测手填的 7）⇒ `retryId` 线上取 `retry-1-chain-1`；
// · 每一档一律走**用例内自带 argv**（`launch_with`）：不改 `fake_launch()`、不设
//   `FAKE_DSH_RETRY` 环境变量 ⇒ 接手时那 63 条既有用例的输入字节流一个都不变。

/// 一档 `--retry=` 在线上该多出的帧标签。档 1/3 是 `dsh-llm-retry` 那两个 append 点
/// （`index.js:141` + `:143-148`）都在；档 2/4/5 只有 scheduled 那一发（档 2 = `index.js:142`
/// 的 backoff 等待被中止；档 4/5 的 `retryId` 缺失让 started 那发的读者无处认领）。
fn retry_tags_of(level: u64) -> &'static [&'static str] {
    match level {
        1 | 3 => &["event:llm/retry", "event:llm/retry-started"],
        2 | 4 | 5 => &["event:llm/retry"],
        _ => &[],
    }
}

/// 在给定帧序里、**既有那发失败的** `event:assistant/attempt` 之后插该档的重试帧。
/// 下标是算出来的（与 `fake_dsh.rs` 的 `position(== "assistant/attempt") + 1` 同法）：
/// 开 `--inject=` 时前面已经多塞五格、开 `--trunc=2/3/4` 时后面还会再多一发 attempt。
fn with_retry_tags(base: &[String], level: u64) -> Vec<String> {
    let mut tags = base.to_vec();
    let added = retry_tags_of(level);
    if !added.is_empty() {
        let at = tags
            .iter()
            .position(|text| text == "event:assistant/attempt")
            .expect("关档帧序里得有那发失败的 assistant/attempt")
            + 1;
        tags.splice(at..at, added.iter().map(|text| text.to_string()));
    }
    tags
}

/// 关档 36 格帧序 + 该档的重试帧 ⇒ 开档该见的完整帧序。
fn retry_frame_tags(level: u64) -> Vec<String> {
    let base: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    with_retry_tags(&base, level)
}

/// 收真 socket 的一整轮 `--retry=`：返回（follow 流帧序 供查帧序、剥壳后的 journal 事件 供查数据面）。
/// `want` 是收帧预算，**每档都得按实测抬**（档 0→36 / 1→38 / 2→37 / 3→38 / 4→37 / 5→37）；
/// 沿用写死的 36 会让 `collect_within` 卡在缺帧上直到 `DRAIN_BUDGET`（12s）超时才返回。
fn retry_turn(session: &str, level: u64, want: usize) -> (Vec<Value>, Vec<Value>) {
    let knob = format!("--retry={level}");
    retry_turn_argv(session, &[&knob], want)
}

/// 同上，但旋钮原样给（`--retry=` 与 `--inject=` / `--trunc=` 是**相加**关系，叠档用例走这里）。
fn retry_turn_argv(session: &str, knobs: &[&str], want: usize) -> (Vec<Value>, Vec<Value>) {
    let mut argv: Vec<&str> = vec!["--pace=0"];
    argv.extend_from_slice(knobs);
    let launch = launch_with(&argv);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args(session, "上游限流了", "queue"))
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 历史回读那一路（`seed_journal` 与 live 取同一档）：种子会话 `s-1001` 三轮的 journal 事件。
fn retry_seed_argv(knobs: &[&str]) -> Vec<Value> {
    let mut argv: Vec<&str> = vec!["--pace=0"];
    argv.extend_from_slice(knobs);
    let mut kernel = Kernel::start(&launch_with(&argv)).expect("握手");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("种子会话该带回历史");
    let events = page_events(&page);
    kernel.shutdown();
    events
}

fn retry_seed_events(level: u64) -> Vec<Value> {
    let knob = format!("--retry={level}");
    retry_seed_argv(&[&knob])
}

/// journal 里某一型重试帧的全部发（按帧序；关档恒空）。
fn retry_events(events: &[Value], kind: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some(kind))
        .cloned()
        .collect()
}

/// 该型帧**恰一发**时把它取出来。多于一条直接炸：内核 `invariant.js:75-82` 禁同一链重复发。
fn only_retry(events: &[Value], kind: &str) -> Value {
    let found = retry_events(events, kind);
    assert_eq!(found.len(), 1, "journal 里 {kind} 该恰一发，实际 {} 发", found.len());
    found.into_iter().next().expect("上面已断过非空")
}

/// 帧序里第一个匹配该标签的下标。
fn tag_index(frames: &[Value], want: &str) -> usize {
    frames
        .iter()
        .map(tag)
        .position(|text| text == want)
        .unwrap_or_else(|| panic!("帧序里该有 {want} 那一格"))
}

/// **键集权威形状**（format 的 `disposition` / `exactRecord` 是双向判据）：返回 `value` 里
/// 不在 `allowed` 之内的键。只断言「这几枚键在」是弱判据——多一枚可选键（`status` /
/// `providerRetryAfterMs` / `requestId`）就过不了内核自己的校验器，这条把它数出来。
fn extra_keys(value: &Value, allowed: &[&str]) -> Vec<String> {
    value
        .as_object()
        .expect("该值得是对象")
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect()
}

/// `llm/retry` 的必填九枚（`disposition([...9...], ["maxRetries"])`）。
const RETRY_REQUIRED_KEYS: [&str; 9] = [
    "retryId", "turn", "step", "provider", "mode", "policyKey", "retry", "delayMs", "failure",
];
/// 必填九枚 + 唯一那枚可选键 `maxRetries`（顺序无关，只喂 `extra_keys`）。
const RETRY_ALLOWED_KEYS: [&str; 10] = [
    "retryId", "turn", "step", "provider", "mode", "policyKey", "retry", "delayMs", "failure",
    "maxRetries",
];
/// `llm/retry-started`：**恰四枚、可选空**（按 `sorted_keys` 的字母序写，直接比集合）。
const STARTED_KEYS: [&str; 4] = ["retry", "retryId", "step", "turn"];
/// `failure`：必填两枚（`exactRecord(…, ["message","code"], […])`，字母序）。
const FAILURE_KEYS: [&str; 2] = ["code", "message"];
/// 必填两枚 + 内核允许、主干不读的三枚可选（顺序无关，只喂 `extra_keys`）。
const FAILURE_ALLOWED: [&str; 5] =
    ["code", "message", "status", "providerRetryAfterMs", "requestId"];
/// 档 4/5 少 `retryId`、档 3 少 `maxRetries`：现测的线上键集（升序，即 `sorted_keys` 口径）。
const SCHEDULED_KEYS_NORMAL: [&str; 10] = [
    "delayMs", "failure", "maxRetries", "mode", "policyKey", "provider", "retry", "retryId",
    "step", "turn",
];
const SCHEDULED_KEYS_ALWAYS: [&str; 9] = [
    "delayMs", "failure", "mode", "policyKey", "provider", "retry", "retryId", "step", "turn",
];
const SCHEDULED_KEYS_LEGACY: [&str; 9] = [
    "delayMs", "failure", "maxRetries", "mode", "policyKey", "provider", "retry", "step", "turn",
];

/// **档 0（显式关）**：一帧不加、一键不改，且与「argv 压根不带 `--retry=`」那一轮逐字节同形。
/// 钉的是「这枚旋钮默认不污染既有 36 格帧序」——接手时那 63 条按帧序钉死的用例吃的就是它。
#[test]
fn retry_level_zero_adds_no_frame_and_matches_the_unflagged_default() {
    let (frames, events) = retry_turn("s-9601", 0, TURN_FRAMES);
    assert_eq!(frames.len(), TURN_FRAMES, "档 0 一轮仍 36 帧（现测）");
    assert_eq!(events.len(), 30, "档 0 一轮仍 30 条 journal 事件");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "档 0 的帧序与关档逐格相等"
    );
    assert!(retry_events(&events, "llm/retry").is_empty(), "档 0 不许发 scheduled");
    assert!(
        retry_events(&events, "llm/retry-started").is_empty(),
        "档 0 不许发 started"
    );
    let round = frames.iter().map(Value::to_string).collect::<String>();
    assert!(
        !round.contains("llm/retry"),
        "整轮序列化里连 `llm/retry` 子串都不许出现（不是发了一发空 data）"
    );
    // 与「压根没给这枚旋钮」那一轮逐字节同形：seq/type/data 全等，只免掉墙钟 time。
    let (_, unflagged) = retry_turn_argv("s-9602", &[], TURN_FRAMES);
    assert_eq!(
        json_without_time(&events),
        json_without_time(&unflagged),
        "显式 `--retry=0` 与不开旋钮该是同一台机器"
    );
}

/// **档 1 = K1+K3**：scheduled 与 started 两发同 `retryId`/`turn`/`step`/`retry`，且
/// `retry-started` **紧跟** `llm/retry`（帧序相邻 + seq 相邻 + 信封时间差恰为 `delayMs`）。
/// 起点是 SK2 §7-R-1 那段可粘贴样例；样例里没有的（seq 相邻、time 差 = delayMs、
/// provider 对上当期 `request/header`、`retry <= maxRetries`）是本片加的。
#[test]
fn retry_level_one_pushes_the_scheduled_pair_over_the_socket() {
    let want = TURN_FRAMES + 2;
    let (frames, events) = retry_turn("s-9603", 1, want);
    assert_eq!(frames.len(), want, "档 1 一轮 38 帧（现测，非照抄 SK2）");
    assert_eq!(events.len(), 32, "档 1 一轮 32 条 journal 事件");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        retry_frame_tags(1),
        "只许在那一发失败的 attempt 之后多两格，其余逐格不动"
    );
    let scheduled = only_retry(&events, "llm/retry");
    let started = only_retry(&events, "llm/retry-started");
    let data = &scheduled["data"];
    let begun = &started["data"];
    // 内核 `invariant.js:71/:78` 的配对判据：链断 = 卡停在「等待重试」。
    assert_eq!(data["retryId"], begun["retryId"], "两发必须同 retryId");
    assert_eq!(data["turn"], begun["turn"], "两发必须同 turn");
    assert_eq!(data["step"], begun["step"], "两发必须同 step");
    assert_eq!(data["retry"], begun["retry"], "两发必须同 retry");
    assert_eq!(begun["retryId"], json!("retry-1-chain-1"), "live 轮号是 1 ⇒ id 也按它编");
    // 紧跟：信封 seq 相邻、帧序相邻、两发时间差就是 data 上那枚 delayMs。
    assert_eq!(
        started["seq"].as_i64(),
        scheduled["seq"].as_i64().map(|seq| seq + 1),
        "started 该紧跟 scheduled，中间不许插别的帧"
    );
    let at = tag_index(&frames, "event:llm/retry");
    assert_eq!(tag(&frames[at + 1]), "event:llm/retry-started");
    assert_eq!(
        data["delayMs"].as_i64(),
        Some(started["time"].as_i64().unwrap() - scheduled["time"].as_i64().unwrap()),
        "delayMs 就是两发的真实间隔（不是随手填的装饰）"
    );
    // 键集权威形状：必填九枚 + 可选只有 maxRetries。
    assert_eq!(sorted_keys(data), SCHEDULED_KEYS_NORMAL, "档 1 的 scheduled 恰十枚");
    assert_eq!(sorted_keys(begun), STARTED_KEYS, "started 恰四枚、可选空");
    assert_eq!(extra_keys(data, &RETRY_ALLOWED_KEYS), Vec::<String>::new(), "多余键为 0");
    assert_eq!(extra_keys(begun, &STARTED_KEYS), Vec::<String>::new(), "多余键为 0");
    // 值面：normal 档的三枚规格值 + 首发必为 1。
    assert_eq!(data["mode"], json!("normal"));
    assert_eq!(data["maxRetries"], json!(2));
    assert_eq!(
        data["policyKey"],
        json!(r#"["normal",2,["provider_rate_limited"],1000,30000,0.2]"#),
        "normal 档的策略串逐字节：它是策略数组的 JSON.stringify，带引号与方括号是形状不是 bug"
    );
    assert_eq!(data["retry"], json!(1), "内核的 retry 是 previousRetry+1 ⇒ 首轮只能是 1");
    assert!(
        data["retry"].as_i64().unwrap() <= data["maxRetries"].as_i64().unwrap(),
        "normal 档规格：retry <= maxRetries"
    );
    // provider 必须等于当期 request/header 报的那一枚（invariant.js:66-67）：
    // 跨帧一致性 ⇒ 桩里那两处 `dsh-test` 字面一起改走时这条照样得红（SK2 §5 M12 的教训）。
    let header = single_event(&events, "request/header");
    assert_eq!(
        data["provider"], header["data"]["header"]["config"]["provider"],
        "重试帧报的 provider 得是本轮实际生效的那一枚"
    );
}

/// **档 2 = K4**：backoff 等待里被中止 ⇒ 线上**只有** scheduled 那一发。且这一发与档 1
/// 的那一发**逐字节相等**（含 seq/type/data）⇒ 档 2 与档 1 的差别只在「少一发」。
#[test]
fn retry_level_two_leaves_the_row_scheduled_without_a_started_frame() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = retry_turn("s-9604", 2, want);
    assert_eq!(frames.len(), want, "档 2 一轮 37 帧");
    assert_eq!(events.len(), 31, "档 2 一轮 31 条 journal 事件");
    assert_eq!(
        retry_events(&events, "llm/retry-started").len(),
        0,
        "档 2 演的是等待被中止：started 永不落盘"
    );
    let mut expected = retry_frame_tags(1);
    expected.retain(|text| text != "event:llm/retry-started");
    assert_eq!(frames.iter().map(tag).collect::<Vec<_>>(), expected);
    let scheduled = only_retry(&events, "llm/retry");
    assert_eq!(sorted_keys(&scheduled["data"]), SCHEDULED_KEYS_NORMAL);
    assert_eq!(scheduled["data"]["retryId"], json!("retry-1-chain-1"), "档 2 仍带 id");
    // 与档 1 那一发逐字节对照：剥掉墙钟 time 后整格相等。
    let (_, one_events) = retry_turn("s-9605", 1, TURN_FRAMES + 2);
    let same_at_level_one = only_retry(&one_events, "llm/retry");
    assert_eq!(
        json_without_time(std::slice::from_ref(&scheduled)),
        json_without_time(std::slice::from_ref(&same_at_level_one)),
        "档 2 的 scheduled 与档 1 那一发该逐字节相等 ⇒ 两档只差后面跟不跟 started"
    );
}

/// **档 3 = K2+K3**：`mode:"always"` 且**按内核规格必须省** `maxRetries`
/// （`invariant.js:55`、format `:383`）。这一档是唯一能把读者 `maximum` 的 `∞` 分支
/// 喂出输入的那一档 ⇒ 线上一旦带上 maxRetries 就是内核自己会拒的形状。
#[test]
fn retry_level_three_omits_max_retries_because_always_mode_must() {
    let want = TURN_FRAMES + 2;
    let (frames, events) = retry_turn("s-9606", 3, want);
    assert_eq!(frames.len(), want, "档 3 一轮 38 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        retry_frame_tags(3),
        "档 3 也是两发：scheduled + started"
    );
    let data = only_retry(&events, "llm/retry")["data"].clone();
    assert_eq!(sorted_keys(&data), SCHEDULED_KEYS_ALWAYS, "档 3 恰九枚");
    assert!(
        data.get("maxRetries").is_none(),
        "always 档多带 maxRetries 就是内核会拒的形状（`always mode must omit maxRetries`）"
    );
    assert_eq!(data["mode"], json!("always"));
    // `policyKey` 就是策略数组的 JSON.stringify ⇒ 串里带引号与方括号是形状不是转义 bug。
    assert_eq!(
        data["policyKey"],
        json!(r#"["always",1000,30000,0.2]"#),
        "always 档的策略串以 [\"always\" 起头，且整枚逐字节可比"
    );
    // 与档 1 的键集差**恰为** maxRetries 一枚（别的九枚一枚不许变）。
    let (_, one_events) = retry_turn("s-9607", 1, TURN_FRAMES + 2);
    let normal = only_retry(&one_events, "llm/retry")["data"].clone();
    let mut minus_max = sorted_keys(&normal);
    minus_max.retain(|key| *key != "maxRetries");
    assert_eq!(sorted_keys(&data), minus_max, "两档键集之差恰为 maxRetries 一枚");
    // 两发仍同链：省一枚可选键不许把配对判据一起省掉。
    let begun = only_retry(&events, "llm/retry-started");
    assert_eq!(begun["data"]["retryId"], data["retryId"]);
    assert_eq!(begun["data"]["turn"], json!(1));
    assert_eq!(sorted_keys(&begun["data"]), STARTED_KEYS, "started 恒四枚，与 mode 无关");
    assert_eq!(extra_keys(&data, &RETRY_ALLOWED_KEYS), Vec::<String>::new());
}

/// **档 4 = K5，仓内录制形**：真 socket 上这一发 `llm/retry` **压根没有 `retryId` 这枚键**。
/// 这条的价值：证「线上真的可以不带 id」——`dsh-client-connection/lib/client.js` 的 fixture
/// 就发过这一型，而补 id 是 `normalizeLegacyRetry` 在 v0→v1 迁移时才做的事（v1 内核自产的
/// 档 1/2/3 一律带 id）。
/// ⚠ 断言只打在 **wire 层**。读者侧那枚回落串 `"retry"+envTime+turn` 属 `kernel.rs` 的
/// `note_llm_retry`（不是本片所有、也不在本片门禁里）⇒ 已写进报告 §7 另派。
#[test]
fn retry_level_four_puts_a_retry_id_free_frame_on_the_real_socket() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = retry_turn("s-9608", 4, want);
    assert_eq!(frames.len(), want, "档 4 一轮 37 帧");
    let scheduled = only_retry(&events, "llm/retry");
    let data = &scheduled["data"];
    // `Value::get` 给 `None` 只可能是「键不存在」——键在而值为 null 会给 `Some(Value::Null)`。
    assert!(data.get("retryId").is_none(), "档 4 的 wire data 不许有 retryId 键");
    assert_eq!(sorted_keys(data), SCHEDULED_KEYS_LEGACY, "缺 id 的那一发仍恰九枚");
    assert_eq!(
        extra_keys(data, &RETRY_ALLOWED_KEYS),
        Vec::<String>::new(),
        "档 4 不许有十枚权威键之外的东西"
    );
    // 集合等式的另一头：缺的**恰为** retryId 一枚，别的必填八枚一枚不少（不是空壳帧）。
    let missing: Vec<&str> = RETRY_REQUIRED_KEYS
        .iter()
        .copied()
        .filter(|key| data.get(*key).is_none())
        .collect();
    assert_eq!(missing, ["retryId"], "档 4 只缺 retryId 这一枚必填键");
    let envelope = scheduled.to_string();
    assert!(
        !envelope.contains("retryId"),
        "整格序列化里连键名都不许出现（不是被谁在客户端剥掉的）：{envelope}"
    );
    // 剩下的九枚一枚不许退化：证这是「少一键的完整帧」，不是空壳。
    assert_eq!(data["mode"], json!("normal"));
    assert_eq!(data["maxRetries"], json!(2));
    assert_eq!(data["delayMs"], json!(300));
    assert_eq!(data["retry"], json!(1), "档 4 的 retry 仍是数值（非数值是档 5 的活）");
    assert_eq!(data["step"], json!(1));
    assert_eq!(data["turn"], json!(1));
    assert_eq!(sorted_keys(&data["failure"]), FAILURE_KEYS);
    // started 仍零发：它的读者（主干 `:616`）没有同款回落，补出来的孤儿帧内核自己会拒。
    assert_eq!(retry_events(&events, "llm/retry-started").len(), 0);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        retry_frame_tags(4),
        "帧序只在 attempt 后多一格"
    );
}

/// **档 5**：`retry` 上线是**字符串** `"9"`。任何真生产者都发不出这一型
/// （`invariant.js:45` 要正安全整数；连档 4 那枚录制形都还是数值）。
/// 断言只打在「桩确实把非数值发上了线」这一件可证的事上 ⇒ 它是主干/分叉那枚
/// 「`retry` 不是 Number 就按 1」的**防御臂的可证性**，**不是**内核会发的形状，
/// 更不构成「线上该这么发」的判据。
#[test]
fn retry_level_five_sends_a_non_numeric_retry_no_producer_would() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = retry_turn("s-9609", 5, want);
    assert_eq!(frames.len(), want, "档 5 一轮 37 帧");
    let data = only_retry(&events, "llm/retry")["data"].clone();
    let retry = &data["retry"];
    assert!(
        retry.is_string() && !retry.is_number(),
        "`retry` 上线必须是非数值，防御臂才有输入：{retry}"
    );
    assert_eq!(retry, &json!("9"), "取 9 不取 1 ⇒ 读者真去 parse 就会显 9/2，糊不过去");
    assert_ne!(retry, &json!(9), "字符串 9 与数值 9 在 wire 上是两个型");
    assert!(data.get("retryId").is_none(), "档 5 = 档 4 的形状 + 非数值 retry");
    assert_eq!(sorted_keys(&data), SCHEDULED_KEYS_LEGACY, "键集与档 4 同（九枚）");
    assert_eq!(data["maxRetries"], json!(2), "非数值只污染 retry 一枚");
    // 反向护栏：把 retry 换回数值 1 后该与档 4 逐字节相等 ⇒ 别的字段一点没跟着动。
    let mut normalized = data.clone();
    normalized["retry"] = json!(1);
    let (_, four_events) = retry_turn("s-9610", 4, TURN_FRAMES + 1);
    let legacy = only_retry(&four_events, "llm/retry")["data"].clone();
    assert_eq!(
        serde_json::to_string(&normalized).unwrap(),
        serde_json::to_string(&legacy).unwrap(),
        "归一化 retry 后档 5 该与档 4 逐字节相等"
    );
    assert_eq!(retry_events(&events, "llm/retry-started").len(), 0, "档 5 也不发 started");
    assert_eq!(frames.iter().map(tag).collect::<Vec<_>>(), retry_frame_tags(5));
}

/// **配对不变量跨档**（内核 `invariant.js:40-82`）：档 1/3 那两发的 `retryId` 非空、
/// `retry` 是正整数、两发落在**开着的 turn/step** 之内（`turn/start` 之后、`turn/end` 之前），
/// 全轮 seq 连号且信封 `time` 不倒走，同 `retryId` 的 started 恰一发（不许重复发）。
#[test]
fn the_retry_pair_stays_inside_the_open_turn_and_keeps_the_chain_whole() {
    for level in [1u64, 3] {
        let (frames, events) = retry_turn(&format!("s-961{level}"), level, TURN_FRAMES + 2);
        let scheduled = only_retry(&events, "llm/retry");
        let started = only_retry(&events, "llm/retry-started");
        let data = &scheduled["data"];
        assert!(
            data["retryId"].as_str().is_some_and(|id| !id.is_empty()),
            "档 {level}：retryId 得是非空串（invariant.js:42）"
        );
        assert!(
            data["retry"].is_number() && data["retry"].as_i64().unwrap() >= 1,
            "档 {level}：retry 得是正整数（invariant.js:45）⇒ 这一条在档 5 上当然不成立"
        );
        assert_eq!(data["step"], json!(1), "档 {level}：step 得是当期开着的 step");
        assert_eq!(data["turn"], json!(1), "档 {level}：data.turn 得等于外层那一轮");
        // 开着的 turn：两发都夹在 turn/start 与 turn/end 之间。
        let start = single_event(&events, "turn/start");
        let end = single_event(&events, "turn/end");
        let inside = [
            scheduled["seq"].as_i64().unwrap(),
            started["seq"].as_i64().unwrap(),
        ];
        assert!(
            inside.iter().all(|seq| *seq > start["seq"].as_i64().unwrap()
                && *seq < end["seq"].as_i64().unwrap()),
            "档 {level}：重试帧该落在开着的 turn 内，实际 {inside:?}"
        );
        assert_eq!(
            data["turn"], start["data"]["turn"],
            "档 {level}：data.turn 与 turn/start 报的轮号得是同一枚"
        );
        // 同链只一发 started（invariant.js:75-82）。
        let same_chain: Vec<Value> = retry_events(&events, "llm/retry-started")
            .into_iter()
            .filter(|event| event["data"]["retryId"] == data["retryId"])
            .collect();
        assert_eq!(same_chain.len(), 1, "档 {level}：同 retryId 的 started 不许重复发");
        // 全轮 seq 连号 + 信封 time 不倒走（插刀必须重排后续信封）。
        let first = events[0]["seq"].as_i64().unwrap();
        for (index, event) in events.iter().enumerate() {
            assert_eq!(
                event["seq"].as_i64(),
                Some(first + index as i64),
                "档 {level}：第 {index} 格 seq 断号 ⇒ 插刀后没重排后续信封"
            );
        }
        for pair in events.windows(2) {
            assert!(
                pair[0]["time"].as_i64().unwrap() <= pair[1]["time"].as_i64().unwrap(),
                "档 {level}：信封 time 不许倒走"
            );
        }
        // 帧序与 journal 事件序同口径（两发都真上了 follow 流）。
        let at = tag_index(&frames, "event:llm/retry");
        assert_eq!(tag(&frames[at + 1]), "event:llm/retry-started");
    }
}

/// **键集权威形状的集合等式（逐档）**：每档三张表——`llm/retry` / `llm/retry-started` /
/// `failure`——全按「双向都不许多」断，不是「这几枚键在」。`failure` 的三枚可选键
/// （`status` / `providerRetryAfterMs` / `requestId`）内核**允许**、主干**不读** ⇒
/// 桩一枚都不许上线（带上去就是没人看的字节，还会把集合等式打歪）。
#[test]
fn the_retry_key_sets_carry_no_extra_keys_at_any_level() {
    // (档, 该档 scheduled 的线上键集, 有没有 started, 该档一轮多几帧) —— 后两颗各自写死，
    // 不许从键数推：档 3 只九枚键但仍发两帧，档 4/5 少的是 retryId 那一枚键。
    let table: [(u64, &[&str], bool, usize); 5] = [
        (1, &SCHEDULED_KEYS_NORMAL, true, 2),
        (2, &SCHEDULED_KEYS_NORMAL, false, 1),
        (3, &SCHEDULED_KEYS_ALWAYS, true, 2),
        (4, &SCHEDULED_KEYS_LEGACY, false, 1),
        (5, &SCHEDULED_KEYS_LEGACY, false, 1),
    ];
    for (level, want_keys, has_started, extra_frames) in table {
        let (frames, events) = retry_turn(&format!("s-962{level}"), level, TURN_FRAMES + extra_frames);
        assert_eq!(
            frames.len(),
            TURN_FRAMES + extra_frames,
            "档 {level}：一轮该多 {extra_frames} 帧（现测，见报告 §2）"
        );
        let data = only_retry(&events, "llm/retry")["data"].clone();
        assert_eq!(sorted_keys(&data), want_keys.to_vec(), "档 {level} 的 scheduled 键集");
        assert_eq!(
            extra_keys(&data, &RETRY_ALLOWED_KEYS),
            Vec::<String>::new(),
            "档 {level}：必填九枚之外只许有 maxRetries"
        );
        let failure = &data["failure"];
        assert_eq!(sorted_keys(failure), FAILURE_KEYS, "档 {level}：failure 恰两枚必填");
        assert_eq!(extra_keys(failure, &FAILURE_ALLOWED), Vec::<String>::new());
        for optional in ["status", "providerRetryAfterMs", "requestId"] {
            assert!(
                failure.get(optional).is_none(),
                "档 {level}：可选键 {optional} 不许上线（主干不读它）"
            );
        }
        assert!(
            failure["message"].is_string() && failure["code"].is_string(),
            "档 {level}：两枚必填得都是非空串"
        );
        let started = retry_events(&events, "llm/retry-started");
        assert_eq!(started.len(), usize::from(has_started), "档 {level} 的 started 发数");
        for event in &started {
            assert_eq!(
                sorted_keys(&event["data"]),
                STARTED_KEYS.to_vec(),
                "档 {level}：started 恰四枚、可选空"
            );
            assert_eq!(extra_keys(&event["data"], &STARTED_KEYS), Vec::<String>::new());
        }
    }
}

/// **三把旋钮相加、不是替代**：`--inject=1 --trunc=2 --retry=1` 同开时预算是
/// 36 + 5(注入) + 1(flat attempt) + 2(重试两发) = **44 帧 / 38 事件**（现测）。
/// 更要紧的是**切点**：重试两发紧跟的是既有那发**失败的** attempt，不是 trunc 补的那一发
/// flat（内核是先落 request-error 才进 backoff）。
#[test]
fn the_retry_frames_add_to_the_injection_and_truncation_budgets() {
    let want = TURN_FRAMES + INJECTED_FRAMES + 1 + 2;
    let knobs = ["--inject=1", "--trunc=2", "--retry=1"];
    let (frames, events) = retry_turn_argv("s-9631", &knobs, want);
    assert_eq!(frames.len(), want, "三把同开一轮 44 帧");
    assert_eq!(events.len(), 38, "三把同开一轮 38 条 journal 事件");
    // 期望帧序 = 注入五格 + 重试两发 + trunc 那一发 flat，各自算下标。
    let injected: Vec<String> = injected_turn_tags();
    let mut expected = with_retry_tags(&injected, 1);
    let at = expected
        .iter()
        .position(|text| text == "event:assistant/message")
        .expect("关档帧序里得有 assistant/message");
    expected.insert(at, "event:assistant/attempt".to_string());
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        expected,
        "开 inject/trunc 时重试帧仍只多两格"
    );
    // 切点：journal 里紧跟**第一发** attempt（失败那发）的才是 llm/retry。
    let attempts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event["type"].as_str() == Some("assistant/attempt"))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(attempts.len(), 2, "档 trunc=2 该有两发 attempt");
    assert_eq!(
        events[attempts[0] + 1]["type"],
        json!("llm/retry"),
        "重试帧该紧跟失败的那一发，不是 flat 那一发"
    );
    assert_ne!(
        events[attempts[1] + 1]["type"],
        json!("llm/retry"),
        "flat attempt 之后不许再挂重试帧"
    );
    // 两发同链在多旋钮同开时仍成立。
    let scheduled = only_retry(&events, "llm/retry");
    let started = only_retry(&events, "llm/retry-started");
    assert_eq!(scheduled["data"]["retryId"], started["data"]["retryId"]);
    // 回读那一路吃的是同一台机器：三轮各 38 事件 ⇒ 种子页 114。
    assert_eq!(
        retry_seed_argv(&knobs).len(),
        3 * 38,
        "种子页与 live 同档 ⇒ 90 + 3×(5+1+2) = 114"
    );
}

/// **回读同形 + 每档预算的第二种量法**：`session/page` 走 `seed_journal`（与 live 同档），
/// 种子会话三轮的事件数 = 3 × 该档每轮事件数 ⇒ 现测 90/96/93/96/93/93。
/// 并钉「每一轮各带自己的链」：`retryId` 按轮号编 ⇒ 三链互不污染；档 1 的回读那一发
/// 与 live 那一发（同为 turn 1）**逐字节同形**。
#[test]
fn the_retry_levels_read_back_from_history_matching_the_live_shape() {
    let table: [(u64, usize, usize, usize); 6] = [
        // 档, 种子页事件, scheduled 发数, started 发数
        (0, 90, 0, 0),
        (1, 96, 3, 3),
        (2, 93, 3, 0),
        (3, 96, 3, 3),
        (4, 93, 3, 0),
        (5, 93, 3, 0),
    ];
    for (level, want_events, want_scheduled, want_started) in table {
        let events = retry_seed_events(level);
        assert_eq!(
            events.len(),
            want_events,
            "档 {level}：种子页三轮事件数 = 3 × 该档每轮事件数"
        );
        let scheduled = retry_events(&events, "llm/retry");
        let started = retry_events(&events, "llm/retry-started");
        assert_eq!(scheduled.len(), want_scheduled, "档 {level}：回读 scheduled 发数");
        assert_eq!(started.len(), want_started, "档 {level}：回读 started 发数");
        let ids: Vec<&str> = scheduled
            .iter()
            .filter_map(|event| event["data"]["retryId"].as_str())
            .collect();
        // 档 4/5 是缺 id 的那两档 ⇒ 带 id 的发数为 0。
        let want_ids = if matches!(level, 1 | 2 | 3) { want_scheduled } else { 0 };
        assert_eq!(ids.len(), want_ids, "档 {level}：带 retryId 的那几发数");
        if want_ids > 0 {
            assert_eq!(
                ids,
                ["retry-1-chain-1", "retry-2-chain-1", "retry-3-chain-1"],
                "档 {level}：三轮各带自己的链，不许共用一枚 id"
            );
        }
    }
    // live 与回读同形：同为 turn 1 的那一发，data 逐字节相等。
    let seed = retry_seed_events(1);
    let seed_first = retry_events(&seed, "llm/retry")
        .into_iter()
        .next()
        .expect("种子页三轮各一发");
    let (_, live) = retry_turn("s-9632", 1, TURN_FRAMES + 2);
    assert_eq!(
        serde_json::to_string(&seed_first["data"]).unwrap(),
        serde_json::to_string(&only_retry(&live, "llm/retry")["data"]).unwrap(),
        "切会话回读与当场推帧该演出同一型（否则 started 找不到回读那行的键）"
    );
}

/// **未定义档与脏值一律回落 0**：`--retry=6` / `=99` / `=nope` / `=-1` 在线上与**不开那枚
/// 旋钮**逐字节同形（帧序 36、事件 30、整轮序列化里连 `llm/retry` 子串都没有）。
/// 现测过这四颗值，才敢说「未定义档不会演出第三种形状」。
#[test]
fn an_undefined_or_dirty_retry_level_falls_back_to_the_off_shape() {
    let (_, unflagged) = retry_turn_argv("s-9633", &[], TURN_FRAMES);
    for knob in ["--retry=6", "--retry=99", "--retry=nope", "--retry=-1"] {
        let (frames, events) = retry_turn_argv("s-9634", &[knob], TURN_FRAMES);
        assert_eq!(frames.len(), TURN_FRAMES, "{knob} 一轮仍 36 帧");
        assert_eq!(
            frames.iter().map(tag).collect::<Vec<_>>(),
            turn_tags(),
            "{knob} 的帧序该与关档逐格相等"
        );
        assert_eq!(
            json_without_time(&events),
            json_without_time(&unflagged),
            "{knob} 剥掉墙钟 time 后该与「不开旋钮」逐字节相等"
        );
        assert!(
            !frames
                .iter()
                .map(Value::to_string)
                .collect::<String>()
                .contains("llm/retry"),
            "{knob} 不许演出既不是关档也不是任何一档的第三种形状"
        );
    }
}

// ===========================================================================
// #128（SK3）· `settings/describe` 台账补全后的三条真 socket 用例
//
// 分叉的设置页初值全部来自这一次 describe：#109 的读取侧照 §5 的公开形状写，
// 所以这里钉的是「13 支全在、每支八键齐、主干实读的字段真在 value 里、写端点对新 ns
// 真的落账、台账扩容没有把模型发现的门顺手撑大」。
// ===========================================================================

/// `settings/describe` 的八键——真内核 `namespaceView` 就产这几把
/// （`dsh-api-settings-controller/lib/index.js:275-289`），且线 codec 只把
/// `base`/`user` 标了 `.optional()`（`dsh-api-remotes/lib/client.js:4716-4752`）。
const NAMESPACE_VIEW_KEYS: &[&str] = &[
    "ns", "schema", "value", "base", "user", "applies", "secrets", "revision",
];

/// (a) 台账 13 支全覆盖 + 每支八键齐；(b) 主干实读的那几根字段真在对应 `value` 里。
#[test]
fn settings_describe_ledger_covers_every_trunk_read_namespace() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let described = kernel
        .call("settings/describe", json!({}))
        .expect("一次回全部命名空间");
    let namespaces = described["namespaces"]
        .as_array()
        .expect("缺 namespaces 主干就当整次失败");

    // 主干 `MainWindow.xaml.cs:10625-10630` 的 SectionNamespaces 四分区 1:1。
    // 断言的是**集合**而非顺序：主干按 ns 名建字典（`_settingsSnapshot`），顺序对它无意义；
    // 但桩侧顺序另有既有下标用例要守，所以这里刻意不钉顺序、只钉「一支不多一支不少」。
    let mut listed: Vec<&str> = namespaces
        .iter()
        .map(|row| row["ns"].as_str().expect("每支都得有 ns 字符串"))
        .collect();
    let expected = [
        "llm-deepseek",
        "llm-pi-ai",
        "agent-presets",
        "ui-theme",
        "locale",
        "ui-chat",
        "ui-conversation",
        "permission",
        "agent-default-model",
        "shell",
        "agent-loop",
        "subagent-model-selection",
        "web-search-deepseek",
    ];
    listed.sort_unstable();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_unstable();
    assert_eq!(
        listed, expected_sorted,
        "台账必须与主干 SectionNamespaces 逐支对齐，多一支都是演真内核没有的能力"
    );
    assert_eq!(namespaces.len(), 13);

    // 八键齐 —— #109 的读取侧照这八把键取值，缺一把它就整段空态。
    for row in namespaces {
        let ns = row["ns"].as_str().expect("ns");
        let keys: Vec<&str> = row
            .as_object()
            .expect("namespaces 元素必须是对象")
            .keys()
            .map(String::as_str)
            .collect();
        for want in NAMESPACE_VIEW_KEYS {
            assert!(keys.contains(want), "{ns} 的视图缺八键里的 {want}：{keys:?}");
        }
        assert_eq!(
            keys.len(),
            NAMESPACE_VIEW_KEYS.len(),
            "{ns} 的视图不该长第八把以外的键（主干按八键 strict 读）：{keys:?}"
        );
        assert_eq!(row["applies"], json!("live"), "{ns} 生效时机");
        assert!(row["schema"].is_object(), "{ns} 的 schema 必须是对象");
        assert_eq!(
            row["schema"]["type"],
            json!("object"),
            "{ns} 的 schema 根节点是 object（主干 SchemaNodeAt 从 dict 下钻）"
        );
        assert!(
            row["schema"]["dict"].is_object(),
            "{ns} 的具名字段挂在 schema.dict 下"
        );
        assert!(row["value"].is_object(), "{ns} 的 value 从来不会是 null");
        assert!(row["secrets"].is_array(), "{ns} 的 secrets 是数组");
        assert_eq!(
            row["revision"].as_f64(),
            Some(0.0),
            "{ns} 的 revision 起点 0（真内核 dsh-settings/lib/index.js:428）"
        );
    }

    // (b) 主干实读的字段 —— `NsString/NsNumber/NsBool("<ns>", "<field>")` 那几对，
    // 逐支必须在 value 里存在，否则设置页初值是兜底值而不是快照值。
    for (ns, field) in [
        ("ui-theme", "preference"),
        ("ui-theme", "fontSize"),
        ("ui-chat", "transcriptView"),
        ("ui-conversation", "busyEnter"),
        ("permission", "defaultPreset"),
        ("agent-default-model", "provider"),
        ("agent-default-model", "model"),
        ("shell", "timeoutMs"),
        ("shell", "maxOutputBytes"),
        ("agent-loop", "maxParallelToolCalls"),
        ("web-search-deepseek", "maxUses"),
        ("web-search-deepseek", "apiKeyEnv"),
        ("web-search-deepseek", "baseURL"),
        ("subagent-model-selection", "enabled"),
        ("subagent-model-selection", "allowedModels"),
    ] {
        let row = namespaces
            .iter()
            .find(|row| row["ns"] == json!(ns))
            .unwrap_or_else(|| panic!("台账里没有 {ns}"));
        assert!(
            row["value"].get(field).is_some(),
            "{ns}.{field} 必须在 value 里，否则 #109 的初值只能走兜底"
        );
        assert!(
            row["schema"]["dict"].get(field).is_some(),
            "{ns}.{field} 必须同时在 schema.dict 里，主干渲染控件要读它的类型"
        );
    }

    // locale 是唯一一根**不该**出现在 value 里的实读字段：内核侧
    // `z.string().pattern(LOCALE_ID_PATTERN).required(false)`（dsh-client-locale/lib/index.js:13）
    // 无默认、注册时也不带 base ⇒ 真内核的 value 里它就是缺席。
    // 这里反向钉住，免得后来人「为了让字段清单齐」给它塞一个假默认语言。
    let locale = namespaces
        .iter()
        .find(|row| row["ns"] == json!("locale"))
        .expect("locale 在台账上");
    assert!(
        locale["schema"]["dict"].get("preference").is_some(),
        "locale.preference 得在 schema 里，主干才知道有这么一根可写的键"
    );
    assert!(
        locale["value"].get("preference").is_none(),
        "locale.preference 不许有假默认：内核侧它是 optional"
    );

    // 联合选项必须长成主干 `UnionChoices`（MainWindow.xaml.cs:10007）认的形状。
    let theme = namespaces
        .iter()
        .find(|row| row["ns"] == json!("ui-theme"))
        .expect("ui-theme 在台账上");
    let choices: Vec<&str> = theme["schema"]["dict"]["preference"]["list"]
        .as_array()
        .expect("preference 是 union，选项挂在 list")
        .iter()
        .map(|choice| {
            assert_eq!(choice["type"], json!("const"), "union 的成员是 const 节点");
            choice["value"].as_str().expect("const 的 value")
        })
        .collect();
    assert_eq!(choices, vec!["light", "dark", "system"], "照 dsh-client-ui-theme 的 THEME_PREFERENCES");
    assert_eq!(
        theme["schema"]["dict"]["preference"]["meta"]["default"],
        json!("system"),
        "默认值得挂主干 SchemaDefaultAt 唯一认的 meta.default"
    );

    // secret 侧车：`apiKey` 在 dsh-web-search-deepseek 里是 `role("secret")`，
    // 内核的走查器把它从 value/base/user 全剔掉、只在 secrets 里留位置
    // （dsh-settings/lib/index.js:19-25 + :63-65 的「缺席也枚举」）。
    let search = namespaces
        .iter()
        .find(|row| row["ns"] == json!("web-search-deepseek"))
        .expect("web-search-deepseek 在台账上");
    assert!(
        search["schema"]["dict"].get("apiKey").is_none(),
        "secret 字段不进 schema.dict：回线形状里没有它的位置"
    );
    assert!(
        search["value"].get("apiKey").is_none(),
        "secret 值绝不回线"
    );
    assert_eq!(
        search["secrets"],
        json!([{ "path": ["apiKey"], "set": false }]),
        "缺席的 secret 也要枚举出位置，配置面才渲染得出那根只写输入框"
    );
    kernel.shutdown();
}

/// (c) 新补的 ns 也走完整写往返：`settings/update` / `settings/mutate` → revision 前进 →
/// 重新 describe 读到新值，且 `base` 层不动（「已覆盖」标记就靠 base≠user 判）。
#[test]
fn newly_ledgered_namespaces_round_trip_writes_and_advance_revisions() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let row_of = |kernel: &mut Kernel, want: &str| -> Value {
        kernel
            .call("settings/describe", json!({}))
            .expect("describe")
            .as_object()
            .expect("对象")
            .get("namespaces")
            .and_then(Value::as_array)
            .expect("namespaces 数组")
            .iter()
            .find(|row| row["ns"] == json!(want))
            .cloned()
            .unwrap_or_else(|| panic!("台账里没有 {want}"))
    };

    // settings/update（patch）—— ui-theme.fontSize，主干「字号」那一行写的就是它。
    let before = row_of(&mut kernel, "ui-theme");
    assert_eq!(before["revision"].as_f64(), Some(0.0), "起点 0（真内核 `:428`）");
    assert_eq!(before["value"]["fontSize"], json!(14), "初值 = dsh-client-ui-theme 的 DEFAULT_FONT_SIZE");
    let updated = kernel
        .call(
            "settings/update",
            json!({ "ns": "ui-theme", "patch": { "fontSize": 16 } }),
        )
        .expect("新 ns 的写端点该放行");
    assert_eq!(updated["ns"], json!("ui-theme"));
    assert_eq!(updated["revision"].as_f64(), Some(1.0), "提交一次 revision +1（0→1）");
    assert_eq!(updated["value"]["fontSize"], json!(16));
    assert_eq!(
        updated["user"]["fontSize"], json!(16), "写落在用户层"
    );
    assert_eq!(
        updated["base"]["fontSize"], json!(14),
        "base 层不许被写动——主干 IsNsFieldOverridden 就靠 base≠user 打「已覆盖」"
    );
    let after = row_of(&mut kernel, "ui-theme");
    assert_eq!(after["value"]["fontSize"], json!(16), "重新 describe 必须读到新值");
    assert_eq!(after["revision"].as_f64(), Some(1.0));
    assert_eq!(
        after["value"]["preference"], json!("system"),
        "同 ns 的没写过的字段仍继承 base，patch 不许把它抹掉"
    );

    // settings/mutate（path 寻址）—— shell.maxOutputBytes。
    let mutated = kernel
        .call(
            "settings/mutate",
            json!({
                "ns": "shell",
                "ops": [{ "op": "set", "path": ["timeoutMs"], "value": 30000 }],
                "expectedRevision": 0.0,
            }),
        )
        .expect("shell 现在在台账上，path 写该放行");
    assert_eq!(mutated["value"]["timeoutMs"], json!(30_000));
    assert_eq!(mutated["revision"].as_f64(), Some(1.0));
    assert_eq!(
        mutated["value"]["maxOutputBytes"], json!(64_000),
        "dsh-bash-local 的默认值不许被同 ns 的另一根写波及"
    );

    // 陈旧写仍撞在 revision 上：乐观锁对新 ns 一样生效。
    // 上面那笔 mutate 把 shell 从 0 推到 1，故「读到的旧值」是 0，不是 1。
    let stale = kernel
        .call(
            "settings/update",
            json!({ "ns": "shell", "patch": { "maxOutputBytes": 1 }, "expectedRevision": 0.0 }),
        )
        .expect_err("陈旧读必须撞锁");
    assert!(
        stale.contains("settings/conflict")
            && stale.contains("expectedRevision 0")
            && stale.contains("实际 1"),
        "{stale}"
    );
    assert_eq!(
        row_of(&mut kernel, "shell")["revision"].as_f64(),
        Some(1.0),
        "撞锁的写不入账"
    );

    // 每支 ns 各记各的账：写 ui-theme/shell 不许把别的推进。
    assert_eq!(row_of(&mut kernel, "agent-loop")["revision"].as_f64(), Some(0.0));
    assert_eq!(row_of(&mut kernel, "llm-deepseek")["revision"].as_f64(), Some(0.0));
    kernel.shutdown();
}

/// (d) 不在台账上的 ns 仍拒 + (e) **反向哨兵**：台账从 3 扩到 13 之后，
/// `llm/discoverModels` 依旧只认 `DISCOVER_NS` 那两支。
/// 这条是拆表的全部意义——不钉住它，扩台账就等于给 11 支 ns 演出了真内核没有的模型发现能力。
#[test]
fn ledger_growth_never_widens_model_discovery_or_write_acceptance() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    let namespaces = kernel
        .call("settings/describe", json!({}))
        .expect("describe")["namespaces"]
        .clone();
    let listed: Vec<&str> = namespaces
        .as_array()
        .expect("namespaces 是数组")
        .iter()
        .map(|row| row["ns"].as_str().expect("ns"))
        .collect();
    assert_eq!(listed.len(), 13, "先确认台账真的扩了，否则下面那圈断言是空跑");

    // 台账里除两支 llm 之外的 11 支，逐支发 discoverModels 都必须被拒。
    for ns in listed
        .iter()
        .filter(|ns| **ns != "llm-deepseek" && **ns != "llm-pi-ai")
    {
        let error = kernel
            .call(
                "llm/discoverModels",
                json!({ "settingsNs": ns, "request": { "provider": "anything" } }),
            )
            .expect_err(&format!("{ns} 在台账上但没有 discovery，真内核不会给它拉模型"));
        assert!(
            error.contains("llm/model-discovery-rejected"),
            "{ns} 该回 llm/model-discovery-rejected，实际 {error}"
        );
    }

    // 两支该过的仍要过——否则上面的圈是靠「全拒」蒙对的。
    for ns in ["llm-deepseek", "llm-pi-ai"] {
        kernel
            .call(
                "llm/discoverModels",
                json!({ "settingsNs": ns, "request": { "provider": "anything" } }),
            )
            .unwrap_or_else(|error| panic!("{ns} 的发现注册不该被拆表拆掉：{error}"));
    }

    // 未知 ns 的两条既有拒绝路径，逐字未动。
    let error = kernel
        .call("settings/update", json!({ "ns": "ui-them", "patch": {} }))
        .expect_err("少一个 e 的 ns 不在台账上（这正是扩容后最容易误收的一类）");
    assert!(error.contains("settings/rejected"), "{error}");
    // 注意 ops 得给非空数组：桩的 `settings/mutate` 先做 wire 字段校验（空 ops 直接
    // `bad_args`），台账判据在那之后才轮到——这里要测的是后者。
    let error = kernel
        .call(
            "settings/mutate",
            json!({ "ns": "shell-x", "ops": [{ "op": "set", "path": ["timeoutMs"], "value": 1 }] }),
        )
        .expect_err("台账外 ns 的 path 写");
    assert!(error.contains("settings/rejected"), "{error}");
    let error = kernel
        .call("settings/replace", json!({ "ns": "settings.onboarding", "section": {} }))
        .expect_err("settings.onboarding 走的是另一条通道，不在 describe 台账上");
    assert!(error.contains("settings/rejected"), "{error}");

    // 拒绝消息里列的是台账，不是 discovery 表——两表分开后这条文案得跟着台账。
    let error = kernel
        .call("settings/update", json!({ "ns": "nope", "patch": {} }))
        .expect_err("未知 ns");
    assert!(
        error.contains("ui-theme") && error.contains("web-search-deepseek"),
        "拒绝文案该把可写的 13 支列全：{error}"
    );
    assert!(
        !error.contains("no model discovery"),
        "写端点的拒绝不许串到 discovery 的文案：{error}"
    );
    kernel.shutdown();
}

// ===========================================================================
// #131（SK4）· 假内核桩的两笔取证欠账
//
// X-2：内核 number 节点的区间线线键名 = `meta.min` / `meta.max` / `meta.step`
//      （@deepseek-ai/schemastery/lib/index.mjs:218-232 生成、:355-359 与 :386-392 校验）。
// X-1：`subagent-model-selection` 的两根键**不独立**——
//      dsh-tool-subagent/lib/model-selection-settings.js:87-90 的 `validate()` 拦
//      `enabled && allowedModels.length === 0`。
// 两笔的拒绝都走 `dsh-api-settings-controller/lib/index.js:572-579` 的 `settings/rejected`，
// message 是内核原文；且被拒的写既不落账也不推进 revision（dsh-settings/lib/index.js:461-466）。
// ===========================================================================

/// 从一次 describe 里取那一段 ns 视图（同 #128 那三条用例的口径，独立一份免得到处牵线）。
fn ns_row(kernel: &mut Kernel, want: &str) -> Value {
    kernel
        .call("settings/describe", json!({}))
        .expect("describe")["namespaces"]
        .as_array()
        .expect("namespaces 是数组")
        .iter()
        .find(|row| row["ns"] == json!(want))
        .unwrap_or_else(|| panic!("台账里没有 {want}"))
        .clone()
}

/// X-2 的线上半段：区间线得随 schema 一起回线（主干将来做前端夹取只认这几把键），
/// 而**取证到的否**（shell 那五根裸 `z.number().default()`）不许长出线来。
/// 线上半段的拒绝由 `a_wire_number_write_...` 那条钉。
#[test]
fn the_kernels_number_lines_ride_the_describe_wire_and_absent_lines_stay_absent() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let theme = ns_row(&mut kernel, "ui-theme");
    assert_eq!(
        theme["schema"]["dict"]["fontSize"]["meta"]["min"],
        json!(12),
        "字号下限来自 dsh-client-ui-theme/lib/index.js:27 的 .min(12)"
    );
    assert_eq!(theme["schema"]["dict"]["fontSize"]["meta"]["max"], json!(17));
    assert_eq!(theme["schema"]["dict"]["fontSize"]["meta"]["step"], json!(1));
    assert_eq!(
        theme["schema"]["dict"]["fontSize"]["meta"]["default"],
        json!(14),
        "加了区间线不许把既有的 meta.default 挤掉——主干 SchemaDefaultAt 只认它"
    );
    // 整数值别在回线上写成 `12.0`：内核侧存的就是 12。
    let raw = serde_json::to_string(&theme["schema"]["dict"]["fontSize"]["meta"]).expect("序列化");
    assert!(
        raw.contains("\"min\":12") && !raw.contains("12.0"),
        "meta 序列化该是整数形状：{raw}"
    );

    let shell = ns_row(&mut kernel, "shell");
    for field in ["timeoutMs", "maxTimeoutMs", "maxOutputBytes", "maxSpillBytes", "graceMs"] {
        for key in ["min", "max", "step"] {
            assert!(
                shell["schema"]["dict"][field]["meta"].get(key).is_none(),
                "dsh-bash-local/lib/index.js:129-134 没给 shell.{field} 任何区间线，回线里冒出 meta.{key} 就是造假形状"
            );
        }
    }
    kernel.shutdown();
}

/// X-2 的写侧半段：越界值**拒**而不是夹取，message 逐字是内核那句（含 `$.` 路径前缀），
/// 且这一笔不推进 revision、重新 describe 仍是默认值。
#[test]
fn a_wire_number_write_outside_the_line_is_refused_with_the_kernels_message() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let before = ns_row(&mut kernel, "ui-theme");
    assert_eq!(before["value"]["fontSize"], json!(14));
    assert_eq!(before["revision"], json!(0.0), "未写过的 ns 起于 0（真内核 index.js:428）");

    for (value, want) in [
        (json!(18), "$.fontSize expected number <= 17 but got 18"),
        (json!(11), "$.fontSize expected number >= 12 but got 11"),
        (json!(13.5), "$.fontSize expected number multiple of 1 but got 13.5"),
    ] {
        let error = kernel
            .call("settings/update", json!({ "ns": "ui-theme", "patch": { "fontSize": value } }))
            .expect_err(&format!("{value} 越界必须被拒"));
        assert!(
            error.contains("settings/rejected") && error.contains(&want),
            "{value} 该回 `settings/rejected` + 内核原文 {want}，实际 {error}"
        );
    }

    // 被拒的三笔一笔都没入账：值仍是 14、revision 仍是起点 0。
    let after = ns_row(&mut kernel, "ui-theme");
    assert_eq!(after["value"]["fontSize"], json!(14), "拒绝之后再 describe 不许读到越界值");
    assert_eq!(after["revision"], json!(0.0), "越界写不推进 revision");

    // 端点是闭区间（内核 `data > max` / `data < min` 才抛）：12 与 17 必须写得进去，
    // 否则上面那圈只是「永远拒」的假绿。
    for edge in [12, 17] {
        let ok = kernel
            .call("settings/update", json!({ "ns": "ui-theme", "patch": { "fontSize": edge } }))
            .unwrap_or_else(|error| panic!("端点 {edge} 合法，却被拒：{error}"));
        assert_eq!(ok["value"]["fontSize"], json!(edge));
    }
    assert_eq!(ns_row(&mut kernel, "ui-theme")["revision"], json!(2.0), "0 →(12) 1 →(17) 2");
    kernel.shutdown();
}

/// X-1：`enabled` / `allowedModels` 的耦合在线上也得成立，且判的是**合并后的整段**——
/// 主干那个开关单发 `{enabled:true}` 时就该撞（`allowedModels` 从 base 层继承空数组）。
/// 反向半边：关着开关的合法态与开着开关带 route 的合法态都必须放行。
#[test]
fn the_subagent_enabled_and_allowed_models_pairing_is_enforced_on_the_wire() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let coupling = "enabled subagent model selection requires at least one allowed model";

    let before = ns_row(&mut kernel, "subagent-model-selection");
    assert_eq!(before["value"]["enabled"], json!(false), "内核默认是关（model-selection-settings.js:45）");
    assert_eq!(before["value"]["allowedModels"], json!([]));

    // ① 整段替换出「开着 + 空数组」：内核 :89 的那句原文。
    let error = kernel
        .call(
            "settings/replace",
            json!({ "ns": "subagent-model-selection", "section": { "enabled": true, "allowedModels": [] } }),
        )
        .expect_err("真内核拒这一态，桩不能演成接受");
    assert!(
        error.contains("settings/rejected") && error.contains(coupling),
        "该回 settings/rejected + {coupling}，实际 {error}"
    );

    // ② 只 patch `enabled`：耦合判的是 base+user 合并值，所以照样拒；且两笔都不入账。
    let error = kernel
        .call(
            "settings/update",
            json!({ "ns": "subagent-model-selection", "patch": { "enabled": true } }),
        )
        .expect_err("单发开关也要撞耦合");
    assert!(error.contains(coupling), "{error}");
    let untouched = ns_row(&mut kernel, "subagent-model-selection");
    assert_eq!(untouched["revision"], json!(0.0), "两笔被拒的写都不入账");
    assert_eq!(untouched["value"]["enabled"], json!(false));
    assert_eq!(untouched["user"], json!({}), "用户层里不许留下半套 enabled");

    // ③ 合法态：开着 + 带一支 route ⇒ 提交，value 里真能看到那支。
    let opened = kernel
        .call(
            "settings/update",
            json!({
                "ns": "subagent-model-selection",
                "patch": {
                    "enabled": true,
                    "allowedModels": [{ "provider": "deepseek-official", "model": "deepseek-chat" }],
                },
            }),
        )
        .expect("开着开关带一支 route 是合法态");
    assert_eq!(opened["revision"], json!(1.0));
    assert_eq!(opened["value"]["allowedModels"][0]["model"], json!("deepseek-chat"));

    // ④ 关掉开关、把那支 route 一并撤掉：这是回到默认态，耦合不该拦它。
    //    （少了这一半，①②那两刀就是「永远拒」的假绿。）
    let closed = kernel
        .call(
            "settings/replace",
            json!({ "ns": "subagent-model-selection", "section": { "enabled": false, "allowedModels": [] } }),
        )
        .expect("关着开关空数组是内核自己的默认态");
    assert_eq!(closed["value"]["enabled"], json!(false));
    assert_eq!(closed["revision"], json!(2.0), "0 →(③ 合法开) 1 →(④ 关掉撤 route) 2");

    // ⑤ 同一份 validate 的另一拍：重复 route（:33-35 的 `subagent model selection repeats route "p/m"`）。
    let error = kernel
        .call(
            "settings/replace",
            json!({
                "ns": "subagent-model-selection",
                "section": {
                    "enabled": true,
                    "allowedModels": [
                        { "provider": "a", "model": "m" },
                        { "provider": "a", "model": "m" },
                    ],
                },
            }),
        )
        .expect_err("同一条 route 出现两次，真内核拒");
    assert!(
        error.contains("subagent model selection repeats route \"a/m\""),
        "{error}"
    );
    assert_eq!(ns_row(&mut kernel, "subagent-model-selection")["revision"], json!(2.0),
        "⑤ 那笔 validate 被拒的写不入账，仍停 2");
    kernel.shutdown();
}

// ==================== SS1：`--session-stats=` 三档与 `asOfSeq` 水位走真 socket ====================
//
// 取证与两处**已确证**的数据 bug 见 `rust/tmp/ss1-report.md` §1：
// ① `session_stats()` 在零轮那一格仍照发 `llmMs: 4200` 等五格非零常量 —— 内核算不出这种形状
//   （`dsh-session-stats/lib/types/projection.js:137-144` 的 `turns`/`steps` 唯一来源是
//   `step/end`，而 `:100-135` 那五格都要先有开着的 step 才入账 ⇒ `turns == 0` 而时长非零不可达）；
// ② `projections_block()` 的 `asOfSeq` 取的是「大纲末条 `turn/start` 的号」而不是折叠水位 ——
//   内核的 `asOfSeq` = `cursorBefore(session.seq)`（`dsh-session-projection/lib/index.js:153`），
//   `cursorBefore(offset) = offset === 0 ? -1 : offset - 1`（同文件 `:23-25`），`session.seq` 是
//   「下一条事件的号」（`dsh-session/lib/index.js:1129-1131`）⇒ 折到就是**末条事件的 seq**、
//   空日志是 **-1**。
// 桩内纯函数层由 `src/bin/fake_dsh.rs::session_stats_tier_tests` 钉；这一批只判**过线的那份字节**
// 与**分叉解析后的读数**（`session/list` 左栏那一路、`session/control` 的 baseline 那一路，
// 桩侧同一个 `projections_for` ⇒ 两门必须同源）。
//
// 档位为什么必须把「整块缺席」与「八个 0」分开演（消费侧判据，两侧各一条）：
// · 主干 `MainWindow.xaml.cs:3184` 的 `projValues.TryGetProperty("sessionStats")` 失败 ⇒ `stats`
//   成 `default` ⇒ `:3185` 的 `turns` 成 0 ⇒ `:3197` 的副标题 `turns > 0 ? … : ""` 收起 ——
//   两型表面上同形，但 `:3189` 把整份 `projValues` 原样 `Clone()` 进 `_sessionProjections`，
//   缓存里**有没有这一键**是投影补齐路径的读法分水岭；
// · 分叉 `kernel.rs:1028` 的 `self.stats = values["sessionStats"].is_object().then(…)` ⇒
//   缺键是 `None`、八键全 0 是 `Some(SessionStats::default())`：#58 那颗状态条要据此分
//   「内核没这条投影」与「跑过零轮」两态，桩把两型压成一样就是替视图层删掉这一支。

/// 一档 `--session-stats=` 下的三张种子行：typed 投影 + 线上 `values` 原样（按 items 序）。
/// 旋钮是**进程级**的（`main` 读一次存进 `FakeState`）⇒ 换档只能重起一条桩，判完即关。
fn session_rows_at(level: &str) -> (Vec<Projections>, Vec<Value>) {
    let mut kernel = Kernel::start(&launch_with_args(&[level])).expect("假内核应完成 dsh web: 握手");
    let raw = kernel
        .call("session/list", json!({ "_request": {} }))
        .expect("session/list 应成功");
    let typed = kernel
        .list_session_rows()
        .expect("typed 行应在")
        .into_iter()
        .map(|row| row.projections)
        .collect();
    kernel.shutdown();
    let values = raw["items"]
        .as_array()
        .expect("items 该是数组")
        .iter()
        .map(|item| item["projections"]["values"].clone())
        .collect();
    (typed, values)
}

/// 内核 `sessionStatsSchema` 的 strict 八键（`dsh-session-stats/lib/types/projection.js:27-35`）。
const STATS_KEYS: [&str; 8] = [
    "decodeMs",
    "decodeTokens",
    "llmMs",
    "steps",
    "toolMs",
    "ttftMs",
    "ttftSteps",
    "turns",
];

/// 零轮那一格的唯一自洽形状：八个 0（bug ① 的正面判据）。
fn zero_stats() -> Value {
    json!({
        "turns": 0,
        "steps": 0,
        "llmMs": 0,
        "toolMs": 0,
        "ttftMs": 0,
        "ttftSteps": 0,
        "decodeMs": 0,
        "decodeTokens": 0,
    })
}

/// 三档在线上分别是哪一型，以及「零轮 ⇒ 八个 0」这条自洽式（SS1 bug ①）。
/// 反向半边三刀：① 有 closed turn 的行在档 1 下字节**逐字不变**（补齐臂不是全局清零，
/// 更不是把有数的行也夹成 0）；② 档 1 只补 `sessionStats` 一键，`plan`/`permissions`/
/// `turnOutline` 仍缺席（不许顺手把别的容错臂也抹掉）；③ 档 2 摘键之后 `values` 其余五键
/// 与三刻度大纲照旧（「缺这一键」不等于「整块缺席」，分叉 `is_empty()` 仍为假）。
#[test]
fn the_session_stats_tier_decides_between_an_absent_block_and_eight_real_zeros() {
    let nonzero = SessionStats {
        turns: 3,
        steps: 6,
        llm_ms: 4200.0,
        tool_ms: 900.0,
        ttft_ms: 310.0,
        decode_ms: 3300.0,
        decode_tokens: 1580.0,
        ttft_steps: 3,
    };

    // ---- 档 0（= 默认 = 接手时的历史行为）：rich 行八键实值，空白行整块缺席 ----
    let (rows, values) = session_rows_at("--session-stats=0");
    assert_eq!(sorted_keys(&values[1]), ["title"], "档 0 的空白行只有 title");
    assert_eq!(rows[1].stats, None, "档 0：零轮那一格是「没这条投影」");
    assert_eq!(rows[0].stats, Some(nonzero), "档 0 的 rich 行照旧");
    assert_eq!(
        values[0]["sessionStats"],
        stats_raw(),
        "线上那一块与 typed 读数同源（默认档的字节一个都不动）"
    );

    // ---- 档 1：空白行补齐成八个 0（分叉读出 Some(default)，与档 0 的 None 分得开）----
    let (rows, values) = session_rows_at("--session-stats=1");
    assert_eq!(
        sorted_keys(&values[1]),
        ["sessionStats", "title"],
        "档 1 只补这一键，`values` 里不许冒出别的投影"
    );
    assert_eq!(sorted_keys(&values[1]["sessionStats"]), STATS_KEYS, "补齐的是 strict 八键，不是 `turns` 一格");
    assert_eq!(values[1]["sessionStats"], zero_stats(), "零轮 ⇒ 八个键全是 0");
    assert_eq!(
        rows[1].stats,
        Some(SessionStats::default()),
        "分叉侧这是「跑过零轮」那一型，不是「内核没这条投影」"
    );
    assert!(rows[1].turn_outline.is_empty(), "turns:0 与「大纲为空」自洽（同一处取数）");
    assert!(rows[1].plan.is_none() && rows[1].permissions.is_none(), "档 1 不顺手补别的投影");
    assert_eq!(
        rows[0].stats,
        Some(nonzero),
        "反向半边：档 1 不换数据源，rich 行的常数一格都不动"
    );
    assert_eq!(values[0]["sessionStats"], stats_raw(), "线上字节也一样逐字同档 0");
    assert_eq!(
        values[2]["sessionStats"]["turns"],
        json!(1),
        "s-1003 单轮种子在档 1 下仍报它自己的轮数（补齐臂不是把 !rich 之外的一律清零）"
    );

    // ---- 档 2：每一行都没有这一键（含 rich 行），主干据此走 `TryGetProperty` 失败那一臂 ----
    let (rows, values) = session_rows_at("--session-stats=2");
    for (index, item) in values.iter().enumerate() {
        assert!(
            item.get("sessionStats").is_none(),
            "档 2 第 {index} 行不许带 sessionStats: {item}"
        );
        assert!(rows[index].stats.is_none(), "档 2：typed 侧也该是 None 而不是零值");
    }
    assert_eq!(
        sorted_keys(&values[0]),
        ["permissions", "plan", "title", "todos", "turnOutline"],
        "摘掉一键之后其余五键照旧（缺键不炸、也不牵连别的键）"
    );
    assert_eq!(rows[0].turn_outline.len(), 3, "副标题没数了，轮轨仍该有三轮");
    assert!(
        !rows[0].is_empty(),
        "反向半边：档 2 的 rich 行不是「整块缺席」，两型不许被压成一型"
    );
    assert_eq!(
        rows[0].plan,
        Some(PlanProjection::default()),
        "同一张表里别的投影仍可读"
    );
}

/// 一档桩的 `session/list` 去掉时钟之后的整串形状（`updatedAt` 是墙钟，两次起桩必不等；
/// 其余键里没有时钟）。用来判「回落 = 默认档**逐字节**」，而不是「行为差不多」。
fn ledger_shape(launch: &Launch) -> String {
    let mut kernel = Kernel::start(launch).expect("假内核应完成 dsh web: 握手");
    let raw = kernel
        .call("session/list", json!({ "_request": {} }))
        .expect("session/list 应成功");
    kernel.shutdown();
    let stripped: Vec<Value> = raw["items"]
        .as_array()
        .expect("items 该是数组")
        .iter()
        .map(|item| {
            json!({
                "sessionId": item["sessionId"],
                "cwd": item["cwd"],
                "blank": item["blank"],
                "projections": item["projections"],
            })
        })
        .collect();
    serde_json::to_string(&stripped).expect("台账该能序列化")
}

/// 认不出的档位 = 默认档，逐字节相等（`knob()` 的回落臂：argv 压过 env，脏值不落进任何档）。
/// 判的是「三档之外不存第四种形状」这条不变式 —— 一旦回落被改成「脏值按最后一位数字」或
/// 「脏值当档 1」，线上就会长出内核产不出的那一型。
/// 反向半边：认得的 1/2 两档必须与默认档**不等**，否则上面那几条 `==` 是空判。
#[test]
fn an_unrecognized_session_stats_level_is_the_default_tier_byte_for_byte() {
    let default_shape = ledger_shape(&fake_launch());
    assert_eq!(
        default_shape,
        ledger_shape(&launch_with_args(&["--session-stats=0"])),
        "显式写 0 与不带旋钮同一形状（默认档的定义）"
    );
    for dirty in ["--session-stats=7", "--session-stats=nope", "--session-stats=-1", "--session-stats="] {
        assert_eq!(
            default_shape,
            ledger_shape(&launch_with_args(&[dirty])),
            "{dirty} 不在三档之内 ⇒ 该整串回落成默认档，而不是猜一个档"
        );
    }
    // 反向半边：旋钮真的被读到了 —— 认得的档位与默认档逐字节不同。
    assert_ne!(
        default_shape,
        ledger_shape(&launch_with_args(&["--session-stats=1"])),
        "档 1 会补齐空白行：与默认档不等（否则上面那圈 == 只是把旋钮关掉测的）"
    );
    assert_ne!(
        default_shape,
        ledger_shape(&launch_with_args(&["--session-stats=2"])),
        "档 2 会摘掉 rich 行那一键：与默认档不等"
    );
}

/// SS1 bug ②：`asOfSeq` 是**折叠水位**（该会话 journal 末条事件的 seq，空日志 -1），
/// 不是大纲末条 `turn/start` 的号。两条**互相独立**的门一起判才不作伪：
/// · 无过滤的 `session/page`（日志真源）末条 seq == advertised；
/// · `throughSeq = advertised` 必须**被收下**且翻出整段日志（outline 里每一轮的 `turn/start`
///   都在页内），`advertised + 1` 必须被拒且拒的那句报的就是同一个水位。
/// 旧写法（`outline.last()["seq"].unwrap_or(0)`）两头都错：s-1001 报 88 而水位 117 ⇒
/// 按 88 起页悄悄丢掉最后一轮那 29 条（含那篇答案的 usage，`kernel.rs:ledger_through`
/// 就是拿 advertised 当首页 `throughSeq`，主干 `MainWindow.xaml.cs:15083`），
/// 零事件行报 0 而非 -1 ⇒ 主干 `:15081` 的「没有 journal 游标：跳过」那一臂永不触发。
/// 反向半边：空白行 `throughSeq = 0` 必须越界；跑完一轮之后 advertised 跟着水位走
/// （117 → 147），而**不是**跟着大纲末条走（118）—— 两个数同时钉住才排掉「再混一次」。
#[test]
fn the_advertised_as_of_seq_is_the_cursor_the_page_endpoint_itself_accepts() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
    let rows = kernel.list_session_rows().expect("session/list 应成功");
    assert_eq!(rows.len(), 3);

    for (id, watermark, records) in [
        ("s-1001", 117_i64, 90usize),
        ("s-1002", -1, 0),
        ("s-1003", 35, 30),
    ] {
        let row = rows
            .iter()
            .find(|row| row.info.id == id)
            .unwrap_or_else(|| panic!("台账里有 {id}"));
        // 门零：不带 throughSeq 的那页就是日志真源，末条 seq 即水位。
        let whole = kernel
            .call("session/page", page_args(id, None, None))
            .expect("无过滤的一页该回来");
        let events = page_events(&whole);
        assert_eq!(events.len(), records, "{id}：整段日志的条数");
        assert_eq!(
            events.last().and_then(|event| event["seq"].as_i64()).unwrap_or(-1),
            watermark,
            "{id}：无过滤那页的末条 seq"
        );
        assert_eq!(
            row.projections.as_of_seq,
            Some(watermark),
            "{id}：advertised == 日志末条，不是大纲末条的 turn/start 号"
        );

        // 门一：按 advertised 起页该被收下，且一条都不少。
        let cut = kernel
            .call("session/page", page_args(id, Some(watermark), None))
            .unwrap_or_else(|error| panic!("{id}：throughSeq={watermark} 是合法水位，却被拒 {error}"));
        assert_eq!(
            page_events(&cut).len(),
            records,
            "{id}：按 advertised 起页不许少翻一条（旧写法在这里少一整轮）"
        );
        assert_eq!(
            turn_start_seqs(&page_events(&cut)),
            row.projections
                .turn_outline
                .iter()
                .map(|item| item.seq)
                .collect::<Vec<_>>(),
            "{id}：页里的轮锚集 == 块里 advertised 的大纲轮锚集（两份投影咬得上）"
        );
        // 门二：越界一格必须被拒，且那句里的号就是 advertised。
        let error = kernel
            .call("session/page", page_args(id, Some(watermark + 1), None))
            .expect_err(&format!("{id}：越过水位一格必须被拒"));
        assert_eq!(
            error,
            format!("gateway/bad-request: session page through seq {} is past cursor {watermark}", watermark + 1),
            "{id}：page 认的游标与块里 advertised 的必须是同一个数"
        );
    }

    // 旧写法那一格（88）真正会丢的东西：第三轮的答案与 `turn/end` 都在 89..117 之间。
    let cut = kernel
        .call("session/page", page_args("s-1001", Some(117), None))
        .expect("按 advertised 起页");
    let events = page_events(&cut);
    assert_eq!(
        user_texts(&events),
        ["把登录改成走内核", "补上失败分支", "再补一轮测试"],
        "三轮提问都在（按 88 切就只剩两轮）"
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"].as_str() == Some("assistant/message"))
            .count(),
        3,
        "三篇答案都在，含末轮那篇（它的 usage 就是少算的那 1234 tok）"
    );

    // 同一趟聚合走分叉的产品码（主干 `MainWindow.xaml.cs:15075-15083` 的分叉版）：
    // 首页 `throughSeq` 取的就是 advertised ⇒ 三档之外这一判是端到端的。
    let aggregate = kernel.collect_usage_stats(0).expect("一趟用量聚合该成");
    assert_eq!(
        (aggregate.sessions_scanned, aggregate.sessions_skipped),
        (2, 1),
        "两条有日志的会话走查、空白行跳过（主干 :15071 的 blank 那一臂先撞上）"
    );
    assert_eq!(aggregate.usage_messages, 4, "s-1001 三轮 + s-1003 一轮全计入");
    assert_eq!(
        aggregate.total(),
        5322,
        "桩的剧本：奇数轮 totalTokens 1234、偶数轮四桶相加 1620 ⇒ 1234+1620+1234+1234"
    );

    // 反向半边（动态）：一轮落地之后水位跟着日志走，而不是跟着大纲末条走。
    kernel
        .call("session/prompt", prompt_args("s-1001", "水位该跟着走", "queue"))
        .expect("session/prompt 应被接受");
    let after = kernel
        .list_session_rows()
        .expect("session/list 应成功")
        .into_iter()
        .find(|row| row.info.id == "s-1001")
        .expect("台账里有 s-1001");
    let outline = after
        .projections
        .turn_outline
        .last()
        .expect("一轮之后大纲有第四条");
    assert_eq!((outline.turn, outline.seq), (4, 118), "大纲末条是**新轮的锚点** 118");
    assert_eq!(
        after.projections.as_of_seq,
        Some(147),
        "advertised 却是日志末条 147（旧写法会把这两个不同的量再混成一个）"
    );
    assert_eq!(
        kernel
            .call("session/page", page_args("s-1001", Some(147), None))
            .expect("新水位该被收下")["records"]
            .as_array()
            .expect("records 是数组")
            .len(),
        120,
        "按新 advertised 起页翻得到全 120 条"
    );
    let error = kernel
        .call("session/page", page_args("s-1001", Some(148), None))
        .expect_err("新水位再往上一格越界");
    assert!(error.ends_with("is past cursor 147"), "{error}");
    let after_aggregate = kernel.collect_usage_stats(0).expect("第二轮之后再来一趟聚合");
    assert_eq!(after_aggregate.usage_messages, 5, "新那一轮的答案也得进台账");
    assert_eq!(after_aggregate.total(), 6942, "5322 + 第 4 轮（偶数轮）1620");
    kernel.shutdown();
}

/// 两条门读的是同一张投影表：`session/control` 的 baseline 与 `session/list` 必须同源，
/// 且 `--session-stats=` 在两条路上同时生效（桩侧共用 `projections_for`）。
/// 反向半边：baseline 里 s-1001 那格**不是**八个 0（补齐臂没把有数的行夹掉），
/// 而 s-1002 那格的 `values` 只有 title + 补齐的这一键（不是一整块空表）。
#[test]
fn the_control_baseline_and_the_session_list_agree_on_stats_tier_and_watermark() {
    let (mut kernel, mut mux, stream) =
        open_control_stream_by(&launch_with_args(&["--session-stats=1"]));
    let events = collect(&mut mux, &stream, 4);
    let frames = item_values(&events, &stream);
    assert_eq!(tag(&frames[0]), "baseline", "baseline 在增量之前");
    let mut state = ControlState::default();
    assert!(state.apply(&frames[0]).projections, "baseline 该带 projections 表");

    assert_eq!(projection_of(&state, "s-1001").as_of_seq, Some(117), "水位这一路也一样");
    assert_eq!(projection_of(&state, "s-1002").as_of_seq, Some(-1), "零事件行的水位是 -1，不是 0");
    assert_eq!(
        projection_of(&state, "s-1002").stats,
        Some(SessionStats::default()),
        "档 1 在 baseline 这一路也补齐成八个 0"
    );
    assert_eq!(
        projection_of(&state, "s-1001").stats.unwrap().turns,
        3,
        "反向半边：有 closed turn 的那格不许被补齐臂夹成 0"
    );
    assert_eq!(
        sorted_keys(&projection_of(&state, "s-1002").values),
        ["sessionStats", "title"],
        "空白行的 values 仍是「只有 title + 这一键」，不是整块空表"
    );

    let rows = kernel.list_session_rows().expect("session/list 应成功");
    for id in ["s-1001", "s-1002", "s-1003"] {
        let listed = rows
            .iter()
            .find(|row| row.info.id == id)
            .unwrap_or_else(|| panic!("台账里有 {id}"))
            .projections
            .clone();
        assert_eq!(
            projection_of(&state, id),
            &listed,
            "{id}：同一条桩、同一张表，两门读数一字不差"
        );
    }
    kernel.shutdown();
}

// ==================== #141：`--compaction=` / `--workflow=` 走真 socket 的集成用例（SK5 §3） ====
// 两族七型此前在桩里是 `grep -c "compaction/"` = **0**：`is_chat_domain_event`（分叉
// `src/main.rs:4707` 那张 `[&str; 7]`）把 `compaction/{start,summary,end}` 与
// `tool-workflow/{run-start,agent-start,agent-end,run-end}` 整族吞掉，且**零渲染臂**
// ⇒ 这一层的证据天花板是「**线上形状**」：帧发得出、`data` 键集逐字对得上主干读者
// `MainWindow.MessageDetails.cs:682-808` 的 `GetProperty` 表达式。卡要端到端得等 main.rs
// 补那两型臂（规格在 `tmp/sk5-report.md` §5）。
//
// 形状与帧预算**全部现测**（记录在报告 §2/§4，不抄 SK1/SK2/IPC3 的计数）：
// · live 一轮 = 该档 journal 事件数 + 与档无关的 6 枚 `assistant-stream` 逐字增量
//   ⇒ 压缩档 0→5：36 / 39 / 39 / 38 / 37 / 39；workflow 档 0→5：36 / 45 / 38 / 38 / 40 / 39；
// · 两族都插在既有 `event:session/title` 之后、`event:turn/end` 之前（信封时间 8410..8660
//   那段既有空档）⇒ 与 `--inject=` / `--trunc=` / `--retry=`（切点都在正文之前）**相加**；
// · live 轮号是 1 ⇒ 链 id 线上取 `compact-1-cycle-1` / `wf-1-run-1`；
// · 每颗用例自带 argv（`launch_with`）：不改 `fake_launch()`、不设 `FAKE_DSH_COMPACTION` /
//   `FAKE_DSH_WORKFLOW` ⇒ 既有用例的输入字节流一个都不变。

/// 压缩档该多出的帧标签（与 `fake_dsh.rs::compaction_entries` 同一张档表）。
fn compaction_tags_of(level: u64) -> &'static [&'static str] {
    match level {
        1 | 2 | 5 => &[
            "event:compaction/start",
            "event:compaction/summary",
            "event:compaction/end",
        ],
        3 => &["event:compaction/start", "event:compaction/end"],
        4 => &["event:compaction/start"],
        _ => &[],
    }
}

/// workflow 档该多出的帧标签（与 `fake_dsh.rs::workflow_entries` 同一张档表）。
const WF_RS: &str = "event:tool-workflow/run-start";
const WF_AS: &str = "event:tool-workflow/agent-start";
const WF_AE: &str = "event:tool-workflow/agent-end";
const WF_RE: &str = "event:tool-workflow/run-end";

fn workflow_tags_of(level: u64) -> &'static [&'static str] {
    match level {
        1 => &[WF_RS, WF_AS, WF_AS, WF_AS, WF_AS, WF_AE, WF_AE, WF_AE, WF_RE],
        2 => &[WF_RS, WF_RE],
        3 => &[WF_RS, WF_AS],
        4 => &[WF_RS, WF_AS, WF_AE, WF_RE],
        5 => &[WF_RS, WF_AS, WF_AE],
        _ => &[],
    }
}

/// 在给定帧序里、既有那格 `event:session/title` 与 `event:turn/end` 之间插压缩帧。
/// 下标算出来不写死：开 `--inject=` 时前面已多五格、`--trunc=2/3` 再多一发 attempt。
fn with_compaction_tags(base: &[String], level: u64) -> Vec<String> {
    let mut tags = base.to_vec();
    let added = compaction_tags_of(level);
    if !added.is_empty() {
        let at = tags
            .iter()
            .position(|text| text == "event:turn/end")
            .expect("关档帧序里得有 event:turn/end");
        tags.splice(at..at, added.iter().map(|text| text.to_string()));
    }
    tags
}

/// 同上插 workflow 帧（生产侧同一条 `position(== "turn/end")` 切点，压缩族先塞 ⇒ 排在它后面）。
fn with_workflow_tags(base: &[String], level: u64) -> Vec<String> {
    let mut tags = base.to_vec();
    let added = workflow_tags_of(level);
    if !added.is_empty() {
        let at = tags
            .iter()
            .position(|text| text == "event:turn/end")
            .expect("关档帧序里得有 event:turn/end");
        tags.splice(at..at, added.iter().map(|text| text.to_string()));
    }
    tags
}

/// 只开压缩档时该见的完整帧序。
fn compaction_frame_tags(level: u64) -> Vec<String> {
    let base: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    with_compaction_tags(&base, level)
}

/// 只开 workflow 档时该见的完整帧序。
fn workflow_frame_tags(level: u64) -> Vec<String> {
    let base: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    with_workflow_tags(&base, level)
}

/// 收真 socket 的一整轮压缩帧：返回（follow 流帧序、剥壳后的 journal 事件）。
/// `want` 是收帧预算，**逐档现测**（36/39/39/38/37/39）；沿用 36 会卡在缺帧上直到超时。
fn compaction_turn(session: &str, level: u64, want: usize) -> (Vec<Value>, Vec<Value>) {
    let knob = format!("--compaction={level}");
    compaction_turn_argv(session, &[&knob], want)
}

fn compaction_turn_argv(session: &str, knobs: &[&str], want: usize) -> (Vec<Value>, Vec<Value>) {
    let mut argv: Vec<&str> = vec!["--pace=0"];
    argv.extend_from_slice(knobs);
    let launch = launch_with(&argv);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args(session, "上下文压一遍吧", "queue"))
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 收真 socket 的一整轮 workflow 帧（档表 36/45/38/38/40/39）。
fn workflow_turn(session: &str, level: u64, want: usize) -> (Vec<Value>, Vec<Value>) {
    let knob = format!("--workflow={level}");
    workflow_turn_argv(session, &[&knob], want)
}

fn workflow_turn_argv(session: &str, knobs: &[&str], want: usize) -> (Vec<Value>, Vec<Value>) {
    let mut argv: Vec<&str> = vec!["--pace=0"];
    argv.extend_from_slice(knobs);
    let launch = launch_with(&argv);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args(session, "跑一遍工作流", "queue"))
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 历史回读那一路（`seed_journal` 与 live 同一台机器）：种子会话 `s-1001` 三轮的事件数组。
fn compaction_seed_argv(knobs: &[&str]) -> Vec<Value> {
    let mut argv: Vec<&str> = vec!["--pace=0"];
    argv.extend_from_slice(knobs);
    let mut kernel = Kernel::start(&launch_with(&argv)).expect("握手");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("种子会话该带回历史");
    let events = page_events(&page);
    kernel.shutdown();
    events
}

fn workflow_seed_argv(knobs: &[&str]) -> Vec<Value> {
    compaction_seed_argv(knobs)
}

/// journal 里某一型域帧的全部发（按帧序；关档恒空）。
fn chat_events(events: &[Value], kind: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some(kind))
        .cloned()
        .collect()
}

/// 该型帧**恰一发**时取出来；多于一条直接炸（同族多发的档由数量断言自己钉）。
fn only_chat(events: &[Value], kind: &str) -> Value {
    let found = chat_events(events, kind);
    assert_eq!(
        found.len(),
        1,
        "journal 里 {kind} 该恰一发，实际 {} 发",
        found.len()
    );
    found.into_iter().next().expect("上面已断过非空")
}

/// 帧数组里第一个匹配该型事件的下标。
fn chat_index(events: &[Value], kind: &str) -> usize {
    events
        .iter()
        .position(|event| event["type"].as_str() == Some(kind))
        .unwrap_or_else(|| panic!("journal 里该有 {kind} 那一格"))
}

/// 主干读者**实际读**的键 + 桩约定的外层 `turn` 伴随键；`extra_keys` 非空 = 发了无读者的字节。
/// `shadowedRange` 在允许集里（主干 `:718` 真去 `TryGetProperty`），但另有专门一条断言它**不在**。
fn compaction_allowed(kind: &str) -> &'static [&'static str] {
    match kind {
        "compaction/summary" => &[
            "compactionId",
            "turn",
            "shadowedSeqs",
            "shadowedTokenCount",
            "summary",
            "shadowedRange",
        ],
        _ => &["compactionId", "turn"],
    }
}

fn workflow_allowed(kind: &str) -> &'static [&'static str] {
    // 键名是事件的**裸型名**，而 `WF_RS`/`WF_AS`/`WF_AE` 那几枚 const 是**帧标签**
    // （带 `event:` 前缀，喂 `tag()` 那条比较）⇒ 这里只能写字面量，拿 const 当模式会静默落 `_`。
    match kind {
        "tool-workflow/run-start" => &["runId", "turn", "name"],
        "tool-workflow/agent-start" => &["runId", "turn", "seq", "label", "phase", "childId"],
        "tool-workflow/agent-end" => &["runId", "turn", "seq", "outcome"],
        _ => &["runId", "turn", "stopReason"],
    }
}

/// 把两族七型的帧都过一遍「无多余键」这道门。`allowed_for` 决定每型的读集。
fn family_events(events: &[Value], prefix: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|event| {
            event["type"]
                .as_str()
                .unwrap_or_default()
                .starts_with(prefix)
        })
        .cloned()
        .collect()
}

/// **压缩档 0（显式关）**：一帧不加、一键不改，且与「argv 压根不带 `--compaction=`」逐格同形。
/// 钉的是「这枚新旋钮默认不污染既有 36 格帧序」——接手时那 84 条按帧序钉死的用例吃的就是它。
#[test]
fn compaction_level_zero_adds_no_frame_and_matches_the_unflagged_default() {
    let (frames, events) = compaction_turn("s-9701", 0, TURN_FRAMES);
    assert_eq!(frames.len(), TURN_FRAMES, "档 0 一轮仍 36 帧（现测）");
    assert_eq!(events.len(), 30, "档 0 一轮仍 30 条 journal 事件");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "档 0 的帧序与关档逐格相等"
    );
    let round = frames.iter().map(|frame| frame.to_string()).collect::<String>();
    assert!(
        !round.contains("compaction"),
        "整轮序列化里连 `compaction` 子串都不许出现（不是发了一发空 data）"
    );
    let (_, unflagged) = compaction_turn_argv("s-9702", &[], TURN_FRAMES);
    assert_eq!(
        json_without_time(&events),
        json_without_time(&unflagged),
        "显式 `--compaction=0` 与不开旋钮该是同一台机器"
    );
}

/// **压缩档 1 = 「已压缩 N 条（约 M tokens）」且可展**：三发同 `compactionId`、
/// `shadowedSeqs` 真是数组（主干 `:716` 的 `ValueKind == Array` 读形闸）、`shadowedTokenCount`
/// 真是数值、`summary` 是 `ExtractContentText`（`:874-885`）吃得下的 `{type:"text",text}` 数组。
#[test]
fn compaction_level_one_pushes_the_counted_expandable_summary() {
    let want = TURN_FRAMES + 3;
    let (frames, events) = compaction_turn("s-9703", 1, want);
    assert_eq!(frames.len(), want, "档 1 一轮 39 帧（现测，非照抄别档）");
    assert_eq!(events.len(), 33, "档 1 一轮 33 条 journal 事件");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        compaction_frame_tags(1),
        "只许在 session/title 与 turn/end 之间多三格，其余逐格不动"
    );
    let start = only_chat(&events, "compaction/start");
    let summary = only_chat(&events, "compaction/summary");
    let end = only_chat(&events, "compaction/end");
    assert_eq!(sorted_keys(&start["data"]), ["compactionId", "turn"]);
    assert_eq!(sorted_keys(&end["data"]), ["compactionId", "turn"]);
    assert_eq!(
        sorted_keys(&summary["data"]),
        [
            "compactionId",
            "shadowedSeqs",
            "shadowedTokenCount",
            "summary",
            "turn"
        ]
    );
    // 链不断：三发同 id（summary/end 查无 state 就 `return`，卡会永远停在「正在压缩」）。
    assert_eq!(start["data"]["compactionId"], json!("compact-1-cycle-1"));
    assert_eq!(start["data"]["compactionId"], summary["data"]["compactionId"]);
    assert_eq!(end["data"]["compactionId"], summary["data"]["compactionId"]);
    let seqs = summary["data"]["shadowedSeqs"]
        .as_array()
        .expect("shadowedSeqs 必须是数组，否则 :716 读成 0");
    assert_eq!(seqs.len(), 3, "ShadowedItemCount 就是 GetArrayLength()");
    assert!(summary["data"]["shadowedTokenCount"].is_number());
    assert_eq!(summary["data"]["shadowedTokenCount"], json!(1234));
    let blocks = summary["data"]["summary"]
        .as_array()
        .expect("summary 必须是内容块数组");
    assert_eq!(blocks.len(), 1);
    assert_eq!(sorted_keys(&blocks[0]), ["text", "type"]);
    assert_eq!(blocks[0]["type"], json!("text"));
    assert!(blocks[0]["text"].as_str().unwrap().chars().count() > 0);
    // 主干 `:718` 只判 `shadowedRange` 存在、值丢弃 ⇒ 无读者的键一律不发。
    assert!(
        summary["data"].get("shadowedRange").is_none(),
        "发了 shadowedRange 就是发一枚没有读者的键"
    );
    // 排位与信封不变量：新三格整族在 session/title 之后、turn/end 之前；seq 连号；time 不减。
    let title = chat_index(&events, "session/title");
    assert_eq!(chat_index(&events, "compaction/start"), title + 1);
    assert_eq!(chat_index(&events, "compaction/end"), title + 3);
    assert_eq!(events[title + 4]["type"], json!("turn/end"));
    assert!(start["time"].as_i64().unwrap() < summary["time"].as_i64().unwrap());
    assert!(summary["time"].as_i64().unwrap() < end["time"].as_i64().unwrap());
    for (index, event) in events.iter().enumerate() {
        assert!(event["type"].is_string());
        assert!(event["data"].is_object());
        if index > 0 {
            assert!(
                event["seq"].as_i64().unwrap() == events[index - 1]["seq"].as_i64().unwrap() + 1,
                "第 {index} 格 seq 断了"
            );
        }
    }
}

/// **压缩档 2 = 空数组 + 无 token**：count/token 双双读成 0、`Summary` 非空 ⇒
/// 主干 `{Summary: not null}` 那一臂（`:1036`「点击查看」，仍可展）。
#[test]
fn compaction_level_two_pushes_an_empty_shadowed_array_and_no_token_count() {
    let want = TURN_FRAMES + 3;
    let (frames, events) = compaction_turn("s-9704", 2, want);
    assert_eq!(frames.len(), want, "档 2 也是三发 ⇒ 39 帧");
    let summary = only_chat(&events, "compaction/summary");
    assert_eq!(
        sorted_keys(&summary["data"]),
        ["compactionId", "shadowedSeqs", "summary", "turn"],
        "档 2 恰四枚、不得带 shadowedTokenCount"
    );
    assert!(summary["data"]["shadowedSeqs"].is_array());
    assert!(summary["data"]["shadowedSeqs"].as_array().unwrap().is_empty());
    assert!(summary["data"]["summary"].is_array());
    // start/end 与档 1 逐字节同形 ⇒ 档间差别只在 summary 那一发。
    let (_, one) = compaction_turn("s-9703x", 1, want);
    for kind in ["compaction/start", "compaction/end"] {
        assert_eq!(
            serde_json::to_string(&only_chat(&events, kind)["data"]).unwrap(),
            serde_json::to_string(&only_chat(&one, kind)["data"]).unwrap(),
            "{kind} 必须就是档 1 那一发"
        );
    }
}

/// **压缩档 3 = 整发 summary 都不推** ⇒ 主干 `_` 臂（`:1038`「压缩摘要不可用」）：
/// 少发一族里的一发本身就是有读者的形状，不是漏发。
#[test]
fn compaction_level_three_never_pushes_a_summary_frame() {
    let want = TURN_FRAMES + 2;
    let (frames, events) = compaction_turn("s-9705", 3, want);
    assert_eq!(frames.len(), want, "档 3 只多两发 ⇒ 38 帧");
    assert_eq!(events.len(), 32);
    assert!(chat_events(&events, "compaction/summary").is_empty());
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        compaction_frame_tags(3)
    );
    let start = only_chat(&events, "compaction/start");
    let end = only_chat(&events, "compaction/end");
    assert_eq!(start["data"]["compactionId"], end["data"]["compactionId"]);
    assert_eq!(chat_index(&events, "compaction/end"), chat_index(&events, "compaction/start") + 1);
    let round = frames.iter().map(|frame| frame.to_string()).collect::<String>();
    assert!(!round.contains("shadowedSeqs"), "没有 summary 帧就不该出现它的任何键");
    assert!(!round.contains("shadowedTokenCount"));
}

/// **压缩档 4 = 只推 start**：主干 `{Running: true}` 臂（`:1031`「正在压缩…」）。
/// 这一档只为可证性存在（真内核终会补 end），链上缺后两发时读者**不会**凭空造状态。
#[test]
fn compaction_level_four_leaves_the_chain_open_with_only_a_start_frame() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = compaction_turn("s-9706", 4, want);
    assert_eq!(frames.len(), want, "档 4 只多一发 ⇒ 37 帧");
    assert_eq!(events.len(), 31);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        compaction_frame_tags(4)
    );
    let start = only_chat(&events, "compaction/start");
    assert_eq!(sorted_keys(&start["data"]), ["compactionId", "turn"]);
    assert_eq!(start["data"]["compactionId"], json!("compact-1-cycle-1"));
    assert!(chat_events(&events, "compaction/summary").is_empty());
    assert!(chat_events(&events, "compaction/end").is_empty());
}

/// **压缩档 5 = `shadowedSeqs` 发成数字**：主干 `:716` 的读形闸不过 ⇒ count 0，
/// 且这一档无 `summary` ⇒ 「点击查看」却**不可展**。与档 2 同为「点击查看」，
/// 差在可展闸门（`:1039`）—— 光看文案分不开，故线上字节必须分得开。
#[test]
fn compaction_level_five_pushes_a_non_array_shadowed_seq_set() {
    let want = TURN_FRAMES + 3;
    let (frames, events) = compaction_turn("s-9707", 5, want);
    assert_eq!(frames.len(), want, "档 5 也是三发 ⇒ 39 帧");
    let summary = only_chat(&events, "compaction/summary");
    assert_eq!(
        sorted_keys(&summary["data"]),
        ["compactionId", "shadowedSeqs", "shadowedTokenCount", "turn"],
        "档 5 恰四枚、不得带 summary"
    );
    assert!(summary["data"]["shadowedSeqs"].is_number());
    assert!(!summary["data"]["shadowedSeqs"].is_array());
    assert_eq!(summary["data"]["shadowedSeqs"], json!(3), "读者必须读成 0 而不是 3");
    let two = compaction_turn("s-9704x", 2, want);
    assert_ne!(
        serde_json::to_string(&summary["data"]).unwrap(),
        serde_json::to_string(&only_chat(&two.1, "compaction/summary")["data"]).unwrap(),
        "档 5 与档 2 同臂不同形，线上字节得分得开"
    );
}

/// **全档键集**：每一型压缩帧都只带主干读者真读的那几枚 + `turn` 伴随键，
/// 且 `shadowedRange` 在任何一档都不出现。
#[test]
fn the_compaction_frames_carry_no_keys_the_trunk_reader_never_reads() {
    for level in 0..=6u64 {
        let want = TURN_FRAMES + compaction_tags_of(level).len();
        let (_, events) = compaction_turn(&format!("s-9708{level}"), level, want);
        for event in family_events(&events, "compaction/") {
            let kind = event["type"].as_str().unwrap_or_default();
            let allowed = compaction_allowed(kind);
            assert!(
                extra_keys(&event["data"], allowed).is_empty(),
                "档 {level} 的 {kind} 带了主干不读的键: {:?}",
                sorted_keys(&event["data"])
            );
            assert!(
                event["data"].get("shadowedRange").is_none(),
                "档 {level}：shadowedRange 是「读了就丢」的键，不该上线"
            );
            assert_eq!(
                event["data"]["turn"],
                json!(1),
                "档 {level} 的 {kind} 得带上当期轮号"
            );
        }
    }
}

/// **压缩档的回读同形**：`session/page`（`seed_journal`）与 live 吃同一台机器 ⇒
/// 种子会话三轮各带自己的链，事件数现测 90 / 99 / 99 / 96 / 93 / 99。
#[test]
fn the_compaction_levels_read_back_from_history_matching_the_live_shape() {
    let table: [(u64, usize, usize); 6] = [
        // 档, 种子页事件数, summary 发数
        (0, 90, 0),
        (1, 99, 3),
        (2, 99, 3),
        (3, 96, 0),
        (4, 93, 0),
        (5, 99, 3),
    ];
    for (level, expected, summaries) in table {
        let knob = format!("--compaction={level}");
        let events = compaction_seed_argv(&[&knob]);
        assert_eq!(
            events.len(),
            expected,
            "档 {level} 的回读事件数与 §2 现测表不符"
        );
        assert_eq!(chat_events(&events, "compaction/summary").len(), summaries);
        let ids: Vec<String> = chat_events(&events, "compaction/start")
            .iter()
            .map(|event| event["data"]["compactionId"].as_str().unwrap().to_string())
            .collect();
        let want_starts = compaction_tags_of(level)
            .iter()
            .filter(|text| **text == "event:compaction/start")
            .count();
        assert_eq!(ids.len(), 3 * want_starts, "档 {level}：三轮各一枚 start");
        if want_starts > 0 {
            let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
            assert_eq!(
                unique.len(),
                ids.len(),
                "链 id 必须按轮号编，否则三轮共用一条链: {ids:?}"
            );
            assert_eq!(ids[0], "compact-1-cycle-1");
            assert_eq!(ids[2], "compact-3-cycle-1");
        }
    }
    // 回读与 live 同形：档 1 那一轮（turn 1）的 summary data 逐字节等于线上那一发。
    let knob = "--compaction=1";
    let seeded = compaction_seed_argv(&[knob]);
    let (_, live) = compaction_turn("s-9708z", 1, TURN_FRAMES + 3);
    let seeded_first = chat_events(&seeded, "compaction/summary")
        .into_iter()
        .next()
        .expect("种子页档 1 该有 summary");
    assert_eq!(
        serde_json::to_string(&seeded_first["data"]).unwrap(),
        serde_json::to_string(&only_chat(&live, "compaction/summary")["data"]).unwrap(),
        "同一条桩、同一档 ⇒ 回读与 live 的 data 该逐字节相等"
    );
}

/// **脏档与未定义档**：`--compaction=nope` / `=-1` / `=99` 一律回落关档，
/// 且与档 0 **逐格**同形（宁可不演，也不演一枚两头都不像的形状）。
#[test]
fn an_undefined_or_dirty_compaction_level_falls_back_to_the_off_shape() {
    let (off_frames, off_events) = compaction_turn("s-9709", 0, TURN_FRAMES);
    for knob in ["--compaction=nope", "--compaction=-1", "--compaction=", "--compaction=99"] {
        let (frames, events) = compaction_turn_argv("s-9710", &[knob], TURN_FRAMES);
        assert_eq!(frames.len(), TURN_FRAMES, "{knob} 必须等价于关档");
        assert_eq!(
            frames.iter().map(tag).collect::<Vec<_>>(),
            off_frames.iter().map(tag).collect::<Vec<_>>(),
            "{knob} 的帧序与关档不等"
        );
        assert_eq!(json_without_time(&events), json_without_time(&off_events));
    }
}

/// **四把刀相加**：`--inject=1 --trunc=2 --retry=1 --compaction=1` ⇒
/// 帧数 36+5+1+2+3 = 47；压缩那一刀切在 `turn/end` 之前，与另三把（都在正文之前）不相交。
#[test]
fn the_compaction_frames_add_to_the_injection_truncation_and_retry_budgets() {
    let want = TURN_FRAMES + INJECTED_FRAMES + 1 + 2 + 3;
    let knobs = ["--inject=1", "--trunc=2", "--retry=1", "--compaction=1"];
    let (frames, events) = compaction_turn_argv("s-9711", &knobs, want);
    assert_eq!(frames.len(), want, "四把同开一轮 47 帧（现测）");
    let injected: Vec<String> = injected_turn_tags();
    let mut expected = with_retry_tags(&injected, 1);
    let at = expected
        .iter()
        .position(|text| text == "event:assistant/message")
        .expect("帧序里得有 assistant/message");
    expected.insert(at, "event:assistant/attempt".to_string());
    let expected = with_compaction_tags(&expected, 1);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        expected,
        "另三把刀挪不动压缩帧的落点（切点是算出来的）"
    );
    assert_eq!(events.len(), 41, "四把同开一轮 41 条 journal 事件");
    assert!(chat_index(&events, "compaction/start") < chat_index(&events, "turn/end"));
    let start = only_chat(&events, "compaction/start");
    assert_eq!(start["data"]["compactionId"], json!("compact-1-cycle-1"));
    let seed_knobs = ["--inject=1", "--trunc=2", "--retry=1", "--compaction=1"];
    assert_eq!(
        compaction_seed_argv(&seed_knobs).len(),
        3 * 41,
        "回读与 live 同档 ⇒ 种子页 3×41 = 123"
    );
}

/// **workflow 档 0（显式关）**：一帧不加，且与「压根不带旋钮」逐格同形。
#[test]
fn workflow_level_zero_adds_no_frame_and_matches_the_unflagged_default() {
    let (frames, events) = workflow_turn("s-9801", 0, TURN_FRAMES);
    assert_eq!(frames.len(), TURN_FRAMES, "档 0 一轮仍 36 帧（现测）");
    assert_eq!(events.len(), 30);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "档 0 的帧序与关档逐格相等"
    );
    let round = frames.iter().map(|frame| frame.to_string()).collect::<String>();
    assert!(
        !round.contains("tool-workflow"),
        "整轮序列化里连 `tool-workflow` 子串都不许出现"
    );
    let (_, unflagged) = workflow_turn_argv("s-9802", &[], TURN_FRAMES);
    assert_eq!(
        json_without_time(&events),
        json_without_time(&unflagged),
        "显式 `--workflow=0` 与不开旋钮该是同一台机器"
    );
}

/// **workflow 档 1 = 成员四态齐活**：4×agent-start（两名带 `phase`、两名省掉 ⇒ 读者
/// `:778` 给 null）+ 3×agent-end（`completed` / `failed` / `cancelled`）+ 第 4 名无 end ⇒
/// 「运行中」；`Members.Count > 0` ⇒ 主干 `:1142` 不早回、toggle 才存在（`:1190`）。
#[test]
fn workflow_level_one_pushes_the_nine_member_frames() {
    let want = TURN_FRAMES + 9;
    let (frames, events) = workflow_turn("s-9803", 1, want);
    assert_eq!(frames.len(), want, "档 1 一轮 45 帧（现测，非照抄别档）");
    assert_eq!(events.len(), 39);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        workflow_frame_tags(1),
        "只许在 session/title 与 turn/end 之间多九格，其余逐格不动"
    );
    let run = only_chat(&events, "tool-workflow/run-start");
    assert_eq!(sorted_keys(&run["data"]), ["name", "runId", "turn"]);
    assert_eq!(run["data"]["runId"], json!("wf-1-run-1"));
    assert!(run["data"]["name"].is_string());
    let starts = chat_events(&events, "tool-workflow/agent-start");
    let ends = chat_events(&events, "tool-workflow/agent-end");
    assert_eq!((starts.len(), ends.len()), (4, 3), "第四名故意无 end ⇒ 运行中");
    for event in &starts {
        assert_eq!(event["data"]["runId"], json!("wf-1-run-1"), "每一发都得自带链 id");
        assert!(event["data"]["seq"].is_number());
        assert!(event["data"]["label"].is_string());
        assert!(event["data"]["childId"].is_string());
    }
    assert_eq!(
        starts
            .iter()
            .filter(|event| event["data"].get("phase").is_some())
            .count(),
        2,
        "得同时铺开「带 phase」与「省 phase ⇒ null」两条读形"
    );
    assert!(
        starts
            .iter()
            .filter(|event| event["data"].get("phase").is_some())
            .all(|event| event["data"]["phase"].is_string()),
        "phase 有就必须是字符串，否则主干 `Str` 读成空"
    );
    let outcomes: Vec<&str> = ends
        .iter()
        .map(|event| event["data"]["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(
        outcomes,
        vec!["completed", "failed", "cancelled"],
        "成员级失败是 **failed**（`:1153`），不是 error"
    );
    let started: Vec<i64> = starts.iter().map(|e| e["data"]["seq"].as_i64().unwrap()).collect();
    let ended: Vec<i64> = ends.iter().map(|e| e["data"]["seq"].as_i64().unwrap()).collect();
    assert_eq!(started, vec![1, 2, 3, 4]);
    assert_eq!(ended, vec![1, 2, 3], "end 按 seq 配对，缺号那名才是运行中");
    for event in &ends {
        assert_eq!(sorted_keys(&event["data"]), ["outcome", "runId", "seq", "turn"]);
    }
    let run_end = only_chat(&events, "tool-workflow/run-end");
    assert_eq!(sorted_keys(&run_end["data"]), ["runId", "stopReason", "turn"]);
    assert_eq!(run_end["data"]["stopReason"], json!("completed"));
    // 整族排位 + 信封不变量。
    let title = chat_index(&events, "session/title");
    assert_eq!(chat_index(&events, "tool-workflow/run-start"), title + 1);
    assert_eq!(events[title + 9]["type"], json!("tool-workflow/run-end"));
    assert_eq!(events[title + 10]["type"], json!("turn/end"));
    for (index, event) in events.iter().enumerate() {
        if index > 0 {
            assert_eq!(
                event["seq"].as_i64().unwrap(),
                events[index - 1]["seq"].as_i64().unwrap() + 1,
                "第 {index} 格 seq 断了"
            );
            assert!(
                event["time"].as_i64().unwrap() >= events[index - 1]["time"].as_i64().unwrap(),
                "第 {index} 格的时间倒退了"
            );
        }
    }
}

/// **workflow 档 2 = 零成员**：只有 run-start / run-end ⇒ 主干 `:1132`「没有启动成员」
/// 那一臂，且 `:1142` 早回 ⇒ 卡上压根没有 `WorkflowMembersToggle` 那颗钮。
#[test]
fn workflow_level_two_runs_with_zero_members() {
    let want = TURN_FRAMES + 2;
    let (frames, events) = workflow_turn("s-9804", 2, want);
    assert_eq!(frames.len(), want, "档 2 一轮 38 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        workflow_frame_tags(2)
    );
    assert!(chat_events(&events, "tool-workflow/agent-start").is_empty());
    assert!(chat_events(&events, "tool-workflow/agent-end").is_empty());
    let run_end = only_chat(&events, "tool-workflow/run-end");
    assert_eq!(run_end["data"]["stopReason"], json!("cancelled"));
    assert_eq!(chat_index(&events, "tool-workflow/run-end"), chat_index(&events, "tool-workflow/run-start") + 1);
    // 只查 journal：`assistant-stream` 那枚 `end` 元素本来就有 `outcome` 对象，
    // 拿整轮 `frames` 做子串判据会假红（现测踩到过一次）。
    assert!(
        events.iter().all(|event| event["data"].get("childId").is_none()),
        "档 2 零成员 ⇒ 任何 journal data 都不该带 childId"
    );
    assert!(
        events.iter().all(|event| event["data"].get("outcome").is_none()),
        "档 2 零成员 ⇒ 任何 journal data 都不该带 outcome"
    );
}

/// **workflow 档 3 = 运行中**：主干 `HandleWorkflowRunEnd` 用 `Str(data,"stopReason")`，
/// **发了却省键** 读成 `""`（不是 null）⇒ 落 `:1119` 的 `_` 臂。所以「运行中」的**唯一**
/// 真分支是**整发 run-end 都不推** —— 这一条把线上「没有 stopReason 键」钉死。
#[test]
fn workflow_level_three_stays_running_by_omitting_run_end_entirely() {
    let want = TURN_FRAMES + 2;
    let (frames, events) = workflow_turn("s-9805", 3, want);
    assert_eq!(frames.len(), want, "档 3 一轮 38 帧");
    assert!(chat_events(&events, "tool-workflow/run-end").is_empty());
    assert!(chat_events(&events, "tool-workflow/agent-end").is_empty());
    let starts = chat_events(&events, "tool-workflow/agent-start");
    assert_eq!(starts.len(), 1);
    assert_eq!(
        sorted_keys(&starts[0]["data"]),
        ["childId", "label", "runId", "seq", "turn"],
        "省 phase ⇒ 读者给 null"
    );
    let round = frames.iter().map(|frame| frame.to_string()).collect::<String>();
    assert!(!round.contains("stopReason"), "发了 run-end 就不是运行中了");
    // journal 侧再钉一次（`assistant-stream` 的 `end` 元素自带 `outcome`，整轮子串法不适用）：
    assert!(
        events.iter().all(|event| event["data"].get("outcome").is_none()),
        "档 3 无 agent-end ⇒ journal 里不该有 outcome 键"
    );
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        workflow_frame_tags(3)
    );
}

/// **workflow 档 4 = 两族「失败」wire 值不对称**的证据：运行级 `stopReason:"error"`
/// （`:1116`）对成员级 `outcome:"failed"`（`:1153`）；这一档另演成员 `_` 臂（`skipped` ⇒ 原样）。
#[test]
fn workflow_level_four_uses_error_at_run_level_where_members_use_failed() {
    let want = TURN_FRAMES + 4;
    let (frames, events) = workflow_turn("s-9806", 4, want);
    assert_eq!(frames.len(), want, "档 4 一轮 40 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        workflow_frame_tags(4)
    );
    let run_end = only_chat(&events, "tool-workflow/run-end");
    let agent_end = only_chat(&events, "tool-workflow/agent-end");
    assert_eq!(run_end["data"]["stopReason"], json!("error"));
    assert_eq!(agent_end["data"]["outcome"], json!("skipped"));
    assert_ne!(
        run_end["data"]["stopReason"].as_str().unwrap(),
        agent_end["data"]["outcome"].as_str().unwrap()
    );
    assert_ne!(run_end["data"]["stopReason"].as_str().unwrap(), "failed");
    assert_eq!(agent_end["data"]["seq"], json!(1), "end 得按 seq 找得着那名成员");
    assert_eq!(run_end["data"]["runId"], agent_end["data"]["runId"]);
    // 对照：档 1 那枚成员级失败必须就是 failed ⇒ 两档合起来才钉得住这条不对称。
    let (_, one) = workflow_turn("s-9803x", 1, TURN_FRAMES + 9);
    assert!(
        chat_events(&one, "tool-workflow/agent-end")
            .iter()
            .any(|event| event["data"]["outcome"] == json!("failed")),
        "档 1 的成员级失败就是 failed"
    );
    assert!(
        !chat_events(&one, "tool-workflow/run-end")
            .iter()
            .any(|event| event["data"]["stopReason"] == json!("failed")),
        "运行级从来不写 failed；写错就落 `_` 臂把原始串印到卡上"
    );
}

/// **workflow 档 5 = 两枚读形回落**：run-start **省 name** ⇒ 卡名回落 runId（`:751`）；
/// agent-start **省 seq** ⇒ 自动序号 `Members.Count+1`（`:777`）；随后 agent-end 用
/// 回落后的 seq=1 去匹配，配不上就是假绿。
#[test]
fn workflow_level_five_drops_name_and_seq_so_the_reader_synthesizes_them() {
    let want = TURN_FRAMES + 3;
    let (frames, events) = workflow_turn("s-9807", 5, want);
    assert_eq!(frames.len(), want, "档 5 一轮 39 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        workflow_frame_tags(5)
    );
    let run = only_chat(&events, "tool-workflow/run-start");
    assert_eq!(sorted_keys(&run["data"]), ["runId", "turn"], "这一档的前提就是缺 name");
    let start = only_chat(&events, "tool-workflow/agent-start");
    assert_eq!(
        sorted_keys(&start["data"]),
        ["childId", "label", "runId", "turn"],
        "省 seq ⇒ 回落 Members.Count+1"
    );
    assert!(start["data"].get("seq").is_none(), "发了 seq 就不是回落臂");
    let end = only_chat(&events, "tool-workflow/agent-end");
    assert_eq!(end["data"]["seq"], json!(1), "end 的 seq 必须等于读者算出来的那个");
    assert_eq!(end["data"]["outcome"], json!("completed"));
    assert!(chat_events(&events, "tool-workflow/run-end").is_empty());
}

/// **全档键集**：四型 workflow 帧都只带主干读者真读的键 + `turn` 伴随键；
/// 且桩**不推** `tool/call name=="workflow"` 那一型（`NoteWorkflowToolCall`/`WorkflowRunForTool`
/// 是另一张卡的入口，不在本片范围）。
#[test]
fn the_workflow_frames_carry_no_keys_the_trunk_reader_never_reads() {
    for level in 0..=6u64 {
        let want = TURN_FRAMES + workflow_tags_of(level).len();
        let (_, events) = workflow_turn(&format!("s-9808{level}"), level, want);
        for event in family_events(&events, "tool-workflow/") {
            let kind = event["type"].as_str().unwrap_or_default();
            assert!(
                extra_keys(&event["data"], workflow_allowed(kind)).is_empty(),
                "档 {level} 的 {kind} 带了主干不读的键: {:?}",
                sorted_keys(&event["data"])
            );
            assert_eq!(event["data"]["turn"], json!(1));
            assert_eq!(event["data"]["runId"], json!("wf-1-run-1"), "{kind} 得自带链 id");
        }
        for tool_call in chat_events(&events, "tool/call") {
            assert_ne!(
                tool_call["data"]["name"],
                json!("workflow"),
                "档 {level}：workflow-run 卡只走那四型事件，别拿 tool/call 混水"
            );
        }
    }
}

/// **workflow 档的回读同形**：种子页事件数现测 90 / 117 / 96 / 96 / 102 / 99，
/// 且链按轮号编（三轮三链）。
#[test]
fn the_workflow_levels_read_back_from_history_matching_the_live_shape() {
    let table: [(u64, usize, usize); 6] = [
        // 档, 种子页事件数, agent-start 发数
        (0, 90, 0),
        (1, 117, 12),
        (2, 96, 0),
        (3, 96, 3),
        (4, 102, 3),
        (5, 99, 3),
    ];
    for (level, expected, agent_starts) in table {
        let knob = format!("--workflow={level}");
        let events = workflow_seed_argv(&[&knob]);
        assert_eq!(events.len(), expected, "档 {level} 的回读事件数与 §2 现测表不符");
        assert_eq!(
            chat_events(&events, "tool-workflow/agent-start").len(),
            agent_starts
        );
        let runs: Vec<String> = chat_events(&events, "tool-workflow/run-start")
            .iter()
            .map(|event| event["data"]["runId"].as_str().unwrap().to_string())
            .collect();
        let want_runs = workflow_tags_of(level)
            .iter()
            .filter(|text| **text == WF_RS)
            .count();
        assert_eq!(runs.len(), 3 * want_runs, "档 {level}：三轮各一枚 run-start");
        let unique: std::collections::BTreeSet<&String> = runs.iter().collect();
        if want_runs > 0 {
            assert_eq!(unique.len(), runs.len(), "runId 必须按轮号编: {runs:?}");
            assert_eq!(runs[0], "wf-1-run-1");
        }
    }
    let seeded = workflow_seed_argv(&["--workflow=1"]);
    let (_, live) = workflow_turn("s-9808z", 1, TURN_FRAMES + 9);
    // 种子页三轮各一发 run-end ⇒ 取第一轮（turn 1）那一发，与 live（同为 turn 1）逐字节对。
    let seeded_run_end = chat_events(&seeded, "tool-workflow/run-end")
        .into_iter()
        .next()
        .expect("种子页档 1 该有 run-end");
    assert_eq!(seeded_run_end["data"]["runId"], json!("wf-1-run-1"));
    assert_eq!(
        serde_json::to_string(&seeded_run_end["data"]).unwrap(),
        serde_json::to_string(&only_chat(&live, "tool-workflow/run-end")["data"]).unwrap(),
        "同一条桩、同一档 ⇒ 回读与 live 的 data 该逐字节相等"
    );
}

/// **脏档与未定义档**：`--workflow=` 认不出的一律回落关档，与档 0 逐格同形。
#[test]
fn an_undefined_or_dirty_workflow_level_falls_back_to_the_off_shape() {
    let (off_frames, off_events) = workflow_turn("s-9809", 0, TURN_FRAMES);
    for knob in ["--workflow=nope", "--workflow=-1", "--workflow=", "--workflow=99"] {
        let (frames, events) = workflow_turn_argv("s-9810", &[knob], TURN_FRAMES);
        assert_eq!(frames.len(), TURN_FRAMES, "{knob} 必须等价于关档");
        assert_eq!(
            frames.iter().map(tag).collect::<Vec<_>>(),
            off_frames.iter().map(tag).collect::<Vec<_>>(),
            "{knob} 的帧序与关档不等"
        );
        assert_eq!(json_without_time(&events), json_without_time(&off_events));
    }
}

/// **五把刀同开**：`--inject=1 --trunc=2 --retry=1 --compaction=1 --workflow=1` ⇒
/// 帧数 36+5+1+2+3+9 = 56；两族各自整族排在 `turn/end` 之前、压缩族在 workflow 族之前
/// （信封时间 8410..8470 vs 8500..8660），链 id 互不串。
#[test]
fn both_new_families_stack_with_the_three_existing_knobs() {
    let want = TURN_FRAMES + INJECTED_FRAMES + 1 + 2 + 3 + 9;
    let knobs = [
        "--inject=1",
        "--trunc=2",
        "--retry=1",
        "--compaction=1",
        "--workflow=1",
    ];
    let (frames, events) = workflow_turn_argv("s-9811", &knobs, want);
    assert_eq!(frames.len(), want, "五把同开一轮 56 帧（现测）");
    assert_eq!(events.len(), 50, "五把同开一轮 50 条 journal 事件");
    let injected: Vec<String> = injected_turn_tags();
    let mut expected = with_retry_tags(&injected, 1);
    let at = expected
        .iter()
        .position(|text| text == "event:assistant/message")
        .expect("帧序里得有 assistant/message");
    expected.insert(at, "event:assistant/attempt".to_string());
    let expected = with_workflow_tags(&with_compaction_tags(&expected, 1), 1);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        expected,
        "另三把刀挪不动两族的落点，且两族相加不改序"
    );
    assert!(
        chat_index(&events, "compaction/end") < chat_index(&events, "tool-workflow/run-start"),
        "压缩三发得整族排在 workflow 之前（信封时间就是序）"
    );
    assert!(chat_index(&events, "tool-workflow/run-end") < chat_index(&events, "turn/end"));
    assert_eq!(
        only_chat(&events, "tool-workflow/run-start")["data"]["runId"],
        json!("wf-1-run-1")
    );
    assert_eq!(
        only_chat(&events, "compaction/start")["data"]["compactionId"],
        json!("compact-1-cycle-1")
    );
    assert_eq!(
        workflow_seed_argv(&knobs).len(),
        3 * 50,
        "回读与 live 同档 ⇒ 种子页 3×50 = 150"
    );
}


// ------------------------------------------------------- 台账 #135 刀 4：`--step=` 五档 × 壳侧折叠
//
// 这批用例把「桩发的真 socket 帧」直接喂进**产品侧**那两张折叠台账
// （`blade2_rs::kernel::RunStatsLedger` = 主干家 A 的 `_runStats`，
// `blade2_rs::kernel::TrajectoryLedger` = 家 B 那一行的步计数），
// 逐档对表母本 rt5 §6 判据 1 与 ST3 §2 的现测帧预算 36/38/40/37/38。
//
// 刻意**不**动桩、也不动本文件里那三颗既有 `steps: 6` 钉子：那三颗钉的是 RPC 投影读数
// （`session/list` 的 `sessionStats`），本批钉的是壳侧折叠，两本账分列，不就地调和。

use blade2_rs::kernel::{RunStatsLedger, RunStepFold, TrajectoryLedger};

/// 同一档的**同一次启动**里取两样东西：种子会话 s-1001 的回读 journal +
/// 同一份内核在 `session/list` 上报的 RPC `sessionStats`。
/// 两条真相必须同源，否则「壳侧折叠 vs 投影读数」就成了跨进程拼数。
fn kw1_step_seed(level: u64) -> (Vec<Value>, Option<SessionStats>) {
    let knob = format!("--step={level}");
    let mut kernel = Kernel::start(&launch_with(&["--pace=0", &knob])).expect("握手");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("种子会话该带回历史");
    let events = page_events(&page);
    let stats = kernel
        .list_session_rows()
        .expect("session/list 应成功")
        .into_iter()
        .find(|row| row.info.id == "s-1001")
        .and_then(|row| row.projections.stats);
    kernel.shutdown();
    (events, stats)
}

/// 当场推帧那一趟 + 同一会话事后 `session/page` 回读那一趟（同一次启动）。
/// 返回（follow 流元素、剥壳后的 live 事件、回读事件）。
fn kw1_step_live(session: &str, level: u64, want: usize) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let knob = format!("--step={level}");
    let launch = launch_with(&["--pace=0", &knob]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call(
            "session/prompt",
            prompt_args(session, "走两步看看", "queue"),
        )
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    let lived = journal_of(&frames);
    let replayed = page_events(
        &kernel
            .call("session/page", page_args(session, None, None))
            .expect("回读该带回当场那一轮"),
    );
    kernel.shutdown();
    (frames, lived, replayed)
}

/// 整份 journal 走一遍产品侧两张台账，返回（家 A 折叠，家 B 逐轮步数 1..=3）。
fn kw1_fold_run(session: &str, events: &[Value]) -> (RunStepFold, Vec<i64>) {
    let mut run = RunStatsLedger::new();
    let mut trail = TrajectoryLedger::new();
    for event in events {
        run.note(session, event);
        trail.note(session, event);
    }
    let per_turn = (1..=3)
        .map(|turn| trail.fold(session).map(|fold| fold.steps(turn)).unwrap_or(0))
        .collect();
    (run.fold(session).cloned().unwrap_or_default(), per_turn)
}

/// 只看两型步边界帧，别的帧一律不进这张表。
fn kw1_step_events(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|event| {
            event["type"]
                .as_str()
                .is_some_and(|kind| kind.starts_with("step/"))
        })
        .cloned()
        .collect()
}

fn kw1_count_kind(events: &[Value], kind: &str) -> usize {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some(kind))
        .count()
}

/// 在 `anchor` 那**一发**之前依次插入 `added`（与桩 `st3_step_boundaries` 的
/// `position(== 锚点)` 同法：锚点现算，不写下标）。
fn kw1_splice_before(tags: &mut Vec<String>, anchor: &str, added: &[&'static str]) {
    assert!(!added.is_empty(), "插入表为空 = 用例自身写歪了");
    let at = tags
        .iter()
        .position(|text| text == anchor)
        .unwrap_or_else(|| panic!("关档那 36 格里该有 {anchor} 那一格"));
    tags.splice(at..at, added.iter().map(|text| text.to_string()));
}

/// 关档 36 格 + 该档的步边界 = 开档该见的完整帧序（逐字钉帧序，不接受「只是多几帧」）。
fn kw1_step_turn_tags(level: u64) -> Vec<String> {
    let mut tags: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    match level {
        1 => {
            kw1_splice_before(&mut tags, "event:request/header", &["event:step/start"]);
            kw1_splice_before(&mut tags, "event:turn/end", &["event:step/end"]);
        }
        2 => {
            kw1_splice_before(&mut tags, "event:request/header", &["event:step/start"]);
            kw1_splice_before(
                &mut tags,
                "event:system/message",
                &["event:step/end", "event:step/start"],
            );
            kw1_splice_before(&mut tags, "event:turn/end", &["event:step/end"]);
        }
        3 => kw1_splice_before(&mut tags, "event:turn/end", &["event:step/end"]),
        4 => {
            kw1_splice_before(&mut tags, "event:request/header", &["event:step/start"]);
            kw1_splice_before(&mut tags, "event:assistant/message", &["event:step/start"]);
        }
        _ => {}
    }
    tags
}

/// **判据一（帧型）**：五档 + 未定义档各演各的，帧数/帧序/`data` 键集逐字钉死。
/// 36/38/40/37/38 是 ST3 §2 的现测值（母本预算表），未定义档 9 回落关档。
#[test]
fn the_step_tiers_add_exactly_the_documented_frames_to_a_live_turn() {
    let table: [(u64, usize, usize, usize, &[i64]); 6] = [
        // 档, 帧数, step/start 发数, step/end 发数, 该见到的 data.step 序列
        (0, 36, 0, 0, &[]),
        (1, 38, 1, 1, &[1, 1]),
        (2, 40, 2, 2, &[1, 1, 2, 2]),
        (3, 37, 0, 1, &[1]),
        (4, 38, 2, 0, &[1, 2]),
        (9, 36, 0, 0, &[]),
    ];
    for (level, want_frames, opens, closes, want_steps) in table {
        let session = format!("s-970{level}");
        let (frames, lived, _) = kw1_step_live(&session, level, want_frames);
        assert_eq!(frames.len(), want_frames, "档 {level}：一轮该推这么多帧");
        assert_eq!(
            frames.iter().map(tag).collect::<Vec<_>>(),
            kw1_step_turn_tags(level),
            "档 {level}：帧序该是关档那 36 格 + 只在该出现的锚点前插该出现的那几格"
        );
        let step = kw1_step_events(&lived);
        assert_eq!(step.len(), opens + closes, "档 {level}：多出来的步帧发数");
        assert_eq!(
            (
                kw1_count_kind(&step, "step/start"),
                kw1_count_kind(&step, "step/end")
            ),
            (opens, closes),
            "档 {level}：两型各几发（档 3 只 end、档 4 只 start 是那两张负形）"
        );
        assert_eq!(
            step.iter()
                .filter_map(|event| event["data"]["step"].as_i64())
                .collect::<Vec<_>>()
                .as_slice(),
            want_steps,
            "档 {level}：`data.step` 序列（last-wins 覆盖起点要看得见那两发的号差）"
        );
        for event in &step {
            assert_eq!(
                sorted_keys(event.get("data").expect("步帧必有 data 对象")),
                ["step", "turn"],
                "档 {level}：步帧 data 的键集恰是 step+turn，多一枚都是自造形状"
            );
            assert_eq!(
                event["data"]["turn"], lived[0]["data"]["turn"],
                "档 {level}：步帧与本开的那一轮同号（内核 `invariant.js:40-49` 的步号闸）"
            );
            assert!(
                event["seq"].as_i64().unwrap_or(0) > 0,
                "档 {level}：真 socket 的步帧必带 seq（家 B 的按 seq 去重靠它）"
            );
        }
    }
}

/// **判据二（折叠 + 两份真相）**：三轮的种子 journal 逐档折出家 A 的开步闸读数，
/// 与同一份内核在 `session/list` 上报的 `sessionStats` 并排钉住。
/// 1/2/3 档两本账同号；**档 0 折叠 0 步、投影报 6 步**（接手态的剧本常量，本文件既有三颗
/// 钉子钉着，不许就地调和）；**档 4 折叠 0 轮 0 步、投影仍报 3 轮**（那条死状态条的输入）。
#[test]
fn the_step_tiers_fold_the_trunk_gate_against_the_projection_reading() {
    let table: [(u64, i64, i64, i64, i64, [i64; 3]); 5] = [
        // 档, 折叠 turns, 折叠 steps, 投影 turns, 投影 steps, 家 B 逐轮步数
        (0, 0, 0, 3, 6, [0, 0, 0]),
        (1, 3, 3, 3, 3, [1, 1, 1]),
        (2, 3, 6, 3, 6, [2, 2, 2]),
        (3, 3, 3, 3, 3, [1, 1, 1]),
        (4, 0, 0, 3, 0, [0, 0, 0]),
    ];
    for (level, turns, steps, rpc_turns, rpc_steps, trail) in table {
        let (events, stats) = kw1_step_seed(level);
        assert_eq!(
            kw1_count_kind(&kw1_step_events(&events), "step/end") as i64,
            steps,
            "档 {level}：种子页也演同一型（live 与回读同档是既有铁律）"
        );
        let (fold, per_turn) = kw1_fold_run("s-1001", &events);
        assert_eq!(
            (fold.turns, fold.steps),
            (turns, steps),
            "档 {level}：家 A 折叠实际 {fold:?}"
        );
        assert_eq!(per_turn, trail.to_vec(), "档 {level}：家 B 逐轮步数");
        assert_eq!(
            fold.open_step_turn, None,
            "档 {level}：走完一整轮还挂着开步闸 ⇒ 关步那一臂没接上"
        );
        assert_eq!(
            fold.strip_visible(),
            steps > 0,
            "档 {level}：可见判据该是主干 `RunStats.cs:224-228` 的 `Steps <= 0 ⇒ Collapsed`"
        );
        let stats = stats.unwrap_or_else(|| panic!("档 {level}：种子行该带 sessionStats"));
        assert_eq!(
            (stats.turns, stats.steps),
            (rpc_turns, rpc_steps),
            "档 {level}：投影读数（既有三颗 `steps: 6` 钉子钉的就是档 0 这一格）"
        );
    }
    // 档 4 那一格单独钉：`llmMs` 已偷偷累计、条却必须整条收起 ⇒ 别拿用量当可见判据。
    let (four, _) = kw1_step_seed(4);
    let (fold, _) = kw1_fold_run("s-1001", &four);
    assert!(
        fold.llm_ms > 0 && fold.steps == 0 && !fold.strip_visible(),
        "档 4 该演「有 llmMs 却没有步」那张死条，实际 {fold:?}"
    );
    // 档 0 也不是空转：usage 与 toolMs 在开步闸之外（主干 `:112` 那半），
    // 少了这一颗钉子，判据二表里的「0 步」可以靠「整趟什么都没折到」假绿。
    let (zero, _) = kw1_step_seed(0);
    let (off, _) = kw1_fold_run("s-1001", &zero);
    assert!(
        off.total_tokens > 0 && off.tool_ms > 0 && off.steps == 0 && off.turns == 0,
        "档 0 该是「落账了但没有步」，不是整趟空转，实际 {off:?}"
    );
}

/// **判据三（两条路同号）**：当场推帧与事后回读同一会话，折出同一个 `(turns, steps)`。
/// 分叉的 live/follow 与 page 回放最终都进 `Shell::apply_journal_event` 那**一个**入口
/// （主干 `RenderEventCore` 同款），两条路不同号就是主干没有的第三种状态。
#[test]
fn a_live_turn_and_its_replay_fold_the_same_steps() {
    for (level, want_frames) in [(1u64, 38usize), (2, 40)] {
        let session = format!("s-971{level}");
        let (frames, lived, replayed) = kw1_step_live(&session, level, want_frames);
        assert_eq!(frames.len(), want_frames, "档 {level}：收帧预算");
        assert!(
            !lived.is_empty() && lived.len() == replayed.len(),
            "档 {level}：live {} 帧 vs 回读 {} 帧该同数",
            lived.len(),
            replayed.len()
        );
        let (live_fold, live_trail) = kw1_fold_run(&session, &lived);
        let (back_fold, back_trail) = kw1_fold_run(&session, &replayed);
        assert_eq!(
            (live_fold.turns, live_fold.steps),
            (back_fold.turns, back_fold.steps),
            "档 {level}：两条路折出的轮数/步数不同号 ⇒ live {live_fold:?} vs 回读 {back_fold:?}"
        );
        assert_eq!(
            live_trail, back_trail,
            "档 {level}：家 B 逐轮步数两条路不同号"
        );
        assert!(
            live_fold.steps == level as i64 && live_fold.turns == 1,
            "档 {level}：单轮该折出 turns=1、steps=该档每轮 step/end 的发数，实际 {live_fold:?}"
        );
        assert_eq!(
            serde_json::to_string(&kw1_step_events(&lived)).unwrap(),
            serde_json::to_string(&kw1_step_events(&replayed)).unwrap(),
            "档 {level}：两条路的步帧逐字节同形（否则同一轮在两条路上折出不同起点）"
        );
    }
}

// ==================== #57：`--ctx=` 走真 socket 的集成用例（SP13 §6.1 的 T0..T7） ====================
// SP13 §5.3-3 + §8-4 的现测：桩侧 `--ctx=` 的档位（`0..=7`，0 = 关）与 fake_dsh 的**桩内自测**都在册，
// 而 `tests/ipc.rs` 里 `ctx` / `request/context` **零命中** ⇒ 「三态端到端」今天在线上一格都没钉。
// 下面这批每一颗都开真 `fake_dsh.exe`、发一条真 prompt、收整轮 follow 流，再按**宿主那一发同名调用**
// （`src/main.rs` 的 `self.context_meters.track(session, kind, &event["data"])`）把线上帧喂进真算子层
// ⇒ `PressureProbe` 的三态 `Tokens` / `Absent` / `Poisoned` 在「线 → 折叠 → 能不能画」这条真链上各得其所。
// 帧预算**以现跑为准**：关档仍 36、插一发 `request/context` 变 37、档 3 那族四发变 40。

/// `--ctx=<level>` 的一整轮：（follow 流帧序列，剥壳后的 journal 事件）。
fn ctx_turn(session: &str, level: u64, want: usize) -> (Vec<Value>, Vec<Value>) {
    let ctx_arg = format!("--ctx={level}");
    let launch = launch_with(&["--pace=0", &ctx_arg]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call(
            "session/prompt",
            prompt_args(session, "容量圈的这一轮", "queue"),
        )
        .expect("session/prompt 应被接受");
    let frames = item_values(&collect_within(&mut mux, &follow, want, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 压根不带 `--ctx=` 的那一轮（档 0 的对照母本，写法照 `retry_level_zero_…` 那颗）。
fn ctx_unflagged_turn(session: &str) -> (Vec<Value>, Vec<Value>) {
    let launch = launch_with(&["--pace=0"]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call(
            "session/prompt",
            prompt_args(session, "容量圈的这一轮", "queue"),
        )
        .expect("session/prompt 应被接受");
    let frames =
        item_values(&collect_within(&mut mux, &follow, TURN_FRAMES, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 开档该见的帧序：`CTX_FRAME_AT = 950` 落在既有 `request/header`(900) 与 `assistant/attempt`(1500)
/// 之间 ⇒ 关档 `turn_tags()` 的下标 3 处插 `count` 发 `event:request/context`（档 3 是四发连排）。
fn ctx_turn_tags(count: usize) -> Vec<String> {
    let mut tags: Vec<String> = turn_tags().into_iter().map(String::from).collect();
    tags.splice(
        3..3,
        std::iter::repeat("event:request/context".to_string()).take(count),
    );
    tags
}

/// 一轮里的 `request/context`（按帧序 = seq 升序；桩那四发的 time 依次 +10ms）。
fn ctx_context_frames(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|event| event["type"].as_str() == Some("request/context"))
        .cloned()
        .collect()
}

/// 宿主口径的折叠：整轮逐帧喂真 `ContextMeterBook`（不拿算子层自己的单测当替身）。
fn ctx_fold(session: &str, events: &[Value]) -> blade2_rs::contextmeter::ContextMeterBook {
    let mut book = blade2_rs::contextmeter::ContextMeterBook::new();
    for event in events {
        if let Some(kind) = event["type"].as_str() {
            book.track(session, kind, &event["data"]);
        }
    }
    book
}

/// 该型 stream 片的存在性判据：**无** usage 型片（档 6/7 都要它，写成一棵免得两处各写一遍）。
fn ctx_no_usage_chunk(data: &Value) -> bool {
    data.get("stream").map_or(true, |pieces| {
        !pieces.as_array().map_or(false, |list| {
            list.iter().any(|piece| {
                piece.get("chunk").map_or(false, |chunk| {
                    chunk.get("type").and_then(Value::as_str) == Some("usage")
                })
            })
        })
    })
}

/// **T0（档 0 = 关）**：一帧不加、一键不改，且与「argv 压根不带 `--ctx=`」那一轮逐字节同形。
/// 反向半颗：整轮序列化里连 `request/context` 子串都不许出现（不是发了一发空 data）。
/// ⇒ 这枚旋钮默认不污染既有那批按 36 格帧序钉死的用例。
#[test]
fn ctx_level_zero_adds_no_request_context_frame_at_all() {
    let (frames, events) = ctx_turn("s-9700", 0, TURN_FRAMES);
    assert_eq!(frames.len(), TURN_FRAMES, "档 0 一轮仍 36 帧（现测）");
    assert_eq!(events.len(), 30, "档 0 一轮仍 30 条 journal 事件");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "档 0 的帧序与关档逐格相等"
    );
    assert!(ctx_context_frames(&events).is_empty(), "档 0 不许发 request/context");
    let round = frames.iter().map(Value::to_string).collect::<String>();
    assert!(
        !round.contains("request/context"),
        "整轮序列化里连 `request/context` 子串都不许出现"
    );
    let (_, plain) = ctx_unflagged_turn("s-9701");
    assert_eq!(
        serde_json::to_string(&json_without_time(&events)).unwrap(),
        serde_json::to_string(&json_without_time(&plain)).unwrap(),
        "显式档 0 与不带旋钮那一轮逐字节同形（母本 = `retry_level_zero_…`）"
    );
    let book = ctx_fold("s-9700", &events);
    assert!(
        book.occupancy("s-9700").is_none(),
        "关档没有分母 ⇒ 圈与面板一律不可达（不许折成 0%）"
    );
}

/// **T1（档 1 = `Tokens`）**：线上补一发 `request/context{contextWindow:128000}`，落点在
/// `request/header` 与 `assistant/attempt` 之间（= T7 的落点序），同轮 `assistant/message` 的
/// prompt 侧三桶和 = 分子 ⇒ 折出 `900 / 128000 = 1%`（`used = 0` 是合法读数、与「没读数」两回事）。
#[test]
fn ctx_level_one_hands_the_panel_its_denominator_over_the_socket() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = ctx_turn("s-9710", 1, want);
    assert_eq!(frames.len(), want, "档 1 一轮 37 帧（现测）");
    assert_eq!(events.len(), 31, "档 1 的 journal 恰多一条 request/context");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(1),
        "新增那发该排在 request/header 之后、assistant/attempt 之前"
    );
    let contexts = ctx_context_frames(&events);
    assert_eq!(contexts.len(), 1);
    let data = &contexts[0]["data"];
    assert_eq!(data["contextWindow"], json!(128_000), "分母单位是 token 个数");
    assert_eq!(data["provider"], json!("dsh-test"), "与同轮 request/header 同源");
    assert_eq!(data["model"], json!("dsh-test-model"));
    assert_eq!(
        sorted_keys(data),
        ["contextWindow", "model", "provider", "turn"]
    );
    let header = single_event(&events, "request/header");
    let attempt = single_event(&events, "assistant/attempt");
    let here = contexts[0]["time"].as_i64().expect("信封 time");
    assert!(
        header["time"].as_i64().unwrap() < here && here < attempt["time"].as_i64().unwrap(),
        "T7：time 必须落在 900 与 1500 之间且单调（CTX_FRAME_AT = 950）"
    );
    assert!(
        contexts[0]["seq"].as_i64().unwrap() < attempt["seq"].as_i64().unwrap(),
        "seq 也得同向，否则 live 与回读两条路折出的分母不同号"
    );
    let book = ctx_fold("s-9710", &events);
    let occ = book.occupancy("s-9710").expect("档 1 六格全可算");
    assert_eq!(
        occ,
        blade2_rs::contextmeter::ContextOccupancy {
            used: 900,
            window: 128_000,
            percent: 1
        },
        "分子 = prompt 侧三桶和（input+cacheRead+cacheWrite），不含 outputTokens/totalTokens"
    );
    assert!(book.is_visible("s-9710"), "读数齐备 ⇒ 圈在场");
}

/// **T2（档 2 = 分母的省略形）**：同位置那一发**整个不带 `contextWindow` 键**（内核
/// `contextWindow === void 0 ? {} : {…}` 的真形）⇒ 分子有、分母 `None` ⇒ **整张面板不可达**。
/// 这颗就是那枚硬闸的线上半边：**缺分母不许显 0%**。
#[test]
fn ctx_level_two_omits_the_context_window_key_so_the_panel_stays_unreachable() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = ctx_turn("s-9720", 2, want);
    assert_eq!(frames.len(), want, "档 2 仍只插那一发 ⇒ 37 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(1)
    );
    let contexts = ctx_context_frames(&events);
    assert_eq!(contexts.len(), 1);
    let data = &contexts[0]["data"];
    assert!(
        data.get("contextWindow").is_none(),
        "省略形 = **整个键都不给**，不是给 `null` 更不是给 0（strict codec 那族红线）"
    );
    assert_eq!(sorted_keys(data), ["model", "provider", "turn"]);
    let message = single_event(&events, "assistant/message");
    assert!(
        message["data"].get("usage").is_some(),
        "这一档剥的是分母不是分子 ⇒ 分子照旧在场（Absent 那一型是档 6 的活）"
    );
    let book = ctx_fold("s-9720", &events);
    let state = book.state("s-9720").expect("格子该在");
    assert_eq!(state.context_window, None, "分母不许被折成 0");
    assert!(
        state.pressure_tokens.is_some(),
        "分子采到了 ⇒ 「不可达」纯粹是分母缺位，不是整帧没数据"
    );
    assert!(
        book.occupancy("s-9720").is_none(),
        "缺分母 ⇒ 圈收起、面板整张不画（主干 CM:222-225）"
    );
}

/// **T3 前半（档 3 = 有毒值族四发）**：`128000` / `"128000"` / `0` / `-5` 依次在场，
/// 主干「是 Number 且 > 0」双闸只认头一发 ⇒ 后三发**必须不改变读数**（有毒 ≠ 归零）。
#[test]
fn ctx_level_three_poisoned_denominators_never_reclaim_the_good_one() {
    let want = TURN_FRAMES + 4;
    let (frames, events) = ctx_turn("s-9730", 3, want);
    assert_eq!(frames.len(), want, "档 3 一轮 40 帧（现测：四发 request/context）");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(4),
        "四发连排插在 request/header 之后"
    );
    let contexts = ctx_context_frames(&events);
    assert_eq!(contexts.len(), 4);
    let windows: Vec<&Value> = contexts.iter().map(|c| &c["data"]["contextWindow"]).collect();
    assert_eq!(windows[0], &json!(128_000));
    assert_eq!(windows[1], &json!("128000"), "字符串那一发是毒的");
    assert_eq!(windows[2], &json!(0));
    assert_eq!(windows[3], &json!(-5));
    let times: Vec<i64> = contexts
        .iter()
        .map(|c| c["time"].as_i64().expect("信封 time"))
        .collect();
    assert!(
        times.windows(2).all(|pair| pair[0] < pair[1]),
        "四发的 time 逐格单调（+10ms 铺开），否则「后到者」的次序没有证据"
    );
    let book = ctx_fold("s-9730", &events);
    let occ = book.occupancy("s-9730").expect("健康那发该把圈点亮");
    assert_eq!(
        occ.window, 128_000,
        "有毒三发一律「忽略并保留上一份」，不许把分母拉成 None/0/-5"
    );
    assert_eq!(occ.percent, 1);
}

/// **T3 后半（档 4 = `Poisoned`）**：`assistant/message` 上补的 stream 片其 `chunk.type` 是**数字** `7`
/// （内核真发不出这一型；桩演它是为了让主干 `CM:106` 那句 `GetString()` 从不可证变可证），
/// 且同一帧仍带 `data.usage` ⇒ 主干抛异常、**整帧作废**：连那份 `data.usage` 也不写。
#[test]
fn ctx_level_four_poisons_the_whole_message_frame_not_just_the_bad_chunk() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = ctx_turn("s-9740", 4, want);
    assert_eq!(frames.len(), want, "档 4 只加那一发分母帧 ⇒ 37 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(1)
    );
    let message = single_event(&events, "assistant/message");
    let pieces = message["data"]["stream"].as_array().expect("档 4 补了 stream");
    assert_eq!(pieces.len(), 1);
    let chunk = &pieces[0]["chunk"];
    assert!(
        chunk["type"].is_number(),
        "`chunk.type` 必须是**数字** —— 字符串标签那一型走的是正常臂（档 5）"
    );
    assert_eq!(chunk["type"], json!(7));
    assert_eq!(chunk["usage"]["inputTokens"], json!(777_000));
    assert_eq!(
        blade2_rs::contextmeter::pressure_tokens_of(&message["data"], true),
        blade2_rs::contextmeter::PressureProbe::Poisoned,
        "「整帧作废」属算子层口径，ipc 这半边只保证帧面给得出这一型"
    );
    assert!(
        message["data"].get("usage").is_some(),
        "同一帧仍带 data.usage ⇒ 「作废」才有代价可证（跳过毒片就会记进 777000）"
    );
    let book = ctx_fold("s-9740", &events);
    let state = book.state("s-9740").expect("格子该在");
    assert_eq!(state.context_window, Some(128_000), "分母那发没被牵连");
    assert_eq!(
        state.pressure_tokens, None,
        "分子必须整帧作废 ⇒ 既不是 777000（跳过毒片）也不是 900（毒帧照收）"
    );
    assert!(book.occupancy("s-9740").is_none(), "算不出 ⇒ 面板整张不画");
}

/// **T4 前半（档 5 = last-wins）**：stream 里那发 `chunk.type == "usage"` 的桶与同帧
/// `data.usage` **不同值** ⇒ 「取错源」在这一档可分；分子取片里那份 52000。
#[test]
fn ctx_level_five_lets_the_usage_chunk_override_data_usage() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = ctx_turn("s-9750", 5, want);
    assert_eq!(frames.len(), want, "档 5 一轮 37 帧");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(1)
    );
    let message = single_event(&events, "assistant/message");
    let pieces = message["data"]["stream"].as_array().expect("档 5 补了 usage 片");
    let chunk = &pieces[0]["chunk"];
    assert_eq!(chunk["type"], json!("usage"), "内核真形：字符串标签");
    assert_ne!(
        chunk["usage"]["inputTokens"],
        message["data"]["usage"]["inputTokens"],
        "两个源不同值，否则「取错源」在这一档分不出来"
    );
    assert_eq!(chunk["usage"]["inputTokens"], json!(50_000));
    assert_eq!(
        blade2_rs::contextmeter::pressure_tokens_of(&message["data"], true),
        blade2_rs::contextmeter::PressureProbe::Tokens(52_000),
        "prompt 侧三桶和 = 50000+1200+800，后到者胜"
    );
    let book = ctx_fold("s-9750", &events);
    let occ = book.occupancy("s-9750").expect("档 5 六格可算");
    assert_eq!(occ.used, 52_000, "分子必须取片里那份（取成 900 就是 bug）");
    assert_eq!(occ.window, 128_000);
    assert_eq!(occ.percent, 41, "100*52000/128000 = 40.625 ⇒ 半数远离零 = 41");
}

/// **T4 后半（档 6 = `Absent`）**：`assistant/message` 的 `usage` 键整个剥掉 ⇒ 两型来源全空。
/// 判据落在**帧面**上（ipc 不判 UI），「壳侧算不出 ⇒ 不画」由 `occupancy` 为 `None` 同口钉住。
#[test]
fn ctx_level_six_strips_the_usage_key_and_leaves_nothing_to_fold() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = ctx_turn("s-9760", 6, want);
    assert_eq!(frames.len(), want, "档 6 一轮 37 帧（剥键不加帧）");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(1)
    );
    let message = single_event(&events, "assistant/message");
    assert!(
        message["data"].get("usage").is_none(),
        "档 6 演的是「整个键都不给」，不是 `usage: null`"
    );
    assert!(
        ctx_no_usage_chunk(&message["data"]),
        "也没有 usage 型 stream 片 ⇒ Absent 才是「两路来源全空」"
    );
    assert_eq!(
        blade2_rs::contextmeter::pressure_tokens_of(&message["data"], true),
        blade2_rs::contextmeter::PressureProbe::Absent
    );
    let book = ctx_fold("s-9760", &events);
    let state = book.state("s-9760").expect("格子该在");
    assert_eq!(state.context_window, Some(128_000), "分母照给：这一档缺的是分子");
    assert_eq!(state.pressure_tokens, None);
    assert!(
        book.occupancy("s-9760").is_none(),
        "有分母没分子 ⇒ 依然整张不画，不许退化成 `0%`"
    );
}

/// **档 7（attempt 的诱饵 = 第二个 `Absent` 出口）**：分母照给，且既有那发失败的
/// `assistant/attempt` 挂着 `data.usage{inputTokens:555000,…}` ⇒ 官方对 attempt 只认 stream 片
/// （`CM:68` 那句实参 `type == "assistant/message"`）⇒ 正确折叠**必须忽略**诱饵。
#[test]
fn ctx_level_seven_ignores_the_usage_bait_hung_on_an_attempt() {
    let want = TURN_FRAMES + 1;
    let (frames, events) = ctx_turn("s-9770", 7, want);
    assert_eq!(frames.len(), want, "档 7 一轮 37 帧（诱饵挂既有帧）");
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        ctx_turn_tags(1)
    );
    let attempts = attempt_events(&events);
    assert_eq!(attempts.len(), 1, "档 7 不加 attempt 帧，只往既有那一发上挂东西");
    let bait = &attempts[0]["data"];
    assert_eq!(bait["usage"]["inputTokens"], json!(555_000));
    assert!(
        ctx_no_usage_chunk(bait),
        "该帧**无** usage 型 stream 片 ⇒ 诱饵只能被「attempt 不读 data.usage」挡住"
    );
    assert_eq!(
        blade2_rs::contextmeter::pressure_tokens_of(bait, false),
        blade2_rs::contextmeter::PressureProbe::Absent,
        "attempt 那一型（`read_direct_usage = false`）对该帧的诱饵就是 Absent"
    );
    let book = ctx_fold("s-9770", &events);
    let occ = book.occupancy("s-9770").expect("正文那一轮的读数照旧齐备");
    assert_eq!(occ.used, 900, "读错成诱饵就是一张假满圈（555000 > 128000 会被夹成 100%）");
    assert_eq!(occ.window, 128_000);
    assert_eq!(occ.percent, 1);
    let round = serde_json::to_string(&events).unwrap();
    assert!(
        round.contains("555000"),
        "诱饵确实挂在帧上（否则上面那枚等式就是恒真）"
    );
}

/// **T5（未定义档回落）+ T6（相加不是替代）**：认不出的档位在线上与不开旋钮同形；
/// `--ctx=1` 叠在 `--trunc=2` 上，增量 = 两档各自增量之和（36 + 1 + 1 = 38）。
#[test]
fn ctx_unknown_levels_fall_back_and_two_knobs_add_instead_of_replacing() {
    for (probe, raw) in [("s-9780", "8"), ("s-9781", "99"), ("s-9782", "nope")] {
        let knob = format!("--ctx={raw}");
        let launch = launch_with(&["--pace=0", &knob]);
        let (mut kernel, mut mux, follow) =
            open_stream_by(&launch, "session/follow", follow_args(probe));
        collect(&mut mux, &follow, 1);
        kernel
            .call("session/prompt", prompt_args(probe, "脏档位", "queue"))
            .expect("脏档位不该影响 prompt 通路");
        let frames =
            item_values(&collect_within(&mut mux, &follow, TURN_FRAMES, DRAIN_BUDGET), &follow);
        let events = journal_of(&frames);
        assert_eq!(frames.len(), TURN_FRAMES, "`--ctx={raw}` 该回落成关档");
        assert!(ctx_context_frames(&events).is_empty(), "回落档不许发帧");
        assert!(ctx_fold(probe, &events).occupancy(probe).is_none());
        kernel.shutdown();
    }
    let trunc_arg = "--trunc=2".to_string();
    let ctx_arg = "--ctx=1".to_string();
    let launch = launch_with(&["--pace=0", &trunc_arg, &ctx_arg]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args("s-9790"));
    collect(&mut mux, &follow, 1);
    kernel
        .call("session/prompt", prompt_args("s-9790", "两枚旋钮叠开", "queue"))
        .expect("叠开不该影响 prompt 通路");
    let frames = item_values(
        &collect_within(&mut mux, &follow, TURN_FRAMES + 2, DRAIN_BUDGET),
        &follow,
    );
    let events = journal_of(&frames);
    kernel.shutdown();
    assert_eq!(
        frames.len(),
        TURN_FRAMES + 2,
        "36 + 1（trunc 档 2 的 attempt）+ 1（ctx 档 1 的分母）= 38 ⇒ 相加不是替代"
    );
    assert_eq!(ctx_context_frames(&events).len(), 1, "ctx 那一发仍在");
    assert_eq!(attempt_events(&events).len(), 2, "trunc 那一发 attempt 也在");
}

/// **T1 的回读半边**：同一次启动里 live 那一轮与 `session/page` 回放必须折出**同一枚**读数
/// （桩 `ctx_level_of_process()` 两颗取法共用的铁律：不同号 ⇒ 切回会话瞬间读数会跳一次）。
#[test]
fn ctx_level_one_live_and_page_replay_fold_the_same_reading() {
    let session = "s-9800";
    let knob = "--ctx=1".to_string();
    let launch = launch_with(&["--pace=0", &knob]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call(
            "session/prompt",
            prompt_args(session, "容量圈的这一轮", "queue"),
        )
        .expect("session/prompt 应被接受");
    let frames = item_values(
        &collect_within(&mut mux, &follow, TURN_FRAMES + 1, DRAIN_BUDGET),
        &follow,
    );
    let lived = journal_of(&frames);
    let replayed = page_events(
        &kernel
            .call("session/page", page_args(session, None, None))
            .expect("回读该带回当场那一轮"),
    );
    kernel.shutdown();
    assert_eq!(
        ctx_context_frames(&replayed).len(),
        1,
        "回放那一版也得带 request/context ⇒ 桩的两颗取法同型"
    );
    let live = ctx_fold(session, &lived)
        .occupancy(session)
        .expect("live 该有读数");
    let back = ctx_fold(session, &replayed)
        .occupancy(session)
        .expect("回读也该有读数");
    assert_eq!(live, back, "两条路折出的读数不同号 ⇒ 切回会话瞬间圈会跳一次");
}

// ==================== #139 件一：`--prompt=` 那两发 system/message 的「真过 socket」证据 ====================
// 桩内那 9 枚 `prompt_frame_tests` 证的是纯函数层；下面四条证的是「真起进程、真走
// session/prompt、真从 session/follow 收帧」。判据全取自报告 §2a 的**现测**表：四档恒
// 36 元素 / 30 事件 / 恰 2 发，与档 0 的差异格 0/2/2/1。本刀属「改 data 不加帧」那一族
// ⇒ 收帧预算恒为 `TURN_FRAMES`，谁按档抬预算谁就是在猜（对照 `trunc_turn()` 那族加帧型）。

/// 收真 socket 的一整轮（自定义 argv），返回（follow 流元素、剥壳后的 journal 事件）。
/// 节奏固定 `--pace=0`：这条只查数据面，节流那一维由下面最后一条单独证。
fn prompt_turn_argv(session: &str, knobs: &[&str]) -> (Vec<Value>, Vec<Value>) {
    let mut argv: Vec<&str> = vec!["--pace=0"];
    argv.extend_from_slice(knobs);
    let launch = launch_with(&argv);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(session));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "开流先有快照");
    kernel
        .call("session/prompt", prompt_args(session, "系统提示词这一轮", "queue"))
        .expect("session/prompt 应被接受");
    let frames =
        item_values(&collect_within(&mut mux, &follow, TURN_FRAMES, DRAIN_BUDGET), &follow);
    let events = journal_of(&frames);
    kernel.shutdown();
    (frames, events)
}

/// 同上，但旋钮按档号给（`--prompt=<level>`）。
fn prompt_turn(session: &str, level: u64) -> (Vec<Value>, Vec<Value>) {
    let knob = format!("--prompt={level}");
    prompt_turn_argv(session, &[&knob])
}

/// **四档恒不加帧**（现测 36/30/2）：帧序与关档逐格相同，且与档 0 的差异格恰为 0/2/2/1。
/// 差异格只数剥掉墙钟之后的 `{seq,type,data}` —— 不剥 `time` 的话每次起桩三十格全不等。
#[test]
fn fake_kernel_prompt_knob_adds_no_frame_at_any_level() {
    let mut base: Vec<Value> = Vec::new();
    let mut changed: Vec<(u64, usize)> = Vec::new();
    for level in 0..=3u64 {
        let session = format!("s-1390{}", 1 + level);
        let (frames, events) = prompt_turn(&session, level);
        assert_eq!(frames.len(), TURN_FRAMES, "档 {level} 的 follow 元素数不是现测的 36");
        assert_eq!(events.len(), 30, "档 {level} 的 journal 事件数不是现测的 30");
        assert_eq!(
            frames.iter().map(tag).collect::<Vec<_>>(),
            turn_tags(),
            "档 {level} 改了关档帧序（本刀只许改 data，一帧不许加减）"
        );
        assert_eq!(event_types(&events), turn_skeleton(), "档 {level} 的 journal 骨架动了");
        let pair = chat_events(&events, "system/message");
        assert_eq!(pair.len(), 2, "档 {level} 该恰有两发 system/message（恒两发是本刀的前提）");
        let stripped = json_without_time(&events);
        if level == 0 {
            base = stripped.clone();
        }
        changed.push((
            level,
            stripped
                .iter()
                .zip(base.iter())
                .filter(|(here, there)| here != there)
                .count(),
        ));
    }
    assert_eq!(base.len(), 30, "档 0 的参照必须先立起来");
    // 现测差异格：档 0 一格不动、档 1/2 各动那两发、档 3 只动首发（第二发整个不带键）。
    assert_eq!(
        changed,
        vec![(0, 0), (1, 2), (2, 2), (3, 1)],
        "逐档差异格数对不上报告 §2a 的现测表（上限就是那两发，永不超过 2）"
    );
}

/// **档 1 走真 socket 的逐字形状**：内核 v3 的 `message` 恰四键、两串互异（first/update 两臂）、
/// `turn` 恒本轮，且单开本刀时 data 恰 `{message,turn}` 两键（`step` 归 `--step=` 所有）。
#[test]
fn fake_kernel_prompt_level_one_carries_the_kernel_message_over_the_wire() {
    let (_, events) = prompt_turn("s-13905", 1);
    let pair = chat_events(&events, "system/message");
    assert_eq!(pair.len(), 2, "档 1 该有恰两发 system/message");
    // 桩的确定串口径（`sys-{turn}-{slot}`）与两串互异，是「同轮演到 first/update」的端到端版。
    let want = |slot: u64, text: &str| {
        json!({
            "turn": 1,
            "message": {
                "content": [{ "text": text, "type": "text" }],
                "id": format!("sys-1-{slot}"),
                "role": "system",
                "source": { "kind": "plugin", "plugin": "@deepseek-ai/dsh-system-prompt" },
            },
        })
    };
    assert_eq!(
        pair[0]["data"],
        want(0, "桩内系统提示词 A：本轮投影出的正文，测试按它逐字断言。"),
        "档 1 首帧的 data 不是内核那一型"
    );
    assert_eq!(
        pair[1]["data"],
        want(1, "桩内系统提示词 B：同轮第二帧的正文，与 A 那一串不同。"),
        "档 1 第二帧的 data 不是内核那一型"
    );
    assert_ne!(pair[0]["data"], pair[1]["data"], "两串相同就演不出 first/update 两臂");
    for (slot, event) in pair.iter().enumerate() {
        assert_eq!(
            sorted_keys(&event["data"]),
            ["message", "turn"],
            "单开 --prompt= 时键集必须是 {{turn,message}}（step 键归 --step= 管）: {event}"
        );
        let message = &event["data"]["message"];
        assert_eq!(
            sorted_keys(message),
            ["content", "id", "role", "source"],
            "内核 schema 把 message 钉成恰四键: {event}"
        );
        assert_eq!(message["role"], json!("system"), "role 只许是 system: {event}");
        assert_eq!(
            sorted_keys(&message["source"]),
            ["kind", "plugin"],
            "source 恰两键: {event}"
        );
        assert_eq!(message["source"]["kind"], json!("plugin"));
        assert_eq!(
            message["id"].as_str().unwrap_or_default(),
            format!("sys-1-{slot}"),
            "id 是桩的确定串，逐字可断言（真内核那是 randomUUID）"
        );
        let content = message["content"].as_array().expect("content 是数组");
        assert_eq!(content.len(), 1, "档 1 每发恰一颗 text 块");
        assert_eq!(sorted_keys(&content[0]), ["text", "type"]);
        assert!(
            content[0]["text"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "档 1 的正文不许是空壳: {event}"
        );
    }
    assert_ne!(
        pair[0]["data"]["message"]["content"][0]["text"],
        pair[1]["data"]["message"]["content"][0]["text"],
        "两发的正文必须互异，否则主干那一臂的 update 无从谈起"
    );
    assert_eq!(chat_index(&events, "system/message"), 24, "两发之前恒有 24 格事件（现测）");
    assert_eq!(
        pair[1]["seq"].as_i64().expect("有 seq"),
        pair[0]["seq"].as_i64().expect("有 seq") + 1,
        "两发连号，中间不许插帧"
    );
}

/// **档 0 的绝对字节哨兵**（补报告 §4 点出的缺口）：关档那两发的 data 逐字 = `{"turn":1}`，
/// 整轮骨架与 `turn_skeleton()` 同序；越界/脏档必须与关档**逐字节**相等（回落是压住的）。
#[test]
fn fake_kernel_prompt_default_level_is_byte_frozen_on_both_frames() {
    let (frames, events) = prompt_turn("s-13906", 0);
    let stripped = json_without_time(&events);
    assert_eq!(stripped.len(), 30, "关档 journal 发数动了");
    assert_eq!(frames.len(), TURN_FRAMES, "关档一轮仍 36 帧");
    assert_eq!(event_types(&events), turn_skeleton(), "关档的 journal 骨架动了");
    let pair = chat_events(&events, "system/message");
    assert_eq!(pair.len(), 2, "档 0 也是恒两发（本刀改的是 data，不是发不发）");
    for event in &pair {
        assert_eq!(
            event["data"],
            json!({ "turn": 1 }),
            "默认档不许给 system/message 补任何一键（#139 病灶本体）: {event}"
        );
        assert_eq!(sorted_keys(&event["data"]), ["turn"], "档 0 多键了: {event}");
    }
    // 越界值与脏串一律回落 0：整轮剥掉墙钟后与关档参照逐格相等。
    for knob in ["--prompt=4", "--prompt=99", "--prompt=三", "--prompt=-1"] {
        let (dirty_frames, dirty_events) = prompt_turn_argv("s-13906", &[knob]);
        assert_eq!(dirty_frames.len(), TURN_FRAMES, "{knob} 必须等价于关档");
        assert_eq!(
            dirty_frames.iter().map(tag).collect::<Vec<_>>(),
            frames.iter().map(tag).collect::<Vec<_>>(),
            "{knob} 的帧序与关档不等"
        );
        assert_eq!(
            json_without_time(&dirty_events),
            stripped,
            "脏档 {knob} 必须与关档逐字节相等"
        );
    }
}

/// **关节奏时那两帧拍得到中途**：现测关档帧序里两发 `system/message` 占第 31/32 格
/// （前 24 格 journal 事件 + 6 格流片元素），故中途窗口取 32 元素 —— 此时轮还没收尾
/// （`turn/end` 在最后一格），UI 抢到的这两格就已经带着可读正文。
#[test]
fn fake_kernel_prompt_frames_are_observable_mid_stream_when_paced() {
    const MID_WINDOW: usize = 32;
    let pace_arg = format!("--pace={MID_PACE_MS}");
    let launch = launch_with(&[pace_arg.as_str(), "--prompt=1"]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args("s-13907"));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "快照是现成的，不该被节奏拖住");

    let started = Instant::now();
    kernel
        .call("session/prompt", prompt_args("s-13907", "节流 + 提示词", "queue"))
        .expect("session/prompt 应被接受");
    let head = collect_within(&mut mux, &follow, MID_WINDOW, DRAIN_BUDGET);
    assert_eq!(head.len(), MID_WINDOW, "只想要 {MID_WINDOW} 帧就该只到 {MID_WINDOW} 帧");
    let intervals = MID_WINDOW - 1;
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(MID_PACE_MS * intervals as u64 - 60),
        "{MID_WINDOW} 帧之间该睡出 {intervals} 个间隔（并成一批就拍不到中途）: {elapsed:?}"
    );
    let frames = item_values(&head, &follow);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags()[..MID_WINDOW],
        "节流只许改「什么时候到」，不许改「到什么」"
    );
    let events = journal_of(&frames);
    assert_eq!(events.len(), 26, "中途窗口的 32 元素 = 26 条 journal 事件 + 6 格流片（现测）");
    assert!(
        event_types(&events).iter().all(|kind| *kind != "turn/end"),
        "这一窗口必须停在轮尾之前，否则中途态不存在"
    );
    let pair = chat_events(&events, "system/message");
    assert_eq!(pair.len(), 2, "两发 system/message 必须都在中途窗口里");
    for event in &pair {
        assert!(
            event["data"]["message"]["content"][0]["text"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "中途拍到的这一格必须有可读正文，否则 UI 抢到的是一张空壳: {event}"
        );
    }
    let rest = collect_within(&mut mux, &follow, TURN_FRAMES - MID_WINDOW, DRAIN_BUDGET);
    let tail = item_values(&rest, &follow);
    assert_eq!(tail.len() + frames.len(), TURN_FRAMES, "关节奏的整轮仍是 36 格");
    let mut whole = frames.clone();
    whole.extend(tail);
    assert_eq!(
        whole.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "关节奏补齐后的帧序与不关节奏逐字同序"
    );
    assert_eq!(
        chat_events(&journal_of(&whole), "system/message").len(),
        2,
        "整轮收完仍是恰两发"
    );
    kernel.shutdown();
}

// ==================== RT6 §表5 X-1…X-4：轮次轨四条种子的真 socket 用例（ST9） ==================
// 四条全藏在三枚显式旋钮后面（`fake_dsh.rs` 的 `--rail-count=` / `--rail-shapes=` /
// `--projections=3`），**档 0 逐字节不变**的取证走 `rust/tmp/st9-wire-level0-{before,after}.txt`
// 的 `cmp`，不靠这批用例背书；这批用例负责「开档之后线上真的长这样」。
// 两口径分开数（IP4 的 K1/K10 教训）：
// · **元素口径** = 一发 projection 的 `value` 整表长度（`--rail-count` 改的是它）；
// · **事件口径** = follow 流 journal 事件数（`TURN_FRAMES=36` 那族钉子改的是另一个东西）。
// 桩的 `seed_journal` 从大纲派生 ⇒ 开条数档时 asOfSeq 跟着涨，那是**副作用**，本刀只按不等式钉。
// 会话 id 另起 `s-190xx` 段：`grep -c "s-190" tests/ipc.rs` 动手前是 0，不与任何既有用例同槽。

/// 带旋钮开一条控制流并收 `want` 发：`(Kernel, Mux, 流名, 帧值)`。
/// 帧值走 `item_values` ⇒ 混进 `end`/`failure` 当场炸，这本身就是 X-3「不炸流」的一半判据。
fn st9_control_open(knobs: &[&str], want: usize) -> (Kernel, Mux, String, Vec<Value>) {
    let (kernel, mut mux, stream) = open_control_stream_by(&launch_with_args(knobs));
    let events = collect(&mut mux, &stream, want);
    let frames = item_values(&events, &stream);
    (kernel, mux, stream, frames)
}

/// 首批四帧的形状（:685 钉着的地基）+ 取出种子 `turnOutline` 的 `value` 整表。
fn st9_outline(frames: &[Value]) -> Vec<Value> {
    assert_eq!(
        frames[..4].iter().map(tag).collect::<Vec<_>>(),
        ["baseline", "queue", "jobs", "projection"],
        "轮次轨种子不许动首批四帧的帧型: {:?}",
        frames.iter().map(tag).collect::<Vec<_>>()
    );
    assert_eq!(
        frames[3]["key"].as_str().unwrap_or_default(),
        "turnOutline",
        "第四发的 key 恒是 turnOutline（桩的 `control_lead_frames`）"
    );
    frames[3]["value"]
        .as_array()
        .expect("wire.view 是整表数组，不是补丁")
        .clone()
}

/// 大纲的一列整数（`turn` / `seq`）；读不到整数直接炸，别把坏形状折成 0 蒙过去。
fn st9_i64_column(outline: &[Value], key: &str) -> Vec<i64> {
    outline
        .iter()
        .map(|entry| {
            entry[key].as_i64().unwrap_or_else(|| {
                panic!("条目的 {key} 不是整数: {entry}")
            })
        })
        .collect()
}

/// 线上条目折成四元组，好与分叉 typed 条目直接比（缺键按空串 = `string_field` 同口径）。
fn st9_wire_rows(outline: &[Value]) -> Vec<(i64, i64, String, String)> {
    outline
        .iter()
        .map(|entry| {
            (
                entry["turn"].as_i64().unwrap_or_default(),
                entry["seq"].as_i64().unwrap_or_default(),
                entry["prompt"].as_str().unwrap_or_default().to_string(),
                entry["response"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// 同上，分叉侧（`TurnOutlineItem`）折一遍 ⇒ 「同一张表两门读数」可比。
fn st9_typed_rows(items: &[blade2_rs::kernel::TurnOutlineItem]) -> Vec<(i64, i64, String, String)> {
    items
        .iter()
        .map(|item| {
            (
                item.turn,
                item.seq,
                item.prompt.clone(),
                item.response.clone(),
            )
        })
        .collect()
}

/// 桩在 `--rail-count=N` 下**应当**产出的那张表（四元组口径）：前三条是 `tests/ipc.rs`
/// 一整个家族地基的那三条种子，其后是 `rail_pad_to_count_at` 的确定生成条目。
/// 写在这里而不是去 `fake_dsh.rs` 里读常量 ⇒ 期望值与实现各处一侧，抄错就红。
fn st9_padded_rows(count: usize) -> Vec<(i64, i64, String, String)> {
    let mut rows = vec![
        (
            1,
            3,
            "把登录改成走内核".to_string(),
            "已经改成 session/create 了".to_string(),
        ),
        (
            2,
            41,
            "补上失败分支".to_string(),
            "失败分支走 AppendSystemMessage".to_string(),
        ),
        (
            3,
            88,
            "再补一轮测试".to_string(),
            "测试补在 tests/ipc.rs".to_string(),
        ),
    ];
    for turn in 4..=count as i64 {
        rows.push((
            turn,
            100 + turn,
            format!("第 {turn} 轮：把轮次轨的大纲灌到一屏装不下"),
            format!("第 {turn} 轮：host_height 的夹紧臂与轨内滚动这才演得到"),
        ));
    }
    rows
}

/// **X-1/X-2 的档面总表**（元素口径）：`(档, 大纲条数, 投影帧 seq)` = `(0,3,88) (1,4,104)
/// (2,6,106) (3,8,108)`，并且高档只追加、不改写上一档的任何一个字节。
#[test]
fn st9_rail_shapes_tiers_carry_the_documented_outline_element_counts() {
    let table: [(u64, usize, i64); 4] = [(0, 3, 88), (1, 4, 104), (2, 6, 106), (3, 8, 108)];
    let mut seen: Vec<(u64, usize, i64)> = Vec::new();
    let mut previous: Vec<Value> = Vec::new();
    for (level, want_entries, want_watermark) in table {
        let knob = format!("--rail-shapes={level}");
        let (mut kernel, mut mux, stream, frames) = st9_control_open(&[knob.as_str()], 4);
        let outline = st9_outline(&frames);
        assert_eq!(
            outline.len(),
            want_entries,
            "档 {level} 的大纲元素数不是 {want_entries}（元素口径，与 follow 的事件口径无关）"
        );
        assert_eq!(
            frames[3]["seq"].as_i64().unwrap_or_default(),
            want_watermark,
            "档 {level} 的投影水位该跟着末条条目的 seq 走（末条 = {want_watermark}）"
        );
        assert_eq!(
            st9_i64_column(&outline, "turn"),
            (1..=want_entries as i64).collect::<Vec<_>>(),
            "档 {level} 的轮号不再连续严格递增（主干按 turn 排序）"
        );
        let seqs = st9_i64_column(&outline, "seq");
        assert!(
            seqs.windows(2).all(|pair| pair[0] < pair[1]),
            "档 {level} 的 seq 必须严格递增，否则条目会互相盖号: {seqs:?}"
        );
        if level > 0 {
            assert_eq!(
                &outline[..previous.len()],
                &previous[..],
                "档 {level} 改写了上一档的条目：开档只许在尾部追加"
            );
        }
        seen.push((level, outline.len(), frames[3]["seq"].as_i64().unwrap_or_default()));
        assert!(
            collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
            "形状轴只灌同一发 projection 的 value，一帧都不许多推"
        );
        previous = outline;
        kernel.shutdown();
        drop(mux);
    }
    assert_eq!(
        seen,
        table.iter().map(|&(l, n, s)| (l, n, s)).collect::<Vec<_>>(),
        "档面总表与实测不符 ⇒ 报告 §1 的那张表得跟着改"
    );
}

/// **X-1**：`--rail-shapes=1` 把一条**长 Windows 绝对路径**放上主行 —— 驱动器冒号 + 反斜杠 +
/// 点文件名，且**整串不带空格** ⇒ 分叉卡宽 360 下 `break_long_tokens` 第一次有主行断点样本
/// （既有三条种子的断点全在 `response` 侧）。
#[test]
fn st9_rail_shapes_one_puts_a_long_windows_path_on_the_main_line() {
    let (mut quiet, _qmux, _qstream, quiet_frames) = st9_control_open(&[], 4);
    let quiet_outline = st9_outline(&quiet_frames);
    quiet.shutdown();

    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--rail-shapes=1"], 4);
    // ⚠ 靶子改过（报告 §3-B2）：ST9 原文是「`frames[..3]` 逐字节不变」，但 **baseline 那一发
    // 本身就带 `projections.s-1001.values.turnOutline`**，且 `sessionStats.turns`/`steps`/
    // `ttftSteps`、`todos.done`、`asOfSeq` 全从同一张大纲派生（现测 117→133、turns 3→4）⇒
    // 往大纲追加条目必然同时挪第一发。这与 ST9 自己在 `fake_dsh.rs:1264-1272` 记的
    // 「session/list 与 baseline 的 projections.s-1001.values.turnOutline … 全被逐字钉着」
    // 是同一件事，两条断言不可能同时成立 ⇒ 保留「别的一律不许动」那一半，只放行 s-1001 那格。
    assert_eq!(
        frames[1..3].iter().map(Value::to_string).collect::<Vec<_>>(),
        quiet_frames[1..3].iter().map(Value::to_string).collect::<Vec<_>>(),
        "开形状档只许动 baseline 与第四帧：queue/jobs 一个字节都不许变"
    );
    for cell in ["s-1002", "s-1003"] {
        assert_eq!(
            frames[0]["value"]["projections"][cell].to_string(),
            quiet_frames[0]["value"]["projections"][cell].to_string(),
            "baseline 里别的会话被形状档碰到了：{cell}"
        );
    }
    for cell in ["jobs", "queues"] {
        assert_eq!(
            frames[0]["value"][cell].to_string(),
            quiet_frames[0]["value"][cell].to_string(),
            "baseline 的 {cell} 与大纲无关，一字节都不许动"
        );
    }
    assert_ne!(
        frames[0]["value"]["projections"]["s-1001"]["values"]["turnOutline"].to_string(),
        quiet_frames[0]["value"]["projections"]["s-1001"]["values"]["turnOutline"].to_string(),
        "形状档没作用到 baseline 那一门读数 ⇒ 控制流与台账两门读数不同源了"
    );
    let outline = st9_outline(&frames);
    assert_eq!(&outline[..3], &quiet_outline[..], "既有三条种子被挪了窝");
    assert_eq!(outline.len(), 4, "档 1 只追加一条");

    let entry = &outline[3];
    assert_eq!(
        sorted_keys(entry),
        ["prompt", "response", "seq", "turn"],
        "形状条目仍是内核那四键，多一键少一键都是形状不合: {entry}"
    );
    assert_eq!(entry["turn"], json!(4), "新条目接在种子三条之后");
    assert_eq!(
        entry["seq"],
        json!(104),
        "seq = RAIL_SEQ_BASE + turn ⇒ 与 --projections 的 89..=96 那条带不相撞"
    );
    let prompt = entry["prompt"].as_str().expect("prompt 是字符串");
    assert_eq!(
        prompt,
        "C:\\Users\\Admin\\.qoder\\projects\\E--Syncthing-DshWinUI\\specs\\dr1-mainline-drift.md",
        "X-1 的主行逐字可断言（桩常量与本用例同一份字面量）"
    );
    assert!(
        prompt.contains('\\') && prompt.contains(':'),
        "断点样本得真有反斜杠与驱动器冒号: {prompt}"
    );
    assert_eq!(
        prompt.split('\\').count(),
        8,
        "驱动器 + 7 段路径 = 8 个可断段，卡宽 360 下才谈得上「主行断行」"
    );
    assert!(
        prompt.chars().all(|ch| !ch.is_whitespace()),
        "X-1 要的是**一整颗不带空格的长 token**；带空格就能按词折行，断点样本就白造了: {prompt}"
    );
    assert!(
        !entry["response"].as_str().unwrap_or_default().is_empty(),
        "副行必须非空，否则分不清是主行还是副行收起"
    );
    let items = blade2_rs::kernel::parse_turn_outline(&frames[3]["value"]);
    assert_eq!(items.len(), 4, "长路径条目是合法条目，分叉解析器一条都不许丢");
    assert_eq!(items[3].prompt, prompt, "主行过完 socket 与解析器仍是同一串");
    assert_eq!(items[3].turn, 4);
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "X-1 不加帧"
    );
    kernel.shutdown();
}

/// **X-2 的真形状**：`--rail-shapes=2` 给两条**空串**条目（一条 `prompt` 空、一条 `response` 空），
/// 键**在**、值是 `""` —— 内核 `dsh-session-turn-outline` 的四键 schema 必填 ⇒ 缺键那一型内核发不出，
/// 「空」在线上的真形状就是空串。分叉侧要锁的是两臂：空侧兜底文案、另一侧照常渲染。
#[test]
fn st9_rail_shapes_two_hands_the_two_empty_string_arms_as_the_kernel_sends_them() {
    const OTHER_SIDE: &str = "空字段那一侧之外的正文，副行/主行按它判收起";
    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--rail-shapes=2"], 4);
    let outline = st9_outline(&frames);
    assert_eq!(outline.len(), 6, "档 2 = 三条种子 + X-1 一条 + 空串两臂");

    let empty_prompt = &outline[4];
    let empty_response = &outline[5];
    for (entry, empty_key, other_key, want_turn) in [
        (empty_prompt, "prompt", "response", 5i64),
        (empty_response, "response", "prompt", 6),
    ] {
        assert!(
            entry.get(empty_key).is_some(),
            "{empty_key} 键必须在：内核 schema 必填 ⇒ 空的是**空串**不是缺键（缺键那型在档 3）: {entry}"
        );
        assert_eq!(
            entry[empty_key],
            json!(""),
            "档 2 的 {empty_key} 侧得是空串本体，不许是 null 也不许是省略号文案: {entry}"
        );
        assert_eq!(
            entry[other_key],
            json!(OTHER_SIDE),
            "另一侧必须逐字非空，否则两臂分不清谁收的是谁: {entry}"
        );
        assert_eq!(
            sorted_keys(entry),
            ["prompt", "response", "seq", "turn"],
            "空串条目与真内核条目同键集: {entry}"
        );
        assert_eq!(entry["turn"], json!(want_turn));
        assert_eq!(entry["seq"], json!(100 + want_turn));
    }

    let items = blade2_rs::kernel::parse_turn_outline(&frames[3]["value"]);
    assert_eq!(items.len(), 6, "两条空串条目都合法，分叉不许把「空」当坏形状丢掉");
    assert_eq!(items[4].prompt, "", "解析器不该把空串折成缺省文案（那是视图层的活）");
    assert_eq!(items[4].response, OTHER_SIDE);
    assert_eq!(items[5].response, "");
    assert_eq!(items[5].prompt, OTHER_SIDE);
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "X-2 不加帧"
    );
    kernel.shutdown();
}

/// **X-2 的缺键负形**：`--rail-shapes=3` 再加两条 —— 一条整个不发 `prompt` 键、一条不发
/// `response` 键。备案：内核发不出这一型（四键必填），它与 `--prompt=3`、`--trunc=2`、
/// `--retry=4/5` 同族，只为把分叉 `string_field` 的**落空臂**从「解析单测里的假设」变成端到端可证。
#[test]
fn st9_rail_shapes_three_adds_the_two_missing_key_negative_forms() {
    const OTHER_SIDE: &str = "空字段那一侧之外的正文，副行/主行按它判收起";
    let (mut level_two, _mux2, _stream2, level_two_frames) = st9_control_open(&["--rail-shapes=2"], 4);
    let level_two_outline = st9_outline(&level_two_frames);
    level_two.shutdown();

    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--rail-shapes=3"], 4);
    let outline = st9_outline(&frames);
    assert_eq!(outline.len(), 8, "档 3 = 档 2 的六条 + 两型缺键负形");
    assert_eq!(
        &outline[..6],
        &level_two_outline[..],
        "档 3 不许改写档 2 的任何一个字节"
    );

    let no_prompt = &outline[6];
    let no_response = &outline[7];
    assert_eq!(
        sorted_keys(no_prompt),
        ["response", "seq", "turn"],
        "第一型必须**整个不发** prompt 键（少一键才算这一臂，空串是档 2 那一臂）: {no_prompt}"
    );
    assert_eq!(
        sorted_keys(no_response),
        ["prompt", "seq", "turn"],
        "第二型整个不发 response 键: {no_response}"
    );
    assert_eq!(no_prompt["turn"], json!(7), "负形从 turn 7 起接号");
    assert_eq!(no_prompt["seq"], json!(107));
    assert_eq!(no_response["turn"], json!(8));
    assert_eq!(no_response["seq"], json!(108));
    assert_eq!(no_prompt["response"], json!(OTHER_SIDE));
    assert_eq!(no_response["prompt"], json!(OTHER_SIDE));

    let items = blade2_rs::kernel::parse_turn_outline(&frames[3]["value"]);
    assert_eq!(
        items.len(),
        8,
        "缺键条目在分叉侧是**降级**（string_field 折成空串）而不是丢弃 —— 轮次可导航优先于预览完整"
    );
    assert_eq!(items[6].prompt, "", "缺 prompt 键该落成空串，与档 2 的空串臂同一收敛点");
    assert_eq!(items[6].response, OTHER_SIDE);
    assert_eq!(items[7].response, "");
    assert_eq!(items[7].prompt, OTHER_SIDE);
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "缺键负形也不加帧"
    );
    kernel.shutdown();
}

/// **X-4**：`--rail-count=` 把 s-1001 的大纲灌到 N 条，**每一条都真到**（含区间边界 4/60，
/// 与母本要的 25–40 三档）；并核对另一门读数 `session/list` 与 `sessionStats.turns` 同源。
#[test]
fn st9_rail_count_delivers_every_generated_outline_entry_over_the_socket() {
    for count in [4usize, 25, 30, 40, 60] {
        let knob = format!("--rail-count={count}");
        let (mut kernel, mut mux, stream, frames) = st9_control_open(&[knob.as_str()], 4);
        let outline = st9_outline(&frames);
        assert_eq!(outline.len(), count, "--rail-count={count} 该把大纲灌到恰好 {count} 条");
        assert_eq!(
            st9_i64_column(&outline, "turn"),
            (1..=count as i64).collect::<Vec<_>>(),
            "档 {count} 的轮号必须 1..={count} 连续，一屏装不下也要能轮轮点过去"
        );
        let seqs = st9_i64_column(&outline, "seq");
        assert_eq!(&seqs[..3], [3, 41, 88], "既有三条的号一格不动");
        assert_eq!(
            &seqs[3..],
            &(4..=count as i64).map(|turn| 100 + turn).collect::<Vec<i64>>(),
            "生成条目走桩自己的编号规则 seq = RAIL_SEQ_BASE + turn（既有三条之后 ⇒ 首条 104）：\
             {count} 条该一路铺到 {base_count}",
            base_count = 100 + count as i64,
        );
        assert_eq!(
            frames[3]["seq"].as_i64().unwrap_or_default(),
            100 + count as i64,
            "投影帧水位跟末条走：灌到 {count} 条还报 88 就是自相矛盾的线上形状"
        );
        let last = &outline[count - 1];
        assert_eq!(
            last["prompt"],
            json!(format!("第 {count} 轮：把轮次轨的大纲灌到一屏装不下")),
            "末条是**全数到达**的证据，不是被截断的中间态"
        );
        assert_eq!(
            last["response"],
            json!(format!("第 {count} 轮：host_height 的夹紧臂与轨内滚动这才演得到")),
            "生成条目两串都非空：X-4 演的是条数，不该顺手把 X-2 的空臂混进来"
        );

        let rows = kernel.list_session_rows().expect("session/list 应成功");
        let listed = rows
            .iter()
            .find(|row| row.info.id == "s-1001")
            .expect("台账里有 s-1001")
            .projections
            .clone();
        assert_eq!(
            st9_typed_rows(&listed.turn_outline),
            st9_wire_rows(&outline),
            "档 {count}：控制流那一发与 session/list 的同一张表一字不差"
        );
        assert_eq!(
            listed.stats.map(|stats| stats.turns),
            Some(count as i64),
            "左栏「N 轮对话」读的 sessionStats.turns 也得跟着大纲（档 0 s-1001 本来就带这一键）"
        );
        assert!(
            listed.as_of_seq.unwrap_or_default() > 117,
            "事件口径的副作用：大纲一灌，seed_journal 派生的水位也涨（关档是 117，见 :8111）"
        );
        assert!(
            collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
            "条数轴是「加元素不加帧」：{count} 条仍只有一发 projection"
        );
        kernel.shutdown();
        drop(mux);
    }
}

/// **两枚新旋钮的档 0 哨兵**：区间外/脏串一律等价于关档，且与关档**逐字节**相同。
/// `--rail-count=` 的下限是 4（三条种子 + 至少一条生成），故 `=3` 也在此列 —— 绝不「按下限灌」。
#[test]
fn st9_rail_knobs_outside_the_interval_are_byte_identical_to_default() {
    let (mut quiet, _qmux, _qstream, quiet_frames) = st9_control_open(&[], 4);
    let want = quiet_frames.iter().map(Value::to_string).collect::<Vec<_>>();
    assert_eq!(want.len(), 4, "关档首批恒四帧，参照得先立起来");
    assert_eq!(
        st9_outline(&quiet_frames).len(),
        3,
        "关档大纲仍是三条（本刀不改默认档的地基）"
    );
    quiet.shutdown();
    for knob in [
        "--rail-count=0",
        "--rail-count=3",
        "--rail-count=61",
        "--rail-count=1000",
        "--rail-count=-1",
        "--rail-count=nope",
        "--rail-shapes=4",
        "--rail-shapes=99",
        "--rail-shapes=三",
        "--rail-shapes=-1",
    ] {
        let (mut kernel, mut mux, stream, frames) = st9_control_open(&[knob], 4);
        assert_eq!(
            frames.iter().map(Value::to_string).collect::<Vec<_>>(),
            want,
            "{knob} 必须与关档逐字节相等（回落是压住的，不是按边界灌）"
        );
        assert!(
            collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
            "{knob} 一帧都不许多推"
        );
        kernel.shutdown();
        drop(mux);
    }
}

/// **X-3**：`--projections=3` 的四发 turnOutline 负形过完 socket 之后
/// ① 2 档那 12 帧的字节一格不动（只追加），② 分叉按「坏条目跳过、好条目留下」逐发折叠，
/// ③ 控制流**没被炸**（既无 end/failure，静默窗口里也不加帧），④ HTTP 层仍正常服务。
#[test]
fn st9_negative_turn_outline_frames_do_not_break_the_control_stream() {
    let (mut level_two, _mux2, _stream2, level_two_frames) =
        st9_control_open(&["--projections=2"], 12);
    assert_eq!(level_two_frames.len(), 12, "2 档仍是 4 + 6 + 2 帧（:6051 的地基）");
    level_two.shutdown();

    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--projections=3"], 16);
    assert_eq!(frames.len(), 16, "3 档该是 4 + 6 + 2 + 4 帧");
    assert_eq!(
        frames[..12].iter().map(Value::to_string).collect::<Vec<_>>(),
        level_two_frames.iter().map(Value::to_string).collect::<Vec<_>>(),
        "往 2 档塞帧就是顶掉既有用例：3 档只许在 12 帧之后追加"
    );
    let mut want_tags = vec!["baseline", "queue", "jobs"];
    want_tags.extend(std::iter::repeat("projection").take(13));
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        want_tags,
        "3 档的帧型序"
    );
    assert_eq!(
        frames[4..]
            .iter()
            .map(|frame| frame["key"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        [
            "plan",
            "permissions",
            "schedule",
            "goal",
            "modelSelection",
            "todos",
            "goal",
            "todos",
            "turnOutline",
            "turnOutline",
            "turnOutline",
            "turnOutline",
        ],
        "key 序 = 主干 ApplyProjectionValues 的分派序，四发负形排在最后"
    );
    assert_eq!(
        frames[4..]
            .iter()
            .map(|frame| frame["seq"].as_i64().unwrap_or_default())
            .collect::<Vec<_>>(),
        (89i64..=100).collect::<Vec<_>>(),
        "seq 一格不跳号：负形接在 95/96 之后 = 97..=100"
    );
    let negatives = &frames[12..];
    for frame in negatives {
        assert_eq!(
            sorted_keys(frame),
            ["key", "seq", "sessionId", "type", "value"],
            "负形的外壳五键与成功帧同一型: {frame}"
        );
        assert_eq!(frame["sessionId"], json!("s-1001"));
    }
    assert_eq!(
        negatives
            .iter()
            .map(|frame| frame["value"].as_array().map(Vec::len).unwrap_or(0))
            .collect::<Vec<_>>(),
        [2, 2, 2, 3],
        "每发演一种坏条目，且都带同批的好条目做对照（整键替换要两型同框才辨得出）"
    );

    let mut state = ControlState::default();
    for frame in &frames[..12] {
        state.apply(frame);
    }
    assert_eq!(
        projection_of(&state, "s-1001").turn_outline.len(),
        3,
        "负形之前大纲仍是种子三条：六型增量各换各的键，不去动 turnOutline"
    );
    let fold = |state: &mut ControlState, frame: &Value| {
        state.apply(frame);
        projection_of(state, "s-1001").turn_outline.clone()
    };

    let first = fold(&mut state, &frames[12]);
    assert_eq!(
        first.iter().map(|item| item.turn).collect::<Vec<_>>(),
        vec![11],
        "turn:0 那一型该被丢弃、同批好条目该留着"
    );
    assert_eq!(first[0].prompt, "同批里唯一的好条目");

    let second = fold(&mut state, &frames[13]);
    assert_eq!(
        second.iter().map(|item| item.turn).collect::<Vec<_>>(),
        vec![13],
        "缺 seq 那一型被丢弃；整键替换语义下第二发盖掉第一发（wire.view 是全量表）"
    );

    let third = fold(&mut state, &frames[14]);
    assert_eq!(
        third.iter().map(|item| item.turn).collect::<Vec<_>>(),
        vec![14, 15],
        "prompt 非字符串是**降级**不是丢弃：条目留下"
    );
    assert_eq!(third[0].prompt, "", "非字符串折成空串（主干 ParseTurnOutline 的同一条兜底）");
    assert!(!third[0].response.is_empty(), "坏一侧不该把另一侧一起带走");
    assert_eq!(third[1].prompt, "同批里唯一的好条目");

    let fourth = fold(&mut state, &frames[15]);
    assert_eq!(
        fourth.iter().map(|item| item.turn).collect::<Vec<_>>(),
        vec![16],
        "非对象元素（字符串/数字混在数组里）该被丢弃，只剩同批那条好条目"
    );
    assert_eq!(fourth[0].seq, 916, "存活条目的 seq 逐字过完 socket");

    assert_eq!(
        collect_within(&mut mux, &stream, 17, QUIET_BUDGET).len(),
        0,
        "炸流的两型（end/failure）与第五型帧都不许出现；静默窗口里流还得开着"
    );
    let rows = kernel.list_session_rows().expect("四发负形之后 HTTP 层仍应服务");
    let listed = rows
        .iter()
        .find(|row| row.info.id == "s-1001")
        .expect("台账里有 s-1001")
        .projections
        .clone();
    assert_eq!(
        listed.turn_outline.len(),
        3,
        "负形只是**线上形状**：桩自己的大纲台账一条没多，另一门读数照旧三条"
    );
    kernel.shutdown();
}

/// **条数轴不动实时轮**：`--rail-count=25` 之下开一条关节奏（150ms > READ_TICK 120ms）的
/// 实时轮，中途窗仍拍得到、整轮仍是 `TURN_FRAMES` 格；两口径当场分开数（36 元素 / 30 事件），
/// 且 25 条属于 s-1001，一条也没漏进这条实时会话。
#[test]
fn st9_rail_count_leaves_a_paced_live_turn_frame_for_frame_identical() {
    const LIVE: &str = "s-19031";
    const MID_WINDOW: usize = 3;
    const COUNT: usize = 25;
    let pace_arg = format!("--pace={MID_PACE_MS}");
    let count_arg = format!("--rail-count={COUNT}");
    let launch = launch_with(&[pace_arg.as_str(), count_arg.as_str()]);
    let (mut kernel, mut mux, follow) =
        open_stream_by(&launch, "session/follow", follow_args(LIVE));
    assert_eq!(collect(&mut mux, &follow, 1).len(), 1, "快照是现成的，不该被节奏拖住");

    let started = Instant::now();
    kernel
        .call(
            "session/prompt",
            prompt_args(LIVE, "大纲灌到 25 条之后的实时一轮", "queue"),
        )
        .expect("session/prompt 应被接受");
    let head = collect_within(&mut mux, &follow, MID_WINDOW, DRAIN_BUDGET);
    assert_eq!(head.len(), MID_WINDOW, "只想要 {MID_WINDOW} 帧就该只到 {MID_WINDOW} 帧");
    assert!(
        started.elapsed() >= Duration::from_millis(MID_PACE_MS * (MID_WINDOW as u64 - 1) - 60),
        "灌了 25 条大纲之后节奏粒度必须还在，否则并成一批就拍不到中途"
    );
    let frames = item_values(&head, &follow);
    assert_eq!(
        frames.iter().map(tag).collect::<Vec<_>>(),
        turn_tags()[..MID_WINDOW],
        "条数轴只许改大纲，不许改实时轮的帧序"
    );
    let midway = journal_of(&frames);
    assert_eq!(
        midway.len(),
        MID_WINDOW,
        "中途这三格元素口径 = 事件口径 = 3（本轮前三格全是 journal 事件，没有流片）"
    );
    assert!(
        event_types(&midway).iter().all(|kind| *kind != "turn/end"),
        "这一窗必须停在轮尾之前"
    );
    assert_eq!(
        chat_events(&midway, "system/message").len(),
        0,
        "两发 system/message 在第 31/32 格，中途拍不到才是对的"
    );

    let rest = collect_within(&mut mux, &follow, TURN_FRAMES - MID_WINDOW, DRAIN_BUDGET);
    let tail = item_values(&rest, &follow);
    let mut whole = frames.clone();
    whole.extend(tail);
    assert_eq!(whole.len(), TURN_FRAMES, "25 条大纲不许顶掉冻住的 36 格预算");
    assert_eq!(
        whole.iter().map(tag).collect::<Vec<_>>(),
        turn_tags(),
        "关节奏的整轮帧序与不关节奏逐字同序"
    );
    let all = journal_of(&whole);
    assert_eq!(
        all.len(),
        30,
        "整轮 36 **元素** = 30 **事件** + 6 流片：两口径永远别混着数"
    );
    assert_eq!(
        chat_events(&all, "system/message").len(),
        2,
        "恒两发是**桩**的口径；真内核每轮 0..N 发（含 0 发），此数不可当产品判据"
    );

    let rows = kernel.list_session_rows().expect("session/list 应成功");
    let seeded = rows
        .iter()
        .find(|row| row.info.id == "s-1001")
        .expect("台账里有 s-1001")
        .projections
        .clone();
    assert_eq!(
        seeded.turn_outline.len(),
        COUNT,
        "跑过一轮实时会话之后，灌进去的 25 条一条不涨一条不落"
    );
    assert_eq!(
        st9_typed_rows(&seeded.turn_outline),
        st9_padded_rows(COUNT),
        "分叉侧折出来的 25 条与桩生成的那 25 条逐字同表（前三条是冻住的种子）"
    );

    assert!(
        rows.iter().all(|row| row.info.id != LIVE),
        "实时会话不在这张台账里（桩只列种子 + fork 行）⇒ 大纲按会话分表，别把两件事混成一个数"
    );
    kernel.shutdown();
}


// ==================== ST12 · RT6 §表5 X-1…X-4 的**消费侧**端到端用例 ====================
// 上面那批 `st9_*`（ST9 落数据面 / ST10 抢救落用例）把四条档的**线上形状**钉死了。本刀复核的
// 结论是：四条 X 的立档理由全都是「让 `turnrail`/`main.rs` 某个消费者臂端到端可达」，而那四个
// 臂一个都没被**过完 socket 的数据**喂过（动手前现测：`grep -n "turnrail::" tests/ipc.rs`
// = 0 命中，只有 `:10244` 一处注释）。⇒ 桩侧一行不改（§3 自证），只补这五发消费者用例。
// 三处消费者式子是**逐字镜像**生产码的，抄错就地红：
//   · 刻度表 = `main.rs:8011` `turnrail::marks_from(self.control.turn_outline(session))`
//   · 预览卡 = `main.rs:16743-16768`（主行 `break_long_tokens(选择式)`、副行 `!is_empty()` 才进子树；
//     主干同臂 `MainWindow.TurnRail.cs:372` 与 `:373-381`）
//   · 宿主高 = `main.rs:16613` `host_height(LIST_INSET, SLOT_PITCH, count, CHAT_BAND_DIP)`
// 档面帧预算全按本刀现测（就是下面这些断言本身，不抄别的档的常数）：`--rail-count=4/20/21/25/40`、
// `--rail-shapes=1/2/3` 与两轴同开 ⇒ 开 `session/control` 首批恒 **4 帧**；`--projections=3` = **16 帧**。

/// wire 值 → 分叉刻度表：`main.rs:8011` 的 `rail_marks()` 同一条式子（投影表不另存真相）。
fn st12_marks(value: &Value) -> Vec<blade2_rs::turnrail::TurnRailMark> {
    blade2_rs::turnrail::marks_from(&blade2_rs::kernel::parse_turn_outline(value))
}

/// 同上，读 `ControlState` 那一门（增量帧折叠之后轨真正吃的那张表）。
fn st12_state_marks(state: &ControlState, session: &str) -> Vec<blade2_rs::turnrail::TurnRailMark> {
    blade2_rs::turnrail::marks_from(projection_of(state, session).turn_outline.as_slice())
}

/// 分叉预览卡的行构造镜像（`main.rs:16743-16768`）：返回「进子树的那几行」，
/// 一行 = 只有主行（副行收起臂）、两行 = 主行 + 副行。
fn st12_card_lines(marks: &[blade2_rs::turnrail::TurnRailMark], catalog: &Catalog) -> Vec<Vec<String>> {
    marks
        .iter()
        .map(|mark| {
            let fallback = catalog.lf("第 {0} 轮", &[mark.turn.to_string()]);
            let mut lines = vec![blade2_rs::turnrail::break_long_tokens(if mark.prompt.is_empty() {
                &fallback
            } else {
                &mark.prompt
            })];
            if !mark.response.is_empty() {
                lines.push(blade2_rs::turnrail::break_long_tokens(&mark.response));
            }
            lines
        })
        .collect()
}

/// 一页回读记录 → `jump_target` 吃的那三张事实（`main.rs:8050-8056` 的镜像）。
/// 只收两种会成真气泡的行；桩每轮恒一发 `assistant/message`（`fake_dsh.rs:5832-5836`）
/// ⇒ `is_answer` 取「该型本身」，不必再抄主干 `_transcriptAnswers[turn]` 那一层。
fn st12_rows(events: &[Value]) -> Vec<blade2_rs::turnrail::BubbleRow<'static>> {
    events
        .iter()
        .filter_map(|event| {
            let role = match event["type"].as_str().unwrap_or_default() {
                "user/message" => "user",
                "assistant/message" => "assistant",
                _ => return None,
            };
            Some(blade2_rs::turnrail::BubbleRow {
                turn: event["data"]["turn"].as_i64()?,
                role,
                is_answer: role == "assistant",
            })
        })
        .collect()
}

/// 一条会话的「每颗刻度都点得过去」判据（主干 `OnTurnRailMarkClick` 的三级回落第①级）。
fn st12_assert_marks_have_targets(marks: &[blade2_rs::turnrail::TurnRailMark], rows: &[blade2_rs::turnrail::BubbleRow<'_>]) {
    use blade2_rs::turnrail::jump_target;
    for mark in marks {
        let index = jump_target(rows, mark.turn)
            .unwrap_or_else(|| panic!("第 {} 轮点过去是空目标（{} 行气泡里没一条带这个轮号）", mark.turn, rows.len()));
        assert_eq!(rows[index].role, "user", "三级回落第①级该命中该轮首条 user 气泡: {:?}", rows[index]);
    }
}

/// **X-1**：`--rail-shapes=1` 那条长路径主行是**今天第一颗**过完 socket 还会被
/// `break_long_tokens` 改写的主行 —— 关档三条种子的主行在这枚断行器下恒等（副行侧才有断点），
/// 这正是母本 §表5 X-1 说的「端到端演不出来」。
#[test]
fn st12_the_long_path_seed_is_the_first_main_line_the_token_breaker_actually_bends() {
    use blade2_rs::turnrail::{break_long_tokens, BREAK_AFTER, WORD_JOINER};
    const LONG: &str = "C:\\Users\\Admin\\.qoder\\projects\\E--Syncthing-DshWinUI\\specs\\dr1-mainline-drift.md";

    let (mut quiet, _qmux, _qstream, quiet_frames) = st9_control_open(&[], 4);
    let quiet_marks = st12_marks(&quiet_frames[3]["value"]);
    quiet.shutdown();
    assert_eq!(quiet_marks.len(), 3, "关档仍是三条种子（本刀不动地基）");
    for mark in &quiet_marks {
        assert_eq!(
            break_long_tokens(&mark.prompt),
            mark.prompt,
            "关档主行没有一颗断点字符 ⇒ 断行器对它恒等，X-1 之前主行断行不可演: {mark:?}"
        );
    }
    assert!(
        quiet_marks.iter().any(|mark| break_long_tokens(&mark.response) != mark.response),
        "既有种子的断点全在副行侧（`/` 与 `.`）⇒ 母本 X-1 钉的就是这处不对称"
    );

    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--rail-shapes=1"], 4);
    let marks = st12_marks(&frames[3]["value"]);
    assert_eq!(marks.len(), 4, "现测：档 1 首批仍只有那一发 baseline/queue/jobs/projection");
    let mark = &marks[3];
    assert_eq!(mark.prompt, LONG, "过完 socket 与解析器，主行仍是那一串");
    assert_eq!(mark.turn, 4);
    assert_eq!(mark.seq, 104);

    let broken = break_long_tokens(&mark.prompt);
    assert_ne!(broken, mark.prompt, "X-1 的整条理由：断行器对**这一行**不再是恒等");
    let breakable = mark.prompt.chars().filter(|ch| BREAK_AFTER.contains(ch)).count();
    assert_eq!(
        breakable, 15,
        "驱动器冒号 1 + 反斜杠 7 + 连字符 5（`E--Syncthing-DshWinUI` 三颗、`dr1-mainline-drift` 两颗）\
         + 点 2 = 15 颗断点字符（改常量就得同步这里的计数）"
    );
    assert_eq!(
        broken.matches(WORD_JOINER).count(),
        breakable,
        "每颗断点字符后面恰补一颗零宽空格，不多补不少补"
    );
    assert_eq!(broken.replace(WORD_JOINER, ""), mark.prompt, "只插不断：不吞字符、不重排");
    assert_eq!(break_long_tokens(&broken), broken, "幂等：重复过断行器不再变长");

    let zh = Catalog::load("zh", None);
    let lines = st12_card_lines(std::slice::from_ref(mark), &zh);
    assert_eq!(lines[0].len(), 2, "主行走非空支、副行照常渲染 ⇒ 才分得清断的是哪一行");
    assert_eq!(lines[0][0], broken, "预览卡主行那一格 = 断过行的长路径");
    assert_eq!(lines[0][1], mark.response, "副行不该被主行的断行牵到");

    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "X-1 不加帧（现测：4 帧之后静默窗零帧）"
    );
    kernel.shutdown();
}

/// **X-2**：`--rail-shapes=2` 那两条空串条目把 `main.rs:16743` 与 `:16757` 两个兜底臂的
/// **判据输入**端到端喂出来（此前 `note_turn` 的 `text`/`reply` 恒非空，两臂永远走不到）：
/// 主行落兜底文案、副行那一行不进子树；并钉「prompt 空 ≠ 该轮点过去是空目标」。
#[test]
fn st12_the_two_empty_field_seeds_open_both_preview_card_fallback_gates() {
    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--rail-shapes=2"], 4);
    let marks = st12_marks(&frames[3]["value"]);
    assert_eq!(marks.len(), 6, "档 2 = 三条种子 + 长路径 + 空串两臂");
    assert!(
        marks[4].prompt.is_empty() && !marks[4].response.is_empty(),
        "第一臂的判据输入：空主行 + 非空副行 {:?}",
        marks[4]
    );
    assert!(
        marks[5].response.is_empty() && !marks[5].prompt.is_empty(),
        "第二臂的判据输入：非空主行 + 空副行 {:?}",
        marks[5]
    );

    let zh = Catalog::load("zh", None);
    let en = Catalog::load("en", None);
    let zh_cards = st12_card_lines(&marks, &zh);
    let en_cards = st12_card_lines(&marks, &en);
    assert_eq!(
        zh_cards.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![2, 2, 2, 2, 2, 1],
        "六颗刻度里只有那颗空副行的收起副行（两臂各自只动一行）"
    );
    assert_eq!(
        zh_cards.iter().map(Vec::len).collect::<Vec<_>>(),
        en_cards.iter().map(Vec::len).collect::<Vec<_>>(),
        "换语言不许改**行数**（那是 is_empty 判据的活，不是文案的活）"
    );
    for (index, (card_zh, card_en)) in zh_cards.iter().zip(en_cards.iter()).enumerate() {
        assert!(
            !card_zh[0].trim().is_empty() && !card_en[0].trim().is_empty(),
            "第 {index} 颗刻度的主行落成空洞（兜底臂没接住）: {:?}",
            marks[index]
        );
    }
    assert_eq!(
        zh_cards[4][0],
        zh.lf("第 {0} 轮", &["5".to_string()]),
        "空主行走的是轮次号兜底文案（主干 `TurnRail.cs:372` 的 `LF(\"第 {{0}} 轮\", mark.Turn)`）"
    );
    assert_eq!(
        en_cards[4][0], zh_cards[4][0],
        "主干 `MainWindow.TurnRail.cs` 的 `ParseTurnOutline` 走 `LF(\"第 {{0}} 轮\", mark.Turn)`：\
         通道 A 且主干 `ShellEnglish` **没有**裸 `第 {{0}} 轮` 这枚键 ⇒ 无键必回中文，\
         英文档与中文档**同串**才是忠实复刻。分叉不许靠 EN 表补一枚主干没有的英文名（KX1 刀 C 撤表）"
    );
    assert_eq!(
        zh_cards[5][0],
        blade2_rs::turnrail::break_long_tokens(&marks[5].prompt),
        "空副行那颗的主行走非空支"
    );
    assert_ne!(
        zh_cards[5][0], marks[5].prompt,
        "现测的一处附带事实：X-2 那枚「另一侧正文」常量自带一颗 `/` ⇒ 档 2 的主行也有断点样本\
         （X-1 那发钉的是「长路径」那一型，两型不冲突）: {:?}",
        marks[5].prompt
    );

    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("六条种子大纲该回六轮历史");
    let events = page_events(&page);
    assert_eq!(
        turn_start_seqs(&events),
        marks.iter().map(|mark| mark.seq).collect::<Vec<_>>(),
        "每颗刻度都有自己的 `turn/start`，且锚点 seq 与大纲同源（`seed_journal` 从大纲派生）"
    );
    st12_assert_marks_have_targets(&marks, &st12_rows(&events));
    assert_eq!(
        events
            .iter()
            .filter(|event| event["type"].as_str() == Some("user/message")
                && event["data"]["turn"].as_i64() == Some(5))
            .count(),
        1,
        "空主行那轮的气泡仍在（正文是空串，行不缺席）"
    );

    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "X-2 不加帧"
    );
    kernel.shutdown();
}

/// **X-3**：`--projections=3` 的四发负形折叠之后，**存活刻度**在分叉侧落到哪一臂 ——
/// 丢弃 ⇒ 条数掉到 `rail_visible` 门槛以下（整条轨不进子树）；降级 ⇒ 空主行落回 X-2 那同一收敛点。
#[test]
fn st12_the_folded_negative_marks_land_on_the_same_view_gates() {
    use blade2_rs::turnrail::{break_long_tokens, host_height, preview_y_for_slot, LIST_INSET, SLOT_PITCH};
    const SID: &str = "s-1001";
    const CHAT_BAND_DIP: f64 = 640.0; // main.rs:3419，生产调用点唯一喂给 host_height 的带子

    let (mut kernel, mut mux, stream, frames) = st9_control_open(&["--projections=3"], 16);
    assert_eq!(frames.len(), 16, "本用例的帧预算：4 + 6 + 2 + 4（现测）");
    let mut state = ControlState::default();
    for frame in &frames[..12] {
        state.apply(frame);
    }
    assert_eq!(st12_state_marks(&state, SID).len(), 3, "负形之前仍是种子三条");

    let zh = Catalog::load("zh", None);
    let fold = |state: &mut ControlState, frame: &Value| {
        state.apply(frame);
        st12_state_marks(state, SID)
    };

    let first = fold(&mut state, &frames[12]);
    assert_eq!(
        first.iter().map(|mark| mark.turn).collect::<Vec<_>>(),
        vec![11],
        "`turn:0` 那一型被丢弃，只剩同批好条目"
    );
    assert!(
        !blade2_rs::turnrail::rail_visible(first.len()),
        "存活 1 颗 < MIN_MARKS(2) ⇒ 整条轨不进子树（主干 `UpdateTurnRailVisibility` 同口径）"
    );
    assert_eq!(
        host_height(LIST_INSET, SLOT_PITCH, first.len(), CHAT_BAND_DIP),
        24.0,
        "单颗刻度的宿主高 = 2*2 + 1*20，未触任何夹紧臂"
    );

    let _second = fold(&mut state, &frames[13]);
    let third = fold(&mut state, &frames[14]);
    assert_eq!(
        third.iter().map(|mark| mark.turn).collect::<Vec<_>>(),
        vec![14, 15],
        "`prompt` 非字符串是**降级**：条目留在表里"
    );
    assert!(
        third[0].prompt.is_empty(),
        "降级把主行折成真空串，所以视图层必须自己兜: {:?}",
        third[0]
    );
    assert_eq!(
        break_long_tokens(&third[0].prompt),
        "",
        "断行器不会替空主行长出文案（`turnrail.rs:337` 那处恒等）⇒ 兜底臂是唯一防线"
    );
    assert!(blade2_rs::turnrail::rail_visible(third.len()), "两颗就上屏");
    let cards = st12_card_lines(&third, &zh);
    assert_eq!(cards[0][0], zh.lf("第 {0} 轮", &["14".to_string()]), "降级臂与 X-2 空主行臂同一收敛点");
    assert_eq!(cards[0].len(), 2, "坏一侧不许把另一侧一起带走");
    assert_eq!(
        preview_y_for_slot(1, LIST_INSET, SLOT_PITCH, blade2_rs::turnrail::CARD_NOMINAL_HEIGHT, 44.0),
        0.0,
        "两颗刻度的宿主（44 DIP）装不下标称卡高 100 ⇒ 卡贴宿主顶，不探出带外"
    );

    let fourth = fold(&mut state, &frames[15]);
    assert_eq!(
        fourth.iter().map(|mark| mark.turn).collect::<Vec<_>>(),
        vec![16],
        "非对象元素（字符串/数字混在数组里）该被丢弃"
    );
    assert_eq!(fourth[0].seq, 916, "存活条目的 seq 逐字过完 socket");
    assert!(!blade2_rs::turnrail::rail_visible(fourth.len()), "轨跟着存活条数收回去");

    assert_eq!(
        st12_marks(&frames[12]["value"]).len(),
        1,
        "整键替换语义：直接喂线上那发也折出同一张表（台账门与解析门不造第二份真相）"
    );
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "四发负形之外一帧都不许多推（流也没被炸）"
    );
    kernel.shutdown();
}

/// **X-4**：`--rail-count=` 灌出来的条数真的把 `host_height` 的 `420` 夹紧臂顶开（边界在
/// 20/21 颗：21 颗的 natural 424 才越 420），夹紧之后**溢出宿主** = 轨内滚动与
/// `EnsureActiveMarkInView` 缺失的那块暴露面；并回读 `session/page` 钉每颗刻度都有跳转目标。
#[test]
fn st12_the_padded_counts_reach_the_host_height_clamps_and_every_mark_has_a_target() {
    use blade2_rs::turnrail::{
        host_height, rail_visible, CARD_NOMINAL_HEIGHT, HOST_BAND_GATE, HOST_BAND_SLACK,
        HOST_MAX_HEIGHT, LIST_INSET, SLOT_PITCH,
    };
    /// `main.rs:16613` 调用点唯一喂进去的带子（生产里 `band-64 = 576 > 420` ⇒ 恒由 420 先夹）。
    const CHAT_BAND_DIP: f64 = 640.0;
    const PADDED: usize = 25;

    let mut measured: Vec<usize> = Vec::new();
    let mut padded_marks = Vec::new();
    for count in [4usize, 20, 21, 25, 40] {
        let knob = format!("--rail-count={count}");
        let (mut kernel, mut mux, stream, frames) = st9_control_open(&[knob.as_str()], 4);
        let marks = st12_marks(&frames[3]["value"]);
        assert_eq!(marks.len(), count, "线上灌到 {count} 条（现测：首批仍 4 帧）");
        assert!(rail_visible(count), "{count} 条该上屏");
        let natural = 2.0 * LIST_INSET + count as f64 * SLOT_PITCH;
        let host = host_height(LIST_INSET, SLOT_PITCH, count, CHAT_BAND_DIP);
        if count <= 20 {
            assert_eq!(host, natural, "{count} 条还装得下：natural {natural} 未触 {HOST_MAX_HEIGHT}");
            assert_eq!(
                ((host - 2.0 * LIST_INSET) / SLOT_PITCH).floor() as usize,
                count,
                "溢出宿主 0 条 ⇒ 母本说的「永远不触发」正是这一档以下"
            );
        } else {
            assert_eq!(
                host, HOST_MAX_HEIGHT,
                "{count} 条的 natural {natural} 该被 420 那臂夹住（母本 X-4 的靶子）"
            );
            let fits = ((host - 2.0 * LIST_INSET) / SLOT_PITCH).floor() as usize;
            assert!(
                count > fits,
                "{count} 条只装得下 {fits} 颗 ⇒ 溢出 {} 颗，就是轨内滚动 + EnsureActiveMarkInView 的暴露面",
                count - fits
            );
            assert_eq!(
                host_height(LIST_INSET, SLOT_PITCH, count, CHAT_BAND_DIP - HOST_BAND_SLACK),
                natural.min(CHAT_BAND_DIP - 2.0 * HOST_BAND_SLACK).min(HOST_MAX_HEIGHT),
                "带子刚好让 `band-64` 与 420 同高那一格：两条臂同时可达"
            );
        }
        if count == 25 {
            padded_marks = marks.clone();
            let page = kernel
                .call("session/page", page_args("s-1001", None, None))
                .expect("25 条大纲全在 journal 里（`seed_journal` 从大纲派生）");
            let events = page_events(&page);
            assert_eq!(
                turn_start_seqs(&events),
                marks.iter().map(|mark| mark.seq).collect::<Vec<_>>(),
                "灌进去的第 4..=25 条各有自己的 `turn/start`，seq = 100 + turn"
            );
            let rows = st12_rows(&events);
            assert_eq!(rows.len(), 50, "25 轮 × (user + assistant) = 50 行真气泡");
            st12_assert_marks_have_targets(&marks, &rows);
        }
        measured.push(marks.len());
        assert!(
            collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
            "条数轴是加元素不是加帧：{count} 条仍只有一发 projection"
        );
        kernel.shutdown();
        drop(mux);
    }
    assert_eq!(measured, vec![4, 20, 21, 25, 40], "五档全数到达，一档都没被截");

    // `band-64` 那一臂在生产调用点（640）今天轮不到，但它就是 `host_height` 的第二条臂：
    // 母本 X-4 写的是「`band-64`/`420` 夹紧臂」，两臂都得拿线上条数演一遍。
    let natural = 2.0 * LIST_INSET + padded_marks.len() as f64 * SLOT_PITCH;
    assert_eq!(padded_marks.len(), PADDED);
    assert_eq!(natural, 504.0, "25 颗的 natural = 4 + 500");
    assert_eq!(
        host_height(LIST_INSET, SLOT_PITCH, padded_marks.len(), 300.0),
        300.0 - HOST_BAND_SLACK,
        "带子 300 ⇒ 夹到 band-64 = 236（这条臂比 420 先赢）"
    );
    assert_eq!(
        host_height(LIST_INSET, SLOT_PITCH, padded_marks.len(), HOST_BAND_GATE),
        natural,
        "band <= 64（首帧没布局出高度）⇒ 回落到 natural，宁可不夹也不把轨压成 0 高"
    );
    assert_eq!(
        host_height(LIST_INSET, SLOT_PITCH, padded_marks.len(), f64::NAN),
        natural,
        "非有限带子同样回落 natural"
    );
    // 夹过的宿主高度是真的改变了卡位：末颗刻度的卡被压进 `[0, host - 卡高]`
    let host = host_height(LIST_INSET, SLOT_PITCH, padded_marks.len(), CHAT_BAND_DIP);
    let last_y = blade2_rs::turnrail::preview_y_for_slot(
        padded_marks.len() - 1,
        LIST_INSET,
        SLOT_PITCH,
        CARD_NOMINAL_HEIGHT,
        host,
    );
    assert_eq!(last_y, host - CARD_NOMINAL_HEIGHT, "25 颗的末卡被 420 夹到贴底");
    assert!(
        blade2_rs::turnrail::slot_center(padded_marks.len() - 1, LIST_INSET, SLOT_PITCH) > host,
        "末颗槽心已在宿主之外 ⇒ 夹紧之后确实「一屏装不下」"
    );
}

/// **两轴同开**（母本没写、桩实现里已存在的那条路）：`seeded_outline_with`（`fake_dsh.rs:1247-1256`）
/// 先 `extend` 形状轴、后 `rail_pad_to_count_at` 补条数轴 ⇒ 形状条目编号被保留、补齐条目从 turn 9
/// 起；且条数轴**只补不摘**（`--rail-count=4 --rail-shapes=3` 仍八条，一条都不回收）。
#[test]
fn st12_both_rail_axes_open_together_pad_after_the_shapes() {
    let (mut kernel, mut mux, stream, frames) =
        st9_control_open(&["--rail-shapes=3", "--rail-count=25"], 4);
    let marks = st12_marks(&frames[3]["value"]);
    assert_eq!(marks.len(), 25, "形状八条在前，补齐到 25 条在后");
    assert_eq!(
        marks.iter().map(|mark| mark.turn).collect::<Vec<_>>(),
        (1..=25).collect::<Vec<_>>(),
        "两轴同开也不许留轮号空洞"
    );
    assert_eq!(
        marks.iter().map(|mark| mark.seq).take(8).collect::<Vec<_>>(),
        vec![3, 41, 88, 104, 105, 106, 107, 108],
        "前三条种子 + 五条形状条目一格不动"
    );
    assert_eq!(
        frames[3]["seq"].as_i64().unwrap_or_default(),
        125,
        "水位跟末条走：25 条 ⇒ 100 + 25"
    );
    for turn in 9..=25i64 {
        let mark = &marks[turn as usize - 1];
        assert_eq!(mark.prompt, format!("第 {turn} 轮：把轮次轨的大纲灌到一屏装不下"));
        assert_eq!(mark.response, format!("第 {turn} 轮：host_height 的夹紧臂与轨内滚动这才演得到"));
    }
    let zh = Catalog::load("zh", None);
    let cards = st12_card_lines(&marks, &zh);
    assert_eq!(
        cards
            .iter()
            .enumerate()
            .filter(|(_, lines)| lines.len() == 1)
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
        vec![5, 7],
        "两枚兜底臂在混合档原位可演：空副行与缺 `response` 键那两条"
    );
    assert_eq!(cards[4][0], zh.lf("第 {0} 轮", &["5".to_string()]), "空主行臂");
    assert_eq!(cards[6][0], zh.lf("第 {0} 轮", &["7".to_string()]), "缺 prompt 键臂折成同一个收敛点");
    let rows = st12_rows(
        &page_events(
            &kernel
                .call("session/page", page_args("s-1001", None, None))
                .expect("25 条混合档大纲该整页回读"),
        ),
    );
    st12_assert_marks_have_targets(&marks, &rows);
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "两轴同开仍不加帧"
    );
    kernel.shutdown();

    let (mut kernel, mut mux, stream, frames) =
        st9_control_open(&["--rail-shapes=3", "--rail-count=4"], 4);
    let marks = st12_marks(&frames[3]["value"]);
    assert_eq!(marks.len(), 8, "条数轴只补不摘：形状档已经 8 条，=4 一条都不回收");
    assert_eq!(
        frames[3]["seq"].as_i64().unwrap_or_default(),
        108,
        "水位仍是末条 108，不是被条数档改写的 104"
    );
    assert!(
        collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
        "只补不摘这一路也不加帧"
    );
    kernel.shutdown();
}

// ==================== ST11 · 表C 十发端点（五颗旋钮）的真 socket 用例 ====================
// 母本 `rust/tmp/ip5-tablec-shapes.md` §6 的用例清单，按本仓口径落：
//   ① 数元素 ≠ 数事件（每条断言指名数的是哪一头）；② 跨进程比对先剥墙上时钟（本轮用
//   `raw_response_body` 比的是 404 那一族，载荷里没有时钟）；③ 一律 `--pace=0`
//   （`launch_with_args` 已压着）；④ 哨兵用例走 `is_empty()`，不用「把帧预算抽干再数」
//   那种自证失败的写法（ST10 §哨兵坑）。桩侧十发的档 0 = `route` 的 404 臂。

/// 表C 十发的方法名（与 `fake_dsh.rs::TABLEC_ENDPOINTS` 同序）。
const TABLEC: &[&str] = &[
    "session/search",
    "session/updateQueue",
    "sessionFeedback/record",
    "fileReferences/list",
    "sessionReferenceResolver/candidates",
    "pluginInventory/list",
    "subagents/list",
    "subagents/prompt",
    "subagents/interruptByParent",
    "session/attachment",
];

/// 十发的主干形状默认 args（母本 §2：五发裹 `request`、五发平铺，**不成律**）。
fn tablec_args(endpoint: &str) -> Value {
    match endpoint {
        "session/search" => json!({ "request": { "query": "登录" } }),
        "session/updateQueue" => json!({ "request": {
            "sessionId": "s-1001", "itemId": "q-1", "action": { "kind": "steer" } } }),
        "sessionFeedback/record" => json!({ "request": { "sessionId": "s-1001" } }),
        "fileReferences/list" => json!({ "agentId": "s-1001", "query": "ma" }),
        "sessionReferenceResolver/candidates" => json!({ "agentId": "s-1001", "query": "补" }),
        "pluginInventory/list" => json!({}),
        "subagents/list" => json!({ "parentSessionId": "s-1001" }),
        "subagents/prompt" => json!({ "request": {
            "requestId": "c2-st11-0001", "parentSessionId": "s-1001", "childSessionId": "s-2101",
            "mode": "continuable", "delivery": "queue",
            "content": [{ "type": "text", "text": "接着上一轮跑" }] } }),
        "subagents/interruptByParent" => json!({
            "childSessionId": "s-2101", "parentSessionId": "s-1001", "mode": "continuable" }),
        "session/attachment" => json!({ "request": {
            "sessionId": "s-1001", "attachmentId": "att-1" } }),
        other => panic!("表C 没有这一发：{other}"),
    }
}

/// 带表C 旋钮的启动（`launch_with_args` 已压 `--pace=0`）；一颗旋钮一个 token，
/// 空串 = 压根不带新旋钮。**不能**把「一坨带空格的档」当一个 argv 传：桩的 `knob()` 是
/// `strip_prefix` + `parse::<u64>`，`"1 --refs=1"` 会静默回落 0（母本 §6 的脏值口径）。
fn tablec_kernel(knobs: &str) -> Kernel {
    let owned: Vec<String> = knobs.split_whitespace().map(str::to_string).collect();
    let argv: Vec<&str> = owned.iter().map(String::as_str).collect();
    Kernel::start(&launch_with_args(&argv)).expect("假内核应完成 dsh web: 握手")
}

/// **用例 1**：不带任何新旋钮 ⇒ 十发逐发 `Err` 以 `not_found:` 开头（今天的现状）。
#[test]
fn tablec_all_ten_are_404_when_no_knob_is_given() {
    let mut kernel = tablec_kernel("");
    for endpoint in TABLEC {
        let error = kernel
            .call(endpoint, tablec_args(endpoint))
            .expect_err("十发在关档不该有路由");
        assert!(
            error.contains("HTTP 404"),
            "{endpoint} 关档该落 404 默认臂。现测：`Kernel::call`（`kernel.rs:3468`）对 \
             status>=400 走 `\"<method> → HTTP <status>: <body>\"` 早退臂，**不**是母本 §6 \
             说的 `not_found:` 前缀。实际 {error}"
        );
        assert!(error.contains(endpoint), "{endpoint} 的错误串该点着自己的名：{error}");
    }
    kernel.shutdown();
}

/// **用例 2**：显式 `--retrieval=0` 与压根不带 argv 是**同一台机器**（原始响应体逐字节同）。
#[test]
fn tablec_retrieval_zero_is_the_same_machine_as_no_argv() {
    let mut off = tablec_kernel("--retrieval=0");
    let mut none = tablec_kernel("");
    for endpoint in ["session/search", "session/updateQueue", "sessionFeedback/record"] {
        let args = tablec_args(endpoint);
        let a = raw_response_body(&off, endpoint, "c2-st11", args.clone());
        let b = raw_response_body(&none, endpoint, "c2-st11", args);
        assert_eq!(a, b, "{endpoint} 的 0 档与不带旋钮该逐字节同");
        assert!(a.contains("假内核不支持"), "{a}");
    }
    off.shutdown();
    none.shutdown();
}

/// **用例 3**：`session/search` 的回执是 `{items,hasMore}`（**不是** `{sessions}`），
/// 数的是**数组元素**；档 2 翻 `hasMore`。
#[test]
fn tablec_search_receipt_is_items_not_sessions_and_has_more_flips() {
    let mut hit = tablec_kernel("--retrieval=1");
    let value = hit
        .call("session/search", tablec_args("session/search"))
        .expect("档 1 该回命中");
    assert_eq!(sorted_keys(&value), vec!["hasMore", "items"]);
    assert!(value.get("sessions").is_none(), "键名是 items，不是 RD9 说的 sessions");
    let items = value["items"].as_array().expect("items 得是数组");
    assert_eq!(items.len(), 2, "数的是数组元素，实数 {}", items.len());
    for item in items {
        assert_eq!(sorted_keys(item), vec!["sessionId", "snippet"]);
        assert!(!item["sessionId"].as_str().unwrap_or_default().is_empty());
        assert!(item["snippet"].is_string(), "snippet 是 schema 的必填串");
    }
    assert_eq!(value["hasMore"], json!(false));
    hit.shutdown();

    let mut empty = tablec_kernel("--retrieval=2");
    let value = empty
        .call("session/search", tablec_args("session/search"))
        .expect("档 2 也该回执");
    assert_eq!(value["items"].as_array().expect("空命中也是数组").len(), 0);
    assert_eq!(value["hasMore"], json!(true), "档 2 的 hasMore 该翻成 true");
    empty.shutdown();
}

/// **用例 4**：档 3「检索未开启」的两条 needle 都在 message 里；脏档值回落 = 与关档同形。
#[test]
fn tablec_search_disabled_arm_matches_both_needles_and_dirty_knob_falls_back() {
    let mut disabled = tablec_kernel("--retrieval=3");
    let error = disabled
        .call("session/search", tablec_args("session/search"))
        .expect_err("档 3 该拒");
    assert!(
        error.contains("session search is disabled")
            || error.contains("SESSION_QUERY_SEARCH_DISABLED"),
        "主干 `IsSearchDisabled` 两条 needle 至少中一条：{error}"
    );
    disabled.shutdown();

    let mut dirty = tablec_kernel("--retrieval=abc");
    let error = dirty
        .call("session/search", tablec_args("session/search"))
        .expect_err("脏值该回落 0 = 关档");
    assert!(error.contains("HTTP 404"), "{error}");
    dirty.shutdown();
}

/// **用例 5**：`updateQueue` 三臂（edit/steer/remove）都过闸，回执是字面量 `{accepted:true}`。
#[test]
fn tablec_update_queue_accepts_all_three_actions_with_a_literal_receipt() {
    let mut kernel = tablec_kernel("--retrieval=1");
    for (kind, action) in [
        ("steer", json!({ "kind": "steer" })),
        ("remove", json!({ "kind": "remove" })),
        ("edit", json!({ "kind": "edit", "content": [
            { "type": "text", "text": "改成这一句" },
            { "type": "image", "mediaType": "image/png", "data": "AA==" }
        ] })),
    ] {
        let value = kernel
            .call(
                "session/updateQueue",
                json!({ "request": { "sessionId": "s-1001", "itemId": "q-1", "action": action } }),
            )
            .unwrap_or_else(|e| panic!("{kind} 臂该被接受: {e}"));
        assert_eq!(value, json!({ "accepted": true }), "{kind} 臂回执是字面量");
    }
    let bad = kernel
        .call(
            "session/updateQueue",
            json!({ "request": { "sessionId": "s-1001", "itemId": "q-1",
                                 "action": { "kind": "drop" } } }),
        )
        .expect_err("第四种 kind 不存在于 union");
    assert!(bad.starts_with("bad_args"), "{bad}");
    kernel.shutdown();
}

/// **用例 6**：档 4 的错误**按 message 文本**命中主干那条 `Contains("no longer pending")`。
#[test]
fn tablec_queue_item_not_found_matches_on_message_text() {
    let mut kernel = tablec_kernel("--retrieval=4");
    let error = kernel
        .call("session/updateQueue", tablec_args("session/updateQueue"))
        .expect_err("档 4 该拒");
    assert!(
        error.to_lowercase().contains("no longer pending"),
        "主干是 Contains 判据，不是码比对：{error}"
    );
    assert!(error.contains("session/queue-item-not-found"), "{error}");
    assert!(error.contains("q-1"), "message 该把 itemId 带上：{error}");
    kernel.shutdown();
}

/// **用例 7（十发里唯一加帧的那一档）**：档 5 接受 `updateQueue` 后往控制流追推**恰好一帧**
/// `queue`；同一条用例的另一头是档 1 的哨兵（**不抽干帧预算**，静默窗里该是空批）。
#[test]
fn tablec_queue_tier5_pushes_exactly_one_queue_frame_and_tier1_pushes_none() {
    let (mut kernel, mut mux, stream) =
        open_control_stream_by(&launch_with_args(&["--retrieval=5"]));
    let lead = item_values(&collect(&mut mux, &stream, 4), &stream);
    assert_eq!(
        lead.iter().map(tag).collect::<Vec<_>>(),
        vec!["baseline", "queue", "jobs", "projection"],
        "首批四帧一个字节都不该被这一刀动到"
    );
    kernel
        .call("session/updateQueue", tablec_args("session/updateQueue"))
        .expect("档 5 该接受");
    let extra = item_values(&collect(&mut mux, &stream, 1), &stream);
    assert_eq!(extra.len(), 1, "追推的**元素数**该恰好 1，实数 {}", extra.len());
    assert_eq!(extra[0]["type"], json!("queue"));
    assert_eq!(extra[0]["sessionId"], json!("s-1001"));
    assert_eq!(extra[0]["items"].as_array().expect("items 得是数组").len(), 0);
    kernel.shutdown();

    let (mut quiet, mut qm, qs) = open_control_stream_by(&launch_with_args(&["--retrieval=1"]));
    let _ = collect(&mut qm, &qs, 4);
    quiet
        .call("session/updateQueue", tablec_args("session/updateQueue"))
        .expect("档 1 也接受");
    let idle: Vec<MuxEvent> = qm
        .collect(WAIT_BUDGET)
        .expect("静默窗读取不该失败")
        .into_iter()
        .filter(|event| event_stream(event) == qs)
        .collect();
    assert!(idle.is_empty(), "档 1 receipt-only，一帧都不该加，实际 {idle:?}");
    quiet.shutdown();
}

/// **用例 8**：双层 `ok` 只有 `sessionFeedback/record` 一发，且成功格是 `{recorded:true}`
/// ——**桩绝不「顺手修好」成 `{ok:true}`**（主干成功臂恒假是主干的现状，母本 §8-②）。
#[test]
fn tablec_session_feedback_record_is_the_only_double_ok_receipt() {
    let mut ok = tablec_kernel("--retrieval=1");
    let stripped = ok
        .call("sessionFeedback/record", tablec_args("sessionFeedback/record"))
        .expect("外层 ok:true，剥一层还剩内层");
    assert_eq!(stripped["ok"], json!(true), "第一层剥完看到的是内层的 ok");
    assert_eq!(stripped["value"], json!({ "recorded": true }));
    let raw: Value =
        serde_json::from_str(&raw_response_body(&ok, "sessionFeedback/record", "c2-st11-rec",
            tablec_args("sessionFeedback/record")))
            .expect("原始信封得是 JSON");
    assert_eq!(raw["result"]["ok"], json!(true), "第一层");
    assert_eq!(raw["result"]["value"]["ok"], json!(true), "第二层");
    assert_eq!(
        sorted_keys(&raw["result"]["value"]["value"]),
        vec!["recorded"],
        "第三层只有 recorded —— 桩绝不「顺手修好」成 ok（母本 §8-②）"
    );
    ok.shutdown();

    let mut failed = tablec_kernel("--retrieval=4");
    let stripped = failed
        .call("sessionFeedback/record", tablec_args("sessionFeedback/record"))
        .expect("失败臂外层仍是 ok:true");
    assert_eq!(stripped["ok"], json!(false));
    assert_eq!(stripped["error"]["code"], json!("session-not-found"));
    assert_eq!(stripped["error"]["sessionId"], json!("s-1001"));
    assert!(stripped.get("value").is_none(), "两态互斥：失败臂没有 value");
    failed.shutdown();
}

/// **用例 9**：record 的可选键「整个不存在」过闸，`null` 被拒（主干注释明说内核 zod 拒 null）。
#[test]
fn tablec_record_optional_keys_are_absent_not_null() {
    let mut kernel = tablec_kernel("--retrieval=1");
    kernel
        .call("sessionFeedback/record", json!({ "request": { "sessionId": "s-1001" } }))
        .expect("只带 sessionId 是最小合法形");
    kernel
        .call(
            "sessionFeedback/record",
            json!({ "request": { "sessionId": "s-1001", "text": "备注", "category": "task-result" } }),
        )
        .expect("两颗可选键都带也合法");
    let error = kernel
        .call(
            "sessionFeedback/record",
            json!({ "request": { "sessionId": "s-1001", "text": null } }),
        )
        .expect_err("null 该被拒");
    assert!(error.starts_with("bad_args"), "{error}");
    assert!(error.contains("\"text\""), "message 该点名被拒的键：{error}");
    let unknown = kernel
        .call(
            "sessionFeedback/record",
            json!({ "request": { "sessionId": "s-1001", "category": "made-up" } }),
        )
        .expect_err("category 是七值 union");
    assert!(unknown.starts_with("bad_args"), "{unknown}");
    kernel.shutdown();
}

/// **用例 10**：两发引用的 `value` **顶层就是数组**（不许是 `{items:[]}`）；档 3 单路失败、
/// 档 4 两路都失败 ⇒ 主干「单路失败另一路仍出候选」与「双路失败摊头部」两型各有靶子。
#[test]
fn tablec_reference_pair_returns_top_level_arrays_and_fails_one_side_first() {
    let mut full = tablec_kernel("--refs=1");
    for endpoint in ["fileReferences/list", "sessionReferenceResolver/candidates"] {
        let value = full
            .call(endpoint, tablec_args(endpoint))
            .unwrap_or_else(|e| panic!("{endpoint} 档 1 该出候选: {e}"));
        let items = value
            .as_array()
            .unwrap_or_else(|| panic!("{endpoint} 顶层得是数组，实为 {value}"));
        assert_eq!(items.len(), 2, "数的是数组元素，实数 {}", items.len());
    }
    let files = full
        .call("fileReferences/list", tablec_args("fileReferences/list"))
        .expect("files");
    for entry in files.as_array().expect("数组") {
        assert_eq!(sorted_keys(entry), vec!["kind", "path"], "元素不许有 size/mtime");
        assert!(matches!(entry["kind"].as_str(), Some("file") | Some("directory")));
    }
    let candidates = full
        .call(
            "sessionReferenceResolver/candidates",
            tablec_args("sessionReferenceResolver/candidates"),
        )
        .expect("candidates");
    let rows = candidates.as_array().expect("数组");
    assert_eq!(
        sorted_keys(&rows[0]),
        vec!["createdAt", "cwd", "label", "mention", "sameWorkspace", "sessionId"],
        "createdAt 是必填 number，主干不读也得在线上留着"
    );
    assert_eq!(
        sorted_keys(&rows[1]),
        vec!["createdAt", "label", "mention", "sameWorkspace", "sessionId"],
        "cwd 是这颗 schema 里唯一的可选键"
    );
    assert_eq!(rows[1]["label"], json!(""), "空 label ⇒ 主干回落 sessionId");
    assert_eq!(rows[1]["sameWorkspace"], json!(false));
    full.shutdown();

    let mut one_sided = tablec_kernel("--refs=3");
    one_sided
        .call("fileReferences/list", tablec_args("fileReferences/list"))
        .expect("档 3 的 files 仍出候选");
    assert!(
        one_sided
            .call("sessionReferenceResolver/candidates", tablec_args("sessionReferenceResolver/candidates"))
            .is_err(),
        "档 3 只该塌掉 candidates"
    );
    one_sided.shutdown();

    let mut both = tablec_kernel("--refs=4");
    for endpoint in ["fileReferences/list", "sessionReferenceResolver/candidates"] {
        assert!(
            both.call(endpoint, tablec_args(endpoint)).is_err(),
            "{endpoint} 档 4 该失败"
        );
    }
    both.shutdown();

    // 档 5 = files 少 `kind` 的负形（strict 会拒 ⇒ 看分叉回落方向），candidates 照常。
    let mut negative = tablec_kernel("--refs=5");
    let files = negative
        .call("fileReferences/list", tablec_args("fileReferences/list"))
        .expect("档 5 仍是 200");
    assert_eq!(sorted_keys(&files.as_array().expect("数组")[0]), vec!["path"]);
    negative
        .call("sessionReferenceResolver/candidates", tablec_args("sessionReferenceResolver/candidates"))
        .expect("档 5 的 candidates 不受影响");
    negative.shutdown();
}

/// **用例 11**：引用两发的 wire 键是**平铺** `{agentId,query}` ⇒ 裹 `request` 当场 `bad_args`；
/// `pluginInventory/list` 的 `parameters` 是空数组 ⇒ 多带一颗键就拒。
#[test]
fn tablec_reference_wire_keys_are_flat_agentid_query() {
    // 这一条同时用到两把刀：`--refs=1` 开引用两发，`--inventory=1` 开台账那一发
    // （台账关档时它落的是 404 臂，不是 `bad_args`，混在一条里会看不出是谁在拒）。
    let mut kernel = tablec_kernel("--refs=1 --inventory=1");
    let error = kernel
        .call(
            "fileReferences/list",
            json!({ "request": { "agentId": "s-1001", "query": "ma" } }),
        )
        .expect_err("这一发不裹 request");
    assert!(error.contains("missing \"agentId\""), "{error}");
    assert!(error.contains("unexpected \"request\""), "{error}");
    let empty_args = kernel
        .call("pluginInventory/list", json!({ "agentId": "s-1001" }))
        .expect_err("pluginInventory/list 的 parameters 是空数组");
    assert!(empty_args.contains("unexpected \"agentId\""), "{empty_args}");
    kernel.shutdown();
}

/// **用例 12**：插件台账四档 —— `fiberPhase:null` 合法、空 `entries` **不是错误**、
/// `agentPresets` 只在档 3、档 4 走 `gateway/internal`。
#[test]
fn tablec_plugin_inventory_four_tiers_agree_with_the_descriptor() {
    let mut mixed = tablec_kernel("--inventory=1");
    let value = mixed
        .call("pluginInventory/list", json!({}))
        .expect("档 1 该出台账");
    assert_eq!(sorted_keys(&value), vec!["entries"]);
    let entries = value["entries"].as_array().expect("entries 得是数组");
    assert_eq!(entries.len(), 4, "数的是条目元素数，实数 {}", entries.len());
    for entry in entries {
        assert_eq!(
            sorted_keys(entry),
            vec!["enabled", "entryId", "fiberPhase", "moduleName"],
            "条目不许有 status 这类 schema 外的键（只有 fiberPhase）"
        );
        assert!(entry["enabled"].is_boolean(), "enabled 只认字面 true/false");
    }
    assert!(entries.iter().any(|e| e["fiberPhase"].is_null()), "无 fiber ⇒ fiberPhase 是 null");
    assert!(entries.iter().any(|e| e["fiberPhase"] == json!("failed")));
    assert!(entries.iter().any(|e| e["fiberPhase"] == json!("loading")));
    assert!(entries.iter().any(|e| e["enabled"] == json!(false)), "得有归属给「未启用」计数");
    mixed.shutdown();

    let mut empty = tablec_kernel("--inventory=2");
    let value = empty
        .call("pluginInventory/list", json!({}))
        .expect("空台账不是错误");
    assert_eq!(value["entries"].as_array().expect("数组").len(), 0);
    empty.shutdown();

    let mut presets = tablec_kernel("--inventory=3");
    let value = presets.call("pluginInventory/list", json!({})).expect("档 3");
    assert_eq!(sorted_keys(&value), vec!["agentPresets", "entries"]);
    let group = presets
        .call("pluginInventory/list", json!({}))
        .expect("档 3 第二次")["agentPresets"]
        .as_array()
        .expect("agentPresets 得是数组")
        .clone();
    let rows = group[0]["rows"].as_array().expect("rows 得是数组");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row["enabled"] == json!("conditional")));
    assert!(rows.iter().any(|row| row["entryId"].is_null()), "entryId 可为 null");
    assert!(group[1].get("broken").is_some(), "坏预设要有 broken 原因");
    assert!(group[1].get("name").is_none(), "name 是可选键");
    assert_eq!(sorted_keys(&group[1]), vec!["broken", "id", "isDefault", "rows", "trust"]);
    presets.shutdown();

    let mut refused = tablec_kernel("--inventory=4");
    let error = refused
        .call("pluginInventory/list", json!({}))
        .expect_err("档 4 该拒");
    assert!(error.starts_with("gateway/internal"), "{error}");
    refused.shutdown();
}

/// **用例 13**：附件回执带**整颗 `attachment` 块**；档 2 缺 `data`、档 3 `attachment-invalid`、
/// 档 4 的 `data` 解码后过 16 MiB（主干只留占位不出图）。
#[test]
fn tablec_attachment_receipt_tiers_are_the_four_documented_states() {
    let mut ok = tablec_kernel("--attachment=1");
    let value = ok
        .call("session/attachment", tablec_args("session/attachment"))
        .expect("档 1 该出图");
    assert_eq!(sorted_keys(&value), vec!["attachment", "data"]);
    assert_eq!(
        sorted_keys(&value["attachment"]),
        vec!["attachmentId", "bytes", "height", "mediaType", "name", "width"],
        "attachment 整颗必填（主干不读但线上有）"
    );
    assert_eq!(value["attachment"]["attachmentId"], json!("att-1"), "attachmentId 要跟着入参走");
    assert_eq!(value["attachment"]["mediaType"], json!("image/png"));
    let data = value["data"].as_str().expect("data 得是串");
    // 96 字符 base64（一组 4 字符 ↔ 3 字节，末组两个 `=`）⇒ 70 字节，与 `bytes` 自洽；
    // 70 是 `printf '%s' "$PNG" | base64 -d | wc -c` 的现测值。
    assert_eq!(data.len(), 96, "base64 长度与 bytes 该自洽");
    assert_eq!(value["attachment"]["bytes"], json!(70));
    assert_eq!(data.len() / 4 * 3 - 2, 70, "解码长度算法");
    assert!(
        data.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c)) && data.ends_with("=="),
        "必须是主干 `Convert.FromBase64String` 吃得下的合法 base64"
    );
    ok.shutdown();

    let mut nodata = tablec_kernel("--attachment=2");
    let value = nodata
        .call("session/attachment", tablec_args("session/attachment"))
        .expect("档 2 仍 200");
    assert!(value.get("data").is_none(), "data 整颗缺席，不是 null：{value}");
    nodata.shutdown();

    let mut missing = tablec_kernel("--attachment=3");
    let error = missing
        .call("session/attachment", tablec_args("session/attachment"))
        .expect_err("档 3 该拒");
    assert!(error.starts_with("session/attachment-invalid"), "{error}");
    assert!(error.contains("att-1"), "{error}");
    missing.shutdown();

    let mut huge = tablec_kernel("--attachment=4");
    let value = huge
        .call("session/attachment", tablec_args("session/attachment"))
        .expect("档 4 仍 200");
    let decoded = value["data"].as_str().expect("串").len() / 4 * 3;
    assert!(decoded > 16 * 1024 * 1024, "解码长度该过 16 MiB，实为 {decoded}");
    assert_eq!(value["attachment"]["bytes"], json!(16 * 1024 * 1024 + 1));
    huge.shutdown();
}

/// **用例 14**：子代理目录三型各有形状（**条目不许有 title/parentSessionId/sessionId**）、
/// `prompt` 回执是 `{messageId}`、`interruptByParent` 的 `mode` 是字面量、`list` 平铺不裹。
#[test]
fn tablec_subagents_catalog_prompt_and_interrupt_keep_the_documented_shapes() {
    let mut kernel = tablec_kernel("--subagents=1");
    let value = kernel
        .call("subagents/list", tablec_args("subagents/list"))
        .expect("档 1 该出目录");
    assert_eq!(sorted_keys(&value), vec!["entries", "parentAvailable"]);
    assert_eq!(value["parentAvailable"], json!(true));
    let entries = value["entries"].as_array().expect("entries 得是数组");
    assert_eq!(entries.len(), 3, "数的是条目元素数，实数 {}", entries.len());
    assert_eq!(
        sorted_keys(&entries[0]),
        vec!["activity", "hasChildren", "id", "kind", "label", "mode"],
        "continuable 的 label 是必填，且没有 title/parentSessionId 这两颗线键"
    );
    assert_eq!(entries[0]["mode"], json!("continuable"));
    assert_eq!(sorted_keys(&entries[1]), vec!["activity", "hasChildren", "id", "kind", "mode"]);
    assert_eq!(entries[1]["mode"], json!("one-shot"), "one-shot 的 label 可选 ⇒ 缺席");
    assert_eq!(sorted_keys(&entries[2]), vec!["id", "kind", "reason"]);
    assert!(matches!(
        entries[2]["reason"].as_str(),
        Some("corrupt") | Some("unsupported") | Some("unavailable")
    ));
    let prompt = kernel
        .call("subagents/prompt", tablec_args("subagents/prompt"))
        .expect("prompt 该接受");
    assert_eq!(prompt, json!({ "messageId": "m-c2-2101" }), "回执是 messageId，不是空对象");
    let interrupted = kernel
        .call("subagents/interruptByParent", tablec_args("subagents/interruptByParent"))
        .expect("interrupt 该接受");
    assert_eq!(interrupted, json!({ "accepted": true }));
    let wrong_mode = kernel
        .call(
            "subagents/interruptByParent",
            json!({ "childSessionId": "s-2101", "parentSessionId": "s-1001", "mode": "one-shot" }),
        )
        .expect_err("mode 是 z.literal(\"continuable\")");
    assert!(wrong_mode.starts_with("bad_args"), "{wrong_mode}");
    let wrapped = kernel
        .call("subagents/list", json!({ "request": { "parentSessionId": "s-1001" } }))
        .expect_err("这一发平铺，不裹 request");
    assert!(wrapped.contains("missing \"parentSessionId\""), "{wrapped}");
    let bad_delivery = kernel
        .call(
            "subagents/prompt",
            json!({ "request": { "requestId": "c2-x", "parentSessionId": "s-1001",
                                 "childSessionId": "s-2101", "mode": "continuable",
                                 "delivery": "immediate",
                                 "content": [{ "type": "text", "text": "x" }] } }),
        )
        .expect_err("delivery 只有 queue|steer");
    assert!(bad_delivery.starts_with("bad_args"), "{bad_delivery}");
    kernel.shutdown();
}

/// **用例 15**：档 2 父不在线、档 3 两条错误码、档 4 元素非对象（整次作废那一型）、
/// 档 5 one-shot 带 `hasChildren:true`（折叠钮负形）。
#[test]
fn tablec_subagents_error_and_negative_tiers_are_distinct() {
    let mut offline = tablec_kernel("--subagents=2");
    let value = offline
        .call("subagents/list", tablec_args("subagents/list"))
        .expect("档 2 仍出目录");
    assert_eq!(value["parentAvailable"], json!(false));
    assert_eq!(value["entries"].as_array().expect("数组").len(), 3);
    offline.shutdown();

    let mut errors = tablec_kernel("--subagents=3");
    let prompt = errors
        .call("subagents/prompt", tablec_args("subagents/prompt"))
        .expect_err("档 3 的 prompt 该拒");
    assert!(prompt.starts_with("subagent/parent-unavailable"), "{prompt}");
    let interrupt = errors
        .call("subagents/interruptByParent", tablec_args("subagents/interruptByParent"))
        .expect_err("档 3 的 interrupt 该拒");
    assert!(interrupt.starts_with("subagent/not-resumable"), "{interrupt}");
    errors.shutdown();

    let mut negative = tablec_kernel("--subagents=4");
    let value = negative
        .call("subagents/list", tablec_args("subagents/list"))
        .expect("档 4 仍 200");
    let entries = value["entries"].as_array().expect("数组");
    assert_eq!(entries.len(), 1, "数的是元素数");
    assert!(entries[0].is_string(), "元素不是对象 ⇒ 主干 parse_catalog 整次作废");
    negative.shutdown();

    let mut five = tablec_kernel("--subagents=5");
    let value = five
        .call("subagents/list", tablec_args("subagents/list"))
        .expect("档 5");
    let entries = value["entries"].as_array().expect("数组");
    assert_eq!(entries[1]["mode"], json!("one-shot"));
    assert_eq!(entries[1]["hasChildren"], json!(true), "one-shot 带 hasChildren 的负形");
    assert_eq!(entries[0]["hasChildren"], json!(false), "另一格不受这一档影响");
    five.shutdown();
}

/// **用例 16（总闸）**：五刀叠开（各档 1）⇒ 十发**逐发顶层键集**等于 typert 允许集，
/// 且九发单层那一族剥完 `Kernel::call` 之后**没有第二层 `ok`**（防 `sessionStats.totalTokens`
/// 那一型复发）；两发数组那一对的顶层就是数组，不是对象。
#[test]
fn tablec_all_five_knobs_stacked_emit_no_top_level_key_outside_the_schema() {
    let mut stacked = tablec_kernel(
        "--retrieval=1 --refs=1 --inventory=1 --attachment=1 --subagents=1",
    );
    let expected: BTreeMap<&str, Vec<&str>> = [
        ("session/search", vec!["hasMore", "items"]),
        ("session/updateQueue", vec!["accepted"]),
        ("sessionFeedback/record", vec!["ok", "value"]),
        ("subagents/list", vec!["entries", "parentAvailable"]),
        ("subagents/prompt", vec!["messageId"]),
        ("subagents/interruptByParent", vec!["accepted"]),
        ("pluginInventory/list", vec!["entries"]),
        ("session/attachment", vec!["attachment", "data"]),
    ]
    .into_iter()
    .collect();
    let array_pair = ["fileReferences/list", "sessionReferenceResolver/candidates"];
    for endpoint in TABLEC {
        let value = stacked
            .call(endpoint, tablec_args(endpoint))
            .unwrap_or_else(|e| panic!("五刀叠开时 {endpoint} 该有路由: {e}"));
        if array_pair.contains(endpoint) {
            assert!(value.is_array(), "{endpoint} 顶层得是数组，实为 {value}");
            continue;
        }
        assert_eq!(sorted_keys(&value), expected[endpoint], "{endpoint} 顶层键集");
        if *endpoint != "sessionFeedback/record" {
            assert!(
                value.get("ok").is_none(),
                "{endpoint} 是单层族，剥完一层之后不该还有第二个 ok：{value}"
            );
        }
    }
    stacked.shutdown();
}

// ==================== 台账 #144 · KW2 §6 那批：cordis 八发构造器 × 桩键集闸（ST13 落地）=========
//
// 母本 = `tmp/kw2-report.md` §6（那里给的是**四发** `#[test]` + 一颗 helper，派单说的「五发」按
// CR2 §6 第 12 项补齐成五发；差异记 st13-report §6）与 `tmp/cr2-cordis-host.md` §1（12 端调用形状）。
// 与母本的两处做法不同，都是**收紧**不是放松：
// ① 母本每发各起一颗桩进程 ⇒ 五档 × 八发 = 40 起子进程；本轮一档一颗（八发同进程），
//    并且**判据名单取自分叉自己的 `CORDIS_METHODS`**，测试侧不写第二份 `dynamicCordisRunner/<x>` 全名。
// ② 母本那颗 `assert!(outcome.is_ok() || true)` 是恒真式（=假绿），本轮换成可伪的成/败态台账。
// 这一族全部**实跑过**（`--cordis=1..5`），不是照构造器与键集闸推出来的判据。

use blade2_rs::kernel::{
    CORDIS_METHODS, CordisRunResolution, cordis_get_client_code, cordis_inventory,
    cordis_resolve_request_run, cordis_run_host_half, cordis_settle_user_run,
    cordis_stop_from_panel, cordis_sync_inspect_manifest, cordis_undefine_from_panel,
    parse_cordis_rows,
};

/// 八发的**样例实参**：形状全由 B1 的构造器给（这才是本族的判据 —— 一旦 `kernel.rs` 的键集与
/// 桩的 `st6_cordis_wire_spec` 漂移，这几发立刻红）。假世界身份取自桩的 `CORDIS_AGENT/PLUGIN/
/// PACKAGE/RUN/REQUEST`（`fake_dsh.rs:8612-8618`）：`s-1001` / `demo-cordis` / `pkg-1` / `run-1` / `req-1`。
fn st13_cordis_build(tail: &str) -> RpcCall {
    match tail {
        "inventory" => cordis_inventory(),
        "runHostHalf" => {
            cordis_run_host_half("s-1001", "demo-cordis", "pkg-1", "run", Some("req-1"), false)
        }
        "resolveRequestRun" => cordis_resolve_request_run(
            "req-1",
            &CordisRunResolution::Activated {
                plugin_run_id: "run-1".to_string(),
            },
        ),
        "settleUserRun" => {
            cordis_settle_user_run("s-1001", "demo-cordis", &CordisRunResolution::Rejected)
        }
        "stopFromPanel" => cordis_stop_from_panel("s-1001", "demo-cordis"),
        "undefineFromPanel" => cordis_undefine_from_panel("s-1001", "demo-cordis"),
        "getClientCode" => cordis_get_client_code("s-1001", "demo-cordis", "run-1"),
        "syncInspectManifest" => cordis_sync_inspect_manifest(),
        other => panic!("{other} 不在 CORDIS_METHODS 的八发名单上"),
    }
}

/// 名单取自分叉的登记表（**恰八颗**，不含主干零调用方那四面）；这里只剥出尾段用于分派。
fn st13_cordis_tails() -> Vec<&'static str> {
    CORDIS_METHODS
        .iter()
        .map(|method| method.rsplit('/').next().expect("端点名带斜杠"))
        .collect()
}

/// 一档一颗桩，`f` 拿到的就是那颗会话线上的 `Kernel`（所有出口统一 shutdown，防孤儿进程）。
fn st13_in_cordis_level<T>(level: u64, f: impl FnOnce(&mut Kernel) -> T) -> T {
    let knob = format!("--cordis={level}");
    let mut kernel = Kernel::start(&launch_with(&["--pace=0", &knob])).expect("假内核握手");
    let out = f(&mut kernel);
    kernel.shutdown();
    out
}

/// 八发在同一档里各打一发（母本 §6 的 `kw2_cordis_call` 换形：省掉 35 起子进程）。
fn st13_cordis_level_outcomes(level: u64) -> Vec<(&'static str, Result<Value, String>)> {
    st13_in_cordis_level(level, |kernel| {
        st13_cordis_tails()
            .into_iter()
            .map(|tail| {
                let call = st13_cordis_build(tail);
                (tail, kernel.call(call.method, call.args))
            })
            .collect()
    })
}

/// **判据一（键集闸）**：八发的 args 全由分叉构造器产出 ⇒ 档 1..5 任何一档都不许落
/// `not_found`（那一档没臂）也不许落 `bad_args`（键集与 descriptor 漂移）。
/// 成/败台账也逐档钉死：本族**只有 `getClientCode` 会落 RPC 层错误**，且只有档 3 成功 ——
/// 内核那三条失败路径是 `throw`（`index.js:1819` 无此插件 / `:1821` 不是活动 run /
/// `:1823` 该包无 Client 半边；KW2 §7-4 把 RD6 的 `:1818-1825` 收紧成这三格），
/// 桩按档分配：1⇒无插件、2/4⇒无活动 run、5⇒无 Client 半边、3⇒真回源码。
#[test]
fn st13_cordis_argument_builders_pass_the_stub_key_gate_at_every_level() {
    for level in 1..=5 {
        let outcomes = st13_cordis_level_outcomes(level);
        assert_eq!(outcomes.len(), 8, "档 {level}：名单不是八发");
        for (tail, outcome) in outcomes {
            assert_eq!(
                outcome.is_err(),
                tail == "getClientCode" && level != 3,
                "档 {level} 的 {tail} 成/败态与桩的台账不符：{outcome:?}"
            );
            if let Err(err) = &outcome {
                assert!(
                    !err.contains("not_found"),
                    "档 {level} 的 {tail} 没臂（本档该有）：{err}"
                );
                assert!(
                    !err.contains("bad_args"),
                    "档 {level} 的 {tail}：分叉 args 的键集与桩的 descriptor 漂移 ⇒ {err}"
                );
                assert!(
                    err.starts_with("gateway/internal:"),
                    "档 {level} 的 {tail} 落到了第三种失败（本族唯一该有的 RPC 码是 gateway/internal）：{err}"
                );
            }
            if tail == "getClientCode" && level == 3 {
                assert_eq!(
                    sorted_keys(outcome.as_ref().expect("档 3 该有回执")),
                    ["code", "name", "packageId", "pluginId", "pluginRunId"],
                    "getClientCode 的 result 键集（`typert.remote-client.js:11-17`）"
                );
            }
        }
    }
}

/// **判据二（面板路的 null 键）**：主干两条路唯一的差别就是 `requestId` ——
/// 审批路给已登记的 id、面板「运行」路给**显式 null**（`Cordis.cs:759` 传的是 C# 的 `null`）。
/// 把 `None` 折成「键不存在」会先被键集闸拒成 `missing "requestId"` ⇒ 正反两臂都要在这里钉住。
#[test]
fn st13_panel_path_null_request_id_survives_the_socket_and_hits_the_pending_branch() {
    let approval_call = cordis_run_host_half(
        "s-1001",
        "demo-cordis",
        "pkg-1",
        "run",
        Some("req-1"),
        false,
    );
    let panel_call =
        cordis_run_host_half("s-1001", "demo-cordis", "pkg-1", "run", None, false);
    let method = panel_call.method;
    // 键在、值是 null —— 这一格若被谁「顺手」改成省键，下面的 socket 反证就会变成唯一取证。
    assert_eq!(
        panel_call.args["requestId"],
        Value::Null,
        "面板路该发显式 null 键，实际 {panel_call:#?}"
    );
    let (approval, panel, omitted) = st13_in_cordis_level(2, |kernel| {
        let approval = kernel.call(approval_call.method, approval_call.args);
        let panel = kernel.call(method, panel_call.args.clone());
        // 反证：把同一发的 `requestId` **整颗删掉** ⇒ 必须被键集闸拒（否则「发 null 键」这条判据是假的）。
        let mut stripped = panel_call.args.clone();
        stripped
            .as_object_mut()
            .expect("args 是平铺对象")
            .remove("requestId");
        let omitted = kernel.call(method, stripped);
        (approval, panel, omitted)
    });
    let approval = approval.expect("审批路（带已登记 id）该有回执");
    assert_eq!(approval["ok"], json!(true), "档 2 带 req-1 该授权成功：{approval}");
    assert_eq!(
        sorted_keys(&approval),
        ["ok", "packageId", "pluginId", "pluginRunId", "startedHere", "waitingFor"],
        "成功格的键集（`typert.remote-client.js` 的 runHostHalf result + 桩的 `running()`）"
    );
    let panel = panel.expect("面板路该有信封（业务失败也在 value 里，不是 RPC 错）");
    assert_eq!(panel["ok"], json!(false), "档 2 面板路该撞待审：{panel}");
    assert_eq!(sorted_keys(&panel), ["message", "ok"]);
    assert!(
        panel["message"]
            .as_str()
            .is_some_and(|m| m.contains("has pending run request")),
        "档 2 的面板路该落 `index.js:1789-1791` 那句：{panel}"
    );
    let omitted = omitted.expect_err("省掉 requestId 这一颗必须被网关的 `assertExactArguments` 拒掉");
    assert!(
        omitted.contains("bad_args") && omitted.contains("requestId"),
        "省键那一臂该落 `missing \"requestId\"`，实际：{omitted}"
    );
}

/// **判据三（省略式投影）**：inventory 的回帧过**分叉的** `parse_cordis_rows`（主干
/// `ParseCordisRow:233-295` 的直译），不是手写 `TryGetProperty`。逐档行数/状态现测：
/// 档 1 与档 0 同号 = **零行**（母本 §6 那里写的「1」是推证，实跑不符 ⇒ 以实跑为准，见 §6 纠正）。
#[test]
fn st13_cordis_inventory_projects_through_the_fork_parser_at_every_level() {
    // 档 → 该见的行：(status, approvalRequestId, activeRunId, latestError, hasClientHalf)
    let table: [(u64, &[(&str, Option<&str>, Option<&str>, Option<&str>, bool)]); 5] = [
        (1, &[]),
        (
            2,
            &[(
                "awaiting-approval",
                Some("req-1"),
                None,
                None,
                true,
            )],
        ),
        (3, &[("running", None, Some("run-1"), None, true)]),
        (
            4,
            &[(
                "failed",
                None,
                None,
                Some("host half threw at load"),
                false,
            )],
        ),
        (5, &[("running", None, Some("run-1"), None, true)]),
    ];
    let call = cordis_inventory();
    for (level, want_rows) in table {
        let value = st13_in_cordis_level(level, |kernel| kernel.call(call.method, call.args.clone()))
            .unwrap_or_else(|e| panic!("档 {level} inventory 失败：{e}"));
        let rows = parse_cordis_rows(&value);
        assert!(value.is_array(), "档 {level} 的 inventory 回帧不是数组：{value}");
        assert_eq!(
            rows.len(),
            value.as_array().map_or(0, Vec::len),
            "档 {level}：投影丢/补了行"
        );
        assert_eq!(rows.len(), want_rows.len(), "档 {level}：现测行数 {rows:?}");
        // 排序不变式（主干 `:212-218`）：待审段在前，段内 pluginId 码位序。
        let flags: Vec<bool> = rows.iter().map(|r| !r.is_awaiting()).collect();
        assert!(
            flags.windows(2).all(|w| w[0] >= w[1]),
            "档 {level} 排序不满足待审优先"
        );
        for (row, (status, request_id, active_run, latest_error, client_half)) in
            rows.iter().zip(want_rows)
        {
            assert_eq!(row.status, *status, "档 {level} 状态归并：{row:?}");
            assert_eq!(
                row.approval_request_id.as_deref(),
                *request_id,
                "档 {level} approvalRequestId：{row:?}"
            );
            assert_eq!(
                row.active_run_id.as_deref(),
                *active_run,
                "档 {level} activeRun（省略式：不适用时整颗键都不发）：{row:?}"
            );
            assert_eq!(
                row.latest_error.as_deref(),
                *latest_error,
                "档 {level} latestRun.error：{row:?}"
            );
            assert_eq!(row.has_client_half, *client_half, "档 {level} hasClientHalf：{row:?}");
            // 主干那两道「读得出才成立」的不变式，逐行扫（不靠上面那张表兜）。
            if row.active_run_id.is_none() {
                assert_ne!(row.status, "running", "省了 activeRun 却折成 running：{row:?}");
            }
            assert!(
                !(row.is_awaiting() && row.approval_request_id.is_none()),
                "待审行没有 approvalRequestId ⇒ 审批卡永远收不掉：{row:?}"
            );
        }
    }
    // 档 0（关档）也在同一把锁里：八发全 404 ⇒ 投影拿不到数组 ⇒ **零行**，
    // 主干 `:666-669` 那句「整族收起」的判据正是零行（不是「调用失败」）。
    let off = st13_in_cordis_level(0, |kernel| kernel.call(call.method, call.args));
    let err = off.expect_err("档 0 的 inventory 不该有臂");
    assert!(
        err.contains("HTTP 404") && err.contains("not_found"),
        "档 0 该落 `other` 臂那枚 404（ST11 §7-① 纠正过的写法）：{err}"
    );
    assert_eq!(parse_cordis_rows(&Value::Null).len(), 0, "非数组回帧 ⇒ 零行，不补占位");
}

/// **判据四（反向锁）**：主干零调用方的那四面（`Cordis.cs:386/408/415/422`）**任何一档都不许有臂**
/// —— 桩故意不给它们注册路由，让它们落 `other` 那臂（`fake_dsh.rs:3418-3424` 的注释即此规格）。
/// 与桩内 `the_four_methods_without_a_mainline_caller_have_no_route_at_any_level` 成对。
#[test]
fn st13_the_four_callerless_faces_still_have_no_route_at_any_level() {
    let inventory = cordis_inventory();
    for level in 0..=5 {
        let outcomes = st13_in_cordis_level(level, |kernel| {
            let mut out: Vec<(String, Result<Value, String>)> = Vec::new();
            for tail in [
                "invoke",
                "resolveInspectQuery",
                "reportClientGuardFailure",
                "reportRenderFailure",
                "totallyNotAnEndpoint",
            ] {
                let endpoint = format!("dynamicCordisRunner/{tail}");
                out.push((endpoint.clone(), kernel.call(&endpoint, json!({}))));
            }
            // 档 0 的那八发也必须**逐字节**回成 `other` 臂的样子（旋钮关掉 ⇒ 路由表不变）。
            if level == 0 {
                out.push((
                    inventory.method.to_string(),
                    kernel.call(inventory.method, inventory.args.clone()),
                ));
            }
            out
        });
        for (endpoint, outcome) in outcomes {
            let err = outcome.expect_err("{endpoint} 在档 {level} 拿到了臂");
            assert!(
                err.contains("HTTP 404") && err.contains("not_found") && !err.contains("bad_args"),
                "档 {level} 的 {endpoint} 该落 404 not_found，实际：{err}"
            );
        }
    }
}

/// **判据五（平铺 + 二次剥 ok）**（CR2 §1 共同口径 1/2 + §6 第 12 项的 RPC 面）：
/// ① 本族**全平铺**，一发都不裹 `request:{}` ⇒ 裹了就 `bad_args`；
/// ② `syncInspectManifest` 的 `providers` 必填、回帧是 `z.literal(null)` ⇒ 线上就是字面 null；
/// ③ `Kernel::call` 已经剥掉网关那一层 `result.ok`（`kernel.rs:3476-3479`），
///    这里现测「value 自带 ok」的有 **四发**（runHostHalf / settleUserRun / stopFromPanel /
///    undefineFromPanel）⇒ CR2 §1 那句「唯一需二次剥 ok 的是 runHostHalf」是**读者侧**口径
///    （主干只有 `Cordis.cs:586-587/760-761/813-814` 真去读那颗 ok），不是线上形状。
#[test]
fn st13_cordis_args_are_flat_and_the_second_ok_peel_is_read_only_on_the_host_half() {
    let wrapped = st13_in_cordis_level(3, |kernel| {
        let flat_inventory = kernel.call(CORDIS_METHODS[0], json!({}));
        let wrapped_inventory = kernel.call(CORDIS_METHODS[0], json!({"request": {}}));
        let host = cordis_run_host_half(
            "s-1001",
            "demo-cordis",
            "pkg-1",
            "run",
            Some("req-1"),
            false,
        );
        let wrapped_host = kernel.call(host.method, json!({"request": host.args.clone()}));
        let manifest = cordis_sync_inspect_manifest();
        let with_providers = kernel.call(manifest.method, manifest.args.clone());
        let without_providers = kernel.call(manifest.method, json!({}));
        (
            flat_inventory,
            wrapped_inventory,
            wrapped_host,
            with_providers,
            without_providers,
        )
    });
    let (flat_inventory, wrapped_inventory, wrapped_host, manifest, missing_providers) = wrapped;
    assert!(
        flat_inventory.expect("平铺的空 args 该过键集闸").is_array(),
        "inventory 顶层该是数组"
    );
    let rejections: Vec<(&str, String)> = [
        ("inventory 裹 request:{}", wrapped_inventory),
        ("runHostHalf 裹 request:{}", wrapped_host),
        ("syncInspectManifest 省 providers", missing_providers),
    ]
    .into_iter()
    .map(|(case, outcome)| (case, outcome.expect_err("这一发本该被键集闸拒掉")))
    .collect();
    for (case, err) in &rejections {
        assert!(err.contains("bad_args"), "{case} 该落 bad_args，实际：{err}");
    }
    assert!(
        rejections[0]
            .1
            .contains("unexpected \"request\""),
        "多出来的那颗键要能在错误里读出来：{rejections:?}"
    );
    assert_eq!(
        manifest.expect("`providers:[]` 那发该成功"),
        Value::Null,
        "`syncInspectManifest` 的 result 是 `z.literal(null)`（`typert.remote-client.js:191`）"
    );

    // 二次剥 ok 的**现测台账**：档 3 八发全有回执，逐数 value 顶层带不带 `ok`。
    let census = st13_cordis_level_outcomes(3);
    let with_domain_ok: Vec<&str> = census
        .iter()
        .filter(|(tail, outcome)| {
            let value = outcome
                .as_ref()
                .unwrap_or_else(|e| panic!("档 3 的 {tail} 该有回执：{e}"));
            value.get("ok").is_some()
        })
        .map(|(tail, _)| *tail)
        .collect();
    assert_eq!(
        with_domain_ok,
        ["runHostHalf", "settleUserRun", "stopFromPanel", "undefineFromPanel"],
        "线上带第二颗 ok 的那几发变了（读者侧只有 runHostHalf 被主干读：CR2 §1 表第 2 行）"
    );
}

// ==================== 台账 #135 / #149 · MR5 §7 欠的那发：M1 之后 `steps` 不翻倍 ============
//
// 母本 = `tmp/mr5-report.md` §7 第一条（「`tests/ipc.rs` 缺 `--step=1` 的折叠用例：M1 之后六型都喂，
// 步臂只喂轨迹 ⇒ 真 socket 侧的『steps 不翻倍』端到端断言」）。既有 KW1 那三发（`:9063-9347`）
// 钉的是**帧型/两本账/两条路同号**，`kw1_fold_run:9121` 把两张台账都按「逐帧一次」喂 ⇒ 它测不到
// 「步臂里多留一发 `run_stats.note`」这一型回归（那一发正是 MR4 留给 MR5 的唯一静默 bug）。
// 本发补三件：① 按 **M1 之后**的宿主喂法折（blanket 一次 + 步臂只喂轨迹）；
// ② 把 **M1 之前**的双喂形状当**反证**跑一遍，证这条判据真的能红（否则 ①「steps == 3」可以靠
// 两本账一起被喂歪假绿）；③ 钉「家 B 仍然收到两型步帧」（搬家不许把轨迹那一臂喂空）。
// 宿主侧的源码级位置闸在 `src/main.rs` 的 `kw1_step_arm_lock_tests`，本发是它的运行时对位。

/// 一帧是不是 `step/*`（桩的两型 = `st3_step_boundaries`，`fake_dsh.rs:5101-5135`）。
fn st13_is_step_frame(event: &Value) -> bool {
    event["type"]
        .as_str()
        .is_some_and(|kind| kind.starts_with("step/"))
}

/// **M1 之后的宿主喂法**（`src/main.rs:9663` 的 blanket 位 + `:9969/9977` 两臂）：
/// 回（家 A 折叠、家 B 三轮步数之和、进过家 B 的步帧数）。
/// 家 B 的读数是**按轮**存的（`TrajectoryFold::steps(turn)`），这里把 1..=3 三轮加总，
/// 与家 A 那一个全局 `steps` 放在同一格比。
fn st13_fold_after_m1(session: &str, events: &[Value]) -> (RunStepFold, i64, usize) {
    let mut run = RunStatsLedger::new();
    let mut trail = TrajectoryLedger::new();
    let mut step_frames = 0usize;
    for event in events {
        run.note(session, event);
        if st13_is_step_frame(event) {
            trail.note(session, event);
            step_frames += 1;
        }
    }
    let trail_total: i64 = (1..=3)
        .map(|turn| {
            trail
                .fold(session)
                .map_or(0, |fold| fold.steps(turn))
        })
        .sum();
    (
        run.fold(session).cloned().unwrap_or_default(),
        trail_total,
        step_frames,
    )
}

/// **M1 之前的形状**（反证用）：blanket 位照喂，步臂里**再**喂一次家 A ⇒ `step/end` 折两次。
fn st13_fold_before_m1(session: &str, events: &[Value]) -> RunStepFold {
    let mut run = RunStatsLedger::new();
    for event in events {
        run.note(session, event);
        if st13_is_step_frame(event) {
            run.note(session, event);
        }
    }
    run.fold(session).cloned().unwrap_or_default()
}

#[test]
fn st13_step_frames_reach_the_run_stats_ledger_exactly_once_after_the_feed_move() {
    // (档, 一轮的 live 帧预算, 单轮 step/end 数, 单轮 step 帧总数) —— 38/40 复用了 KW1 §判据一
    // 的现测值并在本轮复跑复核（本函数第一颗断言就是它）。
    for (level, want_frames, closes, step_frames) in [(1u64, 38usize, 1usize, 2usize), (2, 40, 2, 4)] {
        let session = format!("s-972{level}");
        let (frames, lived, replayed) = kw1_step_live(&session, level, want_frames);
        assert_eq!(frames.len(), want_frames, "档 {level}：现测帧预算");
        for (path, events) in [("live 推流", &lived), ("事后回读", &replayed)] {
            let ends = kw1_count_kind(&kw1_step_events(events), "step/end");
            assert_eq!(ends, closes, "档 {level} {path}：桩发的 `step/end` 数");
            let (faithful, trail_steps, fed_steps) = st13_fold_after_m1(&session, events);
            assert_eq!(
                fed_steps, step_frames,
                "档 {level} {path}：两型步帧都该进家 B（搬家不许把轨迹那一臂喂空）"
            );
            assert_eq!(
                trail_steps as usize, closes,
                "档 {level} {path}：家 B 的步数 = `step/end` 数（主干 `Trajectory.cs:334-337` 无条件 +1）"
            );
            assert_eq!(
                faithful.steps as usize, closes,
                "档 {level} {path}：家 A 的 `steps` **不翻倍** ⇒ 恰等于 `step/end` 数，实际 {faithful:?}"
            );
            assert_eq!(faithful.turns, 1, "档 {level} {path}：单轮");
            let naive = st13_fold_before_m1(&session, events);
            assert_eq!(
                naive.steps as usize,
                closes * 2,
                "反证失效：M1 前那把双喂形状没翻两倍 ⇒ 上面那条『不翻倍』是假绿，实际 {naive:?}"
            );
            assert_ne!(
                faithful.steps, naive.steps,
                "档 {level} {path}：两种喂法折出同一个数 ⇒ 本发测不出被钉的那件事"
            );
        }
    }

    // 种子会话那一趟（三轮，`--step=1`）：把「不翻倍」放到 n>1 上再钉一次，并与同一颗桩在
    // `session/list` 上报的投影读数**同号**（真翻倍会立刻露出 6 vs 3）。
    let (events, stats) = kw1_step_seed(1);
    let ends = kw1_count_kind(&kw1_step_events(&events), "step/end");
    assert_eq!(ends, 3, "档 1 的三份种子 journal：每轮恰一条 `step/end`");
    let (faithful, trail_steps, fed_steps) = st13_fold_after_m1("s-1001", &events);
    assert_eq!((fed_steps, trail_steps), (6, 3), "两型各三条 ⇒ 家 B 收到六帧、步数三条");
    assert_eq!(
        (faithful.turns, faithful.steps),
        (3, 3),
        "三轮该折出 3 轮 3 步，实际 {faithful:?}"
    );
    assert_eq!(
        st13_fold_before_m1("s-1001", &events).steps,
        6,
        "反证：同一批帧按 M1 前的形状喂就该是 6"
    );
    let stats = stats.expect("种子行该带 sessionStats");
    assert_eq!(
        (stats.turns, stats.steps),
        (faithful.turns, faithful.steps),
        "档 1：宿主折叠与内核投影读数不同号 ⇒ 两份真相（投影侧的档 0 偏差由 KW1 判据二钉，不在此调和）"
    );
}
// ==================== 台账 #144 · 刀5：cordis 事件族 + 审批 RPC 的**真 socket** 用例（IP6 落地稿）====
//
// 母本 = `tmp/ip6-cordis-socket-cases.md`（只读取证报告 IP6）。母本里的**行号一律不抄**：
// `src/kernel.rs` 与 `src/i18n.rs` 此刻正被并发改（`src/main.rs` 刚交稿），行号会漂 ⇒
// 下面只按**符号名**引用（`cordis_pending_from_event` / `st6_cordis_outcome` / `handle_open` …）。
//
// 三条本仓已确证的坑，已落到代码里：
// ① `collect_within` 按 `streamId` 过滤、**同批别的流的帧被 `filter` 直接丢弃** ⇒
//    本族**一律单流串行**：每枚用例只开 `$events` 这一条，收干净之后才打 RPC（RPC 走 HTTP POST
//    `/api/{method}`，压根不占 mux 帧）。绝不写「先开两条再分别收」。
// ② 不订流、不发 prompt 的一次性 RPC **帧预算 = 0**（`DEFAULT_PACE_MS` / `READ_TICK` 那把尺只在
//    prompt/follow 族咬人）⇒ T9/T9b/T10 全用既有 `st13_in_cordis_level`，**不写 sleep、不写预算**；
//    `$events` 那几枚的 `want` 写**实测值**（2/3/3/3/4/6），`WAIT_BUDGET` 足够。
// ③ 只走 **argv** 传旋钮（`--cordis=` / `--cordis-events=` / `--waterfall=`；`knob()` 里 argv 压过
//    env），**绝不 `set_var`**：`FAKE_DSH_CORDIS_EVENTS` 会跨用例泄漏到同进程所有桩，母本 ④ R3
//    实测正是用它把 `mux_events_stream_sends_ready_then_emit` 打红的。
//
// 判据口径：主干 handler 拿到的是**载荷本身**（`MainWindow.Cordis.cs` 从 ev 根上读字段），
// 分叉侧走 `EventsFrame::Emit{event,args}` 的 **`args.first()`**（桩的 `cordis_emit_frame` 出帧）
// ⇒ 本族所有断言先剥 `args[0]` 再喂 `cordis_pending_from_event`；拿 emit 帧**根**去读必须全空
// （T3 把这条当反证钉住）。

mod ip6_cordis_socket_tests {
    use super::*;
    use blade2_rs::kernel::{
        CORDIS_METHODS, CordisPendingApproval, CordisRunResolution, EventsFrame,
        cordis_host_half_message, cordis_host_half_ok, cordis_host_half_plugin_run_id,
        cordis_inventory, cordis_pending_from_event, cordis_resolve_request_run,
        cordis_run_host_half, cordis_settle_user_run, cordis_still_pending,
        cordis_stop_from_panel, cordis_undefine_from_panel, events_subscription,
        parse_cordis_rows, parse_events_frame,
    };

    /// 假世界身份（桩的 `CORDIS_AGENT / CORDIS_PLUGIN / CORDIS_PACKAGE / CORDIS_RUN / CORDIS_REQUEST`
    /// 逐字）：s-1001 / demo-cordis / pkg-1 / run-1 / req-1。既有 `st13_cordis_build` 用的同一套。
    const AGENT: &str = "s-1001";
    const PLUGIN: &str = "demo-cordis";
    const PACKAGE: &str = "pkg-1";
    const RUN: &str = "run-1";
    const REQUEST: &str = "req-1";

    /// 本族四发事件名，**顺序 = 桩 `cordis_event_frames(5)` 的自然序**。
    /// 登记表 `EVENTS_SUBSCRIPTIONS` 是十颗混表（含 `api-session/*` 与那两发 waterfall）⇒
    /// 剥不出「本族那四颗」，所以这里给字面串，但**每处都同时**用 `events_subscription()`
    /// 反查登记表，两处对不上就红。
    const FOUR: [&str; 4] = [
        "cordis/request-run",
        "cordis/request-run-resolved",
        "cordis/dynamic-package",
        "cordis/dynamic-retract",
    ];

    /// 一档一颗桩，在 `$events` 上收 `want` 帧 + 用 `QUIET_BUDGET` 探一次「该没有更多帧」。
    /// 出口统一 `drop(mux)` → `shutdown()`（防孤儿进程）。
    /// ⚠ 只开这一条流（坑 ①）；`want` 必须写**实测值**（坑 ②）。
    fn events_at(events_level: u64, want: usize) -> Vec<Value> {
        let knob = format!("--cordis-events={events_level}");
        events_at_knob(&knob, want)
    }

    /// 同上，但旋钮原样给（用来演「脏档号并到关档」那一格：`--cordis-events=nope`）。
    fn events_at_knob(knob: &str, want: usize) -> Vec<Value> {
        let (mut kernel, mut mux, stream) =
            open_stream_by(&launch_with(&["--pace=0", knob]), "$events", json!({}));
        let frames = item_values(&collect(&mut mux, &stream, want), &stream);
        // `collect_within` 一到 `want` 就返回 ⇒ 「不许多推」只能靠短静默窗口反证。
        let extra = collect_within(&mut mux, &stream, 1, QUIET_BUDGET);
        assert!(
            extra.is_empty(),
            "`--cordis-events` 的定长前导帧推完了还多出 {} 帧：{extra:?}",
            extra.len()
        );
        drop(mux);
        kernel.shutdown();
        frames
    }

    /// 一帧的 `event` 名（ready 帧没有这颗键 ⇒ 空串，正好当帧序表的第一格）。
    fn event_name(frame: &Value) -> &str {
        frame["event"].as_str().unwrap_or_default()
    }

    /// emit 帧的载荷 = `args.first()`（**分叉侧唯一读法**）；不是 emit、或 args 是空数组 ⇒ `None`。
    fn payload_of(frame: &Value) -> Option<Value> {
        match parse_events_frame(frame)? {
            EventsFrame::Emit { args, .. } => args.first().cloned(),
            _ => None,
        }
    }

    /// `cordis/request-run` 的八键期望（桩 `cordis_request_run_payload(Some(true))` 的逐字）。
    /// `serde_json` 没开 `preserve_order` ⇒ `Map` 是 `BTreeMap`，整颗对象比**与键序无关**。
    fn want_request_run() -> Value {
        json!({
            "requestId": REQUEST,
            "agentId": AGENT,
            "pluginId": PLUGIN,
            "packageId": PACKAGE,
            "mode": "run",
            "name": "Demo Cordis",
            "purpose": "ST6 取证用的假动态插件",
            "requiresApproval": true,
        })
    }

    /// 剥掉 `requiresApproval` 之后的载荷：用来证「档 1/2/3 只差这一颗键」。
    fn strip_approval_key(value: &Value) -> Value {
        let mut map = value.as_object().expect("载荷是对象").clone();
        map.remove("requiresApproval");
        Value::Object(map)
    }

    /// T1 · 那一族帧 × 六档旋钮：`--cordis-events=` 每档的 `$events` **前导帧数 + 帧序 + 帧型**。
    /// 帧数台账是**实测**（母本 §1 表）：0→2、1→3、2→3、3→3、4→4、5→6（**不可外推**：
    /// 1/2/3 三档同为 3）。顺带钉「ready 永远第一帧」「status 腿是**两**参数」「本族四发是**单**参数」
    /// 「`cordis/inspect-query{,-resolved}` 一帧都不许出现」。
    #[test]
    fn ip6_the_events_knob_pushes_exactly_the_measured_frame_count_at_every_level() {
        let table: [(u64, &[&str]); 6] = [
            (0, &["", "api-session/status"]),
            (1, &["", "api-session/status", FOUR[0]]),
            (2, &["", "api-session/status", FOUR[0]]),
            (3, &["", "api-session/status", FOUR[0]]),
            (4, &["", "api-session/status", FOUR[0], FOUR[1]]),
            (
                5,
                &["", "api-session/status", FOUR[0], FOUR[1], FOUR[2], FOUR[3]],
            ),
        ];
        for (level, want_names) in table {
            let frames = events_at(level, want_names.len());
            assert_eq!(
                frames.len(),
                want_names.len(),
                "档 {level} 的 `$events` 前导帧数变了"
            );
            let names: Vec<&str> = frames.iter().map(event_name).collect();
            assert_eq!(names.as_slice(), want_names, "档 {level} 的帧序/帧名：{frames:?}");
            assert_eq!(frames[0]["type"], json!("ready"), "ready 永远第一帧");

            // 第二帧是接手态那发 status：args 是**两颗**（`["s-1", true]`），
            // 与既有 `mux_events_stream_sends_ready_then_emit` 同口径。⚠ 母本 ② 的 T1 把
            // 「args 恰一颗」套到了这一帧上 ⇒ 那样必假红；这里收窄到 index>=2 的本族帧。
            assert_eq!(
                frames[1]["args"],
                json!(["s-1", true]),
                "status 腿的两参数不许被本族旋钮改动：档 {level}"
            );

            for (index, frame) in frames.iter().enumerate().skip(2) {
                assert_eq!(frame["type"], json!("emit"), "档 {level} 第 {index} 帧不是 emit");
                assert_eq!(
                    frame["args"].as_array().expect("emit 必带 args").len(),
                    1,
                    "单参事件 ⇒ 载荷只在 args[0]（档 {level} 第 {index} 帧）"
                );
                assert!(
                    events_subscription(frame).is_some(),
                    "第 {index} 帧的名字该被登记表认出，实际 {frame}"
                );
            }
            assert!(
                !names.iter().any(|name| name.starts_with("cordis/inspect-query")),
                "主干订阅数 = 0 的那两发不许推（档 {level}）：{names:?}"
            );
        }
    }

    /// T2 · 那一族帧 × 关档（默认档 = 接手态）：一帧 cordis 都不许多推，
    /// 且「越界档号 / 脏串」在 `cordis_events_level` 里已**夹回 0**（不是夹顶）。
    /// 这是既有 `mux_events_stream_sends_ready_then_emit`（只数帧）的**镜像**：这里数**名字**。
    #[test]
    fn ip6_default_level_still_has_no_cordis_frame_at_all() {
        let frames = events_at(0, 2);
        assert_eq!(frames.len(), 2, "默认档必须逐字 = 接手态两帧：{frames:?}");
        assert!(
            frames.iter().all(|f| !event_name(f).starts_with("cordis/")),
            "`--cordis-events` 关档时 `$events` 上不许出现本族帧：{frames:?}"
        );
        for knob in ["--cordis-events=6", "--cordis-events=99", "--cordis-events=nope"] {
            assert_eq!(
                events_at_knob(knob, 2).len(),
                2,
                "{knob} 该并到关档（`cordis_events_level` 的 >MAX / parse 失败都回 0）"
            );
        }
    }

    /// T3 · 那一帧 `cordis/request-run`（档 1 正形）：`args.first()` ⇒ 折出**恰好一张**待审卡，
    /// 键集逐字八颗。**反证**：拿 emit 帧**根**去读（主干 handler 的读法）在分叉侧必须 `None`
    /// ⇒ 证明桩出的是分叉形状、母本 §2 那条口径不是空话。
    #[test]
    fn ip6_request_run_frame_folds_into_the_fork_card_only_via_args_first() {
        let frames = events_at(1, 3);
        let emit = &frames[2];
        assert_eq!(event_name(emit), FOUR[0]);
        let payload = payload_of(emit).expect("args[0] 就是载荷（分叉侧唯一读法）");
        assert_eq!(payload, want_request_run(), "载荷逐键：{payload}");
        assert_eq!(
            sorted_keys(&payload),
            [
                "agentId",
                "mode",
                "name",
                "packageId",
                "pluginId",
                "purpose",
                "requestId",
                "requiresApproval"
            ],
            "八颗键不多不少（桩 `cordis_request_run_payload` 的键集）"
        );
        assert_eq!(
            cordis_pending_from_event(&payload),
            Some(CordisPendingApproval {
                request_id: REQUEST.to_string(),
                agent_id: AGENT.to_string(),
                plugin_id: PLUGIN.to_string(),
                package_id: PACKAGE.to_string(),
                mode: "run".to_string(),
                name: "Demo Cordis".to_string(),
                purpose: "ST6 取证用的假动态插件".to_string(),
            }),
            "分叉 `cordis_pending_from_event` 的折叠结果"
        );
        assert!(
            cordis_pending_from_event(emit).is_none(),
            "拿 emit 帧**根**读字段必须折不出卡 —— 否则本族的 args[0] 口径是假判据：{emit}"
        );
        assert_eq!(
            events_subscription(emit),
            Some(FOUR[0]),
            "登记表 `EVENTS_SUBSCRIPTIONS` 认这一发（emit 侧）"
        );
    }

    /// T4 · 那**两枚**负形帧：档 2 给字面 `false`、档 3 **整颗省键**（主干 `TryGetProperty` 上是
    /// 两条不同的路 ⇒ 合成一档就抹平了）。两档都**不许**建卡，但其余七键必须与档 1 逐字相同。
    #[test]
    fn ip6_the_two_negative_levels_differ_only_by_the_approval_key_over_the_socket() {
        let yes = payload_of(&events_at(1, 3)[2]).expect("档 1 载荷");
        let no = payload_of(&events_at(2, 3)[2]).expect("档 2 载荷");
        let missing = payload_of(&events_at(3, 3)[2]).expect("档 3 载荷");

        assert_eq!(no["requiresApproval"], json!(false), "档 2 给字面 false");
        assert!(
            missing.get("requiresApproval").is_none(),
            "档 3 必须**省键**、不给 null：{missing}"
        );
        assert_eq!(
            strip_approval_key(&yes),
            strip_approval_key(&no),
            "只差一颗键：档 1 vs 档 2"
        );
        assert_eq!(strip_approval_key(&yes), missing, "只差一颗键：档 1 vs 档 3");
        assert_eq!(sorted_keys(&no).len(), 8, "档 2 仍八颗键");
        assert_eq!(sorted_keys(&missing).len(), 7, "档 3 七颗键");
        for dirty in [&no, &missing] {
            assert_eq!(
                cordis_pending_from_event(dirty),
                None,
                "`requiresApproval` 只认字面 true ⇒ 这两档都不建卡：{dirty}"
            );
        }
        // 反证补一格：档 1 那张卡确实建得起来（否则上面那两条 `None` 是因为压根没读通）。
        assert!(cordis_pending_from_event(&yes).is_some());
    }

    /// T5 · 那一帧 `cordis/request-run-resolved`（档 4 的收卡腿）：与档 4 的开卡腿**同一颗
    /// `requestId`**，载荷**只有这一颗**；桩刻意不编内核的 `outcome`（主干整发只读 requestId）
    /// ⇒ 正向断言 = 只有 requestId，反向断言 = `outcome` 不许出现。
    #[test]
    fn ip6_resolved_frame_pairs_the_same_request_id_and_carries_only_it() {
        let frames = events_at(4, 4);
        let opened = payload_of(&frames[2]).expect("request-run 载荷");
        let closed = payload_of(&frames[3]).expect("request-run-resolved 载荷");
        assert_eq!(event_name(&frames[2]), FOUR[0]);
        assert_eq!(event_name(&frames[3]), FOUR[1]);
        assert_eq!(closed, json!({"requestId": REQUEST}), "整发只一颗键：{closed}");
        assert_eq!(closed["requestId"], opened["requestId"], "两发必须同 id 才配得成对");
        assert!(
            closed.get("outcome").is_none(),
            "`outcome` 主干一次都不读 ⇒ 桩不许编、用例也不许假装有：{closed}"
        );
        assert_eq!(events_subscription(&frames[3]), Some(FOUR[1]));
    }

    /// T6 · 那两帧 `cordis/dynamic-package` / `cordis/dynamic-retract`（档 5）：**匿名重拉信号**——
    /// 主干形参就叫 `_payload`（一发都不看载荷），但桩仍给 `args[0]` 一个**空对象占位**
    /// （省格会让分叉的 `args.first()` 变 `None`，而内核那两支发的是有载荷的 emit）。
    #[test]
    fn ip6_the_dynamic_pair_are_anonymous_repull_signals_in_the_locked_order() {
        let frames = events_at(5, 6);
        let names: Vec<&str> = frames.iter().map(event_name).collect();
        assert_eq!(
            names,
            ["", "api-session/status", FOUR[0], FOUR[1], FOUR[2], FOUR[3]],
            "四发的自然序（桩 `cordis_event_frames(5)`）"
        );
        for index in [4, 5] {
            assert_eq!(
                frames[index]["args"][0],
                json!({}),
                "第 {index} 帧载荷该是**空对象占位**，不是省格"
            );
            assert_eq!(
                payload_of(&frames[index]),
                Some(json!({})),
                "分叉侧 `args.first()` 读得到那格空对象"
            );
            assert_eq!(
                events_subscription(&frames[index]),
                Some(FOUR[index - 2]),
                "两发都在登记表上（否则事件泵的分流臂认不出）"
            );
            // 不编键的反证：载荷对象**零颗**键。
            assert_eq!(
                frames[index]["args"][0].as_object().expect("载荷是对象").len(),
                0,
                "第 {index} 帧载荷不许多编任何键（主干形参是 `_payload`）"
            );
        }
    }

    /// T7 · 那两枚生产者旋钮**同档在架**：`--cordis-events=5 --waterfall=1` 的实测帧序
    /// `ready → status → cordis×4 → approval/request → user-questions/request`（**8 帧**）。
    /// 桩 `handle_open` 那句「cordis 腿插在 waterfall 腿之前」在这里过真 socket 复核。
    /// ⚠ 待主代理核：`tests/ipc.rs` 里**从来没有**用例走过 `--waterfall=` argv
    ///   （`grep -n waterfall tests/ipc.rs` 零命中）⇒ 档名取自桩的 `WATERFALL_ARG` 常量 +
    ///   母本 ① §1 的实测（env 同档）。若这一枚红在「帧数不是 8」，先 `--nocapture` 打一遍再改表，
    ///   **不要**反过来放宽顺序断言。
    #[test]
    fn ip6_waterfall_and_cordis_frames_share_the_preamble_in_the_locked_order() {
        let (mut kernel, mut mux, stream) = open_stream_by(
            &launch_with(&["--pace=0", "--cordis-events=5", "--waterfall=1"]),
            "$events",
            json!({}),
        );
        let frames = item_values(&collect(&mut mux, &stream, 8), &stream);
        assert!(
            collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
            "两枚旋钮都不许追加第三批帧"
        );
        drop(mux);
        kernel.shutdown();

        assert_eq!(frames.len(), 8, "实测 2 + 4 + 2 = 8 帧：{frames:?}");
        let names: Vec<&str> = frames.iter().map(event_name).collect();
        assert_eq!(
            names,
            [
                "",
                "api-session/status",
                FOUR[0],
                FOUR[1],
                FOUR[2],
                FOUR[3],
                "approval/request",
                "user-questions/request"
            ],
            "cordis 腿不许顶掉 waterfall 腿，也不许插进它们中间"
        );
        // 两族各走各的帧型：cordis 四发是 emit、waterfall 两发是 waterfall（#74 的形状）。
        for index in 2..6 {
            assert!(
                matches!(parse_events_frame(&frames[index]), Some(EventsFrame::Emit { .. })),
                "本族第 {index} 帧该是 emit：{:?}",
                frames[index]
            );
        }
        for index in [6, 7] {
            assert!(
                matches!(
                    parse_events_frame(&frames[index]),
                    Some(EventsFrame::Waterfall { .. })
                ),
                "waterfall 那一支仍走 #74 的形状（两族不互相掩护）：{:?}",
                frames[index]
            );
        }
    }

    /// T8 · **刀5 主用例**：一发事件 + 三发 RPC 的**同进程贯通**（`--cordis=2 --cordis-events=4`）。
    /// `$events` 收到 `request-run` ⇒ 折出卡 ⇒ 拿**卡里的** id/agent/plugin/package/mode 去
    /// `runHostHalf` 授权 ⇒ `resolveRequestRun` 回 `{accepted:true}`（**单层：value 里没有 `ok`**）。
    /// 中间夹一发 `inventory` 现测「这一发仍待审」。
    /// ⚠ 单流串行：先把 `$events` 的四帧整批收干净（含静默反证）才打 RPC（坑 ①）。
    /// ⚠ 桩无状态 ⇒ 「批准完 inventory 该变 running」必假；这里只把「照旧待审」记成桩的备案。
    #[test]
    fn ip6_the_approval_card_from_the_wire_resolves_against_level_two_receipts() {
        let (mut kernel, mut mux, stream) = open_stream_by(
            &launch_with(&["--pace=0", "--cordis=2", "--cordis-events=4"]),
            "$events",
            json!({}),
        );
        let frames = item_values(&collect(&mut mux, &stream, 4), &stream);
        assert!(
            collect_within(&mut mux, &stream, 1, QUIET_BUDGET).is_empty(),
            "RPC 腿的三发不许往 `$events` 上追加帧（它压根不发 mux 帧）"
        );
        drop(mux);
        assert_eq!(frames.len(), 4, "两枚旋钮同档在架：帧数该 = 单拧 events=4 的 4 帧：{frames:?}");

        // ① 线上那张卡。
        let card = cordis_pending_from_event(&payload_of(&frames[2]).expect("档 4 的正形载荷"))
            .expect("requiresApproval 字面 true ⇒ 必建卡");
        assert_eq!(card.request_id, REQUEST);
        assert_eq!(card.agent_id, AGENT);
        assert_eq!(card.plugin_id, PLUGIN);

        // ② 卡里那颗 requestId 必须能在同进程的 inventory 行上对上（否则卡永远收不掉）。
        let inventory = cordis_inventory();
        let rows = parse_cordis_rows(
            &kernel
                .call(inventory.method, inventory.args)
                .expect("档 2 inventory 该有回执"),
        );
        assert_eq!(rows.len(), 1, "档 2 一行 awaiting-approval：{rows:?}");
        assert!(
            cordis_still_pending(&rows, &card.request_id),
            "事件里的 requestId 与 inventory 行的 approvalRequestId 必须同号：{rows:?}"
        );

        // ③ 拿卡的六格喂审批路（带**已登记**的 req-1）。
        let approval = cordis_run_host_half(
            card.agent_id.as_str(),
            card.plugin_id.as_str(),
            card.package_id.as_str(),
            card.mode.as_str(),
            Some(card.request_id.as_str()),
            false,
        );
        let started = kernel
            .call(approval.method, approval.args)
            .expect("档 2 带已登记 req-1 该授权成功");
        assert!(
            cordis_host_half_ok(&started),
            "成功判据 = 回帧是对象且 `ok` 是字面 true：{started}"
        );
        assert_eq!(cordis_host_half_plugin_run_id(&started), RUN);
        assert_eq!(cordis_host_half_message(&started), "", "成功格不带 message（主干读出空串）");
        assert_eq!(started["startedHere"], json!(true), "档 2 授权那发是真起一发");
        assert_eq!(
            sorted_keys(&started),
            ["ok", "packageId", "pluginId", "pluginRunId", "startedHere", "waitingFor"],
            "成功格键集（与既有 `st13_panel_path_null_request_id…` 同号）"
        );

        // ④ 结算：`resolveRequestRun` 的回执是**单层**（value 里根本没有 `ok`）。
        let resolve = cordis_resolve_request_run(
            card.request_id.as_str(),
            &CordisRunResolution::Activated {
                plugin_run_id: cordis_host_half_plugin_run_id(&started),
            },
        );
        let resolved = kernel
            .call(resolve.method, resolve.args)
            .expect("resolveRequestRun 该有回执");
        assert_eq!(resolved, json!({"accepted": true}), "只有档 2 才 accepted：{resolved}");
        assert!(
            resolved.get("ok").is_none(),
            "`resolveRequestRun` **不在**带第二层 `ok` 的那四发里"
        );

        // ⑤ 桩的备案（**不是**回归闸）：批准完再拉 inventory 照旧待审 ⇒ 收卡动作属宿主队列。
        let again = cordis_inventory();
        let rows = parse_cordis_rows(
            &kernel
                .call(again.method, again.args)
                .expect("第二次 inventory 该有回执"),
        );
        assert!(
            cordis_still_pending(&rows, &card.request_id),
            "桩不回写世界（`st6_cordis_outcome` 只读 level + args）⇒ 这一格记的是桩、不是产品"
        );
        kernel.shutdown();
    }

    /// T9 · 那一发 `runHostHalf` × **两条路**（审批 `Some(req-1)` / 面板 `None`）× 档 1..5，
    /// 只用**分叉真有的那三把尺**（`cordis_host_half_ok` / `…_plugin_run_id` / `…_message`）；
    /// 外加 `resolveRequestRun` 的**单层**逐档值与成功格键集。
    /// 主干整个丢弃的那三发（settle/stop/undefine）**不在这里**（见 T9b 骨架）。
    /// 一档一颗桩（5 起，非 40 起），复用既有 `st13_in_cordis_level`（出口统一 shutdown）。
    /// ⚠ 无帧预算：全族走 HTTP POST `/api/{method}`（坑 ②）。
    #[test]
    fn ip6_run_host_half_readers_are_measured_on_both_paths_at_every_level() {
        let started_keys = ["ok", "packageId", "pluginId", "pluginRunId", "startedHere", "waitingFor"];
        for level in 1..=5u64 {
            let (approval_path, panel_path, resolved) = st13_in_cordis_level(level, |kernel| {
                let approval = cordis_run_host_half(AGENT, PLUGIN, PACKAGE, "run", Some(REQUEST), false);
                let approval = kernel.call(approval.method, approval.args);
                let panel = cordis_run_host_half(AGENT, PLUGIN, PACKAGE, "run", None, false);
                let panel = kernel.call(panel.method, panel.args);
                let resolve = cordis_resolve_request_run(
                    REQUEST,
                    &CordisRunResolution::Activated {
                        plugin_run_id: RUN.to_string(),
                    },
                );
                let resolved = kernel.call(resolve.method, resolve.args);
                (approval, panel, resolved)
            });
            let approval = approval_path.expect("runHostHalf 全档都有信封（业务失败在 value 里）");
            let panel = panel_path.expect("面板路同上");
            let resolved = resolved.expect("resolveRequestRun 全档都有信封");

            // 分叉侧唯一读者：`cordis_host_half_ok` **只认字面 true**。
            assert_eq!(
                cordis_host_half_ok(&approval),
                level == 2,
                "档 {level} 审批路（带已登记 req-1）只有档 2 的世界授权得起：{approval}"
            );
            assert_eq!(
                cordis_host_half_ok(&panel),
                matches!(level, 3 | 4),
                "档 {level} 面板路（显式 null requestId）只有 3/4 起得来：{panel}"
            );
                // 档 1/5 这两条路**都**是失败形（故障档连面板路都不起）⇒ 成功格键集只在
                // 2/3/4 上断，其余档断「两条路都落 `{ok:false,message}`」。
                let winner = if cordis_host_half_ok(&approval) { &approval } else { &panel };
                if matches!(level, 2 | 3 | 4) {
                    assert_eq!(
                        sorted_keys(winner),
                        started_keys,
                        "档 {level}：成功格键集不许漂（审批路给 2、面板路给 3/4）"
                    );
                } else {
                    assert_eq!(
                        sorted_keys(winner),
                        ["message", "ok"],
                        "档 {level}：本档两条路都失败，失败形只两颗键：{winner}"
                    );
                }
            assert_eq!(
                cordis_host_half_plugin_run_id(&approval),
                if level == 2 { RUN } else { "" },
                "档 {level}：失败格读不出 pluginRunId ⇒ 分叉给空串（不替主干兜底）"
            );
            // 失败原因上屏用的那一颗（主干确实读它）⇒ 断言只打在**稳定子串**上，不锁整句措辞。
            match level {
                1 => {
                    assert!(
                        cordis_host_half_message(&approval).contains("no dynamic plugin"),
                        "档 1 缺插件：{approval}"
                    );
                    assert_eq!(
                        cordis_host_half_message(&approval),
                        cordis_host_half_message(&panel),
                        "档 1：两条路都落「缺插件」那一句：{panel}"
                    );
                }
                2 => assert!(
                    cordis_host_half_message(&panel).contains("has pending run request"),
                    "档 2 面板路该撞「有 pending run request」：{panel}"
                ),
                3 => assert_eq!(
                    panel["startedHere"],
                    json!(false),
                    "档 3 已有活动 run ⇒ 重挂（attaching），不是真起：{panel}"
                ),
                4 => assert_eq!(
                    panel["startedHere"],
                    json!(true),
                    "档 4 没有活动 run ⇒ 真起一发：{panel}"
                ),
                // 故障档**故意不自洽**（档 5 inventory 回 running 行、这一发回不授权）：
                // 只记「两条路同一串」，不记「一致」（母本 ③ 第 8 条）。
                _ => {
                    assert!(
                        cordis_host_half_message(&approval).contains("does not authorize"),
                        "档 5 故障档：审批路 {approval}"
                    );
                    assert_eq!(
                        cordis_host_half_message(&approval),
                        cordis_host_half_message(&panel),
                        "档 5：桩这一发压根不看 requestId ⇒ 两条路同串（备案，不是产品行为）"
                    );
                }
            }

            // 单层回执逐档：`{accepted: level == 2}`，**没有** `ok`。
            assert_eq!(
                resolved,
                json!({"accepted": level == 2}),
                "档 {level} resolveRequestRun 的回执形状"
            );
            assert_eq!(sorted_keys(&resolved), ["accepted"]);
            assert!(resolved.get("ok").is_none(), "档 {level}：这一发不带第二层 ok");
        }
    }

    /// T9b · **骨架**（母本 ④ R4：`settleUserRun` 档 1..4 的 `message` 逐档值**未逐档现测**）。
    ///
    /// TODO(主代理现测)：跑
    /// ```text
    ///   cargo test --offline --test ipc ip6_settle_user_run -- --nocapture
    /// ```
    /// 读五档打印出来的**真 socket 回执**，把 `reason` / `message` 填成逐档表，然后把下面那三条
    /// `println!` 换成 `assert_eq!` 表。
    ///
    /// ⚠ 不许照源码分支推证先写死：母本 §4 那张表是从 `st6_cordis_outcome` **读**出来的推证
    ///   （`reason` 逐档各异：档 1 `plugin-missing` / 档 2 `client-half-failed` / 档 3 是 `ok:true`
    ///   且没有 reason / 档 4 `host-half-failed` / 档 5 `rejected` + **回显调用方给的** message，
    ///   缺省 `the run request was declined`）。母本 ② 里那句
    ///   `assert_eq!(settled["reason"], json!("rejected"))` 全档断言 **必假红**，故不进成品。
    ///   且那三发的回帧**主干整个丢弃**（`MainWindow.Cordis.cs` 无接收者）⇒ 填表后也只当**台账**，
    ///   别升成产品断言（母本 ③ 第 7 条）。
    /// 这里只保留**现在就成立**的一条：三发在档 1..5 全有 value（不是 RPC 层错）
    /// 且 value 顶层带第二层 `ok`（既有 `st13_…second_ok_peel…` 只在档 3 数过 ⇒ 这是增量）。
    #[test]
    fn ip6_settle_user_run_and_panel_faces_print_their_per_level_receipts() {
        for level in 1..=5u64 {
            let (settle, stop, undefine) = st13_in_cordis_level(level, |kernel| {
                let settle = cordis_settle_user_run(AGENT, PLUGIN, &CordisRunResolution::Rejected);
                let settle = kernel.call(settle.method, settle.args);
                let stop = cordis_stop_from_panel(AGENT, PLUGIN);
                let stop = kernel.call(stop.method, stop.args);
                let undefine = cordis_undefine_from_panel(AGENT, PLUGIN);
                let undefine = kernel.call(undefine.method, undefine.args);
                (settle, stop, undefine)
            });
            println!("档 {level} settleUserRun    = {settle:#?}");
            println!("档 {level} stopFromPanel      = {stop:#?}");
            println!("档 {level} undefineFromPanel  = {undefine:#?}");

            for (face, outcome) in [
                ("settleUserRun", &settle),
                ("stopFromPanel", &stop),
                ("undefineFromPanel", &undefine),
            ] {
                let value = outcome
                    .as_ref()
                    .unwrap_or_else(|e| panic!("档 {level} 的 {face} 该有 value、不是 RPC 错：{e}"));
                assert!(
                    value.get("ok").is_some(),
                    "档 {level} 的 {face} 属「带第二层 ok」那四发 ⇒ 这一格变了：{value}"
                );
            }
            // TODO(主代理现测)：`settle["reason"]` / `settle["message"]` 的逐档表；
            // TODO(主代理现测)：`stop` 的 `reason`（推证：档 1 plugin-missing、2/4/5 not-running、
            //   档 3 `{ok:true}` 只有一颗键）；
            // TODO(主代理现测)：`undefine["wasRunning"]` 的逐档值（推证：档 3 true、2/4 false、
            //   1/5 落 `plugin-missing` 无该键）——主干不读 ⇒ 记账即可。
        }
    }

    /// T10 · 那**四发无臂端点** × 「事件在册」的对位：本族四发的名字在分叉登记表上（emit 侧），
    /// 而主干零调用方那四发端点在任何一档都**没有臂**。
    /// ⚠ 去重：`--cordis=` 逐档（0..=5）404 已被既有
    ///   `st13_the_four_callerless_faces_still_have_no_route_at_any_level` 锁死 ⇒ 这里**不**重跑
    ///   那三圈子进程，只留「一档 + 在册反证」。名单沿用既有构造法：只拼尾段，测试侧不出现四个全名。
    /// ⚠ 待主代理核：既有 `st13_in_cordis_level` 只拧 `--cordis=` 一枚，**没有**同时拧
    ///   `--cordis-events=` 的现成 helper ⇒ 这一枚没在同进程里开 `$events` 流。
    ///   要「事件腿在架 + RPC 腿无臂」同进程版，照 T8 起手式手写 `launch_with(&["--pace=0",
    ///   "--cordis=2", "--cordis-events=5"])` 那一枚桩即可（母本 ② T10 的 `[0, 2, 5]` 三圈属重复取证）。
    #[test]
    fn ip6_the_four_callerless_faces_still_get_404_beside_the_events_knob() {
        // ① 事件在册（纯函数，不起子进程）。
        for name in FOUR {
            assert_eq!(
                events_subscription(&json!({"type": "emit", "event": name, "args": [{}]})),
                Some(name),
                "{name} 该被 `EVENTS_SUBSCRIPTIONS` 认在 emit 侧"
            );
        }
        // ② RPC 无臂：那四发端点不在分叉的八发名单上。
        for tail in [
            "invoke",
            "resolveInspectQuery",
            "reportClientGuardFailure",
            "reportRenderFailure",
        ] {
            let endpoint = format!("dynamicCordisRunner/{tail}");
            assert!(
                !CORDIS_METHODS.contains(&endpoint.as_str()),
                "{endpoint} 不许进 CORDIS_METHODS 的八发名单"
            );
        }
        // ③ 真 socket 那一格（母本 T10 的六分之一，剩下的既有台账已覆盖）。
        st13_in_cordis_level(2, |kernel| {
            for tail in ["invoke", "resolveInspectQuery"] {
                let endpoint = format!("dynamicCordisRunner/{tail}");
                let err = kernel
                    .call(&endpoint, json!({}))
                    .expect_err("{endpoint} 在档 2 拿到了臂");
                assert!(
                    err.contains("HTTP 404") && err.contains("not_found") && !err.contains("bad_args"),
                    "档 2 的 {endpoint} 该落 404 not_found：{err}"
                );
            }
        });
    }
}

// ==================== 粘贴指引（落地时照这一段走，**一律不给行号**）====================
//
// A. **整族的形状**：本文件已经是 `mod ip6_cordis_socket_tests { … }`（母本 ④ R1 要求自成一 mod），
//    **整块**贴进 `tests/ipc.rs` 即可，模内首行就是 `use super::*;`（`super` = ipc 这棵测试 crate
//    的根，`collect_within` / `item_values` / `sorted_keys` / `launch_with` / `open_stream_by` /
//    `st13_in_cordis_level` / `Value` / `json!` 全从那儿来）。
//    ⚠ 本文件**顶部**那段 `// ====` 注释块是母本口径说明，不属于 mod；贴的时候连注释一起贴也行，
//    但 mod 外壳只有一层（别把 `use` 提到文件作用域）。
//
// B. **插到哪一类位置**：`tests/ipc.rs` 的**最末尾**——即「cordis 八发构造器 × 桩键集闸（ST13 落地）」
//    那一大族之后。按锚串找：
//      · 后向锚（贴它后面）：`fn st13_step_frames_reach_the_run_stats_ledger_exactly_once_after_the_feed_move()`
//        所在的那一族结尾（该族的段首注释锚串 = `台账 #135 / #149 · MR5 §7`），它已是文件最后一段。
//      · 前向锚（别贴到它前面）：`use blade2_rs::kernel::{` 那一块（文件作用域、紧跟
//        `台账 #144 · KW2 §6 那批：cordis 八发构造器 × 桩键集闸` 的段首注释）——本族必须**在它之后**，
//        这样 `use super::*` 才看得见 `CORDIS_METHODS` 等名字。
//      · 段首注释（照本仓风格，粘在 mod 之前）：
//        `// ==================== 台账 #144 · 刀5：cordis 事件族 + 审批 RPC 的真 socket 贯通（IP6 落地）====`
//
// C. **要新增哪些 `use`**：文件作用域**一个都不加**。
//    模内**必须**新增（文件作用域没有；`grep -n 'events_subscription' tests/ipc.rs` 等零命中）：
//      `EventsFrame`、`parse_events_frame`、`events_subscription`、`CordisPendingApproval`、
//      `cordis_pending_from_event`、`cordis_host_half_ok`、`cordis_host_half_plugin_run_id`、
//      `cordis_host_half_message`、`cordis_still_pending`。
//    其余（`CORDIS_METHODS`、`CordisRunResolution`、`cordis_inventory`、`parse_cordis_rows`、
//    八发构造器）文件作用域已导入 ⇒ 走 `use super::*` 就够；本稿仍在模内显式列出，
//    靠「显式 import 遮蔽 glob」不触发 E0252（母本 ④ R1 的原话）。
//    **不许**在文件作用域重复 import 这些名字 ⇒ 那才是 E0252。
//
// D. **文字账（母本 ④ R2，落地后改）**：本稿实交 **11** 枚 `#[test]`
//    （T1–T8 + T9 + T9b + T10：T9 按母本 ③/④ 的约束拆成「分叉有读者」的成品 + 「无读者回执」的骨架）。
//    ⇒ `cargo test --test ipc -- --list` 从 **161** 变 **172**。
//    ⚠ 更正母本：`tests/ipc.rs` 里**没有**任何「161 枚」的文字账
//    （`grep -n "161" tests/ipc.rs` 只命中 `dsh-permission-presets/lib/index.js:161-181` 这类
//    **无关行号引用**）⇒ 要改的是**台账/报告**：母本 `tmp/ip6-cordis-socket-cases.md` §1「既有 ipc
//    用例总数：161」与 §4 R2 那两处，改成 172（或按实际贴入枚数 = 161 + N）。
//    `tmp/accept-*.log` 里的 `running 161 tests` 是**历史验收日志**，不要动。
//    主仓里若还有 `tests/*.md` / `*.ps1` 的枚数账，本代理 grep 无命中 ⇒ 无连带。
//
// E. **桩侧同步（母本 ④ R5）**：本稿**不改桩**，`--cordis-events=` 只用既有的 0..=5 档。
//    若决定加新档（例如 6），三处**必须**同步（都是计数/形状闸）：
//      ① `src/bin/fake_dsh.rs` 的 `CORDIS_EVENTS_MAX_LEVEL`（不改它，6 会被 `cordis_events_level`
//         夹回 0 ⇒ 本稿 T1/T2 的红法整个变）；
//      ② 桩自测族 `cordis_events_tests` 里那张**逐档表**（`the_per_level_table…` 那枚按符号名找）；
//      ③ 本稿 T1 的 `table: [(u64, &[&str]); 6]`（枚数 + 帧序两处）。
//
// F. **跑法与判绿顺序**（本代理按禁令**没跑过任何 cargo**，本稿是**未编译验证**的成品）：
//    1) `cargo test --offline --test ipc ip6_ -- --nocapture` 先单跑这一族 ⇒ 顺手把 T9b 的
//       五档回执打出来，按 R4 把表钉完再把 `println!` 换 `assert_eq!`；
//    2) 再确认既有两把尺仍绿：`mux_events_stream_sends_ready_then_emit`（恰 2 帧）与 `st13_*` 六枚。
//       它们红的唯一成因就是有人在测试里 `set_var(FAKE_DSH_CORDIS_EVENTS)`（坑 ③）⇒ 本稿零 `set_var`。
//    3) 落地前请先确认 `st13_*` 全绿（母本 ④ R6：本族与 `src/main.rs` 的 dispatch 臂无关，可并行）。
//
// G. **本稿与母本 ② 的差异（都记在这里，免得落地时被当成草案笔误）**：
//    · T1：母本对**每一帧**断言 `args.len()==1`，但 `api-session/status` 是**两**参数
//      （既有起手式就写着 `json!(["s-1", true])`）⇒ 母本那格必假红。已收窄到 index≥2 的本族帧，
//      并补了 status 腿的两参数断言。
//    · `events_at`：加了 `QUIET_BUDGET` 静默反证——`collect_within` 一到 `want` 就返回，
//      「不许多推」不这么写就证不到（母本 ② 的 T1/T2 只数了「够不够」）。
//    · T9：母本那枚把 `settleUserRun` 的 `reason` 当全档 `"rejected"` 断言（**必假红**），
//      且末尾写了 `assert_eq!(…).ok()`（`()` 没这个方法 ⇒ **编译不过**）⇒ 拆成 T9（只打**分叉真有
//      读者**的三把尺 + 单层 `resolveRequestRun`）与 T9b（骨架，主干整个丢弃的那三发）。
//    · T10：母本重跑 `[0, 2, 5]` 三圈 × 四发的 404 ⇒ 既有 `st13_the_four_callerless_faces_…`
//      已把档 0..=5 全跑过 ⇒ 本稿只留一档两发 + 「事件在册」反证，省两次子进程。
//    · 通用写法：RPC 一律 `let call = …(); kernel.call(call.method, call.args)`（不再把构造器写两遍）；
//      `Some(&card.request_id)` 改成 `Some(card.request_id.as_str())`（签名是 `Option<&str>`）；
//      `Kernel::shutdown(&mut self)` ⇒ 接 kernel 的元组一律 `let (mut kernel, …)`。
//    · 本稿**没有**用到的既有助手：`expect_item` / `event_stream` / `open_stream` / `open_stream_with`
//      / `paced_launch`（本族不需要节奏）⇒ 未引用，也没发明任何新助手。

// ============ 台账 #167 · IP8：`settings/mutate` 的真 socket 贯通（第 ③+④ 层）============
//
// 四层判据里 ①（`kernel.rs` 的 ctor，B-1/#167）与 ②（`main.rs` 消费，被 #165 那 11 枚主干锁压着）
// 之外，本族交 ③（桩臂复核）+ ④（真 socket）。S4 已实测证明 ④ 不依赖 ②：`tests/ipc.rs` 全族零
// `Shell`、零 `Msg`，走的是「ctor 造帧 → 真 socket → 桩 dispatch → 回执解析」的进程内贯通。
//
// 三条本仓纪律，已落到代码里：
// ① **帧预算 = 0**：`settings/mutate` 不订流、不发 prompt ⇒ 全族零 `collect_within`、零 `sleep`、
//    零节奏常量（`DEFAULT_PACE_MS` / `READ_TICK` / `TURN_FRAMES` 一把都不用），起桩一律 `--pace=0`
//    （走既有 `fake_launch()`，它只带这一颗）。
// ② **桩不被「修好」**：ip8 登记时桩与真内核不一致的六处（回执少 `autoGenerate` / revision 起点 1 vs 0 /
//    no-op 写也抬 revision / `ops:[]` 桩拒真内核收 / 空 path 的 unset 桩清空整段而真内核折成写回 base /
//    unset 在 base 有值时真内核折成「set 回继承值」而桩直接删）**全部用等值断言钉住现状**，
//    差异登记在 `tmp/ip8-report.md` §4；ip8 那一刀 `src/bin/fake_dsh.rs` **零改动**。
//    ⚠ **RS1 更新（`tmp/rs1-report.md` §2）**：其中前两处（§4-D4 revision 起点、§4-D2 no-op 抬格）
//    已按真内核 `dsh-settings/lib/index.js:421-428` **改掉桩** ⇒ 本 mod 里那两族的等值断言
//    现在钉的是**内核规则**（起点 0、raw 没变不抬格），不再是桩的偏差现状。
//    后四处仍未修，本 mod 继续钉桩现状。
// ③ **平铺不裹 `request`**：主干 7 发（`tmp/b1-report.md` §2.1）无一裹 `request`、无一递
//    `expectedRevision` ⇒ 两档都做实测；判据一律 `assert_eq!` 打整颗值，禁 `contains`、禁 `>= N`。
//
// ⚠ 本 mod **只在文件 EOF 追加**，既有 13618 行一字未动；那几枚 ns 计数闸
//   （`namespaces.len() == 13` ×3 + `expected` 13 串数组 + discoverModels 拒绝圈 + 拒绝文案闸）
//   的现测落点与「本族没碰它们」的 `diff` 证据见报告 §5/§7。

mod ip8_settings_mutate_tests {
    use super::*;
    use blade2_rs::kernel::{
        CORDIS_METHODS, RD9_C4_C9_METHODS, RD9_C5_C6_METHODS, SETTINGS_MUTATE,
        SETTINGS_MUTATE_METHODS, settings_mutate, settings_op_set, settings_op_unset,
    };

    /// 桩台账 `fake_dsh.rs` 里 `SETTINGS_NS` 的**现值逐字**（顺序也照桩数组，T8 那句拒绝文案要用它）。
    const LEDGER: [&str; 13] = [
        "llm-deepseek",
        "llm-pi-ai",
        "agent-presets",
        "ui-theme",
        "locale",
        "ui-chat",
        "ui-conversation",
        "permission",
        "agent-default-model",
        "shell",
        "agent-loop",
        "subagent-model-selection",
        "web-search-deepseek",
    ];

    /// 桩回执的键集合（**排序后**）：`fake_dsh.rs` 的 `FakeState::namespace_view` 产八颗。
    /// ⚠ 真内核 `typert.host.js:69-82` 的 result schema 是**九**颗（第一颗就是 `autoGenerate`）
    ///   ⇒ 这里钉的是**桩现状**，差异进报告 §4-D1，不在本刀修桩。
    const STUB_VIEW_KEYS: [&str; 8] = [
        "applies", "base", "ns", "revision", "schema", "secrets", "user", "value",
    ];

    /// 排序后的键集合（等值判据用；`assert_eq!` 整串，不用 `contains`）。
    fn ip8_keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .expect("判据对象必须是 JSON 对象")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    fn ip8_sorted(names: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = names.iter().map(|name| (*name).to_string()).collect();
        out.sort();
        out
    }

    /// 单发一次 mutate（自己起桩、自己收尾，帧预算 0），把回执原样交回去。
    fn ip8_one_call(ns: &str, ops: &[Value], revision: Option<f64>) -> Value {
        let call = settings_mutate(ns, ops, revision);
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let out = kernel
            .call(call.method, call.args)
            .unwrap_or_else(|error| panic!("{ns} 的合法 ops 该放行，实际 {error}"));
        kernel.shutdown();
        out
    }

    /// T1 · 简报 §2-1：方法串 + args 顶层键**恰** `{ns, ops}`（平铺、不裹 `request`、不带 revision），
    /// 且这颗帧在真 socket 上打得通。反向半段：裹进 `request` 必须撞桩的 wire 形状闸（逐字整串）。
    #[test]
    fn ip8_the_ctor_frame_lands_on_the_socket_with_exactly_ns_and_ops() {
        let call = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(16))],
            None,
        );
        assert_eq!(call.method, SETTINGS_MUTATE);
        assert_eq!(call.method, "settings/mutate", "方法串逐字");
        assert_eq!(
            ip8_keys(&call.args),
            vec!["ns".to_string(), "ops".to_string()],
            "顶层键集合恰 ns 与 ops 两颗：多一颗 expectedRevision、或少一颗都算形状漂"
        );
        assert_eq!(call.args["ns"], json!("ui-theme"));
        assert_eq!(
            call.args["ops"],
            json!([{ "op": "set", "path": ["fontSize"], "value": 16 }]),
            "ops 整颗逐字 = 主干 xaml.cs:16640 那一发的载荷（op + path 数组 + value 三支）"
        );

        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let receipt = kernel
            .call(call.method, call.args.clone())
            .expect("平铺 ns + ops 两颗顶层键该放行");
        assert_eq!(receipt["ns"], json!("ui-theme"));
        assert_eq!(receipt["value"]["fontSize"].as_f64(), Some(16.0));

        let wrapped = kernel
            .call(
                SETTINGS_MUTATE,
                json!({ "request": { "ns": "ui-theme", "ops": call.args["ops"].clone() } }),
            )
            .expect_err("主干 7 发无一裹 request ⇒ 裹了就该在网关形状闸上红（桩镜像 assertExactArguments）");
        assert_eq!(
            wrapped,
            "bad_args: args fields do not match the descriptor: missing \"ns\", \"ops\"; unexpected \"request\""
        );
        kernel.shutdown();
    }

    /// T2 · 简报 §2-2：单段 path 落位 → `settings/describe` 读回同值；`base` 不动、兄弟键不抹。
    #[test]
    fn ip8_a_single_segment_set_lands_in_the_user_layer_and_reads_back_through_describe() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let before = ns_row(&mut kernel, "ui-theme");
        assert_eq!(before["user"], json!({}), "ui-theme 的用户层初值是空对象");
        assert_eq!(before["value"]["fontSize"].as_f64(), Some(14.0));
        assert_eq!(
            before["revision"].as_f64(),
            Some(0.0),
            "真内核 `dsh-settings/lib/index.js:428` 的起点：从没写过的 ns 首笔 describe 报 0（RS1 已对齐）"
        );

        let call = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(12))],
            None,
        );
        let receipt = kernel
            .call(call.method, call.args)
            .expect("单段 path 该落位");
        assert_eq!(receipt["user"], json!({ "fontSize": 12 }));
        assert_eq!(receipt["value"]["fontSize"].as_f64(), Some(12.0));
        assert_eq!(
            receipt["base"]["fontSize"].as_f64(),
            Some(14.0),
            "base 层不许被写动（主干「已覆盖」标记就靠 base≠user 判）"
        );

        let after = ns_row(&mut kernel, "ui-theme");
        assert_eq!(
            after["value"]["fontSize"].as_f64(),
            Some(12.0),
            "describe 必须读回落下的值"
        );
        assert_eq!(after["user"], json!({ "fontSize": 12 }));
        assert_eq!(
            after["value"]["preference"],
            json!("system"),
            "没写过的兄弟键仍继承 base"
        );
        kernel.shutdown();
    }

    /// T3 · 简报 §2-3：多段 path（主干 `xaml.cs:12152` 那一发就是多段）⇒ 嵌套落位，
    /// 同段下的兄弟提供方与同一颗里的兄弟字段都不许被抹掉。
    #[test]
    fn ip8_a_multi_segment_set_nests_into_place_without_clobbering_its_siblings() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let seeded = ns_row(&mut kernel, "llm-pi-ai");
        assert_eq!(
            seeded["user"]["providers"]["my-gateway"]["displayName"],
            json!("自建网关"),
            "先确认桩的用户层真有一支兄弟提供方，否则下面的「不抹兄弟」是空跑"
        );

        let two_segments = settings_mutate(
            "llm-pi-ai",
            &[settings_op_set(
                &["providers", "ip8-gateway"],
                &json!({ "displayName": "分叉网关", "api": "openai-completions" }),
            )],
            None,
        );
        let receipt = kernel
            .call(two_segments.method, two_segments.args)
            .expect("两段 path");
        assert_eq!(
            receipt["value"]["providers"]["ip8-gateway"],
            json!({ "displayName": "分叉网关", "api": "openai-completions" }),
            "两段 path 要落在 providers 之下，不是把 providers 整段换掉"
        );
        assert_eq!(
            receipt["value"]["providers"]["my-gateway"]["displayName"],
            json!("自建网关"),
            "同一段下的兄弟提供方不许被抹掉"
        );

        let three_segments = settings_mutate(
            "llm-pi-ai",
            &[settings_op_set(
                &["providers", "ip8-gateway", "displayName"],
                &json!("改过名的分叉网关"),
            )],
            None,
        );
        let receipt = kernel
            .call(three_segments.method, three_segments.args)
            .expect("三段 path");
        assert_eq!(
            receipt["user"]["providers"]["ip8-gateway"],
            json!({ "displayName": "改过名的分叉网关", "api": "openai-completions" }),
            "只碰 displayName 一颗，同一颗里的 api 原样留着"
        );
        assert_eq!(
            receipt["value"]["providers"]["my-gateway"]["models"]
                .as_array()
                .map(Vec::len),
            Some(2),
            "兄弟提供方的 models 一条不许少"
        );
        let row = ns_row(&mut kernel, "llm-pi-ai");
        assert_eq!(
            row["value"]["providers"]["ip8-gateway"]["displayName"],
            json!("改过名的分叉网关")
        );
        kernel.shutdown();
    }

    /// T4 · 简报 §2-4：`unset` 删既有键；**同一路径再发一次 unset** ⇒ 幂等 `Ok`、不回错。
    /// RS1 改桩后（`tmp/rs1-report.md` §2）第二发**不再抬 revision** —— 这一格从此与真内核
    /// `dsh-settings/lib/index.js:428` 的 `Number(previous.raw !== raw)` **同判**（原 §4-D2 差异已修）。
    /// ⚠ 仍存的一处差异是 §4-D3：base 有值时真内核把 unset 折成「set 回继承值」（用户层长出该键），
    /// 桩仍是直接删 ⇒ 这里 `user == {}` 钉的是**桩现状**，不是内核现状。
    #[test]
    fn ip8_unset_clears_the_user_key_and_a_second_unset_of_the_same_path_is_idempotent_on_the_stub() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let staged = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(12))],
            None,
        );
        kernel.call(staged.method, staged.args).expect("先铺一层用户覆盖");
        assert_eq!(
            ns_row(&mut kernel, "ui-theme")["revision"].as_f64(),
            Some(1.0),
            "起点 0 + 一发 raw 真变了的写 = 1（真内核 `dsh-settings/lib/index.js:428`）"
        );

        let first = settings_mutate("ui-theme", &[settings_op_unset(&["fontSize"])], None);
        assert_eq!(
            ip8_keys(&first.args["ops"][0]),
            vec!["op".to_string(), "path".to_string()],
            "unset 那一支不许长出 value 键（主干 5 发 unset 全是 op + path 两颗）"
        );
        let receipt = kernel
            .call(first.method, first.args)
            .expect("第一次 unset 该放行");
        assert_eq!(receipt["user"], json!({}), "用户层里那根键要真没了");
        assert_eq!(
            receipt["value"]["fontSize"].as_f64(),
            Some(14.0),
            "删掉覆盖 = 回落到 base"
        );
        assert_eq!(receipt["revision"].as_f64(), Some(2.0), "删掉那根键 = raw 变了 ⇒ 抬一格");

        let second = settings_mutate("ui-theme", &[settings_op_unset(&["fontSize"])], None);
        let receipt = kernel.call(second.method, second.args).unwrap_or_else(|error| {
            panic!("实测档：桩对第二次 unset 同一路径回的是幂等 Ok，不是错误 —— 实际 {error}")
        });
        assert_eq!(receipt["user"], json!({}), "再删一次状态不变（幂等）");
        assert_eq!(receipt["value"]["fontSize"].as_f64(), Some(14.0));
        assert_eq!(
            receipt["revision"].as_f64(),
            Some(2.0),
            "RS1 对齐后：no-op 的 unset **不**抬 revision（真内核 `S:428` 的 `Number(raw !== raw)` = 0）"
        );
        let row = ns_row(&mut kernel, "ui-theme");
        assert_eq!(row["revision"].as_f64(), Some(2.0));
        assert_eq!(row["user"], json!({}));
        kernel.shutdown();
    }

    /// T5 · 简报 §2-5（**实测档**）：同批 3 发 / 2 发 op 一做做完 ⇒ revision **只抬一次**，
    /// 不是每 op 一次；批里每颗 op 全落账。
    #[test]
    fn ip8_a_batch_of_three_ops_bumps_the_revision_exactly_once_not_once_per_op() {
        let batch3 = settings_mutate(
            "ui-theme",
            &[
                settings_op_set(&["fontSize"], &json!(15)),
                settings_op_set(&["preference"], &json!("light")),
                settings_op_unset(&["neverWritten"]),
            ],
            None,
        );
        assert_eq!(batch3.args["ops"].as_array().map(Vec::len), Some(3));
        let receipt = ip8_one_call(
            "ui-theme",
            batch3.args["ops"].as_array().expect("三发 op"),
            None,
        );
        assert_eq!(
            receipt["user"],
            json!({ "fontSize": 15, "preference": "light" }),
            "三发 op 一起落账；删不存在的那颗不许长出键"
        );
        assert_eq!(
            receipt["revision"].as_f64(),
            Some(1.0),
            "同批 3 op ⇒ 0 → 1：**每批一次**，不是每 op 一次（每 op 一次才会到 3.0）"
        );
        assert_eq!(receipt["value"]["fontSize"].as_f64(), Some(15.0));
        assert_eq!(receipt["value"]["preference"], json!("light"));

        let batch2 = json!([
            { "op": "set", "path": ["timeoutMs"], "value": 30_000 },
            { "op": "set", "path": ["graceMs"], "value": 5_000 },
        ]);
        let receipt = ip8_one_call("shell", batch2.as_array().expect("两发 op"), None);
        assert_eq!(
            receipt["revision"].as_f64(),
            Some(1.0),
            "两发同样只抬一次（起点 0 ⇒ 1）"
        );
        assert_eq!(receipt["value"]["timeoutMs"].as_f64(), Some(30_000.0));
        assert_eq!(receipt["value"]["graceMs"].as_f64(), Some(5_000.0));
        assert_eq!(
            receipt["value"]["maxOutputBytes"].as_f64(),
            Some(64_000.0),
            "没写过的第三根仍取默认"
        );
    }

    /// T6 · 简报 §2-6 的三档 + RS1 改桩后的两格：`expectedRevision` **起点 0** ⇒ `Some(0.0)` 是
    /// **合法首写**（真内核 `dsh-settings/lib/index.js:428` 的 `previous === void 0 ? 0`；桩此前起点 1
    /// 必撞锁 = ip8 §4-D4，已修）、对不上的那一格撞锁且既不入账也不抬 revision、
    /// 整键不发 = 无条件写 ⇒ `settings/update` 那族的 `acceptsUndefined` 律**在 mutate 上也成立**
    /// （现测，不是从上一族套过来的）。另钉 RS1 的 D-a 档：同值重写放行但**不抬格**（ip8 §4-D2 已修）。
    #[test]
    fn ip8_expected_revision_starts_at_zero_and_a_no_op_replay_does_not_bump_it() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        assert_eq!(
            ns_row(&mut kernel, "ui-theme")["revision"].as_f64(),
            Some(0.0),
            "桩的 revision 起点 = 0（RS1 对齐真内核 `:428`）"
        );

        let zero = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(16))],
            Some(0.0),
        );
        assert_eq!(
            ip8_keys(&zero.args),
            vec!["expectedRevision".to_string(), "ns".to_string(), "ops".to_string()],
            "Some 那一档长三颗顶层键"
        );
        assert_eq!(zero.args["expectedRevision"].as_f64(), Some(0.0));
        let receipt = kernel
            .call(zero.method, zero.args)
            .expect("起点就是 0 ⇒ 递 Some(0.0) 是合法首写（真内核同判）");
        assert_eq!(receipt["revision"].as_f64(), Some(1.0));
        assert_eq!(receipt["value"]["fontSize"].as_f64(), Some(16.0));

        // 写之前读到的那一格现在陈旧了 ⇒ 撞锁，文案逐字（`{expected}` 打的是 0、`{actual}` 是 1）。
        let stale = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(13))],
            Some(0.0),
        );
        let refused = kernel
            .call(stale.method, stale.args)
            .expect_err("对不上的 expectedRevision 必须撞锁");
        assert_eq!(
            refused,
            "settings/conflict: ui-theme 已被其他地方改动：expectedRevision 0，实际 1"
        );
        let row = ns_row(&mut kernel, "ui-theme");
        assert_eq!(row["revision"].as_f64(), Some(1.0), "撞锁的写不抬 revision");
        assert_eq!(row["value"]["fontSize"].as_f64(), Some(16.0), "撞锁的写不落账");

        // D-a 档：把同一发 op 原样重放（raw 没变）⇒ 放行、回同一颗 revision。
        let replay = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(16))],
            Some(1.0),
        );
        let receipt = kernel
            .call(replay.method, replay.args)
            .expect("同值重写在内核上不是错误，只是一次不抬格的写");
        assert_eq!(
            receipt["revision"].as_f64(),
            Some(1.0),
            "raw 没变的写不抬 revision（真内核 `:428` 的 `Number(previous.raw !== raw)` = 0）"
        );

        // 匹配那一格照常 +1：证明上面那发不是「永远不抬」的假绿。
        let matched = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(17))],
            Some(1.0),
        );
        let receipt = kernel
            .call(matched.method, matched.args)
            .expect("读到哪一格就该能写到哪一格");
        assert_eq!(receipt["revision"].as_f64(), Some(2.0));
        assert_eq!(receipt["value"]["fontSize"].as_f64(), Some(17.0));

        let unconditional = settings_mutate(
            "ui-theme",
            &[settings_op_set(&["preference"], &json!("light"))],
            None,
        );
        assert_eq!(
            ip8_keys(&unconditional.args),
            vec!["ns".to_string(), "ops".to_string()],
            "None 那一档整键不发（主干 7 发全是这一档）"
        );
        let receipt = kernel
            .call(unconditional.method, unconditional.args)
            .expect("不带 revision = 无条件写");
        assert_eq!(receipt["revision"].as_f64(), Some(3.0));
        assert_eq!(receipt["user"], json!({ "fontSize": 17, "preference": "light" }));
        kernel.shutdown();
    }

    /// T7 · 简报 §2-6 的第三档 + 空批：显式 `null` 的 revision 与 `ops:[]` 都在 wire 闸上红，
    /// 且红完一格都没写。⚠ 后半格是**桩比真内核严**的一处（真内核 `ops:[]` 是合法 no-op），
    /// 差异登记报告 §4-D5，本刀不修。
    #[test]
    fn ip8_explicit_null_revision_and_an_empty_ops_array_are_both_refused_without_writing() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let null_revision = kernel
            .call(
                SETTINGS_MUTATE,
                json!({
                    "ns": "ui-theme",
                    "ops": [{ "op": "unset", "path": ["fontSize"] }],
                    "expectedRevision": null,
                }),
            )
            .expect_err("描述符那侧 expectedRevision 只吃 number|undefined，显式 null 非法");
        assert_eq!(
            null_revision,
            "bad_args: expectedRevision 只能是数字，不给就整个键都别发"
        );

        let empty = settings_mutate("ui-theme", &[], None);
        assert_eq!(
            empty.args["ops"],
            json!([]),
            "ctor 不替派发侧兜空批（非空闸在主干 `xaml.cs:12066`）"
        );
        let refused = kernel
            .call(empty.method, empty.args)
            .expect_err("桩的 ops 非空闸");
        assert_eq!(refused, "bad_args: ops 必须是非空数组");

        let row = ns_row(&mut kernel, "ui-theme");
        assert_eq!(row["revision"].as_f64(), Some(0.0), "两发红完一格都没抬（起点 0）");
        assert_eq!(row["user"], json!({}), "两发红完一笔都没落");
        assert_eq!(row["value"]["fontSize"].as_f64(), Some(14.0));
        kernel.shutdown();
    }

    /// T8 · 简报 §2-7：台账外 ns 的**逐字**回执（整串等值，含桩把 13 支台账列进文案那半段），
    /// 并且不许顺手写进名字相近的那支真台账。
    #[test]
    fn ip8_an_unknown_ns_answers_with_the_verbatim_settings_rejected_text() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let call = settings_mutate(
            "shell-x",
            &[settings_op_set(&["timeoutMs"], &json!(1))],
            None,
        );
        let error = kernel
            .call(call.method, call.args)
            .expect_err("台账外 ns（差一个字母那一族）必须被拒");
        assert_eq!(
            error,
            format!(
                "settings/rejected: 假内核没有 shell-x 这个命名空间（可写的是 {:?}）",
                LEDGER
            )
        );
        assert_eq!(
            ns_row(&mut kernel, "shell")["revision"].as_f64(),
            Some(0.0),
            "台账外那一发不占账：起点 0 仍在 0"
        );
        assert_eq!(
            ns_row(&mut kernel, "shell")["value"]["timeoutMs"].as_f64(),
            Some(120_000.0),
            "未知 ns 那一发不许顺手写进名字相近的台账支"
        );
        kernel.shutdown();
    }

    /// T9 · 简报 §2-8：回执形状。桩这一族是**单层 `ok`**（外层 `result:{ok,value}` 被
    /// `Kernel::call` 剥掉，内层不再长第二颗 `ok`）⇒ 与 `messageFeedback/*`、`sessionFeedback/record`
    /// 的双层**不同判**（逐发判，本发是单层）。顺带钉住「桩少 `autoGenerate` 一颗」的现状。
    #[test]
    fn ip8_the_receipt_is_a_flat_namespace_view_with_a_single_ok_layer() {
        let receipt = ip8_one_call(
            "ui-theme",
            &[settings_op_set(&["fontSize"], &json!(13))],
            None,
        );
        assert_eq!(
            ip8_keys(&receipt),
            ip8_sorted(&STUB_VIEW_KEYS),
            "桩的回执恰这八颗，多一颗少一颗都红"
        );
        assert_eq!(receipt.get("ok"), None, "内层不许再长第二颗 ok（本族不是双层）");
        assert_eq!(receipt["ns"], json!("ui-theme"));
        assert_eq!(receipt["applies"], json!("live"));
        assert_eq!(receipt["secrets"], json!([]), "ui-theme 没有 secret 位");
        assert!(receipt["schema"].is_object());
        assert_eq!(
            receipt.get("autoGenerate"),
            None,
            "桩现状钉死：真内核 result schema（typert.host.js:69-82）第一颗就是 autoGenerate，桩发不出"
        );
    }

    /// T10 · 简报 §2-9：`DISCOVER_NS` 反证 —— 对台账内合法 ns 发 mutate，回执里不许长出
    /// 模型发现相关键；且写过之后 discovery 的门仍只开那两支。
    #[test]
    fn ip8_a_mutate_receipt_never_grows_model_discovery_keys_and_the_discovery_gate_stays_closed() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let call = settings_mutate(
            "llm-pi-ai",
            &[settings_op_set(
                &["providers", "ip8-gateway", "displayName"],
                &json!("写完再验发现门"),
            )],
            None,
        );
        let receipt = kernel
            .call(call.method, call.args)
            .expect("台账内 ns 的 path 写该放行");
        assert_eq!(ip8_keys(&receipt), ip8_sorted(&STUB_VIEW_KEYS));
        for forbidden in ["models", "candidates", "discovered", "items", "results", "provider"] {
            assert_eq!(
                receipt.get(forbidden),
                None,
                "mutate 的回执不是模型发现回执：长出 {forbidden} 就是演了真内核没有的行为"
            );
        }

        let described = kernel
            .call("settings/describe", json!({}))
            .expect("describe");
        let listed: Vec<&str> = described["namespaces"]
            .as_array()
            .expect("namespaces 是数组")
            .iter()
            .map(|row| row["ns"].as_str().expect("ns"))
            .collect();
        assert_eq!(listed.len(), 13, "先确认台账真是 13 支，否则下面那圈是空跑");
        for ns in listed
            .iter()
            .filter(|ns| **ns != "llm-deepseek" && **ns != "llm-pi-ai")
        {
            let error = kernel
                .call(
                    "llm/discoverModels",
                    json!({ "settingsNs": ns, "request": { "provider": "anything" } }),
                )
                .expect_err(&format!("{ns} 被 mutate 写过也不该长出 discovery 能力"));
            assert!(
                error.contains("llm/model-discovery-rejected"),
                "{ns} 该回 llm/model-discovery-rejected，实际 {error}"
            );
        }
        for ns in ["llm-deepseek", "llm-pi-ai"] {
            kernel
                .call(
                    "llm/discoverModels",
                    json!({ "settingsNs": ns, "request": { "provider": "anything" } }),
                )
                .unwrap_or_else(|error| panic!("两支该过的 discovery 注册不许被本刀拆掉：{error}"));
        }
        kernel.shutdown();
    }

    /// T11 · 简报 §2-10 的反向锁：**本刀没让台账变长**。台账仍逐字等于既有 13 支（排序后等值），
    /// 且写过三笔之后 describe 的行数与名字集合都不动（mutate 不许物化出新行）。
    /// ⚠ 这是**另开的一份**锁，既有那三枚 `len()==13` 闸与 `expected` 13 串数组一字未动。
    #[test]
    fn ip8_the_write_ledger_is_still_exactly_the_same_thirteen_namespaces() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let names = |kernel: &mut Kernel| -> Vec<String> {
            let mut listed: Vec<String> = kernel
                .call("settings/describe", json!({}))
                .expect("describe")["namespaces"]
                .as_array()
                .expect("namespaces 是数组")
                .iter()
                .map(|row| row["ns"].as_str().expect("ns").to_string())
                .collect();
            listed.sort();
            listed
        };
        assert_eq!(
            names(&mut kernel),
            ip8_sorted(&LEDGER),
            "台账必须逐字 = 既有 13 支；本刀往桩加一支都会在这一格红"
        );

        for (ns, key, value) in [
            ("ui-theme", "fontSize", json!(16)),
            ("shell", "timeoutMs", json!(1000)),
            ("locale", "preference", json!("zh-CN")),
        ] {
            let call = settings_mutate(ns, &[settings_op_set(&[key], &value)], None);
            kernel
                .call(call.method, call.args)
                .unwrap_or_else(|error| panic!("{ns} 在台账上，path 写该放行：{error}"));
        }
        assert_eq!(
            names(&mut kernel),
            ip8_sorted(&LEDGER),
            "写过三笔之后台账既不涨也不缩"
        );
        assert_eq!(
            kernel
                .call("settings/describe", json!({}))
                .expect("describe")["namespaces"]
                .as_array()
                .map(Vec::len),
            Some(13)
        );
        kernel.shutdown();
    }

    /// T12 · 反向锁（ops 支型）：`set` 少 `value`、`op` 字面量不在两支上、`path` 写成字符串、
    /// `path` 里混非字符串四型全在桩的 op 闸上红，且回的是**同一句**逐字文案；红完不落账。
    /// 后半段同时钉 ctor 那两支的形状根（`path` 恒数组、`unset` 恒两颗、`value` 允许 null）。
    #[test]
    fn ip8_every_malformed_op_shape_hits_the_same_verbatim_refusal_and_writes_nothing() {
        const SHAPE_REFUSAL: &str =
            "bad_args: 每个 op 得是 {op:\"set\", path:[字符串], value:…} 或 {op:\"unset\", path:[…]}";
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        for (label, ops) in [
            ("set 少了 value 键", json!([{ "op": "set", "path": ["fontSize"] }])),
            ("op 字面量不在两支上", json!([{ "op": "remove", "path": ["fontSize"] }])),
            ("path 写成字符串", json!([{ "op": "unset", "path": "fontSize" }])),
            ("path 里混进非字符串", json!([{ "op": "unset", "path": [0] }])),
        ] {
            let error = kernel
                .call(SETTINGS_MUTATE, json!({ "ns": "ui-theme", "ops": ops }))
                .expect_err(&format!("{label} 必须被拒"));
            assert_eq!(error, SHAPE_REFUSAL, "{label}");
        }
        assert_eq!(
            ip8_keys(&settings_op_unset(&["provider"])),
            vec!["op".to_string(), "path".to_string()],
            "ctor 造的 unset 支恰 op 与 path 两颗"
        );
        assert_eq!(
            settings_op_unset(&["providers", "my-gateway"])["path"],
            json!(["providers", "my-gateway"]),
            "path 恒为数组（多段就多个元素），不是点号串"
        );
        assert_eq!(
            settings_op_set(&["a", "b"], &json!(null))["value"],
            json!(null),
            "set 的 value 允许 null（JsonValue 全域，本层不 trim 不判空）"
        );
        let row = ns_row(&mut kernel, "ui-theme");
        assert_eq!(row["user"], json!({}), "四发红完一笔都没落");
        assert_eq!(
            row["revision"].as_f64(),
            Some(0.0),
            "四发红完一格都没抬（真内核的起点，见 tmp/rs1-report.md §1）"
        );
        kernel.shutdown();
    }

    /// T13 · 空 path 的 `unset`：**桩现状**是把整段用户层清空（`fake_dsh.rs:6512-6514`
    /// 「`else if path.is_empty() { user = json!({}) }`」那一支）并把 revision 抬 1。真内核走另一条：
    /// `dsh-settings/lib/index.js:488-499` 的 `mutate` 对非数组 path 的 unset 先取 `inherited`
    /// （空 path ⇒ `[].reduce` 直接回 `base` 整份），`inherited !== undefined` ⇒ 折成
    /// `{op:"set", path:[], value: base}` ⇒ 用户层被写成 **base 的副本**（不是 `{}`），
    /// 且**不抛** `applyPathOp`(:213) 那句 `TypeError("Config root must be a plain object")`。
    /// 两边可观察的 `value` 同为默认层，差的正是 `user` 那一层 ⇒ 差异登记报告 §4-D6。
    #[test]
    fn ip8_an_empty_path_unset_clears_the_whole_user_layer_on_the_stub() {
        let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");
        let staged = settings_mutate(
            "shell",
            &[
                settings_op_set(&["timeoutMs"], &json!(1)),
                settings_op_set(&["graceMs"], &json!(2)),
            ],
            None,
        );
        kernel
            .call(staged.method, staged.args)
            .expect("先铺两根覆盖");
        assert_eq!(
            ns_row(&mut kernel, "shell")["user"],
            json!({ "timeoutMs": 1, "graceMs": 2 })
        );

        let empty_path = settings_op_unset(&[]);
        assert_eq!(empty_path["path"], json!([]), "空数组照发 []");
        let call = settings_mutate("shell", &[empty_path], None);
        let receipt = kernel
            .call(call.method, call.args)
            .expect("桩这一支吃空 path（真内核把它折成「写回 base」，见报告 §4-D6）");
        assert_eq!(
            receipt["user"],
            json!({}),
            "桩现状：空 path 的 unset = 整段用户层清空"
        );
        assert_eq!(
            receipt["value"]["timeoutMs"].as_f64(),
            Some(120_000.0),
            "清空后回默认层"
        );
        assert_eq!(
            receipt["revision"].as_f64(),
            Some(2.0),
            "清掉两根真存在的覆盖 = raw 变了 ⇒ 抬一格（起点 0 ⇒ 0→1→2）"
        );
        kernel.shutdown();
    }

    /// T14 · 登记法反向锁（零子进程）：`SETTINGS_MUTATE_METHODS` 基数**恰 1**、只含这颗方法串，
    /// 且新发没落进既有那三族名册；既有名册的基数没被这一刀带漂（等值，不是 `>=`）。
    #[test]
    fn ip8_the_mutate_roster_stays_a_single_entry_and_the_other_rosters_do_not_move() {
        assert_eq!(SETTINGS_MUTATE_METHODS.len(), 1, "自开的名册基数 = 1");
        assert_eq!(SETTINGS_MUTATE_METHODS, [SETTINGS_MUTATE].as_slice());
        assert_eq!(SETTINGS_MUTATE, "settings/mutate");
        assert_eq!(RD9_C5_C6_METHODS.len(), 10);
        assert_eq!(RD9_C4_C9_METHODS.len(), 5);
        assert_eq!(
            SETTINGS_MUTATE_METHODS
                .iter()
                .filter(|method| CORDIS_METHODS.contains(method)
                    || RD9_C5_C6_METHODS.contains(method)
                    || RD9_C4_C9_METHODS.contains(method))
                .count(),
            0,
            "新发不落进既有那三族 ⇒ 那三族的基数闸不用改"
        );
    }
}
