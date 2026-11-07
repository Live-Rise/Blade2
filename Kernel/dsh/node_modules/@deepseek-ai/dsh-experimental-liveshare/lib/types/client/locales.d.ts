import type { Translate } from '@deepseek-ai/dsh-client-ui-slots';
/** Locale namespace owned by the Liveshare owner-side browser plugin. */
export declare const NS = "liveshare";
/** Simplified-Chinese Liveshare owner copy. */
export declare const zh: {
    readonly 'header.action': "分享";
    readonly 'inputOrigin.sharedSession': "来自会话分享【{subjectId}】";
    readonly 'dialog.title': "分享会话";
    readonly 'dialog.done': "完成";
    readonly 'dialog.shareLink': "分享链接";
    readonly 'dialog.sessionMode': "会话权限";
    readonly 'dialog.sessionMode.view': "仅可查看";
    readonly 'dialog.sessionMode.write': "工作区内修改";
    readonly 'dialog.sessionMode.hint.view': "仅可查看：观看者可以读取分享范围内的文件与会话记录，也能向这个会话发消息；所有工具都在只读沙箱中运行，不能写文件。";
    readonly 'dialog.sessionMode.hint.write': "工作区内修改：观看者在可写沙箱里工作——可以在分享范围内修改文件，写入受可写上限约束；分享范围之外的读写都会被拒绝，你的全局设置、凭据与其它会话仍不开放。";
    readonly 'dialog.allowsFork': "允许观看者分叉会话";
    readonly 'dialog.allowsForkHint': "观看者分叉出的会话与源会话权限相同。";
    readonly 'dialog.sessionMode.notGranted': "此 Harness 未回读会话权限：所有者端版本过旧时请先升级，再重新创建分享。";
    readonly 'dialog.permissions': "分享权限";
    readonly 'dialog.permissions.save': "保存修改";
    readonly 'dialog.permissions.saving': "保存中…";
    readonly 'dialog.permissions.saved': "已保存分享权限与插件端点。";
    readonly 'dialog.permissions.failed': "分享权限未保存，请重试。";
    readonly 'dialog.permissions.partial': "会话权限已更新，但插件端点未保存，请重试。";
    readonly 'dialog.permissions.modeUnsupported': "此 Harness 未回读会话权限，不能在这里修改。";
    readonly 'dialog.permissions.unsaved': "有未保存的改动。";
    readonly 'dialog.permissions.discard': "有未保存的改动；再次点击「完成」将丢弃它们。";
    readonly 'dialog.capabilities': "插件端点";
    readonly 'dialog.capabilities.description': "对这个分享统一选一个档位；档位在保存时展开成插件当前声明的全部端点，之后新增的端点不会自动开放。";
    readonly 'dialog.capabilities.none': "不向观看者开放";
    readonly 'dialog.capabilities.count': "已向观看者开放 {count} 个端点";
    readonly 'dialog.capabilities.tier.none': "不可用";
    readonly 'dialog.capabilities.tier.read': "可读";
    readonly 'dialog.capabilities.tier.write': "可写";
    readonly 'dialog.capabilities.summary.read': "只读 · {count} 个端点";
    readonly 'dialog.capabilities.summary.write': "可写 · {readable} 只读 + {writable} 可写";
    readonly 'dialog.capabilities.stale': "当前记录不是整档选择；重新保存会按所选档位覆盖。";
    readonly 'dialog.capabilities.noReadable': "没有插件提供只读端点，因此不能选择「可读」。";
    readonly 'dialog.capabilities.noWritable': "没有插件提供可写端点，因此不能选择「可写」。";
    readonly 'dialog.capabilities.writeHint': "可写端点在被分享会话允许写入时才生效，且在可写分享中具有破坏性。";
    readonly 'dialog.capabilities.empty': "当前没有插件声明可分享端点。";
    readonly 'dialog.capabilities.createFailed': "分享已创建，但插件端点未保存；请到分享权限中重试。";
    readonly 'dialog.linkHelp': "关闭此面板不会停止分享。";
    readonly 'dialog.copyFailed': "未能复制链接。请选中分享链接后手动复制。";
    readonly 'dialog.diagnostics': "连接诊断";
    readonly 'dialog.accessUnavailable': "此分享的访问范围详情暂不可用，已有权限不变。";
    readonly 'dialog.selectedCount': "分享服务已确认 {count} 位所选人员。";
    readonly 'dialog.deploymentAccess': "部署还配置了 {count} 个可访问身份；访问范围不只由此处选择决定。";
    readonly 'dialog.selectedRetired': "这是旧版“指定同事可看”的分享：范围保持不变，新建分享只能选择上面两种方式。";
    readonly 'dialog.description': "创建一个只读 Liveshare 链接：{title}";
    readonly 'dialog.relay': "分享服务地址";
    readonly 'dialog.relaySaved': "已保存，新的分享将使用该地址。";
    readonly 'dialog.relayInvalid': "请输入以 http:// 或 https:// 开头的完整地址。";
    readonly 'dialog.relayCurrent': "当前生效地址";
    readonly 'dialog.relayUnset': "未设置地址";
    readonly 'dialog.relaySource.unset': "未设置";
    readonly 'dialog.relaySource.saved': "已保存地址";
    readonly 'dialog.relayVerified': "已在该地址验证";
    readonly 'dialog.relayUnverified': "该地址未验证";
    readonly 'dialog.relayReverify': "地址已更换，请在新地址重新登录。";
    readonly 'dialog.relayUnsavedHint': "这个地址尚未保存；保存后才会生效。";
    readonly 'dialog.identity.expired': "登录已过期，请重新登录后再创建分享。";
    readonly 'dialog.identity.unverified': "尚未登录，无法创建分享。";
    readonly 'dialog.identity.relayChanged': "当前登录属于另一个 Relay 地址，请在本地址重新登录。";
    readonly 'dialog.identity.unknown': "登录状态暂时不可用，请稍后重试；登录前不会创建分享。";
    readonly 'dialog.identity.retry': "重试";
    readonly 'dialog.identity.verify': "登录";
    readonly 'dialog.identity.reverify': "重新登录";
    readonly 'dialog.identity.verifyBlocked': "请先保存地址并确认可连接，再登录。";
    readonly 'dialog.identity.loginOpening': "正在发起登录…";
    readonly 'dialog.identity.loginWaiting': "请在打开的窗口中完成登录。";
    readonly 'dialog.identity.loginVerifying': "正在确认登录结果…";
    readonly 'dialog.identity.loginDone': "登录已完成，正在读取状态。";
    readonly 'dialog.identity.loginCancelled': "已取消登录，未保存任何凭据。";
    readonly 'dialog.identity.loginExpired': "这次登录已过期，请重新发起。";
    readonly 'dialog.identity.loginMismatch': "登录回答不属于本次请求，已忽略。";
    readonly 'dialog.identity.loginReplayed': "这次登录已被使用过，请重新发起。";
    readonly 'dialog.identity.loginUnavailable': "无法发起登录：Relay 不可用或该地址尚不支持。";
    readonly 'dialog.identity.loginBlocked': "无法打开登录窗口（可能被浏览器拦截）。请允许弹窗后重试。";
    readonly 'dialog.identity.loginFailed': "登录未完成，请重试。";
    readonly 'dialog.identity.expires': "有效期至 {time}";
    readonly 'dialog.identity.renewed': "已在有效授权内自动续期。";
    readonly 'dialog.identity.relayUnsupported': "当前 Relay 地址还不支持身份认证，请先升级 Relay 后再创建分享；不会创建匿名分享。";
    readonly 'dialog.expiry': "有效期";
    readonly 'dialog.expiry.never': "永不过期";
    readonly 'dialog.expiry.hour': "1 小时";
    readonly 'dialog.expiry.day': "24 小时";
    readonly 'dialog.expiry.week': "7 天";
    readonly 'dialog.expiry.until': "至 {time}";
    readonly 'dialog.linkSharing': "链接分享";
    readonly 'dialog.help': "只分享这个会话；对方能做什么由“会话权限”决定，随时可以在“管理分享”里停止。";
    readonly 'dialog.mode.open': "组织内任何持链接的人可查看";
    readonly 'dialog.mode.selected': "指定同事可看";
    readonly 'dialog.expiry.clear': "改为永不过期";
    readonly 'dialog.create': "创建分享";
    readonly 'dialog.creating': "创建中…";
    readonly 'dialog.cancel': "取消";
    readonly 'dialog.copy': "复制链接";
    readonly 'dialog.copied': "已复制";
    readonly 'dialog.manage': "管理分享";
    readonly 'dialog.hostUnavailable': "无法连接 DSH Liveshare 分享服务。";
    readonly 'dialog.testing': "正在连接分享服务…";
    readonly 'dialog.testOk': "Relay 连接正常";
    readonly 'dialog.testWarn': "Relay HTTP 可访问，但注册未完成（服务端未给出原因）；仍可创建，网关会自动重试。";
    readonly 'dialog.testFail': "Relay 连接失败（服务端未给出原因）。";
    readonly 'dialog.addressHint': "请先保存分享服务地址。";
    readonly 'dialog.connectivityChecking': "正在测试分享服务地址连通性…";
    readonly 'dialog.permissionChecking': "正在校验本机能否在该地址发布分享…";
    readonly 'dialog.permissionOk': "已登录，本机可以发布分享。";
    readonly 'dialog.permissionPending': "需要先校验权限，才能创建分享。";
    readonly 'dialog.permissionFailed': "权限校验未完成（Host 未给出原因）。";
    readonly 'dialog.outgoing': "我分享出去的会话";
    readonly 'dialog.noOutgoing': "还没有分享出去的会话";
    readonly 'dialog.share.online': "在线";
    readonly 'dialog.share.offline': "离线";
    readonly 'dialog.open': "打开";
    readonly 'dialog.stop': "停止分享";
    readonly 'dialog.stopFailed': "停止分享失败，请稍后重试。";
    readonly 'manage.title': "分享管理";
    readonly 'manage.open': "管理";
    readonly 'manage.cancelShare': "取消分享";
    readonly 'manage.description': "查看每个分享的在线状态与访问范围。";
    readonly 'manage.stop.title': "停止分享？";
    readonly 'manage.stop.description': "已发布的链接会立即失效，正在查看的人将无法继续访问。";
    readonly 'manage.stop.acknowledge': "我明白已发布的链接会立即失效";
    readonly 'manage.identityExpired': "登录凭据已过期：只影响新建分享。下列已发布分享不受影响，链接仍可复制、仍可管理。";
    readonly 'manage.loading': "正在加载分享…";
    readonly 'manage.empty': "还没有分享出去的会话。";
    readonly 'manage.unavailable': "无法获取分享列表，显示的是最后一次已知状态。";
    readonly 'manage.refresh': "重新加载";
    readonly 'manage.history': "历史记录";
    readonly 'manage.historyLoading': "正在加载历史记录…";
    readonly 'manage.historyFailed': "历史记录读取失败，请重试。";
    readonly 'manage.expiresAt': "{time} 过期";
    readonly 'manage.expiredAt': "已于 {time} 过期";
    readonly 'manage.sessionMode': "会话权限";
    readonly 'manage.sessionModeSwitch': "设置「{title}」的会话权限";
    readonly 'manage.allowsForkSwitch': "设置「{title}」是否允许观看者分叉会话";
    readonly 'manage.sessionModeUnsupported': "此 Harness 未回读会话权限，无法在这里设置。";
    readonly 'manage.sessionModeFailed': "会话权限未保存，请重试。";
    readonly 'manage.capabilities': "插件端点";
    readonly 'manage.audit.shareCreate': "创建分享";
    readonly 'manage.audit.accessRequest': "收到查看申请";
    readonly 'manage.audit.accessApproved': "审批通过";
    readonly 'manage.audit.accessGrant': "授予查看权限";
    readonly 'manage.audit.accessRevoke': "撤销查看权限";
    readonly 'manage.audit.shareStop': "停止分享";
    readonly 'manage.done': "完成";
    readonly 'viewer.title': "Liveshare 只读会话";
    readonly 'viewer.sharedBy': "由 {owner} 分享 · {title}";
    readonly 'viewer.readonlyNote': "仅可查看：可以驱动这个会话，但只能在分享范围内读取文件，不能修改内容。";
    readonly 'viewer.writeUnavailableNote': "分享者已授予写入，但这个会话当前无法被驱动；就绪后即可发送消息。";
    readonly 'viewer.model.title': "这个会话正在使用的模型，由分享者决定";
    readonly 'viewer.model.locked': "观看者不能切换模型";
    readonly 'viewer.permission.title': "这份分享的会话权限，由分享者决定";
    readonly 'viewer.permission.locked': "观看者不能自行更改";
    readonly 'viewer.permission.unconfirmed': "尚未确认";
    readonly 'viewer.transcriptLabel': "会话内容（只读）";
    readonly 'viewer.role.user': "用户";
    readonly 'viewer.role.assistant': "助手";
    readonly 'viewer.role.tool': "工具";
    readonly 'viewer.loading': "正在连接会话…";
    readonly 'viewer.empty': "这个会话还没有可显示的内容。";
    readonly 'viewer.reconnecting': "连接中断，正在重试…";
    readonly 'viewer.offline': "分享暂时离线，稍后会自动重连。";
    readonly 'viewer.retry': "立即重试";
    readonly 'viewer.expired': "该分享已过期，不再提供访问。请联系分享者重新分享。";
    readonly 'viewer.revoked': "分享者已停止或撤销你的访问。";
    readonly 'viewer.reauthenticate': "登录状态已失效，请重新登录。";
    readonly 'viewer.reauthenticateAction': "重新登录";
    readonly 'viewer.status.connecting': "正在连接分享…";
    readonly 'viewer.status.invalid': "链接无效，或这条分享已被删除。";
    readonly 'viewer.status.invalidHint': "请检查链接是否完整；也可以找分享者要一条新链接。";
    readonly 'viewer.status.offline': "分享者当前不在线，会自动重连。";
    readonly 'viewer.status.offlineHint': "稍等片刻或点「重试」；长时间仍然离线请联系分享者。";
    readonly 'viewer.status.stopped': "分享者已停止这条分享。";
    readonly 'viewer.status.stoppedHint': "如需继续访问，请联系分享者重新开启。";
    readonly 'viewer.status.expired': "这条分享已过期。";
    readonly 'viewer.status.expiredHint': "请联系分享者重新分享。";
    readonly 'viewer.status.reauthenticate': "登录已过期。";
    readonly 'viewer.status.unavailable': "服务暂时不可用，正在重试。";
    readonly 'viewer.status.unavailableHint': "请稍后重试；持续不可用请联系分享者或稍后再来。";
    readonly 'viewer.action.reload': "刷新";
    readonly 'viewer.action.retry': "重试";
    readonly 'viewer.action.signIn': "重新登录";
    readonly 'viewer.action.directory': "回到分享列表";
    readonly 'viewer.expiresAt': "有效期至 {time}";
    readonly 'viewer.picker.ownerUnknown': "未知分享者";
    readonly 'viewer.picker.version': "DSH";
    readonly 'viewer.picker.groupCount': "{count} 个分享";
    readonly 'viewer.picker.online': "在线";
    readonly 'viewer.picker.offline': "离线";
    readonly 'viewer.picker.subjectLabel': "主体 …{label}";
    readonly 'viewer.picker.historical': "历史未验证";
    readonly 'viewer.picker.historicalNote': "这些分享创建于身份验证之前，访问范围保持不变。";
    readonly 'viewer.brand': "LiveShare";
    readonly 'viewer.home': "回到 LiveShare 首页";
    readonly 'viewer.sharedByShort': "由 {owner} 分享";
    readonly 'viewer.relayVersion': "relay {release}";
    readonly 'viewer.refused': "liveshare：{subject} 被拒绝（{status}）";
    readonly 'viewer.relayAnswered': "liveshare：中继返回 {status}";
    readonly 'viewer.relaySilent': "liveshare：中继未对 {subject} 返回内容";
    readonly 'viewer.unreachable': "liveshare：{subject} 未能送达中继，正在重试";
    readonly 'viewer.noAnswer': "无应答";
    readonly 'viewer.row.fork': "分叉会话";
    readonly 'viewer.row.forkSource': "来自「{title}」的分支";
    readonly 'viewer.row.branchRefused': "未能完成分支：{reason}";
    readonly 'viewer.sharePath.unavailable': "此页面无法打开工作区文件：分享提供的是文件路径，不是文件内容。";
    readonly 'viewer.reason.fork-not-permitted': "分享者未允许分支此会话";
    readonly 'viewer.reason.fork-not-implemented': "此部署尚不支持分支";
    readonly 'viewer.reason.write-not-implemented': "此部署尚不支持写入";
    readonly 'viewer.reason.share-not-found': "该分享已不存在";
    readonly 'viewer.reason.share-stopped': "分享已停止";
    readonly 'viewer.reason.share-expired': "分享已过期";
    readonly 'viewer.reason.source-session-mismatch': "分支请求指向的会话不是该分享的会话";
    readonly 'viewer.reason.fork-operation-conflict': "这次分支请求指定的前缀与之前那次不同，未再新建会话";
    readonly 'viewer.reason.fork-enforcement-failed': "分支会话已创建，但沙箱限制安装失败，无法使用";
    readonly 'viewer.reason.mode-view': "分享者已把此分享改为仅可查看";
    readonly 'viewer.reason.unconfirmed': "无法确认该分享的当前状态，确认前保持只读";
    readonly 'viewer.reason.fork-viewer-unknown': "中继未确认你的身份，无法完成分支";
    readonly 'viewer.reason.fork-unknown-child': "此会话不是分享产生的分支";
    readonly 'viewer.reason.fork-viewer-mismatch': "该分支属于其他观看者";
    readonly 'viewer.reason.fork-source-revoked': "该分支来源的分享已不可用";
    readonly 'viewer.reason.fork-ceiling-uninstalled': "此 Host 尚未安装只读执行上限";
    readonly 'viewer.reason.fork-prepare-failed': "Host 未能准备好此分支，本次操作没有执行";
    readonly 'viewer.reason.share-not-writable': "分享者已不允许写入此会话";
    readonly 'viewer.reason.source-write-unavailable': "此 Host 未能限制共享会话的执行，因此无法驱动它";
    readonly 'viewer.reason.prompt-identity-missing': "这次发送没有可供署名的请求标识，Host 没有执行";
    readonly 'viewer.reason.viewer-not-authorized': "你的账号已无权访问该分享";
    readonly 'viewer.reason.liveshare/read-only': "这个会话不在你持有的分享范围内，未写入";
};
/** English Liveshare owner copy. */
export declare const en: Record<keyof typeof zh, string>;
/** Japanese Liveshare owner copy. */
export declare const ja: Record<keyof typeof zh, string>;
/** Spanish Liveshare owner copy. */
export declare const es: Record<keyof typeof zh, string>;
/** French Liveshare owner copy. */
export declare const fr: Record<keyof typeof zh, string>;
/** German Liveshare owner copy. */
export declare const de: Record<keyof typeof zh, string>;
/** Russian Liveshare owner copy. */
export declare const ru: Record<keyof typeof zh, string>;
/** Brazilian-Portuguese Liveshare owner copy. */
export declare const pt: Record<keyof typeof zh, string>;
/** Stable locale keys consumed by the share dialog and the viewer surfaces. */
export type LiveshareKey = keyof typeof zh;
/**
 * Translate one dictionary key in the active language.
 *
 * The viewer surfaces read their copy through the client locale seat — the same dictionary the owner's
 * own dialogs register — so a reader's chosen language, not the browser alone, decides what the page
 * states. Passing the seat explicitly keeps every helper below independent of where it is rendered.
 */
