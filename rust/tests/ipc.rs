use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use blade2_rs::i18n::Catalog;
use blade2_rs::kernel::{
    CONTROL_FRAME_TYPES, CommandReply, ControlDelta, ControlState, Kernel, Launch,
    PermissionsProjection, PlanProjection, Projections, SessionStats, SubmittedAttachment,
    WorkspaceTree, control_frame_type, permission_preset_zh, session_status_event,
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
    assert_eq!(first.projections.as_of_seq, Some(88), "游标取大纲末条的 seq");
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

    let empty = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("没发过提示的会话是一页空的");
    assert_eq!(
        empty["records"].as_array().expect("records 是数组").len(),
        0
    );
    assert_eq!(empty["hasMore"], json!(false));

    kernel
        .call("session/prompt", prompt_args("s-1001", "回填我", "queue"))
        .expect("session/prompt 应被接受");
    let page = kernel
        .call("session/page", page_args("s-1001", None, None))
        .expect("一轮之后 journal 能回填");
    let records = page["records"].as_array().expect("records 是数组");
    // 回填的 journal 要与 follow 流推过的那批同形：卅条事件，突变/结果成对铺在答案之前。
    let mut expected: Vec<&str> = vec![
        "turn/start",
        "user/message",
        "request/header",
        "assistant/attempt",
    ];
    for _ in 0..10 {
        expected.extend(["tool/call", "tool/result"]);
    }
    expected.extend([
        "system/message",
        "system/message",
        "assistant/message",
        "deliverables/presented",
        "session/title",
        "turn/end",
    ]);
    assert_eq!(
        records
            .iter()
            .map(|record| {
                assert_eq!(record["type"], json!("event"));
                record["event"]["type"].as_str().unwrap_or_default()
            })
            .collect::<Vec<_>>(),
        expected,
        "回填的 journal 要与 follow 流推过的那批同形"
    );
    assert_eq!(
        records[26]["event"]["data"]["message"]["content"][0]["text"],
        json!("已收到: 回填我")
    );
    // 主干按信封 seq 定位回放锚点：page 的 seq 也得连号，且与 time 一样逐个不同。
    let seqs: Vec<i64> = records
        .iter()
        .map(|record| record["event"]["seq"].as_i64().unwrap_or_default())
        .collect();
    assert_eq!(seqs, (1..=30).collect::<Vec<i64>>());

    // 主干翻历史的两把刀：throughSeq 截断、maxMessages 取尾部。
    let capped = kernel
        .call("session/page", page_args("s-1001", Some(2), None))
        .expect("throughSeq");
    assert_eq!(
        capped["records"].as_array().expect("数组").len(),
        2,
        "只留 seq<=2"
    );
    let tail = kernel
        .call("session/page", page_args("s-1001", None, Some(2)))
        .expect("maxMessages");
    let tail_types: Vec<&str> = tail["records"]
        .as_array()
        .expect("数组")
        .iter()
        .map(|record| record["event"]["type"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(tail_types, vec!["session/title", "turn/end"]);

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
    assert_eq!(namespaces.len(), 3);
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
    assert_eq!(pi["revision"].as_f64(), Some(1.0), "revision 从 1 起");
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
/// 重渲染」，所以假内核必须真的把写入落进台账：revision 逐次 +1、陈旧写回
/// `settings/conflict`、被拒的写不入账、`z.void()` 那两条线上根本没有 `value` 键。
#[test]
fn settings_write_endpoints_round_trip_and_track_revisions() {
    let mut kernel = Kernel::start(&fake_launch()).expect("假内核应完成 dsh web: 握手");

    // describe 先把三个命名空间各起一行账：revision 从 1 起。
    let described = kernel
        .call("settings/describe", json!({}))
        .expect("一次回全部命名空间");
    assert_eq!(
        described["namespaces"][1]["revision"].as_f64(),
        Some(1.0),
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
                Some(1.0),
                "llm-pi-ai",
            ),
        )
        .expect("带正确 expectedRevision 的写应提交");
    assert_eq!(mutated["ns"], json!("llm-pi-ai"));
    assert_eq!(
        mutated["revision"].as_f64(),
        Some(2.0),
        "提交一次 revision +1"
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
                Some(1.0),
                "llm-pi-ai",
            ),
        )
        .expect_err("乐观锁必须挡住陈旧写");
    assert!(error.contains("settings/conflict"), "{error}");
    assert!(
        error.contains("expectedRevision 1") && error.contains("实际 2"),
        "details 里得铺出 expected/actual，主干才有的可展示：{error}"
    );

    // 冲突不入账：按真实 revision 接着写还是能过；unset 整段路由后行卡就该消失。
    let removed = kernel
        .call(
            "settings/mutate",
            mutate_args(
                json!([{ "op": "unset", "path": ["providers", "my-gateway"] }]),
                Some(2.0),
                "llm-pi-ai",
            ),
        )
        .expect("陈旧写之后真实 revision 仍可用");
    assert_eq!(removed["revision"].as_f64(), Some(3.0));
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
    assert_eq!(replaced["revision"].as_f64(), Some(4.0));
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
        Some(2.0),
        "每个命名空间各记各的账，互不牵连"
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

    // 被拒的写一律不入账：三个命名空间的账仍停在最后一次成功之后。
    let after = kernel
        .call("settings/describe", json!({}))
        .expect("复查快照");
    assert_eq!(
        after["namespaces"][0]["revision"].as_f64(),
        Some(1.0),
        "没写过的命名空间也占一行，revision 停在 1"
    );
    assert_eq!(after["namespaces"][1]["revision"].as_f64(), Some(4.0));
    assert_eq!(after["namespaces"][2]["revision"].as_f64(), Some(2.0));
    kernel.shutdown();
}

/// 复查一次预设花名册（`agentPresets/list` 的 `presets` 段）：写入之后主干就是这么重拉的。
fn preset_rows(kernel: &mut Kernel) -> Vec<Value> {
    kernel
        .call("agentPresets/list", json!({}))
        .expect("花名册")["presets"]
        .as_array()
        .expect("presets 是数组（items 那条 fallback 内核从来不发）")
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
            {
                "id": "my-reviewer", "trust": "user", "isDefault": false, "name": "我的审阅",
                "description": null,
            },
        ]),
        "trust 分内置/自定义、isDefault 画那枚标记；description 缺省是 null 而不是缺键"
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
fn answer_of(records: &Value) -> String {
    records
        .as_array()
        .expect("records 是数组")
        .iter()
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
                    records.iter().find_map(|record| {
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
    assert_eq!(message_id, "msg-1-27", "答案 id 的格式是 msg-<turn>-<seq>，atSeq 靠它算");
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

    // 轮次游标也随日志继承：子会话下一轮是 turn 2，答案 id 不能退回 msg-1-*（气泡 key 会撞）。
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
        2,
        "继承的那轮算一条、分支后又问出一轮 ⇒ 恰好两条答案，多一条就是重复回填"
    );
    assert_eq!(answers[0], message_id, "第一条就是分叉前那条答案（原样继承）");
    assert!(
        answers[1].starts_with("msg-2-"),
        "子会话的下一轮 turn 必须续号，实际 {}",
        answers[1]
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

