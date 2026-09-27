using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;

namespace Blade2;

/// <summary>
/// 工作区快照（启动性能：优先加载工作区）：侧栏的工作区/会话清单来自内核，而内核起
/// 进程要数秒。这里把最近一次内核给的清单落盘（DataHome\shell-workspace-snapshot.json），
/// 下次启动壳一亮就把快照渲染上屏——工作区选择器、会话侧栏零等待可用；内核基线到达后
/// 整表替换（基线是权威全量，不做合并），快照只在两者之间垫场。
///
/// 快照过期是常态而非异常：内核不可达时点击快照会话，走引导期排队（见 OpenSessionAsync
/// 的 _pendingOpenSessionId），就绪后自动打开；引导失败则失败卡上屏，快照仅作展示。
/// 写入跟着数据落地走（工作区基线/upsert/remove/order/archived + 会话清单实变），原子写
/// （临时文件 + Move）防半截文件被下一次启动读走。
/// </summary>
public sealed partial class MainWindow
{
    /// <summary>快照文件：%LOCALAPPDATA%\Blade2\shell-workspace-snapshot.json。</summary>
    private static string SnapshotFile => Path.Combine(DataHome, "shell-workspace-snapshot.json");

    /// <summary>内核工作区基线是否已到（快照只在基线未到时上屏，防旧数据覆盖新数据）。</summary>
    private volatile bool _workspaceBaselineArrived;

    private const int SnapshotVersion = 1;

    /// <summary>
    /// 启动时读快照并先行上屏（构造末尾、StartKernelBoot 之前调一次）。文件读放线程池，
    /// 上屏走 PostUi；内核基线若抢先到达（极快热启）则放弃应用。
    /// </summary>
    private void LoadWorkspaceSnapshot()
    {
        _ = Task.Run(async () =>
        {
            try
            {
                using var doc = JsonDocument.Parse(await File.ReadAllTextAsync(SnapshotFile));
                var root = doc.RootElement;
                if (root.TryGetProperty("version", out var v) && v.GetInt32() > SnapshotVersion)
                {
                    return; // 新版本壳写的新格式：旧壳不敢猜，放弃（内核基线马上会到）
                }
                var workspaces = new List<WorkspaceVm>();
                if (root.TryGetProperty("workspaces", out var ws) && ws.ValueKind == JsonValueKind.Array)
                {
                    foreach (var w in ws.EnumerateArray())
                    {
                        var ids = new List<string>();
                        if (w.TryGetProperty("sessionIds", out var sids) && sids.ValueKind == JsonValueKind.Array)
                        {
                            foreach (var s in sids.EnumerateArray())
                            {
                                var id = s.GetString();
                                if (!string.IsNullOrEmpty(id))
                                {
                                    ids.Add(id);
                                }
                            }
                        }
                        workspaces.Add(new WorkspaceVm
                        {
                            WorkspaceId = w.TryGetProperty("workspaceId", out var wid) ? wid.GetString() ?? "" : "",
                            Title = w.TryGetProperty("title", out var t) ? t.GetString() ?? "" : "",
                            Path = w.TryGetProperty("path", out var p) ? p.GetString() ?? "" : "",
                            SessionIds = ids,
                        });
                    }
                }
                var archived = new List<string>();
                if (root.TryGetProperty("archived", out var ar) && ar.ValueKind == JsonValueKind.Array)
                {
                    foreach (var s in ar.EnumerateArray())
                    {
                        var id = s.GetString();
                        if (!string.IsNullOrEmpty(id))
                        {
                            archived.Add(id);
                        }
                    }
                }
                var sessions = new List<SessionVm>();
                if (root.TryGetProperty("sessions", out var ss) && ss.ValueKind == JsonValueKind.Array)
                {
                    foreach (var s in ss.EnumerateArray())
                    {
                        var turns = s.TryGetProperty("turns", out var tn) && tn.ValueKind == JsonValueKind.Number ? tn.GetInt32() : 0;
                        sessions.Add(new SessionVm
                        {
                            SessionId = s.TryGetProperty("sessionId", out var sid) ? sid.GetString() ?? "" : "",
                            Title = s.TryGetProperty("title", out var t) ? t.GetString() ?? "" : "",
                            // 副标题从轮数现场重算（快照不存本地化文案，语言切换不吃陈旧串）
                            Subtitle = turns > 0 ? LF("{0} 轮对话", turns) : "",
                            Turns = turns,
                            Cwd = s.TryGetProperty("cwd", out var cwd) ? cwd.GetString() ?? "" : "",
                            UpdatedAt = s.TryGetProperty("updatedAt", out var ua) && ua.ValueKind == JsonValueKind.Number ? (long)ua.GetDouble() : 0,
                            Blank = s.TryGetProperty("blank", out var b) && b.ValueKind == JsonValueKind.True,
                            Origin = s.TryGetProperty("origin", out var org) && org.ValueKind == JsonValueKind.String ? org.GetString() : null,
                            ParentSessionId = s.TryGetProperty("parentSessionId", out var psid) && psid.ValueKind == JsonValueKind.String ? psid.GetString() : null,
                        });
                    }
                }
                if (workspaces.Count == 0 && sessions.Count == 0)
                {
                    return; // 空快照（首启/文件损坏）无内容可垫，等内核基线
                }
                PostUi(() =>
                {
                    if (_workspaceBaselineArrived)
                    {
                        return; // 内核基线已到：快照过期，整表丢弃
                    }
                    _workspaces.Clear();
                    _workspaces.AddRange(workspaces);
                    _archivedSessions.Clear();
                    _archivedSessions.AddRange(archived);
                    _sessions.Clear();
                    _sessions.AddRange(sessions);
                    RebuildNavMenu();
                });
            }
            catch (Exception)
            {
                // 无快照/损坏/读取失败都不致命：启动本来就以内核基线为准，这里只是垫场
            }
        });
    }