export type ViewerTranslate = Translate<LiveshareKey>;
/**
 * Copy for one stable reason code the owner host or the Relay closed an ability with.
 *
 * The Relay's capability answer carries one of its own stable codes — `fork-not-permitted`,
 * `share-stopped`, `share-expired`, `viewer-not-authorized` — and a code this build does not know is
 * stated as the code itself: the page never invents copy for a reason it was not told, and never
 * hides one it was.
 * @param t - Translate seat bound to this package's namespace.
 * @param code - Stable reason code carried by the share's abilities.
 * @returns The localized reason.
 */
export declare function viewerReasonText(t: ViewerTranslate, code: string): string;
/**
 * What this page states for one refusal it knows, and nothing for one it does not.
 *
 * The reason is this page's own words for the stable code, with that code beside it so a reader can
 * quote what was refused. The code is the whole of what this page can vouch for: the Host's own
 * message for a refusal is composed from whatever its service raised, and may name a path, a
 * credential, or another reader's Session, so this page never states any of it — a reason a reader has
 * to read the Host's own words for is the owner's to look up, not this page's to spread. A code this
 * build does not know stays unstated here, and the surface then keeps the Host's message with its code.
 * @param t - Translate seat bound to this package's namespace.
 * @param code - Stable code carried by the refusal.
 * @returns The stated refusal, or undefined when this page does not know that code.
 */
export declare function viewerRefusalText(t: ViewerTranslate, code: string): string | undefined;
//# sourceMappingURL=locales.d.ts.map