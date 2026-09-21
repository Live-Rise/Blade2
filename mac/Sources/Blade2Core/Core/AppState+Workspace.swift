import Foundation

/// 侧栏工作区/会话动作的 RPC 扩展(与 Win 版 MainWindow.xaml.cs 工作区右键菜单对齐)。
/// rename/delete/create 已在 AppState 主文件,这里只放新增动作;
/// 失败经返回值上报视图层展示——工作区操作失败必须可见(Win xaml.cs:2477 同教训),
/// 视图层用 alert 呈现(Win 版 ShowErrorAsync 同为对话框)。
extension AppState {
    /// 打开工作区路径:先 session/canOpenWorkspacePath(无参 → bool)探能力,
    /// 再 session/openWorkspacePath(request{path, action:"reveal"})执行
    /// (Win xaml.cs:2787-2836 同构)。返回用户可见提示,nil=成功静默。
    func openWorkspacePath(_ ws: WorkspaceVm) async -> String? {
        guard let rpc = rpcClient else { return nil }
        if ws.path.isEmpty {
            return L("该工作区没有路径记录。")
        }
        var can = false
        if let result = try? await rpc.callOk("session/canOpenWorkspacePath", .object([:])) {
            can = result.boolValue ?? false
        }
        guard can else {
            return L("当前部署不支持打开工作目录。")
        }
        do {
            _ = try await rpc.callOk("session/openWorkspacePath", .obj(
                ("request", .obj(("path", .string(ws.path)), ("action", .string("reveal"))))
            ))
            return nil
        } catch let err as DshRpcError {
            return LF("打开路径失败:{0}", err.message)
        } catch {
            return nil
        }
    }

    /// 工作区排序:workspace/insertBefore(request{workspaceId, beforeWorkspaceId?})。
    /// 上移 = 插到前一个工作区之前;下移 = 插到后一个之后(末位则省略 beforeWorkspaceId 追加到尾部)
    /// (Win xaml.cs:2840-2871 同构)。返回用户可见提示,nil=成功静默。
    func moveWorkspace(_ ws: WorkspaceVm, delta: Int) async -> String? {
        guard let rpc = rpcClient, !ws.workspaceId.isEmpty else { return nil }
        guard let index = workspaces.firstIndex(where: { $0.workspaceId == ws.workspaceId }) else { return nil }
        let target = index + delta
        guard workspaces.indices.contains(target) else { return nil } // 已在端点,无操作
        let beforeId: String? = delta < 0
            ? workspaces[target].workspaceId
            : (target + 1 < workspaces.count ? workspaces[target + 1].workspaceId : nil)
        var request: [(String, JSON)] = [("workspaceId", .string(ws.workspaceId))]
        if let beforeId {
            request.append(("beforeWorkspaceId", .string(beforeId)))
        }
        do {
            _ = try await rpc.callOk("workspace/insertBefore", .obj(("request", .dict(request))))
            try? await refreshWorkspaces()
            return nil
        } catch let err as DshRpcError {
            return LF("调整顺序失败:{0}", err.message)
        } catch {
            return nil
        }
    }

    /// 取消运行中的会话:session/cancel(request{sessionId};参数键是 request,_request 会 arguments-invalid,
    /// Win xaml.cs:2511-2526 同构)。返回用户可见提示,nil=成功静默。
    func cancelSession(_ sid: String) async -> String? {
        guard let rpc = rpcClient else { return nil }
        do {
            _ = try await rpc.callOk("session/cancel", .obj(("request", .obj(("sessionId", .string(sid))))))
            return nil
        } catch let err as DshRpcError {
            return LF("取消失败:{0}", err.message)
        } catch {
            return nil
        }
    }
}