    /// <summary>把当前工作区/会话清单原子落盘（线程池写，不占 UI 帧）。数据落地处调用。</summary>
    private void PersistWorkspaceSnapshot()
    {
        // UI 线程取值（_workspaces/_sessions 无并发保护），序列化与写盘放线程池
        var workspaces = _workspaces.ToArray();
        var archived = _archivedSessions.ToArray();
        var sessions = _sessions.ToArray();
        _ = Task.Run(async () =>
        {
            try
            {
                var wArr = new object[workspaces.Length];
                for (var i = 0; i < workspaces.Length; i++)
                {
                    var w = workspaces[i];
                    wArr[i] = new
                    {
                        workspaceId = w.WorkspaceId,
                        title = w.Title,
                        path = w.Path,
                        sessionIds = w.SessionIds,
                    };
                }
                var sArr = new object[sessions.Length];
                for (var i = 0; i < sessions.Length; i++)
                {
                    var s = sessions[i];
                    sArr[i] = new
                    {
                        sessionId = s.SessionId,
                        title = s.Title,
                        turns = s.Turns,
                        cwd = s.Cwd,
                        updatedAt = s.UpdatedAt,
                        blank = s.Blank,
                        origin = s.Origin,
                        parentSessionId = s.ParentSessionId,
                    };
                }
                var payload = JsonSerializer.Serialize(new
                {
                    version = SnapshotVersion,
                    workspaces = wArr,
                    archived,
                    sessions = sArr,
                });
                var file = SnapshotFile;
                var dir = Path.GetDirectoryName(file);
                if (!string.IsNullOrEmpty(dir))
                {
                    Directory.CreateDirectory(dir);
                }
                var tmp = file + "." + Guid.NewGuid().ToString("N") + ".tmp";
                await File.WriteAllTextAsync(tmp, payload);
                File.Move(tmp, file, overwrite: true);
            }
            catch (Exception)
            {
                // 快照写失败不影响任何功能：下次启动少了垫场数据而已
            }
        });
    }
}
