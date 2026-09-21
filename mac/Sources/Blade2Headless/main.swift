import Foundation
import Blade2Core

/// 无头端到端验证(等价 Windows 版 check-dsh-contract.ps1 的角色)。
/// 阶段一(协议):boot → session/create → session/prompt → session/page 全链路实测。
/// 阶段二(渲染):注入合成 journal 页(user/message + assistant/message + tool/invoke),
/// 断言气泡管线。无凭据数据家 turn 会停在 turn/start(空家 MISSING_CREDENTIAL 正常现象),
/// 因此 assistant 输出的端到端形态用阶段二合成页验证。
@main
struct Blade2Headless {
    static func main() async {
        let app = await MainActor.run { AppState() }
        await MainActor.run {
            Task {
                print("== [1] boot ==")
                await app.boot()
                guard app.phase == .ready else {
                    print("FAIL: boot not ready: \(app.phase)")
                    exit(1)
                }
                print("== ready, models=\(app.catalog.models.count) ==")
                print("== [2] protocol: send ==")
                await app.send(text: "你好,请用一句话自我介绍")
                // send 内部已完成:create → prompt → 兜底页拉 → 轮询启动
                print("== [3] render: synthetic journal page ==")
                let page: JSON = .obj(("records", .array([
                    syntheticEvent(seq: 10, type: "user/message", data: .obj(
                        ("content", .array([.obj(("type", .string("text")), ("text", .string("你好")))])),
                        ("source", .obj(("kind", .string("user"))))
                    )),
                    syntheticEvent(seq: 11, type: "assistant/message", data: .obj(
                        ("message", .obj(
                            ("role", .string("assistant")),
                            ("id", .string("msg-1")),
                            ("content", .array([
                                .obj(("type", .string("reasoning")), ("text", .string("思考中"))),
                                .obj(("type", .string("text")), ("text", .string("你好!我是 Blade²"))),
                            ]))
                        ))
                    )),
                    syntheticEvent(seq: 12, type: "tool/invoke", data: .obj(("name", .string("bash")))),
                    syntheticEvent(seq: 13, type: "assistant/attempt", data: .obj(
                        ("stream", .array([.obj(("chunk", .obj(
                            ("type", .string("finish")),
                            ("reason", .obj(("kind", .string("error")), ("failure", .obj(("code", .string("MISSING_CREDENTIAL")), ("message", .string("no api key"))))))
                        )))]))
                    )),
                ])))
                await MainActor.run { app.debugRenderPage(page) }
                print("== bubbles (\(app.bubbles.count)) ==")
                for b in app.bubbles {
                    print("[\(b.role.rawValue)] \(b.text.prefix(100).replacingOccurrences(of: "\n", with: " "))")
                }
                let roles = Set(app.bubbles.map(\.role))
                let ok = roles.contains(.user) && roles.contains(.assistant) &&
                    roles.contains(.reasoning) && roles.contains(.tool) && roles.contains(.error)
                print(ok ? "== E2E PASS ==" : "== E2E FAIL: roles=\(roles) ==")
                await app.shutdown()
                exit(ok ? 0 : 2)
            }
        }
        try? await Task.sleep(nanoseconds: 240_000_000_000)
        print("FAIL: timeout")
        exit(3)
    }

    static func syntheticEvent(seq: Int, type: String, data: JSON) -> JSON {
        .obj(("event", .obj(("seq", .int(seq)), ("time", .int(1_789_835_592_987)), ("type", .string(type)), ("data", data))))
    }
}
