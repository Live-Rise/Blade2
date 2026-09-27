use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::i18n::Catalog;

const MARKER: &str = "dsh web: ";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(90);

/// RPC 单次请求的读超时。**对齐主干的 120 秒**：主干那颗 `HttpClient` 全局唯一，
/// `CallAsync` / `CallOkAsync` / 上传走的都是它的 `Timeout = TimeSpan.FromSeconds(120)`
/// （`Dsh/DshRpcClient.cs:67`）。分叉原先这里是 20 秒，于是长任务（一次要跑一分多钟的
/// `session/send`、慢工具的 `tool/execute`）在主干上等得到回包、在分叉上会被本地掐死——
/// 这不是「更保守的设计」，是行为分叉：掐早了前端只会看到一条假的失败，内核那边还在跑。
///
/// 主干**没有**分档超时：`DshRpcClient` 只有这一发 120 s。仓库里另外两颗同名量级的常数
/// 都跟 RPC 无关（`DshPetStore.cs:37` 贴纸包下载 120 s、`DshPluginBootstrap.cs:36` 插件
/// 安装 300 s、`DshRpcClient.cs:51` 重连退避上限 30 s、`:678` 等 $events ready 10 s），
/// 不抄进这里。分叉 `mux.rs` 的 `READ_TICK`（120 ms）是轮询节拍、不是超时，主干无对应物。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct Launch {
    pub exe: PathBuf,
    pub args: Vec<String>,
    /// #84-A：`args` 要不要**原样**交给进程（回退级那串 cmd.exe 嵌套引号）。见 [`SpawnPlan::args_verbatim`]。
    pub args_verbatim: bool,
    /// #84-A 保真陷阱①：主干 `IsBundled` = 内置那两颗 `File.Exists` 的与，**与本次实际走了哪一级无关**。
    /// 主干的失败文案二选一只吃这颗（`MainWindow.xaml.cs:2816`）。刻意**不许**拿"实际级别"代替它。
    pub is_bundled: bool,
    pub dsh_home: Option<PathBuf>,
    pub path_prepend: Option<PathBuf>,
    pub working_dir: Option<PathBuf>,
}

// =============== 缺口 #84-A：两级 launcher（主干 `DshKernelHost.cs:48-64 ResolveLauncher()`）===============
//
// 主干的解析器只有这一个函数，形状是「两级候选 + 一个 null」：**没有第三级、没有环境变量开关、
// 没有配置项参与、没有版本匹配**（规格 §1.1）。分叉此前只有第 1 级，第 2 级 `%APPDATA%\npm\dsh.cmd`
// 整块缺失（§1.6 表第 2 行）。这里补齐，并刻意保留主干两处**不准**的地方（照抄，不"改对"）：
//   ① 失败文案只吃 `IsBundled`（那两个 `File.Exists`），**不吃本次实际走了哪一级**（§1.7-1）——
//      于是"内置缺一半、dsh.cmd 在但起不来"的开发形态下，加载卡说的仍是「未找到内置内核；请安装
//      npm 版 dsh…」这句措辞并不准确的话。见 [`is_bundled`]，函数注释里钉死了这条。
//   ② 回退形态下主干的孤儿清扫是**关掉**的（§1.7-3：`DshKernelHost.cs:268` 取不到 `BundledNode`
//      直接 `return 0`）。分叉沿用的 `procguard::sweep_orphan_kernels()` 以 [`bundled_kernel_exe`]
//      为靶、拿不到就一个都不杀，**天然同为 no-op ⇒ 这里一行代码都不加，加反而是偏离**。

/// argv 尾部：主干 `:54`（内置）与 `:61`（回退）**共用同一串、一字不差**（§1.3）。
/// 规格 §1.3 把它数成"三个 token"，实际是 `web` / `--no-open` / `--port` / `0` **四个**；
/// 分叉既有实现（老 `from_env`）发的也是这四个 —— 这里按**代码事实**取四，不跟规格的口误。
const KERNEL_ARG_TAIL: [&str; 4] = ["web", "--no-open", "--port", "0"];

/// 回退级起的进程名，主干 `:61` 写的就是字面量 `"cmd.exe"`（相对名，交给内核的 PATH 解析）。
const CMD_EXE: &str = "cmd.exe";

/// 主干 `ResolveLauncher()` 的三态结果（`Option<(fileName, arguments)>` 的分叉形状）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launcher {
    /// 第 1 级：打包内置内核。**两颗都在**才命中；任一缺失整级跳过（主干是 `&&`，不半用）。
    Bundled { node: PathBuf, bin_js: PathBuf },
    /// 第 2 级：系统 npm 版 `dsh.cmd`。只判存在，**不判可执行、不判版本、不 `where dsh`**。
    NpmDshCmd(PathBuf),
    /// 两级全不命中：主干 `return null` ⇒ `StartAsync` 立刻返回，**一个进程都不 spawn**。
    Missing,
}

/// 一次 spawn 的 argv 形状（§3.2）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnPlan {
    pub exe: PathBuf,
    pub args: Vec<String>,
    /// `true` ⇒ `args` 只有一颗、且必须**原样**交给进程（走 `CommandExt::raw_arg`）。
    /// 回退级那串嵌套引号 `/c ""<dsh>" web --no-open --port 0"` 只有在原样传递时才是主干的形状；
    /// 若按 argv 列表交给 `Command::args`，std 会把内嵌的 `"` 转义成 `\"`，而 **cmd.exe 不认识
    /// 反斜杠转义引号**，路径含空格时就会拆错参数。主干 .NET 走的是 `Arguments` 原样拼接，无此问题。
    pub args_verbatim: bool,
}

/// 纯决策（§3.1）：三颗「在 / 不在」→ 选哪一级。规则只有两条：
/// `node && bin_js ⇒ Bundled`；否则 `npm_dsh_cmd ⇒ NpmDshCmd`；否则 `Missing`。
pub fn decide_launcher(
    bundled_node: Option<PathBuf>,
    bundled_bin_js: Option<PathBuf>,
    npm_dsh_cmd: Option<PathBuf>,
) -> Launcher {
    match (bundled_node, bundled_bin_js) {
        (Some(node), Some(bin_js)) => Launcher::Bundled { node, bin_js },
        // 只有一颗内置文件在 = 整级跳过（**不许**半用：主干是两个 `File.Exists` 的与）。
        _ => npm_dsh_cmd.map_or(Launcher::Missing, Launcher::NpmDshCmd),
    }
}

/// 陷阱①的本体：主干 `IsBundled`（`DshKernelHost.cs:42`）= `BundledNode is not null && BundledBinJs is not null`，
/// 也就是**那两个 `File.Exists` 的与**，每次现算、与"实际选中了哪一级"无关。失败文案二选一只吃这颗。
///
/// 刻意做成吃探针结果、不吃 [`Launcher`]：拿"实际级别"出文案是**看起来更合理但和主干不一致**的写法
/// （§1.7-1 点名的那条），所以这里连 `Launcher` 都不给当参数，防手滑。
pub fn is_bundled(bundled_node: &Option<PathBuf>, bundled_bin_js: &Option<PathBuf>) -> bool {
    bundled_node.is_some() && bundled_bin_js.is_some()
}

/// 第 2 级的路径（纯，§3.1 点名必须是 Roaming）：
/// `SpecialFolder.ApplicationData` = **`%APPDATA%`（Roaming）**，不是 `%LOCALAPPDATA%`——
/// 两者只差一个词、拼错就永远找不到，故单独钉一条测试。
pub fn npm_dsh_cmd_path(app_data: &Path) -> PathBuf {
    app_data.join("npm").join("dsh.cmd")
}

/// 纯（§3.2）：出这一级的 exe + argv。
/// 内置级走**正常 argv 列表**（std 的转义等价于主干 `"{binJs}" …` 那对外层双引号，含空格路径也对）；
/// 回退级给一颗**原样串**（`args_verbatim = true`）；[`Launcher::Missing`] ⇒ `None`（主干的 `null`）。
pub fn launcher_argv(launcher: &Launcher) -> Option<SpawnPlan> {
    match launcher {
        Launcher::Bundled { node, bin_js } => {
            let mut args = vec![bin_js.display().to_string()];
            args.extend(KERNEL_ARG_TAIL.iter().map(|arg| arg.to_string()));
            Some(SpawnPlan { exe: node.clone(), args, args_verbatim: false })
        }
        Launcher::NpmDshCmd(dsh_cmd) => Some(SpawnPlan {
            exe: PathBuf::from(CMD_EXE),
            args: vec![npm_dsh_cmd_arguments(dsh_cmd)],
            args_verbatim: true,
        }),
        Launcher::Missing => None,
    }
}

/// 主干 `DshKernelHost.cs:61` 那串 `$"/c \"\"{dshCmd}\" web --no-open --port 0\""` 的分叉等价物。
/// 逐字符对应：`/c ` + 外层一对 `"` + 内层一对裹住路径 + 尾部四个 token + 收外层 `"`。
pub fn npm_dsh_cmd_arguments(dsh_cmd: &Path) -> String {
    format!("/c \"\"{}\" {}\"", dsh_cmd.display(), KERNEL_ARG_TAIL.join(" "))
}

/// 主干 `ResolveLauncher()` 返回值的**字符串形状**（`fileName + " " + arguments`，即 .NET 交给
/// `CreateProcessW` 的 `lpCommandLine`）。只为离线逐字节比对而存在（`tmp/fg2-cmdline.txt`），
/// **不参与 spawn 路径**，免得有人拿它去 `Command::new` 再转义一遍。
pub fn launcher_command_line(launcher: &Launcher) -> Option<String> {
    let plan = launcher_argv(launcher)?;
    if plan.args_verbatim {
        Some(format!("{} {}", plan.exe.display(), plan.args.join(" ")))
    } else {
        // 内置级主干是 `nodePath + " " + "\"binJs\" web --no-open --port 0"`。
        let mut args = vec![format!("\"{}\"", plan.args[0])];
        args.extend(plan.args[1..].iter().cloned());
        Some(format!("{} {}", plan.exe.display(), args.join(" ")))
    }
}

/// 内置两颗的存在性探针（吃目录、不吃 env，好让「env 指歪了」也能离线钉）。
fn bundled_parts_in(kernel_dir: Option<&Path>) -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(dir) = kernel_dir else {
        return (None, None);
    };
    let node = dir.join("node.exe");
    let bin_js = dir.join("dsh").join("lib").join("bin.js");
    (node.is_file().then_some(node), bin_js.is_file().then_some(bin_js))
}

/// 真机：内置目录 = `BLADE2_KERNEL_DIR` 优先，否则从自身 exe 往上找（分叉既有口径，§1.6 已注明
/// 这一条**比主干宽**，是既有分叉、不在本次范围内，原样留着）。
fn bundled_kernel_dir() -> Option<PathBuf> {
    match std::env::var_os("BLADE2_KERNEL_DIR") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => std::env::current_exe().ok().and_then(|exe| kernel_dir_from(&exe)),
    }
}

/// 真机：第 2 级探针。`APPDATA` 就是 `SpecialFolder.ApplicationData` 的环境变量形态
/// （与主干 `GetFolderPath(ApplicationData)` 同颗）。只判存在，别的都不判。
fn probe_npm_dsh_cmd() -> Option<PathBuf> {
    let app_data = std::env::var_os("APPDATA")?;
    let candidate = npm_dsh_cmd_path(Path::new(&app_data));
    candidate.is_file().then_some(candidate)
}

/// 纯（§3.3）：交给内核的环境变量。主干 `DshKernelHost.cs:90-94` 是**两级共用的一份、无分支**：
/// ① `DSH_HOME = dshHome`（主干无条件给）；
/// ② PATH 前置 `<install>\Kernel\bin`，但 `DshPluginBootstrap.PrependPath`（`:591-594`）在
///    **该目录不存在时直接 return 不注入**，且原 PATH 为空（含空串）时只写新目录。
/// 分叉此前没有那个早退：内置目录缺失时会把一条不存在的路径塞进内核 PATH。这里对齐。
/// `prepend_exists` 把存在性判定从 fs 里拎出来，好让「目录在/不在 × PATH 有/无」四例都能离线钉。
pub fn build_env(
    dsh_home: Option<&Path>,
    path_prepend: Option<&Path>,
    prepend_exists: bool,
    path_env: Option<&str>,
) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(home) = dsh_home {
        env.push(("DSH_HOME".to_string(), home.display().to_string()));
    }
    if let Some(bin) = path_prepend.filter(|_| prepend_exists) {
        let value = match path_env.filter(|current| !current.is_empty()) {
            Some(current) => format!("{};{current}", bin.display()),
            None => bin.display().to_string(),
        };
        env.push(("PATH".to_string(), value));
    }
    env
}

// =============== 缺口 #84-B：Code2 → Blade2 数据目录搬迁（主干 `MainWindow.xaml.cs:8571-8584`）===============
//
// 主干那颗 `DataHome` getter 在做品牌更名的首次访问搬迁：`if (!Directory.Exists(home) &&
// Directory.Exists(legacy)) { try { Directory.Move(legacy, home); } catch {} }`，然后 `return home`。
// 三条必须照抄的语义（§2.2 / §2.3）：
//   · **幂等标记 = 新目录存在本身**，没有 flag 文件、没有版本号、不记日志；
//   · **新旧都在 = 保新、旧目录一个字不动**：不合并、不比时间戳、不删源（宁可留一个孤儿 `Code2\`，
//     也不覆盖已有新数据）；
//   · **搬迁失败就静默按原路径继续**（`catch {}` 连 `Debug.WriteLine` 都没有），本次启动用全新
//     空 `Blade2\`，旧数据仍在盘上，下次启动会**再试一次**。

/// 新数据家目录名（`%LOCALAPPDATA%` 下）。
pub const DATA_HOME_DIR: &str = "Blade2";

/// 老数据家目录名（品牌更名前）。
pub const LEGACY_DATA_HOME_DIR: &str = "Code2";

/// [`plan_data_home`] 的结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataHomePlan {
    /// 一个字都不动（新目录已在 / 老目录不在 / 两者都不在）。
    Keep,
    /// 把 `legacy` 整体改名成 `home`（同卷 = 目录改名，原子，不拷贝、不留副本）。
    Move { legacy: PathBuf, home: PathBuf },
}

/// 纯（§3.5）：主干那个 `if` 条件本体。规则**只有一条** `(!home_exists && legacy_exists) ⇒ Move`。
/// 四条组合里只有第一条动盘；尤其注意「两者都在」是 `Keep` —— 这就是"保新不合并"。
pub fn plan_data_home(
    legacy: &Path,
    home: &Path,
    legacy_exists: bool,
    home_exists: bool,
) -> DataHomePlan {
    if !home_exists && legacy_exists {
        DataHomePlan::Move { legacy: legacy.to_path_buf(), home: home.to_path_buf() }
    } else {
        DataHomePlan::Keep
    }
}

/// 执行侧：`std::fs::rename` + 吞错，逐句对应主干 `Directory.Move` + `catch (Exception) { }`。
/// **不写 flag 文件、不记日志、不合并目录、失败不删源**。返回数据家路径 —— 搬不搬都得给一个
/// 能继续用的路径，这正是主干 getter 的行为（搬迁失败也照样 `return home`，启动不受影响）。
///
/// 真机安全：本函数的 root 由调用方给，测试一律传 `std::env::temp_dir()` 下自建的沙箱，
/// **绝不**拿 `%LOCALAPPDATA%` 当参数去跑测试。
pub fn migrate_data_home(root: &Path) -> PathBuf {
    let home = root.join(DATA_HOME_DIR);
    let legacy = root.join(LEGACY_DATA_HOME_DIR);
    if let DataHomePlan::Move { legacy, home } =
        plan_data_home(&legacy, &home, legacy.is_dir(), home.is_dir())
    {
        let _ = std::fs::rename(&legacy, &home);
    }
    home
}

/// `LOCALAPPDATA` 拿不到时的兜底根（分叉既有行为：退化成 `.` ⇒ 数据家变 `./Blade2`。
/// 主干走 `GetFolderPath(LocalApplicationData)`，**没有** env 口也没有相对路径兜底 —— 这条差异
/// 是既有分叉、不在 #84 范围内，原样保留）。
pub fn data_home_root(local_app_data: Option<&Path>) -> PathBuf {
    local_app_data.unwrap_or_else(|| Path::new(".")).to_path_buf()
}

/// 数据家解析的**顺序**本体（可注入、可离线钉）：先无条件跑搬迁，再看 `BLADE2_DSH_HOME`。
/// 规格 §3.5 点名：那颗自测 env「保留但排在搬迁**之后**，别让它绕过搬迁」—— 因为主干 getter
/// 是无条件副作用，env 只是分叉加的自测口，不该让它改变"盘上有没有搬"这件事。
pub fn resolve_dsh_home_with(root: &Path, env_override: Option<&Path>) -> PathBuf {
    let home = migrate_data_home(root);
    env_override.map_or(home, |override_home| override_home.to_path_buf())
}

/// 真机入口：替换老 `from_env` 里那段 `LOCALAPPDATA + join("Blade2")` 的内联拼接。
/// 主干的对应物是 `MainWindow.xaml.cs:8571` 的 `DataHome` getter（谁先碰谁触发搬迁）。
pub fn resolve_dsh_home() -> PathBuf {
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let env_override = std::env::var_os("BLADE2_DSH_HOME").map(PathBuf::from);
    let root = data_home_root(local_app_data.as_deref());
    resolve_dsh_home_with(&root, env_override.as_deref())
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

/// 本安装**自带**的那颗内核可执行文件 = 主干 `DshKernelHost.BundledNode`（`DshKernelHost.cs:27-30`，
/// `File.Exists(<install>\Kernel\node.exe)` 才给值）。孤儿清扫的判定目标**只**认这一颗：
/// 走 `kernel_dir_from(current_exe())`，刻意不吃 `BLADE2_KERNEL_DIR` / `BLADE2_KERNEL_EXE`
/// 那两颗自测 env —— env 能把分叉起的内核换成假实现，却不能改变「该杀谁」：让 env 决定目标，
/// 就可能把别家安装（例如同一台机器上主干那份）正在跑的内核判成孤儿。
pub fn bundled_kernel_exe() -> Option<PathBuf> {
    let dir = kernel_dir_from(&std::env::current_exe().ok()?)?;
    let node = dir.join("node.exe");
    node.is_file().then_some(node)
}

/// 关于页要用的内核目录：`BLADE2_KERNEL_DIR` 优先（与 `Launch::from_env` 里那颗同名 env 覆盖
/// 同一个口），否则退回 `kernel_dir_from` 的「从自身路径往上找 `Kernel/node.exe`」——
/// 后者就是主干 `KernelVersionText` 读的 `AppContext.BaseDirectory/Kernel`（`About.cs:64-79`）。
fn kernel_dir_for_read() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("BLADE2_KERNEL_DIR") {
        return Some(PathBuf::from(dir));
    }
    kernel_dir_from(&std::env::current_exe().ok()?)
}

/// 一个 `package.json` 的正文 → 版本号。**只认非空字符串**的 `version`：键缺席、值不是字符串、
/// 值是空串、正文不是合法 JSON 都回 `None`（= 关于页那行显示「未知」）。这四条与主干
/// `MainWindow.About.cs:64-79` 逐句对应：`JsonDocument.Parse` 抛异常被 `catch` 掉、
/// `TryGetProperty` 落空、`ValueKind == String` 判定、`!string.IsNullOrEmpty`。
/// 刻意做成只吃内容的纯函数，好让关于页那一行不必真有一个 `Kernel/` 也能离线测全。
pub fn parse_package_version(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    value
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .map(str::to_string)
}

/// 从某个内核目录读 `dsh/package.json` 的版本。读不到文件（没装内核、路径被 `BLADE2_KERNEL_DIR`
/// 指歪了、权限不足）一律 `None`，与主干那句 `catch (Exception) { }` 同口径：关于页其余内容不许
/// 被这一行拖垮。
pub fn kernel_version_from(kernel_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(kernel_dir.join("dsh").join("package.json")).ok()?;
    parse_package_version(&text)
}

/// 关于页「内核版本」那行的取值（缺口 #72 之前是一句字面量「未知」）。
///
/// 主干每次渲染都重读一遍文件（`MainWindow.About.cs:98-101` 直接调 `KernelVersionText()`）；
/// 这份 JSON 随包发行、进程活着就不会变，所以分叉把它缓进 `OnceLock`：值与主干一致，
/// 而 `view()` 里**一次文件 IO 都不会发生**（读盘发生在 `Shell::create` 调的
/// `warm_kernel_version()` 里，那之前没有人渲染，那之后就只是 `OnceLock::get`）。
pub fn kernel_version() -> Option<&'static str> {
    static CACHED: OnceLock<Option<String>> = OnceLock::new();
    CACHED
        .get_or_init(|| kernel_dir_for_read().and_then(|dir| kernel_version_from(&dir)))
        .as_deref()
}

/// 把 `kernel_version()` 那一次读盘提前到开窗口之前（`Shell::create` 里调一次）。
/// 返回值故意丢弃：调用方要的是「缓存放好了」，取值仍然走 `kernel_version()`。
pub fn warm_kernel_version() {
    let _ = kernel_version();
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
            let (node, bin_js) = bundled_parts_in(bundled_kernel_dir().as_deref());
            return Ok(Self {
                exe: PathBuf::from(exe),
                args,
                args_verbatim: false,
                // 主干 `IsBundled`（`DshKernelHost.cs:42`）就是那两个 `File.Exists`，**不吃 env**，
                // 所以走了自测口也不改这个判定（§1.7-1 要的正是"文案只认这两颗存在性"）。
                is_bundled: is_bundled(&node, &bin_js),
                dsh_home: None,
                path_prepend: None,
                working_dir: None,
            });
        }
        // 两级解析：内置优先，缺一颗就整级跳过，再试 `%APPDATA%\npm\dsh.cmd`，全不命中 ⇒ Err
        // （主干 ⇒ `return null`，调用点 `main.rs` 那句文案已与主干同形，见 §1.6 表第 3 行）。
        let kernel_dir = bundled_kernel_dir();
        let (node, bin_js) = bundled_parts_in(kernel_dir.as_deref());
        let bundled = is_bundled(&node, &bin_js);
        let npm_cmd = probe_npm_dsh_cmd();
        let launcher = decide_launcher(node.clone(), bin_js.clone(), npm_cmd.clone());
        let Some(SpawnPlan { exe, args, args_verbatim }) = launcher_argv(&launcher) else {
            let describe = |suffix: &str| {
                kernel_dir
                    .as_ref()
                    .map_or_else(
                        || format!("{suffix}（没有 Kernel 目录）"),
                        |dir| dir.join(suffix).display().to_string(),
                    )
            };
            return Err(format!(
                "内核文件缺失: 内置 {} / {} 不齐，且 {} 不存在",
                describe("node.exe"),
                describe("dsh/lib/bin.js"),
                npm_cmd.map_or_else(
                    || "npm 版 dsh.cmd（%APPDATA% 未设置或该文件不存在）".to_string(),
                    |path| path.display().to_string(),
                )
            ));
        };
        // `path_prepend` 两级共用一颗 `<install>\Kernel\bin`（主干 `BundledBinDir`，`:40`），
        // **回退形态下也照算**；它在不在、要不要注入 PATH 由 [`build_env`] 里那颗早退管。
        let kernel_bin = kernel_dir.map_or_else(
            || {
                std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(|dir| dir.join("Kernel").join("bin")))
                    .unwrap_or_else(|| PathBuf::from("Kernel").join("bin"))
            },
            |dir| dir.join("bin"),
        );
        Ok(Self {
            exe,
            args,
            args_verbatim,
            is_bundled: bundled,
            // #84-B：数据家从这里出，替掉老代码里 `LOCALAPPDATA + join("Blade2")` 的内联拼接。
            dsh_home: Some(resolve_dsh_home()),
            path_prepend: Some(kernel_bin),
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
    /// 缺口 #68：主干 `_job`（`DshKernelHost.cs:24`）的对应物 —— `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
    /// 的作业对象，**最后一个句柄关闭即带走内核整棵树**（主干 `:155` 那句「句柄只在 Dispose 里收」）。
    /// 没有这层，`shutdown()` 那句 `child.kill()` 只杀内核主进程，它起的 MCP 子进程全部活下来继续占租约。
    job: crate::procguard::KillJob,
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
/// 载荷抽取已挪到 `EventsFrame::status_payload`（#74：分流要先认流再认帧型），
/// 这里保留原签名 = `main.rs::mux_frame_sink` 今天的调用口径。
pub fn session_status_event(frame: &Value) -> Option<(String, bool)> {
    parse_events_frame(frame)?.status_payload()
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
    // 内核 schema 保证 turn 严格递增，这里照样防御性排序。主干同一步的可 grep 符号锚：
    // `MainWindow.TurnRail.cs` 的 `ParseTurnOutline` 收尾那句
    // `list.Sort((a, b) => a.Turn.CompareTo(b.Turn))`（原注释抄的是行号，行号会漂 ⇒ 换成符号）。
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

// ---- schedule 投影的记录解析（L2 · L0 残留 ②）------------------------------------
//
// 主干把 `schedule` 投影读成一份**五字段记录表**：`MainWindow.SessionState.cs:124-146` 的
// `ApplyScheduleProjectionWire` 收两型 wire，`:153-172` 的 `ParseScheduleRecordsInto` 逐条读，
// 明细最后喂给「计划（只读）」那张面板（`MainWindow.Capabilities.cs:294-366`）。
// 分叉这边今天只把「有没有活动计划」那一枚 bool 接走（`main.rs` 的 `schedule_active` 走
// `Projections::value_of("schedule")`），**明细当场丢弃** ⇒ 那张面板在分叉取不到数据。
// 本节把记录建模与解析备好（lib 层的货架），宿主与出图归接这张卡的那一轮，不在本轮。

/// 主干那条 `(Id, Kind, Prompt, ScheduledAt, EverySeconds)` 元组。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScheduleRecord {
    pub id: String,
    /// `"at"` | `"every"`（内核口径）。缺键或非串 ⇒ `""`，与主干那三元的回落方向一致。
    pub kind: String,
    pub prompt: String,
    /// RFC3339 UTC **原文**：主干 `DateTimeOffset.TryParse` 住在展示侧
    /// （`Capabilities.cs:306-310` 排序那一刻），解析层不碰它 ⇒ 这里也不碰。
    pub scheduled_at: String,
    /// 主干 `(long)GetDouble()`（`:170`）：非数 ⇒ **0**。这一颗**不是** `Option` ——
    /// `kind=="at"` 那一条天生没有这颗键，主干给的就是 0，用 `None` 表示就是替它改语义。
    pub every_seconds: i64,
}

/// 主干 `ApplyScheduleProjectionWire` 的两型入口 + `ParseScheduleRecordsInto` 的逐条读法，
/// 合成一颗纯函数（判据全在函数体里，注释只记「为什么是这个方向」）：
///
/// * 值既可以是**数组本身**（官方 `scheduleProjectionDefinition.wire.view`），也可以是旧
///   `session/control` 那条路的 `{active:[…]}` 对象 —— 两种都解析，其它形状 ⇒ 空表；
/// * **「没见过投影」与「见过但是空」这两档本函数不分**：主干拿 `_scheduleSeen` 那枚 bool
///   另记（`SessionState.cs:141`，面板文案 `Capabilities.cs:329-333` 就靠它区分两句话），
///   那是调用方的账；
/// * 五根字段一律 [`string_field`] 口径（非串 ⇒ `""`）；`id` 与 `prompt` **同时**为空才丢
///   这一条 —— 主干 `:161-164` 那是两个 `Length == 0` 的 AND，不是「缺 id 就丢」。
pub fn parse_schedule_records(value: Option<&Value>) -> Vec<ScheduleRecord> {
    let items = match value {
        Some(value) if value.is_array() => value.as_array(),
        Some(value) => value.get("active").and_then(Value::as_array),
        None => None,
    };
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|record| {
            let id = string_field(record, "id");
            let prompt = string_field(record, "prompt");
            if id.is_empty() && prompt.is_empty() {
                return None;
            }
            Some(ScheduleRecord {
                id,
                kind: string_field(record, "kind"),
                prompt,
                scheduled_at: string_field(record, "scheduledAt"),
                every_seconds: record["everySeconds"].as_f64().map(|seconds| seconds as i64).unwrap_or(0),
            })
        })
        .collect()
}

#[cfg(test)]
mod l2_schedule_tests {
    //! L2 的 schedule 记录锁：① 五根字段逐字钉回主干那颗元组（含 `everySeconds` 的
    //! 「非数 ⇒ 0」与截断方向）；② 两型 wire 都吃、坏型回落空表；③ 丢条规则是
    //! 「id 与 prompt **同时**为空」，别写成「缺 id 就丢」。
    use super::*;

    /// ① 五字段逐字 + 内核真形状那两档（`at` 无 `everySeconds` / `every` 有）。
    #[test]
    fn a_schedule_projection_array_becomes_five_field_records() {
        let value = json!([
            { "id": "s-1", "kind": "at", "prompt": "明早跑一次", "scheduledAt": "2026-10-01T07:30:00Z" },
            { "id": "s-2", "kind": "every", "prompt": "每天收日报", "scheduledAt": "2026-09-27T00:00:00Z",
              "everySeconds": 86400, "unknownKey": "内核哪天多一颗，本层不许因此丢条" },
        ]);
        let records = parse_schedule_records(Some(&value));
        assert_eq!(
            records,
            vec![
                ScheduleRecord {
                    id: "s-1".to_string(),
                    kind: "at".to_string(),
                    prompt: "明早跑一次".to_string(),
                    scheduled_at: "2026-10-01T07:30:00Z".to_string(),
                    every_seconds: 0,
                },
                ScheduleRecord {
                    id: "s-2".to_string(),
                    kind: "every".to_string(),
                    prompt: "每天收日报".to_string(),
                    scheduled_at: "2026-09-27T00:00:00Z".to_string(),
                    every_seconds: 86400,
                },
            ]
        );
        // 主干 `(long)GetDouble()`：向零截断，非数（这里是字符串）回落 0 —— 两档都不 panic。
        let odd = json!([
            { "id": "s-3", "kind": "every", "prompt": "p", "scheduledAt": "", "everySeconds": 90.7 },
            { "id": "s-4", "kind": "every", "prompt": "p", "scheduledAt": "", "everySeconds": "60" },
        ]);
        let records = parse_schedule_records(Some(&odd));
        assert_eq!(vec![90, 0], records.iter().map(|r| r.every_seconds).collect::<Vec<_>>());
        assert_eq!(records[1].scheduled_at, "", "非串的 scheduledAt 回落空串，不是 None");
    }

    /// ② 两型 wire 都吃；其余形状一律空表（`None` = 投影还没到过，同样给空表 ——
    /// 「没见过」与「见过但空」的区分不在本函数，见函数 doc）。
    #[test]
    fn both_wire_shapes_parse_and_anything_else_is_an_empty_table() {
        let one = json!({ "id": "s-1", "kind": "at", "prompt": "p", "scheduledAt": "t" });
        let as_array = parse_schedule_records(Some(&json!([one])));
        let as_state_object = parse_schedule_records(
            Some(&json!({ "active": [one], "inheritedEventCount": 2, "seenIds": ["s-1"] })),
        );
        assert_eq!(as_array, as_state_object);
        assert_eq!(as_array.len(), 1);
        for bad in [
            None,
            Some(&json!(null)),
            Some(&json!("不是数组")),
            Some(&json!({})),
            Some(&json!({ "active": "还不是数组" })),
        ] {
            assert_eq!(parse_schedule_records(bad), Vec::new(), "{bad:?} 该回落空表");
        }
    }

    /// ③ 丢条规则：主干那句是 `id` 与 `prompt` **两个都空**才丢。
    #[test]
    fn only_a_record_with_both_id_and_prompt_empty_is_dropped() {
        let value = json!([
            { "kind": "at", "prompt": "有 prompt 无 id" },
            { "id": "s-1", "kind": "at", "prompt": "" },
            { "id": "", "kind": "at", "prompt": "" },
            { "id": 7, "kind": "at", "prompt": "id 不是串 ⇒ 视同空" },
            { "id": 7, "kind": "at", "prompt": "" },
        ]);
        let records = parse_schedule_records(Some(&value));
        assert_eq!(records.len(), 3, "该丢的只有第 3、5 两条（id 与 prompt 同时为空）");
        assert_eq!(
            (records[0].id.as_str(), records[0].prompt.as_str()),
            ("", "有 prompt 无 id")
        );
        assert_eq!((records[1].id.as_str(), records[1].prompt.as_str()), ("s-1", ""));
        // 主干那颗判据是 `i.ValueKind == JsonValueKind.String ? i.GetString() ?? "" : ""`：
        // **非串的 id 视同空**，但 prompt 还在 ⇒ 不丢（丢成「缺 id 就丢」才是这里要防的错）。
        assert_eq!(
            (records[2].id.as_str(), records[2].prompt.as_str()),
            ("", "id 不是串 ⇒ 视同空")
        );
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

// ==================== `$events` 交互通道（#74 审批 / 提问） ====================
//
// 主干真值（行号会漂 ⇒ 按符号定位）：`DshRpcClient.cs:570-628`（四型分流、clientId 只认
// ready）、`:676-683`（唯一回帧口）、`:715-720`（审批字面量白名单）、
// `MainWindow.UserQuestions.cs:662-704`（answers 汇总）、
// `dsh-api-gateway/lib/index.js:21-50`（`exactKeys` 硬校验）、`:598`（新 client 重推未答帧）。

/// `$events` 流元素的四型表。与控制面那张 `CONTROL_FRAME_TYPES` 无关：两条流各有帧型，
/// 分流的第一判据是 streamId，这张表只负责「认下之后按哪一型落地」。
pub const EVENTS_FRAME_TYPES: [&str; 4] = ["ready", "cancel", "emit", "waterfall"];

/// 内核 waterfall 白名单（`dsh-api-remotes/lib/types/remote-events.js:14,31`）。
pub const APPROVAL_EVENT: &str = "approval/request";
pub const QUESTION_EVENT: &str = "user-questions/request";
/// `questions[0].id` 的这个值 = 计划评审（`dsh-plan-mode/lib/index.js:36`）。
pub const PLAN_REVIEW_ID: &str = "plan-review";
/// 回帧端点：body 逐字是 `{clientId,eventId,outcome}`，多一个键网关就拒。
pub const EVENT_RESULT_METHOD: &str = "$events/result";
/// 审批卡的两颗钮（`DshRpcClient.cs:717` 硬校验，第三个字面量直接抛）。
pub const APPROVAL_OUTCOMES: [&str; 2] = ["allowed-once", "rejected"];

/// `$events` 上**常驻订阅的事件名全表**（10 发 = 8 发 `OnEvent` + 2 发 `OnWaterfall`）。
///
/// **这张表是唯一事实源**：消费端（`main.rs` 的事件泵）分流只能查它，不许再抄第二张名字表；
/// 内核新增/撤一条常驻订阅时，改这里即可（`events_subscription` 与单测会跟着变红）。
/// 与 `EVENTS_FRAME_TYPES` 是两回事：那张管**帧型**（`ready`/`cancel`/`emit`/`waterfall`），
/// 这张管**事件名** —— 名字只出现在 `emit` 与 `waterfall` 那两支的 `event` 字段上。
///
/// 主干真值（逐条按注册点排序，那四条 cordis 单独在分部文件里）：
/// · `MainWindow.xaml.cs:2845` `OnEvent("commands/change")` —— 命令表失效重拉
/// · `:2918/2919/2920` `OnEvent("api-session/{added,removed,status}")` —— 左栏增行/掉行/状态点
/// · `MainWindow.Cordis.cs:104/105/106/107` `OnEvent("cordis/{request-run,request-run-resolved,dynamic-package,dynamic-retract}")`
/// · `MainWindow.xaml.cs:2912/2925` `OnWaterfall("approval/request")` / `("user-questions/request")` —— #74 已端到端
///
/// 判据口径：后两条走 waterfall 那一支（名字复用 #74 已有的 waterfall 权威判据
/// `PendingKind::of`，**不在这张表里另立第二份归属**）；前 8 条只可能从 `emit` 那一支认出。
pub const EVENTS_SUBSCRIPTIONS: [&str; 10] = [
    "commands/change",
    "api-session/added",
    "api-session/removed",
    "api-session/status",
    "cordis/request-run",
    "cordis/request-run-resolved",
    "cordis/dynamic-package",
    "cordis/dynamic-retract",
    APPROVAL_EVENT,
    QUESTION_EVENT,
];

/// 事件名与帧型**两半都要对得上**才算登记过：主干那两条 `if` 各查各的注册表
/// （`DshRpcClient.cs:590-611` 只喂 `emit`、`:618-628` 只喂 `waterfall`），
/// 所以 `emit` 帧上的 `approval/request` 与 waterfall 帧上的 `api-session/added` 都不该被认出。
fn same_side(name: &str, waterfall_side: bool) -> bool {
    PendingKind::of(name).is_some() == waterfall_side
}

/// 这一帧的 `type` 是不是 `$events` 那四型之一。`type` 缺席或非串一律不认。
pub fn events_frame_type(frame: &Value) -> Option<&'static str> {
    let kind = frame["type"].as_str()?;
    EVENTS_FRAME_TYPES
        .into_iter()
        .find(|known| *known == kind)
}

/// 非空字符串字段：网关的 `isRemoteEventClientId` / `isRemoteEventId` 都要求长度 > 0。
fn non_empty_string(record: &Value, key: &str) -> Option<String> {
    record[key]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// 审批的一颗钮。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalDecision {
    AllowedOnce,
    Rejected,
}

impl ApprovalDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllowedOnce => "allowed-once",
            Self::Rejected => "rejected",
        }
    }

    /// 白名单外的字面量一律认不出（内核词表里的 `cancelled`/`unavailable` 是内核自造的，客户端不许回）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "allowed-once" => Some(Self::AllowedOnce),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// 一帧 waterfall 属于哪张卡。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingKind {
    Approval,
    Question,
}

impl PendingKind {
    /// 事件名 → 卡片归属。白名单外（内核新增第三条 waterfall）返回 `None`，由台账计成未识别。
    pub fn of(event: &str) -> Option<Self> {
        match event {
            APPROVAL_EVENT => Some(Self::Approval),
            QUESTION_EVENT => Some(Self::Question),
            _ => None,
        }
    }

    fn badge(self, plan_review: bool) -> Badge {
        match self {
            Self::Approval => Badge::Approval,
            Self::Question if plan_review => Badge::PlanReview,
            Self::Question => Badge::Question,
        }
    }
}

/// 状态条/左栏那一枚待交互标记（主干 `SessionState.cs:273` 的 `approval > plan-review > question`）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Badge {
    Approval,
    PlanReview,
    Question,
}

impl Badge {
    fn rank(self) -> u8 {
        match self {
            Self::Approval => 3,
            Self::PlanReview => 2,
            Self::Question => 1,
        }
    }
}

/// `$events` 流元素解出来的四型载荷。
#[derive(Clone, Debug, PartialEq)]
pub enum EventsFrame {
    /// 本代次回帧唯一可用的 clientId（emit/waterfall 帧上同名的键一律不认，见 R2）。
    Ready { client_id: String },
    /// 内核放弃了一发交互（取消、释放、或已成功回帧后的收尾）。
    Cancel { event_id: String },
    Emit { event: String, args: Vec<Value> },
    Waterfall {
        event: String,
        event_id: String,
        agent_id: Option<String>,
        request: Value,
    },
}

impl EventsFrame {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Ready { .. } => "ready",
            Self::Cancel { .. } => "cancel",
            Self::Emit { .. } => "emit",
            Self::Waterfall { .. } => "waterfall",
        }
    }

    /// `Emit{event:"api-session/status"}` 的载荷（左栏运行点）。
    pub fn status_payload(&self) -> Option<(String, bool)> {
        match self {
            Self::Emit { event, args } if event == "api-session/status" => Some((
                args.first()?.as_str()?.to_string(),
                args.get(1)?.as_bool()?,
            )),
            _ => None,
        }
    }

    /// 已解析的这一帧落在 `EVENTS_SUBSCRIPTIONS` 的哪一发上：认出就回**登记表里那个
    /// `'static` 名字**（消费端可以拿它直接 `match` 臂），认不出回 `None`。
    ///
    /// 只加访问器、不改任何判据体：`kind()` / `status_payload()` 的语义逐字不变，
    /// `InteractionLedger::{apply,drop_stream}` 也不读这张表 —— 登记层与交互台账（#74）互不影响。
    /// `Ready`/`Cancel` 两支根本没有 `event` 字段，一律 `None`。
    pub fn subscription(&self) -> Option<&'static str> {
        let (event, waterfall_side) = match self {
            Self::Emit { event, .. } => (event, false),
            Self::Waterfall { event, .. } => (event, true),
            Self::Ready { .. } | Self::Cancel { .. } => return None,
        };
        let name = EVENTS_SUBSCRIPTIONS
            .into_iter()
            .find(|known| *known == event)?;
        same_side(name, waterfall_side).then_some(name)
    }
}

/// 解一帧 `$events` 流元素。必填键坏掉（缺 `clientId` / 缺 `eventId` / `event` 非串）判「不认」，
/// 由上层计成未识别帧，绝不能拿半个载荷去回帧。
pub fn parse_events_frame(frame: &Value) -> Option<EventsFrame> {
    match events_frame_type(frame)? {
        "ready" => non_empty_string(frame, "clientId")
            .map(|client_id| EventsFrame::Ready { client_id }),
        "cancel" => non_empty_string(frame, "eventId")
            .map(|event_id| EventsFrame::Cancel { event_id }),
        "emit" => {
            let event = non_empty_string(frame, "event")?;
            let args = frame["args"].as_array().cloned().unwrap_or_default();
            Some(EventsFrame::Emit { event, args })
        }
        "waterfall" => {
            let event = non_empty_string(frame, "event")?;
            let event_id = non_empty_string(frame, "eventId")?;
            Some(EventsFrame::Waterfall {
                event,
                event_id,
                agent_id: non_empty_string(frame, "agentId"),
                request: frame.get("request").cloned().unwrap_or(Value::Null),
            })
        }
        _ => None,
    }
}

/// 一帧 `$events` 流元素**登记到哪个事件名**（登记表 `EVENTS_SUBSCRIPTIONS` 是唯一事实源）。
/// 判据复用 `parse_events_frame`：帧型不认、必填键坏掉、`event` 非串、名字不在表上、
/// 名字在表上但走错了支（`emit` 帧带 waterfall 名，反之亦然）—— 一律 `None`，不 panic。
/// 已经自己解析过一帧的地方请直接用 `EventsFrame::subscription`，别把同一帧解两遍。
pub fn events_subscription(frame: &Value) -> Option<&'static str> {
    parse_events_frame(frame)?.subscription()
}

/// `{kind:"result", value}`：审批的 `value` 是字符串，提问的是 `{answers:[…]}`。
pub fn wf_result(value: Value) -> Value {    json!({ "kind": "result", "value": value })
}

/// `{kind:"next"}`：主干注册了但零调用点；网关只在 `deliveries` 空了时自己 settle 这一型。
pub fn wf_next() -> Value {
    json!({ "kind": "next" })
}

/// `{kind:"rejected", error:{name,message,code?}}`。`error` 的键集合是网关白名单，
/// 塞 `stack` 之类一律被拒（R1）。
pub fn wf_rejected(name: &str, message: &str, code: Option<&str>) -> Value {
    let mut error = json!({ "name": name, "message": message });
    if let Some(code) = code {
        error["code"] = json!(code);
    }
    json!({ "kind": "rejected", "error": error })
}

/// 主干「放弃整组问题 / 去聊天里说」那一发的字面量。
pub fn wf_ask_cancelled() -> Value {
    wf_rejected(
        "UserQuestionError",
        "the user cancelled ask_user_question",
        Some("ASK_CANCELLED"),
    )
}

pub fn approval_outcome(decision: ApprovalDecision) -> Value {
    wf_result(json!(decision.as_str()))
}

pub fn question_outcome(answers: Value) -> Value {
    wf_result(json!({ "answers": answers }))
}

/// 网关 `parseRemoteEventResult` 的镜像校验：合规定返回 `None`。
fn outcome_error(outcome: &Value) -> Option<&'static str> {
    const BAD: &str = "outcome 的键集合不合网关白名单（多一个键就拒）";
    let fields = match outcome.as_object() {
        Some(fields) => fields,
        None => return Some(BAD),
    };
    let exact = |names: &[&str]| {
        fields.len() == names.len() && names.iter().all(|key| fields.contains_key(*key))
    };
    let kind = match fields.get("kind").and_then(Value::as_str) {
        Some(kind) => kind,
        None => return Some(BAD),
    };
    match kind {
        "next" if exact(&["kind"]) => None,
        "result" if exact(&["kind"]) || exact(&["kind", "value"]) => None,
        "rejected" if exact(&["kind", "error"]) => rejection_error(&fields["error"]),
        _ => Some(BAD),
    }
}

/// `error` 只允许 `{name,message}` + 可选 `{code,details}`，且 `name` 非空、`message`/`code` 是串。
fn rejection_error(error: &Value) -> Option<&'static str> {
    const BAD: &str = "rejected 的 error 只许 name/message + 可选 code/details";
    let Some(fields) = error.as_object() else {
        return Some(BAD);
    };
    let allowed = ["name", "message", "code", "details"];
    if !fields.keys().all(|key| allowed.contains(&key.as_str()))
        || !fields.contains_key("name")
        || !fields.contains_key("message")
    {
        return Some(BAD);
    }
    if fields["name"].as_str().is_none_or(|name| name.is_empty()) {
        return Some("rejected 的 error.name 要非空字符串");
    }
    if !fields["message"].is_string()
        || fields
            .get("code")
            .is_some_and(|code| !code.is_string())
    {
        return Some("rejected 的 error.message/code 必须是字符串");
    }
    None
}

/// 一道提问的草稿（UI 侧收集，回帧前交给 `question_answers` 汇总）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestionDraft {
    pub selected: Vec<String>,
    pub custom: String,
    pub skipped: bool,
}

/// 多选题判据：进帧后的键名是 `multiSelect`（工具入参那个 `multi_select` 已被改写过，R4）。
pub fn is_multi_select(question: &Value) -> bool {
    question["multiSelect"].as_bool() == Some(true)
}

/// 一组的 answers（照主干 `SubmitQuestionSetAsync`）：跳过题只回空 `selected`；
/// 单选 + 有补充说明 ⇒ `selected` 置空、只回 `custom`。
pub fn question_answers(questions: &[Value], drafts: &[QuestionDraft]) -> Value {
    let answers: Vec<Value> = questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let id = question["id"].as_str().unwrap_or_default().to_string();
            let draft = drafts.get(index).cloned().unwrap_or_default();
            if draft.skipped {
                return json!({ "id": id, "selected": [] });
            }
            let custom = draft.custom.trim().to_string();
            let selected = if !custom.is_empty() && !is_multi_select(question) {
                Vec::new()
            } else {
                draft.selected
            };
            let mut answer = json!({ "id": id, "selected": selected });
            if !custom.is_empty() {
                answer["custom"] = json!(custom);
            }
            answer
        })
        .collect();
    Value::Array(answers)
}

/// 一条待发的 `$events/result` 回帧（只能由 `InteractionLedger::result_request` 造出来）。
#[derive(Clone, Debug, PartialEq)]
pub struct EventResultRequest {
    pub client_id: String,
    pub event_id: String,
    pub outcome: Value,
}

impl EventResultRequest {
    /// `Kernel::call` 的 `args`：恰好三个键，顺序无所谓、多一个就拒。
    pub fn to_args(&self) -> Value {
        json!({
            "clientId": self.client_id,
            "eventId": self.event_id,
            "outcome": self.outcome,
        })
    }
}

/// 一条挂在台账里的交互请求。
#[derive(Clone, Debug, PartialEq)]
pub struct PendingInteraction {
    pub event_id: String,
    pub kind: PendingKind,
    pub agent_id: Option<String>,
    pub request: Value,
    /// 这一帧到达（或最近一次被重推）时的 mux 代次。
    pub generation: u64,
}

impl PendingInteraction {
    pub fn tool_name(&self) -> &str {
        self.request["toolName"].as_str().unwrap_or_default()
    }

    pub fn reason(&self) -> &str {
        self.request["reason"].as_str().unwrap_or_default()
    }

    pub fn questions(&self) -> &[Value] {
        self.request["questions"].as_array().map_or(&[], Vec::as_slice)
    }

    pub fn is_plan_review(&self) -> bool {
        self.kind == PendingKind::Question && self.request["questions"][0]["id"].as_str() == Some(PLAN_REVIEW_ID)
    }

    fn badge(&self) -> Badge {
        self.kind.badge(self.is_plan_review())
    }
}

/// 一帧被台账挡在门外的原因（UI 侧据此计「未识别/被吞」的日志，别静默丢）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameDrop {
    /// 同代次内已认过这个 eventId（重连重推不带代次，见 R3）。
    Duplicate(String),
    /// 命中取消墓碑（含「取消先于请求到达」）。
    Cancelled(String),
    /// 旧代次的迟到帧：那根 socket 已经作废。
    Stale(u64),
    /// 白名单外的 waterfall 事件名。
    UnknownEvent(String),
}

/// `InteractionLedger::apply` / `settle` 的回执：UI 半边只按这几个布尔决定重画什么。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LedgerDelta {
    /// 本帧落了 clientId（可以开始回帧）。
    pub ready: bool,
    /// 本帧进了哪张卡的台账。
    pub enqueued: Option<PendingKind>,
    /// 在显的那张卡换了（上屏 / 收掉 / 重推后该重新亮）。
    pub active_changed: bool,
    /// 被内核取消的 eventId（UI 要把对应的卡连同草稿收掉）。
    pub cancelled: Vec<String>,
    /// 提交锁解了（成功结算或取消/换代次兜底）。
    pub unlocked: bool,
    pub dropped: Option<FrameDrop>,
    /// mux 代次推进了：clientId 已作废、队列内容按主干保留等重推。
    pub generation_bumped: bool,
}

impl LedgerDelta {
    /// 需要重画（或需要写一行日志）吗。
    pub fn any(&self) -> bool {
        self.ready
            || self.enqueued.is_some()
            || self.active_changed
            || !self.cancelled.is_empty()
            || self.unlocked
            || self.dropped.is_some()
            || self.generation_bumped
    }
}

/// `$events` 侧的交互台账：clientId、两条卡队列、提交锁，以及**带代次**的去重集与取消墓碑。
///
/// 为什么去重必须带代次（R3）：网关在新 client 接入时会把未答帧按原 `eventId` 重推
/// （`dsh-api-gateway/lib/index.js:598`），照抄主干那套全局 `eventId` 墓碑会把重推吞掉、
/// 卡片再也不亮而内核继续挂着。主干是单进程不断 `$events`，没这个问题 ⇒ 不能整体照抄。
#[derive(Clone, Debug, Default)]
pub struct InteractionLedger {
    generation: u64,
    client_id: Option<String>,
    active_approval: Option<PendingInteraction>,
    approval_queue: VecDeque<PendingInteraction>,
    active_question: Option<PendingInteraction>,
    question_queue: VecDeque<PendingInteraction>,
    submitting: Option<String>,
    seen: HashSet<(u64, String)>,
    cancelled: HashSet<(u64, String)>,
}

impl InteractionLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    /// 本代次的 `ready` 已到：回帧的前置条件（主干那 10 秒等的就是它，分叉不另造超时）。
    pub fn is_ready(&self) -> bool {
        self.client_id.as_deref().is_some_and(|id| !id.is_empty())
    }

    pub fn submitting(&self) -> Option<&str> {
        self.submitting.as_deref()
    }

    pub fn active_approval(&self) -> Option<&PendingInteraction> {
        self.active_approval.as_ref()
    }

    pub fn active_question(&self) -> Option<&PendingInteraction> {
        self.active_question.as_ref()
    }

    /// 排在后面还没轮到显的同类卡数（主干审批卡那句「+N 条」）。
    pub fn queued(&self, kind: PendingKind) -> usize {
        match kind {
            PendingKind::Approval => self.approval_queue.len(),
            PendingKind::Question => self.question_queue.len(),
        }
    }

    pub fn is_pending(&self, event_id: &str) -> bool {
        self.find(event_id).is_some()
    }

    /// 状态条那一枚标记：当前所有待答里优先级最高的一个（只升不降是 UI 侧的粘滞，这里给瞬时真相）。
    pub fn badge(&self) -> Option<Badge> {
        [
            self.active_approval.as_ref(),
            self.active_question.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(self.approval_queue.iter().chain(self.question_queue.iter()))
        .map(PendingInteraction::badge)
        .max_by_key(|badge| badge.rank())
    }

    /// 上屏那张卡被提交锁住了吗（主干提交期整卡 `IsEnabled=false`）。
    pub fn mark_submitting(&mut self, event_id: &str) -> bool {
        if !self.is_ready() || !self.is_pending(event_id) || self.submitting.is_some() {
            return false;
        }
        self.submitting = Some(event_id.to_string());
        true
    }

    /// **只**解提交锁、不出账：回帧失败那支专用（主干 `_submittingApprovalEventId = null` +
    /// `ApprovalHost.IsEnabled = true`，`MainWindow.xaml.cs:8030-8033`、
    /// `MainWindow.UserQuestions.cs:711-714`），卡照旧亮着等用户重试。
    /// `settle` 干的是成功那支（摘卡 + 推下一条），两者不可互相顶替：拿 `settle` 兜失败
    /// 就等于「RPC 抖一下 ⇒ 这张卡永远消失而内核还挂着」，主干没有这种语义。
    /// 返回「这次真解开了锁吗」：锁本来不在这一发上（已被取消/换代次解掉）就是 `false`。
    pub fn unlock(&mut self, event_id: &str) -> bool {
        if self.submitting.as_deref() != Some(event_id) {
            return false;
        }
        self.submitting = None;
        true
    }

    /// 造一条回帧。`client_id` 只可能来自本代次的 `ready` 帧，取不到就按主干那句文案失败，
    /// 由上层走「解锁 + 提示重试」（分叉不发明超时应答）。
    pub fn result_request(
        &self,
        event_id: &str,
        outcome: Value,
    ) -> Result<EventResultRequest, String> {
        if !self.is_ready() {
            return Err("事件连接已失效，请等待重新连接后再回答。".to_string());
        }
        if event_id.trim().is_empty() {
            return Err("eventId 为空，网关会拒这一发回帧".to_string());
        }
        // 已出账（结算或被取消）的 eventId 不再给出口：主干那句
        // 「`_pendingApprovalEventId != eventId` 就 return」（MW:8029）的等价物。
        if !self.is_pending(event_id) {
            return Err("这一发已不在待答台账里（内核已取消或已结算）".to_string());
        }
        if let Some(reason) = outcome_error(&outcome) {
            return Err(reason.to_string());
        }
        Ok(EventResultRequest {
            client_id: self.client_id.clone().unwrap_or_default(),
            event_id: event_id.to_string(),
            outcome,
        })
    }

    pub fn approval_result(
        &self,
        event_id: &str,
        decision: ApprovalDecision,
    ) -> Result<EventResultRequest, String> {
        self.result_request(event_id, approval_outcome(decision))
    }

    pub fn question_result(
        &self,
        event_id: &str,
        answers: Value,
    ) -> Result<EventResultRequest, String> {
        self.result_request(event_id, question_outcome(answers))
    }

    /// 回帧成功/放弃后出账：摘掉这一条、解提交锁，并把同类里下一条推上屏。
    pub fn settle(&mut self, event_id: &str) -> LedgerDelta {
        let mut delta = LedgerDelta::default();
        if self.submitting.as_deref() == Some(event_id) {
            self.submitting = None;
            delta.unlocked = true;
        }
        for kind in [PendingKind::Approval, PendingKind::Question] {
            if self.remove(kind, event_id) {
                delta.active_changed = true;
            }
        }
        delta
    }

    /// 丢掉 `$events` 那根流时**主动**推进代次：不等下一帧。
    ///
    /// 为什么要有这一发：`apply` 的换代支只在**真收到新代次的帧**时才清 `client_id`，
    /// 而重连后到新一发 `ready` 之间可能一个帧都没有 —— 那段空窗里 `client_id()` 仍是
    /// **旧 socket 的串**，回帧侧一旦用它就会被内核判 `identifies no active event stream`。
    /// 队列内容按主干保留（内核会把未答帧按原 `eventId` 重推），清的只是流侧身份与提交锁。
    pub fn drop_stream(&mut self, generation: u64) -> LedgerDelta {
        if generation <= self.generation {
            return LedgerDelta::default();
        }
        self.generation = generation;
        self.client_id = None;
        let unlocked = self.submitting.take().is_some();
        let current = generation;
        self.seen.retain(|(seen, _)| *seen == current);
        self.cancelled.retain(|(seen, _)| *seen == current);
        LedgerDelta {
            unlocked,
            generation_bumped: true,
            ..Default::default()
        }
    }

    /// 落一帧。**必须**由上层带上它自己那批帧的 mux 代次：旧代次的帧（`generation` 落后）
    /// 直接判 `Stale` 丢掉，等价于主干「等不到 ready 就抛」那一侧的防线。
    pub fn apply(&mut self, generation: u64, frame: &EventsFrame) -> LedgerDelta {
        let mut delta = LedgerDelta::default();
        if generation < self.generation {
            delta.dropped = Some(FrameDrop::Stale(generation));
            return delta;
        }
        if generation > self.generation {
            self.generation = generation;
            self.client_id = None;
            delta.generation_bumped = true;
            delta.unlocked = self.submitting.take().is_some();
            let current = generation;
            self.seen.retain(|(seen, _)| *seen == current);
            self.cancelled.retain(|(seen, _)| *seen == current);
        }
        match frame {
            EventsFrame::Ready { client_id } => {
                self.client_id = Some(client_id.clone());
                delta.ready = true;
            }
            EventsFrame::Cancel { event_id } => self.apply_cancel(event_id, &mut delta),
            EventsFrame::Emit { .. } => {}
            EventsFrame::Waterfall {
                event,
                event_id,
                agent_id,
                request,
            } => {
                self.apply_waterfall(
                    event,
                    event_id,
                    agent_id.as_deref(),
                    request,
                    &mut delta,
                );
            }
        }
        delta
    }

    fn apply_cancel(&mut self, event_id: &str, delta: &mut LedgerDelta) {
        self.cancelled
            .insert((self.generation, event_id.to_string()));
        if self.submitting.as_deref() == Some(event_id) {
            self.submitting = None;
            delta.unlocked = true;
        }
        let mut hit = false;
        for kind in [PendingKind::Approval, PendingKind::Question] {
            hit |= self.remove(kind, event_id);
        }
        if hit {
            delta.cancelled.push(event_id.to_string());
            delta.active_changed = true;
        }
    }

    /// 从一张卡的台账里摘掉一条，必要时把队列头推上屏；返回「这一类真少了东西」。
    fn remove(&mut self, kind: PendingKind, event_id: &str) -> bool {
        let (active, queue) = self.slot(kind);
        let before = queue.len();
        queue.retain(|item| item.event_id != event_id);
        let dropped_queued = before != queue.len();
        if active.as_ref().is_some_and(|item| item.event_id == event_id) {
            *active = queue.pop_front();
            return true;
        }
        dropped_queued
    }

    fn apply_waterfall(
        &mut self,
        event: &str,
        event_id: &str,
        agent_id: Option<&str>,
        request: &Value,
        delta: &mut LedgerDelta,
    ) {
        let Some(kind) = PendingKind::of(event) else {
            delta.dropped = Some(FrameDrop::UnknownEvent(event.to_string()));
            return;
        };
        let generation = self.generation;
        if self.cancelled.contains(&(generation, event_id.to_string())) {
            delta.dropped = Some(FrameDrop::Cancelled(event_id.to_string()));
            return;
        }
        if self.seen.contains(&(generation, event_id.to_string())) {
            delta.dropped = Some(FrameDrop::Duplicate(event_id.to_string()));
            return;
        }
        if let Some(existing) = self.find_mut(event_id) {
            // 跨代次的重推：只把代次续上，让 UI 重新亮那张卡，不排第二份。
            existing.generation = generation;
            delta.active_changed = true;
            return;
        }
        self.seen.insert((generation, event_id.to_string()));
        let item = PendingInteraction {
            event_id: event_id.to_string(),
            kind,
            agent_id: agent_id.map(str::to_string),
            request: request.clone(),
            generation,
        };
        let (active, queue) = self.slot(kind);
        delta.enqueued = Some(kind);
        if active.is_none() {
            *active = Some(item);
            delta.active_changed = true;
        } else {
            queue.push_back(item);
        }
    }

    fn slot(
        &mut self,
        kind: PendingKind,
    ) -> (
        &mut Option<PendingInteraction>,
        &mut VecDeque<PendingInteraction>,
    ) {
        match kind {
            PendingKind::Approval => (&mut self.active_approval, &mut self.approval_queue),
            PendingKind::Question => (&mut self.active_question, &mut self.question_queue),
        }
    }

    fn find(&self, event_id: &str) -> Option<&PendingInteraction> {
        [
            self.active_approval.as_ref(),
            self.active_question.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(self.approval_queue.iter().chain(self.question_queue.iter()))
        .find(|item| item.event_id == event_id)
    }

    fn find_mut(&mut self, event_id: &str) -> Option<&mut PendingInteraction> {
        let Self {
            active_approval,
            active_question,
            approval_queue,
            question_queue,
            ..
        } = self;
        active_approval
            .as_mut()
            .into_iter()
            .chain(active_question.as_mut())
            .chain(approval_queue.iter_mut())
            .chain(question_queue.iter_mut())
            .find(|item| item.event_id == event_id)
    }
}

// ==================== 会话历史回读（#78 `session/page`） ====================
//
// 主干真值（`MainWindow.xaml.cs`，行号会漂 ⇒ 一律按函数名定位）：
//   LoadSessionHistoryAsync  分页拉历史的主循环 + `session/title` 的 durable title 回填
//   ProbeJournalCursorAsync  一发越界探测拿真实游标（越界错误自带游标），解不出才对半探测
//   ParsePastCursorSeq       从 "… is past cursor <sourceCursor>" 里取那个数
//   HistoryPageMessages = 5000 / JournalProbeSeq = 1 << 30   两个常量
//   调用点：`OpenSessionAsync` 里 `await LoadSessionHistoryAsync(vm.SessionId)`，
//   **紧接着**才 `await FollowSessionAsync(...)` —— 先回放历史、后开实时流，主干靠这个
//   次序天然避免「历史与在途气泡重叠」。分叉的后台消息没有这个顺序保证，所以下面
//   `drop_overlapped` 再兜一道（判据是「live 赢」）。
// RPC 形状（主干 :4588，已核实）：
//   session/page  params = {request:{address:{kind:"session",sessionId},throughSeq,maxMessages}}
//   回包只读 `records`，每条是 `{type:"event", event:{type,seq,time,data}}`（与 follow 流的
//   `event` 帧同一份形状，所以分叉可以复用 `apply_journal_event` 那一台机器）。
//   越界错误串 = `"session page through seq <n> is past cursor <sourceCursor>"`。

/// 主干 `HistoryPageMessages`：内核 `maxMessages` 只对 user/assistant 消息计数、
/// 且只校验正整数无上限，给 5000 = 「一次拉全」，省掉大会话的几十次翻页往返。
pub const HISTORY_PAGE_MESSAGES: i64 = 5000;

/// 主干 `JournalProbeSeq`：远超任何真实 journal 长度的探测水位，一发就能把真实游标
/// 从越界错误里逼出来（旧版对半探测要 ~16 次往返）。
pub const JOURNAL_PROBE_SEQ: i64 = 1 << 30;

/// 主干对半探测兜底的上界（`ProbeJournalCursorAsync` 里的 `hi = 1 << 16`）。
const CURSOR_BISECT_HI: i64 = 1 << 16;

/// `session/page` 的一发请求（主干两个调用点的并集：游标探测不带 `maxMessages`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryPage {
    pub session_id: String,
    pub through_seq: i64,
    /// `None` = 整个键缺席（主干的游标探测就是这一型）。
    pub max_messages: Option<i64>,
}

impl HistoryPage {
    /// 探测页：`{address, throughSeq = JOURNAL_PROBE_SEQ}`，不带 `maxMessages`。
    pub fn probe(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            through_seq: JOURNAL_PROBE_SEQ,
            max_messages: None,
        }
    }

    /// 翻页：`{address, throughSeq, maxMessages = HISTORY_PAGE_MESSAGES}`。
    pub fn page(session_id: &str, through_seq: i64) -> Self {
        Self {
            session_id: session_id.to_string(),
            through_seq,
            max_messages: Some(HISTORY_PAGE_MESSAGES),
        }
    }

    /// 主干发给 `_rpc.CallOkAsync` 的那层 `{request:…}` 包裹（分叉 `Kernel::call` 的入参）。
    pub fn to_args(&self) -> Value {
        let mut request = json!({
            "address": { "kind": "session", "sessionId": self.session_id },
            "throughSeq": self.through_seq,
        });
        if let Some(max) = self.max_messages {
            request["maxMessages"] = json!(max);
        }
        json!({ "request": request })
    }
}

/// 内核那句越界错误的判据（主干 `catch (DshRpcException ex) when (ex.Message.Contains("past cursor"))`；
/// 分叉的 `Kernel::call` 把错误拼成 `"{code}: {message}"`，同一串文本，判据同源）。
pub fn is_past_cursor(message: &str) -> bool {
    message.contains("past cursor")
}

/// 主干 `ParsePastCursorSeq`：消息形如 `"session page through seq <n> is past cursor <sourceCursor>"`，
/// 取**最后一次**出现的 `"past cursor "`（主干 `LastIndexOf` 同一条），后面整段 trim 再解析。
/// 解不出回 `None` = 内核版本差异，调用方回落到对半探测。
pub fn parse_past_cursor_seq(message: &str) -> Option<i64> {
    const CURSOR_MARK: &str = "past cursor ";
    let at = message.rfind(CURSOR_MARK)?;
    message[at + CURSOR_MARK.len()..].trim().parse().ok()
}

/// 一页 `records` 剥掉 `{type:"event", …}` 那层信封后拿到的事件（形状不对回 `None`）。
pub fn page_event(record: &Value) -> Option<&Value> {
    record.get("event").filter(|event| event.is_object())
}

/// 一页 `records` → 事件数组（**坏记录逐条跳过不炸**：主干 `RenderJournal` 是整页套一层
/// `try/catch`，一条形状坏了那一页就全丢；分叉按条判，坏一条少画一条，不牵连其余）。
pub fn page_events(page: &Value) -> Vec<Value> {
    page["records"]
        .as_array()
        .map(|records| {
            records
                .iter()
                .filter_map(page_event)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// 主干翻下一页取的那把刀：`records[0].event.seq`（页内按 seq 升序，首条即本页最早一条）。
pub fn page_first_seq(page: &Value) -> Option<i64> {
    page["records"]
        .as_array()?
        .first()
        .and_then(page_event)?["seq"]
        .as_i64()
}

/// `hasMore`：缺键按 `false`（主干 :4613 判的是 `ValueKind == True`，缺席就是不走）。
pub fn page_has_more(page: &Value) -> bool {
    page["hasMore"].as_bool().unwrap_or(false)
}

/// 一次历史回读的结果。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    /// 按 seq 升序（= 时间序，可以直接喂 `apply_journal_event` 重放）。
    pub events: Vec<Value>,
    /// journal 里最新的 durable title（`session/title`）；空串 = 一条都没有。
    /// 主干拿它回填「投影缺失的旧会话」的兜底标题（`LoadSessionHistoryAsync` 尾段）。
    pub durable_title: String,
}

/// 把「新→旧」收到的若干页折成时间序：页序倒过来、页内保持升序，顺手折出 durable title。
///
/// 与主干的一处**修正**：主干按取页顺序（新→旧）逐条覆盖 `fallbackTitle`，注释说的是
/// 「按 seq 时间序扫描 ⇒ 最后一条即最新」，但多页会话实际会退回**最早**那条标题；
/// 这里按 seq 取最新（单页会话两者同值，主干今天的常见路径看不出差）。
pub fn fold_history(pages: &[Value]) -> History {
    let mut events: Vec<Value> = Vec::new();
    let mut title: Option<(i64, String)> = None;
    for page in pages.iter().rev() {
        for event in page_events(page) {
            if event["type"].as_str() == Some("session/title") {
                let seq = event["seq"].as_i64().unwrap_or(0);
                if let Some(text) = event["data"]["title"].as_str().filter(|text| !text.is_empty())
                {
                    if title.as_ref().is_none_or(|(last, _)| seq >= *last) {
                        title = Some((seq, text.to_string()));
                    }
                }
            }
            events.push(event);
        }
    }
    History {
        events,
        durable_title: title.map(|(_, text)| text).unwrap_or_default(),
    }
}

/// 一条 journal 事件会画出的气泡坐标（与分叉的 `bubble_key` 同参：role / turn / seq）。
/// 可能两条：`assistant/message` 的 reasoning 段与正文共用信封 seq。
/// 合成行（`system/message`、`assistant/attempt`）的 seq 不是 journal 坐标（分叉那边写死 0
/// 再过 `unique_seq`），不参与重叠判定 ⇒ 回空集 = 「永远不因为重叠被丢」。
pub fn event_coords(event: &Value) -> Vec<BubbleCoord> {
    let turn = event["data"]["turn"].as_i64().unwrap_or(0);
    let seq = event["seq"].as_i64().unwrap_or(0);
    let row = |role: &str| BubbleCoord(role.to_string(), turn, seq);
    match event["type"].as_str().unwrap_or_default() {
        "user/message" => vec![row("user")],
        "assistant/message" => {
            let blocks = event["data"]["message"]["content"].as_array();
            let reasoning = blocks.into_iter().flatten().any(|block| {
                block["type"].as_str() == Some("reasoning")
                    && block["text"].as_str().is_some_and(|text| !text.is_empty())
            });
            let mut coords = vec![row("assistant")];
            if reasoning {
                coords.push(row("reasoning"));
            }
            coords
        }
        "tool/call" => vec![row("tool")],
        "deliverables/presented" => {
            if event["data"]["files"]
                .as_array()
                .is_some_and(|files| files.iter().any(|file| file["path"].is_string()))
            {
                vec![row("deliverable")]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// 一条气泡在 journal 里的坐标（role 用 `'static` 存，装箱一次进元组太碎，故包一层新类型）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BubbleCoord(pub String, pub i64, pub i64);

/// 历史事件 vs 已在图上的气泡坐标：**live 赢**，同坐标的历史条目整条丢掉。
///
/// 为什么是 live 赢：在途那条是 follow 流刚推的、用户已经看见了，journal 快照只是它的
/// 一份复读；重放它只会撞 `keyed_children` 的键，`unique_seq` 会把重复行退到负数区 ⇒
/// 屏幕上同一句话画两遍。判据是「这条事件**会画出的每一格**都已经在图上」才丢：
/// 只画过正文、没画过 reasoning 的（流式那一路不产 reasoning）还得把 reasoning 补上。
pub fn drop_overlapped(events: &[Value], rendered: &[BubbleCoord]) -> Vec<Value> {
    if rendered.is_empty() {
        return events.to_vec();
    }
    events
        .iter()
        .filter(|event| {
            let coords = event_coords(event);
            coords.is_empty()
                || !coords
                    .iter()
                    .all(|coord| rendered.iter().any(|held| held == coord))
        })
        .cloned()
        .collect()
}

/// 历史游标探测（主干 `ProbeJournalCursorAsync`）：越界错误里带真实游标 ⇒ 一次往返；
/// 探测值没越界 ⇒ 真实游标 ≥ 探测值，就以探测值为起点（hasMore 链会继续带到 journal 头部）；
/// 越界但解不出数字 ⇒ 回落对半探测（主干 :4701 那条兜底，最坏 ~16 次往返）。
pub fn probe_journal_cursor(
    session_id: &str,
    send: &mut dyn FnMut(&HistoryPage) -> Result<Value, String>,
) -> Result<i64, String> {
    match send(&HistoryPage::probe(session_id)) {
        Ok(_) => Ok(JOURNAL_PROBE_SEQ),
        Err(message) if is_past_cursor(&message) => parse_past_cursor_seq(&message)
            .filter(|cursor| *cursor >= -1)
            .map_or_else(|| bisect_journal_cursor(session_id, send), Ok),
        Err(message) => Err(message),
    }
}

/// 对半探测：夹逼出最后一个「不越界」的 throughSeq。空日志的会话每一步都越界 ⇒ 回 -1，
/// 上层 `while through >= 0` 直接不进循环（主干同结果：什么都拉不到）。
fn bisect_journal_cursor(
    session_id: &str,
    send: &mut dyn FnMut(&HistoryPage) -> Result<Value, String>,
) -> Result<i64, String> {
    let mut lo = 0;
    let mut hi = CURSOR_BISECT_HI;
    while lo < hi {
        let mid = lo + ((hi - lo + 1) >> 1);
        let probe = HistoryPage {
            session_id: session_id.to_string(),
            through_seq: mid,
            max_messages: None,
        };
        match send(&probe) {
            Ok(_) => lo = mid,
            Err(message) if is_past_cursor(&message) => hi = mid - 1,
            Err(message) => return Err(message),
        }
    }
    Ok(lo)
}

/// 分页拉历史的主循环（主干 `LoadSessionHistoryAsync` 的 4570-4627 那一段）。
///
/// `send` 是「发一页」的注入点：单测拿一张内存页表（不出网），`Kernel::load_session_history`
/// 把它换成 `self.call("session/page", …)`。整条循环都是纯逻辑，翻页判据可单测：
/// · 探测出的游标是起点，`hasMore && firstSeq > 0` 才往更早翻（`through = firstSeq - 1`）；
/// · 空页 / `hasMore != true` / `through < 0` 三个终止条件；
/// · 翻页途中再撞 past-cursor ⇒ 按主干 :4591 的那支**正常收手**（不是错误）；
/// · 其余错误（会话不存在、传输断了）才回 `Err` —— 主干那边整段 catch 掉什么都不画。
pub fn walk_history(
    session_id: &str,
    send: &mut dyn FnMut(&HistoryPage) -> Result<Value, String>,
) -> Result<History, String> {
    let mut through = probe_journal_cursor(session_id, send)?;
    let mut pages: Vec<Value> = Vec::new();
    while through >= 0 {
        let page = match send(&HistoryPage::page(session_id, through)) {
            Ok(page) => page,
            Err(message) if is_past_cursor(&message) => break,
            Err(message) => return Err(message),
        };
        if page["records"].as_array().is_none_or(Vec::is_empty) {
            break;
        }
        let first_seq = page_first_seq(&page);
        let has_more = page_has_more(&page);
        pages.push(page);
        // 首条 seq 读不出来（主干这里会直接抛 ⇒ 整段历史都不画）：分叉保住已收到的页，
        // 只是不再往更早翻，宁少画一段也不要整屏空白。
        let Some(first_seq) = first_seq.filter(|seq| *seq > 0) else {
            break;
        };
        if !has_more {
            break;
        }
        through = first_seq - 1;
    }
    Ok(fold_history(&pages))
}

// ==================== 使用统计聚合（#116 设置·「用量」） ====================
//
// 主干真值（`MainWindow.xaml.cs`；行号会漂 ⇒ 按函数名定位）：
//   LoadUsageStatsAsync         :15030  `session/list` 台账 → 逐会话走查（本模块只吃判据，不碰 RPC）
//   AccumulateSessionUsageAsync :15105  单会话翻页 + turn 配对（→ `scan_session_usage`）
//   AccumulateUsage             :15199  按「本地日 × 模型」双维度入账（→ `SessionScan::feed_usage`）
//   TokenOf                     :15254  token 口径（→ `token_of`）
//   SessionUsageVm              :361    「页内不再二次请求」⇒ 汇总只在取数侧算（→ `SessionUsage`）
//   Streaks                     :15327  连续天数（→ `UsageAggregate::streaks`）
//   RenderStatsSourceNote       :15354  `{2}{3}` 两半 = 跳过句 / 分页上限句，计数为 0 时是空串
// 两条与直觉相反、已在原文核实的量：每页 `maxMessages = 500`、单会话上限 40 页（:15113/:15123），
// 不是内核默认的 50；翻页刀取 `records[0]` 的 seq（:15145）⇒ 页内按 seq **升序**，turn 配对天然有序。
// 本模块一律纯函数、**不读时钟**：主干 `DayKey`（:15271）走 `ToLocalTime()`，分叉把「本地 UTC 偏移」
// 与「今天是哪一天」都做成入参，由 `main.rs` 那侧现取（`GetLocalTime`），单测于是能钉死跨日与跨年。

/// 主干 `maxMessages`（:15123 实发值）。
pub const STATS_PAGE_MESSAGES: i64 = 500;
/// 主干 `maxPages`（:15113）：走满 ⇒ `pages_capped++`，该会话只算部分计入。
pub const STATS_MAX_PAGES: usize = 40;
/// 主干 `AccumulateUsage` 里模型名的兜底（:15210）；显示时再过 `L()`。
pub const UNLABELED_MODEL: &str = "未标注模型";

const DAY_MS: i64 = 86_400_000;

/// 主干 `RenderStatsHeatmap`（:15371）的 `const int weeks = 26`。
pub const STATS_HEAT_WEEKS: usize = 26;

/// 热力图口径（主干 `_statsHeatMetric`，:14211 默认 `"day"`；选择器是那条 SelectorBar）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeatMetric {
    /// 每日：格子 = 当天的 token 合计。
    #[default]
    Day,
    /// 每周：同周 7 格都取该周合计（主干 :15393-15403，周日往后不多截，缺的日子天然计 0）。
    Week,
    /// 累计：从窗口第一天起逐日累加（主干 :15404-15407）。
    Total,
}

/// 日序 → 星期几，**0 = 周一**（主干 `(int)DayOfWeek + 6) % 7` 的同果版）。
/// 日序 0 = 1970-01-01 = 周四 ⇒ 偏移 3。
pub fn weekday_index(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

/// 日序 → 该周周一的日序。
pub fn monday_of(day: i64) -> i64 {
    day - weekday_index(day)
}

/// 热力窗口 `[start, end]`（主干 :15381-15382：末列的周一 = 今天减「本周已过天数」，
/// 起点再往前 `weeks-1` 周）。`weeks == 0` ⇒ 空窗（`end < start`，渲染侧 `for` 不进）。
pub fn heat_window(today: i64, weeks: usize) -> (i64, i64) {
    (
        monday_of(today) - (weeks.saturating_sub(1) as i64) * 7,
        today,
    )
}

/// 主干 `HeatLevel`（:15467）：`value <= 0 || max <= 0` ⇒ 0 档，其余按 value/max 比值分四档。
pub fn heat_level(value: i64, max: i64) -> i32 {
    if value <= 0 || max <= 0 {
        return 0;
    }
    let ratio = value as f64 / max as f64;
    if ratio <= 0.25 {
        1
    } else if ratio <= 0.5 {
        2
    } else if ratio <= 0.75 {
        3
    } else {
        4
    }
}

/// 主干 `HeatBrush`（:15484）：强调色只压不透明度，**不新增色值**。0 档不走这条（用空色底板）。
pub fn heat_opacity(level: i32) -> f64 {
    match level {
        1 => 0.25,
        2 => 0.45,
        3 => 0.7,
        _ => 1.0,
    }
}

/// 主干 `TokenOf`：`totalTokens` 是数值且 `> 0` 就用它，否则 4 桶相加（与内核 token-meter 一致）。
/// 逐桶 `(long)GetDouble()` ⇒ 小数**先截后加**，不是加完再截。
pub fn token_of(usage: &Value) -> i64 {
    let bucket = |key: &str| usage[key].as_f64().unwrap_or(0.0) as i64;
    let total = bucket("totalTokens");
    if total > 0 {
        return total;
    }
    ["inputTokens", "outputTokens", "cacheReadTokens", "cacheWriteTokens"]
        .iter()
        .map(|key| bucket(key))
        .sum()
}

/// 主干 `DayKey` 的整数形态：epoch 毫秒 + 本地 UTC 偏移（秒）→ 距 1970-01-01 的本地整天数。
/// `div_euclid` 保证负数（1970 前）也落在正确的格子里。
pub fn day_index(epoch_ms: i64, tz_offset_secs: i64) -> i64 {
    (epoch_ms + tz_offset_secs * 1000).div_euclid(DAY_MS)
}

/// 天数 → 主干显示用的 `yyyy-MM-dd`（`civil_from_days`，公历 proleptic）。
pub fn format_day(day: i64) -> String {
    let (year, month, day) = civil_from_days(day);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant `civil_from_days` 的 i64 版：天数 → (年, 月, 日)。
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 主干 :15068-15082 的台账判据（纯函数）：`blank` 会话、以及没有 `asOfSeq`（主干缺键按 -1）
/// 或 `asOfSeq < 0` 的会话都不走查，两者都进 `sessions_skipped`；`sessionId` 空由调用方丢弃
/// （主干那里 `continue` 而**不计数**）。`Some(_)` = 首页的 `throughSeq`。
pub fn ledger_through(blank: bool, as_of_seq: Option<i64>) -> Option<i64> {
    if blank {
        return None;
    }
    as_of_seq.filter(|seq| *seq >= 0)
}

/// 主干 `SessionUsageVm`（:361）：单会话的 journal 走查汇总，页内不再二次请求。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionUsage {
    pub session_id: String,
    /// `session/list` 的 `updatedAt`（缺键按 0，主干同）。
    pub updated_at: i64,
    /// journal 首末事件跨度（主干 `maxTime > minTime ? maxTime - minTime : 0`）：**存活窗口**，不是聊天时长。
    pub span_ms: i64,
    /// 各 `turn/start → turn/end` 时长之和 = 主干「最长聊天时长」的口径。
    pub talk_ms: i64,
}

/// (键, token 增量) 折叠进一张「首现序」小表：主干用的是 `Dictionary`，LINQ 稳定序 ⇒ 并列时
/// 保持首现序，所以这里不能用 `HashMap`，也不能按名字排序。
fn push_amount<T: PartialEq>(table: &mut Vec<(T, i64)>, key: T, tokens: i64) {
    if let Some(index) = table.iter().position(|(row, _)| *row == key) {
        table[index].1 += tokens;
    } else {
        table.push((key, tokens));
    }
}

/// 同一条折叠，键是「本地日 × 模型」两列（`(日, 模型, tokens)` 那张明细表）。
fn push_entry(entries: &mut Vec<(i64, String, i64)>, day: i64, model: String, tokens: i64) {
    if let Some(index) = entries
        .iter()
        .position(|(row_day, row_model, _)| *row_day == day && *row_model == model)
    {
        entries[index].2 += tokens;
    } else {
        entries.push((day, model, tokens));
    }
}

/// 一个会话走查过程中的累加器（主干 `AccumulateSessionUsageAsync` 的那组局部变量 +
/// `AccumulateUsage` 的入账）。字段全私有 ⇒ 只能经 `feed_page` 前进，判据不会被绕过。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionScan {
    session_id: String,
    updated_at: i64,
    min_time: Option<i64>,
    max_time: Option<i64>,
    talk_ms: i64,
    /// 主干 `turnStartTime`：`Some(_)` = 在途 turn（含 time==0 那一支，主干照样记账）。
    turn_start: Option<i64>,
    /// 成功收到的页数（主干 `pages++` 在 `catch` 之后 ⇒ 报错那一发不计数）。
    pages: usize,
    usage_messages: i64,
    /// `(本地日, 模型, tokens)`，首现序。
    entries: Vec<(i64, String, i64)>,
}

impl SessionScan {
    pub fn new(session_id: &str, updated_at: i64) -> Self {
        Self {
            session_id: session_id.to_string(),
            updated_at,
            ..Self::default()
        }
    }

    /// 走完时是否触到分页上限（主干 `pages >= maxPages`，含等号）。
    pub fn pages_capped(&self) -> bool {
        self.pages >= STATS_MAX_PAGES
    }

    pub fn usage(&self) -> SessionUsage {
        SessionUsage {
            session_id: self.session_id.clone(),
            updated_at: self.updated_at,
            span_ms: match (self.min_time, self.max_time) {
                (Some(min), Some(max)) if max > min => max - min,
                _ => 0,
            },
            talk_ms: self.talk_ms,
        }
    }

    /// 一页 `session/page` 折进累加器（主干 :15136-15167 的那个 `foreach`）。
    /// 坏记录（`event` 不是对象）逐条跳过，与 `page_events` 同一条口径。
    pub fn feed_page(&mut self, page: &Value, tz_offset_secs: i64) {
        self.pages += 1;
        for event in page_events(page) {
            self.feed_event(&event, tz_offset_secs);
        }
    }

    fn feed_event(&mut self, event: &Value, tz_offset_secs: i64) {
        let time = event["time"].as_f64().unwrap_or(0.0) as i64;
        if time > 0 {
            self.min_time = Some(self.min_time.map_or(time, |min| min.min(time)));
            self.max_time = Some(self.max_time.map_or(time, |max| max.max(time)));
        }
        match event["type"].as_str() {
            Some("turn/start") => self.turn_start = Some(time),
            Some("turn/end") => {
                // 主干 :15155-15159：配不上（起点缺失/为 0/时间倒挂）也要作废在途 turn。
                if let Some(start) = self.turn_start.take() {
                    if start > 0 && time >= start {
                        self.talk_ms += time - start;
                    }
                }
            }
            Some("assistant/message") => self.feed_usage(event, time, tz_offset_secs),
            _ => {}
        }
    }

    fn feed_usage(&mut self, event: &Value, time: i64, tz_offset_secs: i64) {
        let data = &event["data"];
        if !data["usage"].is_object() {
            return;
        }
        let tokens = token_of(&data["usage"]);
        if tokens <= 0 {
            return;
        }
        push_entry(
            &mut self.entries,
            day_index(time, tz_offset_secs),
            model_of(data),
            tokens,
        );
        self.usage_messages += 1;
    }
}

/// 主干 :15212-15221：`data.message.source.{provider,model}`，`model` 空 ⇒ `未标注模型`。
fn model_of(data: &Value) -> String {
    let source = &data["message"]["source"];
    let name = source["model"].as_str().unwrap_or("");
    if name.is_empty() {
        return UNLABELED_MODEL.to_string();
    }
    let provider = source["provider"].as_str().unwrap_or("");
    if provider.is_empty() {
        name.to_string()
    } else {
        format!("{provider}/{name}")
    }
}

/// 翻页刀：主干 `firstSeq` = **第一条带数值 seq 的事件**（`page_events` 已经滤掉无 `event` 的记录）。
fn stats_first_seq(page: &Value) -> Option<i64> {
    page_events(page).iter().find_map(|event| event["seq"].as_i64())
}

/// 主干 `AccumulateSessionUsageAsync`（:15105）的完整走查。`send` 是「发一页」的注入点：
/// 单测喂内存页表，`main.rs` 侧换成 `kernel.call("session/page", …)`。
/// 与主干逐条对齐的四个终止条件：`through < 0` / 走满 40 页 / `records` 缺失或空 /
/// `hasMore != true` 或翻页刀解不出来；**RPC 报错也 break**（主干 `catch (DshRpcException)`
/// 那一支：会话在两次调用之间被归档删除 ⇒ 保留已聚合部分，整条走查不回错误）。
pub fn scan_session_usage(
    session_id: &str,
    updated_at: i64,
    as_of_seq: i64,
    tz_offset_secs: i64,
    send: &mut dyn FnMut(&HistoryPage) -> Result<Value, String>,
) -> SessionScan {
    let mut scan = SessionScan::new(session_id, updated_at);
    let mut through = as_of_seq;
    while through >= 0 && scan.pages < STATS_MAX_PAGES {
        let request = HistoryPage {
            session_id: session_id.to_string(),
            through_seq: through,
            max_messages: Some(STATS_PAGE_MESSAGES),
        };
        let Ok(page) = send(&request) else {
            break;
        };
        scan.feed_page(&page, tz_offset_secs);
        if page["records"].as_array().is_none_or(Vec::is_empty) {
            break;
        }
        let Some(first_seq) = stats_first_seq(&page).filter(|seq| *seq > 0) else {
            break;
        };
        if !page_has_more(&page) {
            break;
        }
        through = first_seq - 1;
    }
    scan
}

/// 一个本地日的两列汇总：当日 token 合计 + (模型 → token) 首现序。
/// 主干是 `_statsDayTotals`（`SortedDictionary`）与 `_statsDayModel` 两张字典，分叉合成一张
/// ⇒ 「同键同序」那条不变式不用另外维护（合成前这里就是一个 desync 面）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DayUsage {
    pub day: i64,
    pub tokens: i64,
    /// 模型保持首现序 = 主干 `Dictionary` 的枚举序（趋势图图例与占比条都按它派生）。
    pub models: Vec<(String, i64)>,
}

/// 一次全量聚合的结果（主干 `_statsDayTotals` / `_statsDayModel` / `_statsSessionUsage`
/// 加那三个计数器的分叉版）。`main.rs` 只读它，渲染路径不再算任何数。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UsageAggregate {
    /// **按日升序**（`yyyy-MM-dd` 的序数序就是日期序，与主干 `SortedDictionary` 同）。
    pub days: Vec<DayUsage>,
    pub sessions: Vec<SessionUsage>,
    /// 计入的 `assistant/message` 条数（口径自证，主干 `_statsUsageMessages`）。
    pub usage_messages: i64,
    /// 真正走查过的会话数 = 来源行的 `{0}`。
    pub sessions_scanned: i64,
    /// 空会话 / 无 journal 游标 ⇒ 来源行的 `{2}`。
    pub sessions_skipped: i64,
    /// 触到分页上限的会话数 ⇒ 来源行的 `{3}`。
    pub pages_capped: i64,
}

impl UsageAggregate {
    /// 一个会话走完 ⇒ 记账 + 折进两张日表（主干 :15081-15082 与 `AccumulateUsage` 的落点）。
    pub fn absorb(&mut self, scan: &SessionScan) {
        self.sessions_scanned += 1;
        if scan.pages_capped() {
            self.pages_capped += 1;
        }
        self.usage_messages += scan.usage_messages;
        self.sessions.push(scan.usage());
        for (day, model, tokens) in &scan.entries {
            let slot = self.day_slot(*day);
            slot.tokens += tokens;
            push_amount(&mut slot.models, model.clone(), *tokens);
        }
    }

    /// 台账判为「不走查」的会话（主干 `_statsSessionsSkipped++`）。
    pub fn skip_session(&mut self) {
        self.sessions_skipped += 1;
    }

    /// 升序日表里那一天的槽位（不在就按序插进去）。
    fn day_slot(&mut self, day: i64) -> &mut DayUsage {
        match self.days.binary_search_by_key(&day, |slot| slot.day) {
            Ok(index) => &mut self.days[index],
            Err(index) => {
                self.days.insert(
                    index,
                    DayUsage {
                        day,
                        tokens: 0,
                        models: Vec::new(),
                    },
                );
                &mut self.days[index]
            }
        }
    }

    /// 全部历史的 token 合计（主干 KPI 第一枚 `total`：Σ`_statsDayTotals.Values`）。
    pub fn total(&self) -> i64 {
        self.days.iter().map(|day| day.tokens).sum()
    }

    /// 单日峰值 (tokens, 日)：主干 `v > peak` 的**严格**大于 ⇒ 并列取最早那天。
    pub fn peak(&self) -> Option<(i64, i64)> {
        let mut best: Option<(i64, i64)> = None;
        for day in &self.days {
            if best.is_none_or(|(peak, _)| day.tokens > peak) {
                best = Some((day.tokens, day.day));
            }
        }
        best
    }

    /// 「最长聊天时长」= 各会话 `talk_ms` 的最大值（主干 `Max(s => s.TalkMs)`，无会话按 0）。
    pub fn longest_talk_ms(&self) -> i64 {
        self.sessions.iter().map(|s| s.talk_ms).max().unwrap_or(0)
    }

    pub fn active_days(&self) -> usize {
        self.days.len()
    }

    /// 主干 `Streaks`（:15327）：升序去重日上跑连日计数；当前连续从今天往回数，
    /// 今天还没产生用量就从昨天起算。
    pub fn streaks(&self, today: i64) -> (i64, i64) {
        let days: Vec<i64> = self.days.iter().map(|day| day.day).collect();
        if days.is_empty() {
            return (0, 0);
        }
        let mut longest = 1;
        let mut run = 1;
        for pair in days.windows(2) {
            if pair[1] == pair[0] + 1 {
                run += 1;
            } else {
                run = 1;
            }
            if run > longest {
                longest = run;
            }
        }
        let mut current = 0;
        let mut cursor = if days.contains(&today) { today } else { today - 1 };
        while days.contains(&cursor) {
            current += 1;
            cursor -= 1;
        }
        (current, longest)
    }

    /// 五枚 KPI 的全部输入（含无数据档要的三处计数）。
    pub fn kpis(&self, today: i64) -> UsageKpis {
        let (current_streak, longest_streak) = self.streaks(today);
        UsageKpis {
            total_tokens: self.total(),
            peak_tokens: self.peak().map_or(0, |(tokens, _)| tokens),
            peak_day: self.peak().map(|(_, day)| day),
            longest_talk_ms: self.longest_talk_ms(),
            current_streak,
            longest_streak,
            active_days: self.days.len() as i64,
            usage_messages: self.usage_messages,
            session_count: self.sessions.len() as i64,
            sessions_scanned: self.sessions_scanned,
            sessions_skipped: self.sessions_skipped,
            pages_capped: self.pages_capped,
        }
    }

    /// 主干 `RangeDays(days)`（:15771）的整数形态：`today-days+1 ..= today`（含今天）。
    /// `days <= 0` 主干给空列表，这里同样给空。
    fn range(&self, today: i64, days: i64) -> Vec<i64> {
        if days <= 0 {
            return Vec::new();
        }
        (today.saturating_sub(days - 1)..=today).collect()
    }

    /// 范围内逐日 token（无记录的那天给 0）——趋势图纵轴的输入序列。
    pub fn day_series(&self, today: i64, days: i64) -> Vec<(i64, i64)> {
        self.range(today, days)
            .into_iter()
            .map(|day| (day, self.day_tokens(day)))
            .collect()
    }

    /// 某一天的 token 合计；那天没记录 ⇒ 0（主干 `_statsDayTotals.TryGetValue`）。
    pub fn day_tokens(&self, day: i64) -> i64 {
        self.days
            .iter()
            .find(|slot| slot.day == day)
            .map_or(0, |slot| slot.tokens)
    }

    /// 热力格子的值表（主干 `RenderStatsHeatmap` 里那张 `values` 字典，:15385-15412）：
    /// 窗口 = 「本周周一往前 `weeks-1` 周」到**今天**，逐日一条，按口径换算。
    /// 没数据也照样给满一窗的 0 ⇒ 主干那句「空态下仍铺 26 周空格子」的同果，
    /// 渲染侧不许再往这里补数。
    pub fn heat_cells(&self, today: i64, weeks: usize, metric: HeatMetric) -> Vec<(i64, i64)> {
        let (start, end) = heat_window(today, weeks);
        let mut cells = Vec::new();
        let mut running = 0i64;
        for day in start..=end {
            let value = match metric {
                HeatMetric::Week => {
                    let monday = monday_of(day);
                    (0..7).map(|offset| self.day_tokens(monday + offset)).sum()
                }
                HeatMetric::Total => {
                    running += self.day_tokens(day);
                    running
                }
                HeatMetric::Day => self.day_tokens(day),
            };
            cells.push((day, value));
        }
        cells
    }

    /// 范围内按模型的 token 合计，**降序**（主干 `OrderByDescending`：LINQ 稳定 ⇒ 并列保首现序，
    /// `sort_by` 也是稳定排序）。
    pub fn model_totals(&self, today: i64, days: i64) -> Vec<(String, i64)> {
        let mut totals: Vec<(String, i64)> = Vec::new();
        for day in self.range(today, days) {
            let Some(slot) = self.days.iter().find(|slot| slot.day == day) else {
                continue;
            };
            for (model, tokens) in &slot.models {
                push_amount(&mut totals, model.clone(), *tokens);
            }
        }
        totals.sort_by(|left, right| right.1.cmp(&left.1));
        totals
    }

    /// 范围内的 token 合计（趋势图/模型卡那两句 UIA 名的 `{1}`）。
    pub fn range_total(&self, today: i64, days: i64) -> i64 {
        self.range(today, days).iter().map(|day| self.day_tokens(*day)).sum()
    }
}

/// 五枚 KPI + 来源行四个占位的渲染输入（渲染侧只做「填模板」，不做判据）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsageKpis {
    pub total_tokens: i64,
    pub peak_tokens: i64,
    /// 峰值那天（`None` = 一条用量都没有）。
    pub peak_day: Option<i64>,
    pub longest_talk_ms: i64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub active_days: i64,
    pub usage_messages: i64,
    /// 走查过的会话数（第三枚 KPI 的 hint 用它，主干 `_statsSessionUsage.Count`）。
    pub session_count: i64,
    pub sessions_scanned: i64,
    pub sessions_skipped: i64,
    pub pages_capped: i64,
}

/// 来源行的 `{2}{3}` 两半（主干 :15356-15358 的两个三元式）：计数为 0 ⇒ **空串**，
/// 不是「跳过 0 个」。文案一律过 `Catalog` ⇒ 中英两侧都由 i18n 表钉着，不在这里写死。
pub fn source_note_tails(catalog: &Catalog, kpis: &UsageKpis) -> (String, String) {
    let skipped = if kpis.sessions_skipped > 0 {
        catalog.lf(
            "，跳过 {0} 个空会话",
            &[kpis.sessions_skipped.to_string()],
        )
    } else {
        String::new()
    };
    let capped = if kpis.pages_capped > 0 {
        catalog.lf(
            "；{0} 个超长会话触到分页上限，其数据为部分计入",
            &[kpis.pages_capped.to_string()],
        )
    } else {
        String::new()
    };
    (skipped, capped)
}

/// 一趟用量聚合的两种失败档（主干 `LoadUsageStatsAsync` 的两支，分叉建模成枚举 ⇒ 渲染侧
/// 分得清「Error 横幅」与「Warning 横幅」，而不是只拿到一句人话）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageError {
    /// 主干 :15050：`session/list` 回来了但没有 `items` 数组。
    NoLedger,
    /// 主干 :15093 `catch`：RPC 抛异常（传输断、内核拒）。
    Failed(String),
}

/// 主干 `LoadUsageStatsAsync`（:15030）的整趟：`session/list` 台账 → 逐会话 `session/page`
/// 走查 → 聚合。`call` 是注入点（单测喂假台账；`Kernel::collect_usage_stats` 换成 `self.call`）。
/// 全程只发 RPC 与算数，**不碰渲染**，也不读时钟（`tz_offset_secs` 由调用方给）。
/// 主干那三处「本轮开始前清零」在这里就是新建一个 `UsageAggregate`，语义一致。
pub fn collect_usage_stats(
    call: &mut dyn FnMut(&str, Value) -> Result<Value, String>,
    tz_offset_secs: i64,
) -> Result<UsageAggregate, UsageError> {
    let value = call("session/list", json!({ "_request": {} })).map_err(UsageError::Failed)?;
    if !value["items"].is_array() {
        return Err(UsageError::NoLedger);
    }
    let mut aggregate = UsageAggregate::default();
    for row in parse_session_rows(&value) {
        if row.info.id.is_empty() {
            // 主干 :15069 `sid.Length == 0 ⇒ continue`：这条**既不扫也不跳**，两个计数都不许动。
            continue;
        }
        let Some(through) = ledger_through(row.info.blank, row.projections.as_of_seq) else {
            aggregate.skip_session();
            continue;
        };
        let scan = scan_session_usage(
            &row.info.id,
            row.info.updated_at,
            through,
            tz_offset_secs,
            &mut |request| call("session/page", request.to_args()),
        );
        aggregate.absorb(&scan);
    }
    Ok(aggregate)
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

// ------------------------------------------------------------------ 新建会话的 cwd
//
// 零依赖口径同 `procguard.rs:85-109` / `keys.rs:35-42`：手声明 Win32 入口，**不新增 Cargo 依赖**
// （`--offline`，`Cargo.lock` 里没有 `windows` 主 crate）。

/// `GUID`（`knownfolders.h` 的 `struct IID` 布局：一个 u32 + 两个 u16 + 八个 u8）。
#[repr(C)]
struct KnownFolderId {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// `FOLDERID_Documents = {FDD39AD0-238F-46AF-ADB4-6C85480369C7}`（`knownfolders.h`）。
///
/// **字节必须逐字抄对**：这颗 GUID 错一位，`SHGetKnownFolderPath` 回的是
/// `0x80070002`（`ERROR_FILE_NOT_FOUND`）而不是「GUID 非法」，于是调用点只看得到
/// 「解析失败」，完全指不回真正的原因。上一版这里写的正是错的
/// `{FDD39AD0-1043-4CEE-BEBE-4DDC30CC8C21}`（只有前 32 位对上了），结果
/// `default_session_cwd()` 恒为空串 —— 等于这条修复根本没生效。
/// 认法不再靠人眼：`session_cwd_is_the_documents_known_folder` 拿另一条**不吃这颗 GUID**
/// 的老口径 `SHGetFolderPathW(CSIDL_PERSONAL)`（.NET `GetFolderPath(MyDocuments)` 的同源问题）
/// 交叉比对，GUID 一错两边就对不上 ⇒ 红。
const FOLDERID_DOCUMENTS: KnownFolderId = KnownFolderId {
    data1: 0xFDD39AD0,
    data2: 0x238F,
    data3: 0x46AF,
    data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
};

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        folder_id: *const KnownFolderId,
        flags: u32,
        token: *mut std::ffi::c_void,
        path: *mut *mut u16,
    ) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(pointer: *mut std::ffi::c_void);
}

/// 用户「文档」目录的绝对路径；解析不出来 ⇒ `None`。
/// 主干 `Environment.GetFolderPath(SpecialFolder.MyDocuments)`（`MainWindow.xaml.cs:4606-4607`）
/// 在 Windows 上的本体就是这颗 `SHGetKnownFolderPath`，故走同一个 API、而不是自己拼
/// `%USERPROFILE%\Documents`：后者在 OneDrive 重定向 / 域环境里会指到别的目录上去。
/// `flags = KF_FLAG_DEFAULT(0)`、`token = NULL` ⇒ 当前进程令牌所属用户。
fn documents_folder() -> Option<String> {
    let mut buffer: *mut u16 = std::ptr::null_mut();
    let code = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_DOCUMENTS,
            0,
            std::ptr::null_mut(),
            &mut buffer,
        )
    };
    if code != 0 || buffer.is_null() {
        return None;
    }
    let mut units: Vec<u16> = Vec::new();
    let mut offset = 0usize;
    loop {
        let unit = unsafe { *buffer.add(offset) };
        if unit == 0 {
            break;
        }
        units.push(unit);
        offset += 1;
    }
    unsafe { CoTaskMemFree(buffer as *mut std::ffi::c_void) };
    let text = String::from_utf16_lossy(&units);
    (!text.is_empty()).then_some(text)
}

/// 缺口 #85 第四条：`session/create` 该带的 cwd。
///
/// 主干 `CreateSessionCoreAsync`（`MainWindow.xaml.cs:4620-4628`）二选一：
/// 选了工作区 ⇒ `request["workspaceId"]`，否则 ⇒ `request["cwd"] = DefaultSessionCwd()`
/// （= 文档目录；`:4604-4605` 的理由写得很直白 —— 打包态进程 CWD 是 system32、不可写，
/// 工具全废，所以必须显式给一个可写目录）。分叉原先两处调用都传空串/进程 CWD，
/// 内核只能自己兜底，落到哪个目录取决于分叉从哪儿被启动。
///
/// **未移植的一半**：`workspaceId` 那一支依赖输入区的工作区选择器（#56）与工作区 CRUD
/// （#75）——分叉现在根本没有「待用工作区」这份状态，所以这里只出回落档 `cwd`，
/// 不去伪造一个 workspaceId（两支的形状由 [`session_create_location`] 钉住）。
pub fn default_session_cwd() -> String {
    resolve_session_cwd(documents_folder().as_deref())
}

/// 纯函数（可注入）：文档目录这一档到底填什么。
///
/// 口径 = 主干 `DefaultSessionCwd()`（`MainWindow.xaml.cs:4606-4607`）本体
/// `Environment.GetFolderPath(SpecialFolder.MyDocuments)`：该 API 的官方约定是
/// 「文件夹不物理存在 ⇒ 返回空串」，**没有任何二级回落**。所以这里
/// `None ⇒ ""`，而不是退回进程 CWD / `%TEMP%` / `LOCALAPPDATA` —— 后者正是主干注释
/// `:4604-4605` 点名的反例（打包态进程 CWD = system32，发出去工具全废）。
/// 参数用 `Option<&str>` 表示「OS 那侧解析失败」，与「解析出一个空串」在这一档同形。
pub fn resolve_session_cwd(documents: Option<&str>) -> String {
    documents.unwrap_or_default().to_string()
}

/// 纯函数（可注入）：主干 `session/create` 的 `request` 里 location 那一档的形状
/// （`MainWindow.xaml.cs:4621-4628`）—— **二选一，不是并列**：
/// `workspaceId` 非空 ⇒ 只发 `workspaceId`（此时 `cwd` 键整个不存在）；否则 ⇒ 只发 `cwd`。
/// 分叉产品侧目前只有 `cwd` 那一支（`workspace_id` 恒 `None`，见 [`default_session_cwd`] 的说明），
/// 两支的形状都由这里一处产出，工作区选择器接上时不必再改协议层。
pub fn session_create_location(workspace_id: Option<&str>, documents: Option<&str>) -> Value {
    match workspace_id.filter(|id| !id.is_empty()) {
        Some(id) => json!({ "workspaceId": id }),
        None => json!({ "cwd": resolve_session_cwd(documents) }),
    }
}

/// 纯函数（可注入）：主干 `CreateSessionCoreAsync` 那个 `Dictionary<string, object> request`
/// 的**完整**形状 = location 那一档（[`session_create_location`]）+ 可选的 `agentPreset`。
///
/// 主干原文（`MainWindow.xaml.cs:4621-4631`）逐字对到这里三条：
/// · `if (_pendingWorkspace is { Id.Length: > 0 } ws) request["workspaceId"] = ws.Id; else
///   request["cwd"] = DefaultSessionCwd();` ⇒ location 二选一，见上面那颗；
/// · `if (_pendingPreset is { Id.Length: > 0 } preset) request["agentPreset"] = preset.Id;`
///   ⇒ `agentPreset` 带的是 **id**（不是名字），且**空 id 整键不发**（不是发一个空串）；
/// · 主干 `_pendingPreset` 有初值 `("standard", "标准模式")`（`:2297`）且从不清零
///   （`PickAgentPreset:4787-4791` 只会覆盖它）⇒ 常态是**每发新建会话都带** `agentPreset`，
///   这点反直觉，所以把它写成能被打断言的样子。
///
/// 为什么再拆一颗纯函数而不直接在 `Kernel::create_session_with` 里拼：那一颗要活连接才跑得动，
/// `agentPreset` 那三条（带 id / 空值不带键 / 与 location 并列）就钉不住 ⇒ 离线测没落点。
pub fn session_create_request(
    workspace_id: Option<&str>,
    documents: Option<&str>,
    agent_preset: Option<&str>,
) -> Value {
    let mut request = session_create_location(workspace_id, documents);
    if let Some(preset) = agent_preset.filter(|id| !id.is_empty()) {
        request["agentPreset"] = json!(preset);
    }
    request
}

impl Kernel {
    pub fn start(launch: &Launch) -> Result<Self, String> {
        let mut command = Command::new(&launch.exe);
        if launch.args_verbatim {
            // #84-A 回退级：那一串嵌套引号必须**原样**进命令行。用 `raw_arg` 而不是 `args`，
            // 因为 std 的列表转义会把内嵌 `"` 变成 `\"`，而 cmd.exe 不认反斜杠转义引号 ——
            // 那样主干 `/c ""<dsh>" web --no-open --port 0"` 的形状就保不住（§4-① 的根因）。
            // `args_verbatim` 为真时 `args` 恒为一颗（由 `launcher_argv` 保证），join 即原串。
            #[allow(unused_imports)]
            use std::os::windows::process::CommandExt;
            command.raw_arg(launch.args.join(" "));
        } else {
            command.args(&launch.args);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &launch.working_dir {
            command.current_dir(dir);
        }
        // 两级共用同一份 env（主干 `DshKernelHost.cs:90-94` 在 `ResolveLauncher` 之外统一注入、
        // 无分支）。`Kernel\bin` 不存在时**不**注入 PATH —— 对齐 `DshPluginBootstrap.cs:591-594`
        // 那句 `return`；老代码这里没这个早退，会在目录缺失时把一条不存在的路径塞进内核 PATH。
        for (key, value) in build_env(
            launch.dsh_home.as_deref(),
            launch.path_prepend.as_deref(),
            launch.path_prepend.as_ref().is_some_and(|bin| bin.is_dir()),
            std::env::var("PATH").ok().as_deref(),
        ) {
            command.env(key, value);
        }
        #[allow(unused_imports)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().map_err(|e| format!("内核启动失败: {e}"))?;
        // 缺口 #68 ①：紧跟 spawn 挂「壳亡即杀」作业对象 —— 主干 `DshKernelHost.cs:101` 也是
        // `_process.Start()` 的下一行就 `AttachToKillJob()`，**没有** CREATE_SUSPENDED 那一步，
        // 所以同一处竞态两家都有：内核若在指派完成之前就生了子进程，那一批子进程不在这个作业里。
        // 这里不发明挂起路径（多一个失败模式、且 std 的 Command 拿不到 CREATE_SUSPENDED 之后的恢复口），
        // 靠 `procguard::sweep_orphan_kernels()` 兜同一批漏网（主干 `:168` 那句「仍有 SweepOrphanKernels 兜底」）。
        // 句柄必须留到 `shutdown()`/`Drop` 才关（主干 `:155`），故先揣进 `job`，失败也只落一行诊断。
        let job = crate::procguard::KillJob::attach(child_process_handle(&child));

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
        // 缺口 #68 ⑤：作业对象到底挂没挂上，运行时只能从这一行看出来（主干那句是
        // `DshKernelHost.cs:213` 的 `IsProcessInJob` 复查 + `Debug.WriteLine`）。`log` 会经
        // `Msg::Connected` → `push_log` → `emit_line("DIAG: …")` 出到 stdout / `BLADE2_RS_LOG`。
        log.push(format!(
            "内核作业对象(pid {}): {}",
            child.id(),
            job.note
        ));
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
            job,
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

    /// `$events/result` 回帧出口（#74）：`args` 逐字是 `{clientId,eventId,outcome}`，
    /// 只能由 `InteractionLedger::result_request` 造出来 —— 那里守着代次与键集合。
    pub fn send_event_result(&mut self, request: &EventResultRequest) -> Result<(), String> {
        self.call(EVENT_RESULT_METHOD, request.to_args())
            .map(|_| ())
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

    /// #78：分页拉某会话的历史 journal。纯逻辑全在 `walk_history`（含游标探测），
    /// 这里只把它接到 RPC 管道上 —— `Kernel::call` 已经把信封的 `ok` 剥掉、
    /// 失败拼成 `"{code}: {message}"`，past-cursor 判据因此能在 `Msg` 层拿到（#68 同一条路）。
    pub fn load_session_history(&mut self, session_id: &str) -> Result<History, String> {
        walk_history(session_id, &mut |page| {
            self.call("session/page", page.to_args())
        })
    }

    /// #116：设置·「用量」那一趟全量聚合。逻辑全在自由函数 [`collect_usage_stats`]（可离线测），
    /// 这里只把它接到 RPC 管道上。
    pub fn collect_usage_stats(
        &mut self,
        tz_offset_secs: i64,
    ) -> Result<UsageAggregate, UsageError> {
        collect_usage_stats(&mut |method, args| self.call(method, args), tz_offset_secs)
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

    /// `session/create`：只带 location（`cwd`），**不带**待用的 Agent 预设。
    /// 主干 `CreateSessionCoreAsync`（`MainWindow.xaml.cs:4620-4632`）另有 `workspaceId`
    /// （选了工作区时**替换** cwd，不是并列）与 `agentPreset` 两个可选键 ⇒ 那两半都齐时请改走
    /// [`Self::create_session_with`]；这一颗是它的 `agent_preset = None` 特例，形状逐字同
    /// （`request` 里连 `agentPreset` 这个键都不出现）。
    /// 产品侧该传的值见 [`default_session_cwd`]，`request` 的形状见 [`session_create_request`]。
    pub fn create_session(&mut self, cwd: &str) -> Result<String, String> {
        self.create_session_with(cwd, None)
    }

    /// 同 [`Self::create_session`]，另把输入区那颗 `AgentModeButton` 挑中的预设 id
    /// （主干 `_pendingPreset.Id`）带进 `session/create`：`agentPreset` 是**可选键**，
    /// `None` 或空串都整键不发（主干 `if (_pendingPreset is { Id.Length: > 0 } preset)`）。
    /// 这一发是该待用值**唯一**的消费口 —— 挑档位那一刻一根 RPC 都不发（#56 的规格）。
    pub fn create_session_with(
        &mut self,
        cwd: &str,
        agent_preset: Option<&str>,
    ) -> Result<String, String> {
        let request = session_create_request(None, Some(cwd), agent_preset);
        let value = self.call("session/create", json!({ "request": request }))?;
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
        // 缺口 #68：`child.kill()` 只管主进程，作业句柄一关才是「整棵树」——内核自己起的
        // MCP 子进程（server-memory / playwright 等）连同它们占着的会话写租约一起走。
        // 位置对齐主干 `Dispose` 的 `finally`（`DshKernelHost.cs:152-160`）：先杀再关句柄。
        // 没挂上作业（`armed == false`）时这一句就是一个空操作，行为与今天完全一致。
        self.job.close_now();
    }

    /// 作业对象是否真的挂上了（`IsProcessInJob` 复查过才为 `true`）。运行时观测点：
    /// 同一句话在 `log` 里那行 `内核作业对象(pid …)` 也写着。
    pub fn job_armed(&self) -> bool {
        self.job.is_armed()
    }
}

/// `std::process::Child` 手里那颗进程句柄（`ChildExt::raw_handle`，Windows-only，
/// 与本文件 `command.creation_flags` 那处同源）。null 由 `KillJob::attach` 就地判掉，
/// 不在这里 panic。
fn child_process_handle(child: &Child) -> *mut std::ffi::c_void {
    use std::os::windows::io::AsRawHandle;
    child.as_raw_handle() as *mut std::ffi::c_void
}

impl Drop for Kernel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 只给测试用的第二条口径：老 CSIDL 那一族（`SHGetFolderPathW`），它**不吃** `FOLDERID_*`
    // 那堆 GUID 字节，所以能拿来交叉验证 `documents_folder()` 取的到底是哪颗
    // （见下面 `session_cwd_is_the_documents_known_folder`）。零依赖写法同上面的 `shell32` 块。
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHGetFolderPathW(
            owner: *mut std::ffi::c_void,
            csidl: i32,
            token: *mut std::ffi::c_void,
            flags: u32,
            path: *mut u16,
        ) -> i32;
    }

    /// 缺口 #85 第二条：RPC 超时对齐主干。分叉原先 `REQUEST_TIMEOUT = 20 s`，主干那颗
    /// 唯一的全局 `HttpClient.Timeout = 120 s`（`Dsh/DshRpcClient.cs:67`）——20 s 会把主干
    /// 等得到的长回包在本地掐掉，属行为分叉。这条测试钉住 120 s，改回去就红。
    #[test]
    fn rpc_request_timeout_matches_mainline_http_client_120s() {
        assert_eq!(
            REQUEST_TIMEOUT,
            Duration::from_secs(120),
            "主干 DshRpcClient.cs:67 是 120 s，分叉不得单方面收紧"
        );
        // 主干没有分档超时：分叉侧这几颗都得各归各，别混进 RPC 那一发。
        // · HANDSHAKE_TIMEOUT 是「等内核把端口/ token 吐出来」的自测侧上限，主干无对应 RPC。
        assert_eq!(HANDSHAKE_TIMEOUT, Duration::from_secs(90));
        // · mux 的 `READ_TICK`（`src/mux.rs:134` 用作 `set_read_timeout`）是 120 ms 轮询节拍、
        //   不是 RPC 超时，主干无对应物，故不参与本次对齐。
    }

    /// 缺口 #85 第四条（**注入版 / hermetic**）：`session/create` 的 location 取值口径。
    /// 三条分支全部用注入值钉死，不吃本机 known folder，因此在任何机器上都必跑、且都能真红：
    /// ① Documents 可用 ⇒ 原样用它；② Documents 解析失败（`None`）⇒ 空串；
    /// ③ Documents 解析出一个空串 ⇒ 空串。
    /// 主干背书：`MainWindow.xaml.cs:4606-4607` `DefaultSessionCwd()` = `GetFolderPath(MyDocuments)`，
    /// 该 API 的官方口径是「目录不物理存在 ⇒ 空串」，**没有二级回落**；
    /// `:4604-4605` 明确点名「回落到进程 CWD = system32 ⇒ 工具全废」是反例，
    /// 所以 ② ③ 必须钉成空串，不许哪天有人改成 `%TEMP%` / `LOCALAPPDATA` / 进程 CWD 来「修红」。
    #[test]
    fn resolve_session_cwd_pins_the_documents_or_empty_two_way_choice() {
        assert_eq!(
            resolve_session_cwd(Some("D:\\OneDrive\\文档")),
            "D:\\OneDrive\\文档",
            "① Documents 拿到了就得原样发出去，不裁剪、不绝对化、不换盘"
        );
        assert_eq!(
            resolve_session_cwd(None),
            "",
            "② OS 解析失败 ⇒ 空串（主干 .NET GetFolderPath 同形），不许发明二级回落"
        );
        assert_eq!(resolve_session_cwd(Some("")), "", "③ 解析出空串 ⇒ 同样空串");
        // 反例守卫：② 的落点必须是「什么都没有」而不是任何一个目录 —— 一旦有人把进程 CWD /
        // %TEMP% / LOCALAPPDATA 接成二级回落，这里就会拿到非空值（主干 :4604-4605 点名的就是这种落点）。
        assert!(
            resolve_session_cwd(None).is_empty(),
            "②' 解析失败只能是空串，不许有二级回落"
        );
    }

    /// 缺口 #85 第四条（**注入版 / hermetic**）：主干 `CreateSessionCoreAsync`
    /// （`MainWindow.xaml.cs:4621-4632`）那一发 `request` 里 location 的形状是**二选一**：
    /// 选了工作区 ⇒ 只发 `workspaceId`（`cwd` 整个键不存在）、没选 ⇒ 只发 `cwd`。
    /// 分叉产品侧只走 `cwd` 这一支（#56/#75 未接），但两支的形状在这里一处钉死。
    #[test]
    fn session_create_location_is_an_either_or_choice() {
        // ① 有工作区：只有 workspaceId。
        assert_eq!(
            session_create_location(Some("ws-7"), Some("C:\\Users\\x\\Documents")),
            json!({ "workspaceId": "ws-7" }),
            "主干 :4621-4624：workspaceId 是**替换** cwd，不是并列"
        );
        // ② 无工作区 + Documents 可用：只有 cwd。
        assert_eq!(
            session_create_location(None, Some("C:\\Users\\x\\Documents")),
            json!({ "cwd": "C:\\Users\\x\\Documents" }),
            "主干 :4627"
        );
        // ③ 无工作区 + Documents 不可用：仍然发 cwd 键、值为空串（主干 .NET 同形）。
        assert_eq!(session_create_location(None, None), json!({ "cwd": "" }));
        // ④ 空字符串 workspaceId 视同未选（主干 `is { Id.Length: > 0 }` 那一判）。
        assert_eq!(
            session_create_location(Some(""), Some("D:\\doc")),
            json!({ "cwd": "D:\\doc" }),
            "workspaceId 为空 ⇒ 回落 cwd，不能发一个空 workspaceId 过去"
        );
        // ⑤ 产品侧实际发出去的那一发：`{"request":{…}}` 外壳不变（fake_dsh.rs:905-907 只认这一形状）。
        assert_eq!(
            json!({ "request": session_create_location(None, Some("E:\\demo")) }),
            json!({ "request": { "cwd": "E:\\demo" } }),
            "`create_session` 的线格式不得因这次拆分而变"
        );
    }

    /// 缺口 #85 第四条（**真机版**）：分叉取的确实就是「文档」这一颗已知文件夹。
    ///
    /// 上一版这里是直接 `assert!(!cwd.is_empty())`，红了只会说「解析失败」——真正的原因其实是
    /// `FOLDERID_DOCUMENTS` 的字节抄错了（错的 GUID 也回 `0x80070002`，看不出是 GUID 不对）。
    /// 现在改成拿一条**不吃这颗 GUID** 的老口径 `SHGetFolderPathW(CSIDL_PERSONAL)`
    /// （= .NET Framework 时代 `GetFolderPath(MyDocuments)` 的本体，也就是主干那颗）做交叉比对：
    /// GUID 一错，两个 API 就一个有值一个空 ⇒ 立刻红，且诊断能指回取值处。
    ///
    /// 环境判定是显式的、且断言永不为真空：
    /// - oracle 有值（本机实测 `C:\Users\Admin\Documents`）⇒ 跑全强度四条（相等 + 绝对 + 是目录 + 非 system32）；
    /// - oracle 也没值 ⇒ 退到「两边同败 ⇒ 分叉也必须同败（空串）」这一条相等断言，
    ///   并把诊断打出来；这一支在报告里记为「因环境降强度 1 把」，本机不走这条。
    #[test]
    fn session_cwd_is_the_documents_known_folder() {
        let oracle = unsafe {
            // CSIDL_PERSONAL = 0x05「我的文档」；SHGFP_TYPE_CURRENT = 0。
            let mut buffer = [0u16; 260];
            let code = SHGetFolderPathW(std::ptr::null_mut(), 0x0005, std::ptr::null_mut(), 0, buffer.as_mut_ptr());
            let units: Vec<u16> = buffer.iter().copied().take_while(|u| *u != 0).collect();
            (code == 0).then(|| String::from_utf16_lossy(&units))
        };
        let cwd = default_session_cwd();
        match oracle.as_deref() {
            Some(expected) => {
                assert_eq!(
                    cwd, expected,
                    "分叉取的 known folder 与 CSIDL_PERSONAL 那颗对不上（多半是 FOLDERID_DOCUMENTS 的字节错了）"
                );
                let path = Path::new(&cwd);
                assert!(path.is_absolute(), "{cwd} 必须是绝对路径");
                assert!(path.is_dir(), "{cwd} 必须是已存在的目录（内核要往里写）");
                assert!(
                    !cwd.to_lowercase().contains("system32"),
                    "{cwd} 落回 system32 就是主干注释 :4604-4605 里那个「工具全废」的反例"
                );
            }
            None => {
                println!("CW1 环境诊断：SHGetFolderPathW(CSIDL_PERSONAL) 失败 ⇒ 本机 known folder 表不可用");
                assert_eq!(
                    cwd, "",
                    "oracle 拿不到时分叉也不许凭空造一个目录（主干此时发空串，见 :4606-4607 + GetFolderPath 的空串口径）"
                );
            }
        }
    }

    /// ④ `agentPreset` 那一档：主干 `:4629-4631` 带的是 **id**、且**空 id 整键不发**。
    ///    反向半边：空串与 `None` 同形（都不许长出 `"agentPreset": ""`），并且这一键与
    ///    location 那一档**并列**（不像 `workspaceId` 那样替换 `cwd`）。
    #[test]
    fn session_create_request_adds_agent_preset_only_when_it_is_a_real_id() {
        // 不带预设 ⇒ 与 `session_create_location` 逐字同形（`create_session` 走的就是这一支）。
        assert_eq!(
            session_create_request(None, Some("E:\\demo"), None),
            session_create_location(None, Some("E:\\demo"))
        );
        assert_eq!(
            session_create_request(None, Some("E:\\demo"), Some("")),
            json!({ "cwd": "E:\\demo" }),
            "空 id ⇒ 整键不发，不许发一个空串过去"
        );
        // 带预设 ⇒ 与 location 并列，值是 **id**（不是那一档的显示名）。
        assert_eq!(
            session_create_request(None, Some("E:\\demo"), Some("cordis")),
            json!({ "cwd": "E:\\demo", "agentPreset": "cordis" })
        );
        assert_eq!(
            session_create_request(Some("ws-7"), Some("E:\\demo"), Some("ptc")),
            json!({ "workspaceId": "ws-7", "agentPreset": "ptc" }),
            "workspaceId 替换 cwd 这一条不许顺手把 agentPreset 也带掉"
        );
        // 主干 `_pendingPreset` 有初值 ⇒ 「一发起新会话就带 standard」是常态，不是缺陷。
        assert_eq!(
            json!({ "request": session_create_request(None, Some(""), Some("standard")) }),
            json!({ "request": { "cwd": "", "agentPreset": "standard" } })
        );
    }

    /// 接线锁（与 `main.rs` 里 `pill_tests` / `mux_retry_tests` 同一口径）：两处
    /// `session/create` 的调用点都得改成文档目录，不许再退回空串或进程 CWD。
    /// 真正跑得起来的端到端由 `tests/real_kernel.rs`（真内核）与 `tests/ipc.rs`（假内核）顶，
    /// 那两处传的是显式 cwd、验的是协议，不覆盖产品侧取值。
    ///
    /// #56 第一颗落地后这两处改成了 [`Kernel::create_session_with`]（要带待用预设），故 needle
    /// 换成 `create_session_with(&default_session_cwd(`：**这条锁的原意一条没松** —— 仍然是
    /// 「两处、都走文档目录、都不许凭空造 cwd」。`create_session(` 那一枚旧 needle 已随签名改动
    /// 从产品码里消失（`tests/ipc.rs` 与 `tests/real_kernel.rs` 里剩的是 `create_session("…")`
    /// 显式 cwd 形态，与这条锁无关）。
    #[test]
    fn create_session_call_sites_pass_the_known_folder() {
        let source = include_str!("main.rs");
        assert_eq!(
            source
                .matches(concat!("create_session_with(&default_", "session_cwd("))
                .count(),
            2,
            "两处 `session/create` 调用点（发送时自动新建 + 新建会话钮）都要走主干那颗文档目录"
        );
        assert!(
            !source.contains("create_session(\"\""),
            "session/create 不得再传字面空串 cwd"
        );
        // 反向半边（#56 新增那一半）：不带预设的老签名在产品码里必须一根不剩，
        // 漏改一处就是「挑了档、新建会话却回落到内核默认预设」这种静默偏差。
        // needle 取 `create_session(&`：它既不是 `create_session_with(&` 的前缀，也不会
        // 命中 `tests/ipc.rs` / `tests/real_kernel.rs`（那两处不在本锁的 `include_str!` 里，
        // 且传的是显式 cwd 的 `create_session("…")` 形态）。
        assert_eq!(
            source.matches(concat!("create_", "session(&")).count(),
            0,
            "产品码里还有一处走不带预设的老签名 ⇒ 那一处建出来的会话用的是内核默认档"
        );
        assert_eq!(
            source.matches(concat!("agent_", "preset.as_deref()")).count(),
            2,
            "两处都得把待用预设的 id 递出去（`as_deref()` 那一发），少一处就是静默丢档"
        );
    }

    /// 缺口 #72：关于页「内核版本」那一行的取值口径（主干 `MainWindow.About.cs:64-79`）。
    /// 四条都要守住，因为主干那半截是 `JsonDocument.Parse` + `TryGetProperty` + `ValueKind`
    /// + `IsNullOrEmpty` 四层判定，任何一层松了关于页就会显示出一个不该有的版本号：
    /// 正常值 / 键缺席 / 空串（还有类型不对）/ 整份 JSON 是坏的。
    #[test]
    fn package_version_only_accepts_a_non_empty_string() {
        // ① 正常：随包发行的那份就是这种形状（本机 `Kernel/dsh/package.json` = 0.1.5-rc.2）。
        assert_eq!(
            parse_package_version(r#"{"name":"@deepseek-ai/dsh","version":"0.1.5-rc.2"}"#),
            Some("0.1.5-rc.2".to_string())
        );
        // ② 键缺席 ⇒ None，不许退化成空串（空串在关于页上是一行看得见的空白）。
        assert_eq!(parse_package_version(r#"{"name":"@deepseek-ai/dsh"}"#), None);
        // ③ 空串 ⇒ None。
        assert_eq!(parse_package_version(r#"{"version":""}"#), None);
        // ③' 值不是字符串 ⇒ None（主干 `ValueKind == JsonValueKind.String` 那一判）。
        assert_eq!(parse_package_version(r#"{"version":null}"#), None);
        assert_eq!(parse_package_version(r#"{"version":1}"#), None);
        assert_eq!(parse_package_version(r#"{"version":[1,2]}"#), None);
        // ④ 正文坏了 / 是空文件 ⇒ None，而且**不能 panic**：读不到就退回「未知」。
        assert_eq!(parse_package_version("{ not json"), None);
        assert_eq!(parse_package_version(""), None);
        // ④' 合法 JSON 但根不是对象 ⇒ 同样 None（`TryGetProperty` 在非对象上是抛的）。
        assert_eq!(parse_package_version("[1,2]"), None);
        assert_eq!(parse_package_version("\"0.1.5\""), None);
    }

    /// 同一个函数族在**没有真 `Kernel/` 目录**时的落地：`kernel_version_from` 只吃目录，
    /// 目录里没有那份文件就必须安静地回 None（这正是关于页不许崩的那条例外路径）。
    #[test]
    fn missing_package_json_is_no_version_instead_of_a_panic() {
        let missing = std::env::temp_dir().join("blade2-rs-no-such-kernel-dir");
        assert!(
            kernel_version_from(&missing).is_none(),
            "读不到 package.json 只能回 None"
        );
    }

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

    // ==================== #78 会话历史回读（`session/page`） ====================

    /// 一条 journal 事件（主干读的键：`type` / `seq` / `time` / `data.turn`）。
    fn journal_event(kind: &str, seq: i64, data: Value) -> Value {
        json!({ "type": kind, "seq": seq, "time": 1_000 + seq, "data": data })
    }

    /// `records` 的一条：`{type:"event", event:{…}}`，与 follow 流的 event 帧同一份形状。
    fn record(event: Value) -> Value {
        json!({ "type": "event", "event": event })
    }

    /// 一张内存 journal：`session/page` 的判据按内核口径来 —— `throughSeq` 越过真实游标
    /// 就报 `"… is past cursor <游标>"`、页内按 seq 升序、截不满就是 `hasMore=true`。
    /// 有了它，整条分页链（探测 → 回退 → 翻页 → 收手）不出网就能测。
    #[derive(Default)]
    struct FakeJournal {
        events: Vec<Value>,
        /// 0 = 一次给全（真内核 `maxMessages=5000` 的常见结果）；>0 用来逼出多页链。
        page_size: usize,
        /// 越界错误里**不带**那个数 ⇒ `parse_past_cursor_seq` 回 None，走对半兜底。
        opaque_cursor: bool,
        /// 第 N 发之后改口报越界（测「翻页途中撞 past cursor」那一支）。
        refuse_after: Option<usize>,
        /// 已发出的请求（断言往返次数与每发的 throughSeq）。
        calls: Vec<HistoryPage>,
    }

    impl FakeJournal {
        /// seq 从 `first` 起连号的 `count` 条事件。
        fn with_seq(first: i64, count: usize) -> Self {
            Self {
                events: (0..count)
                    .map(|index| {
                        let seq = first + index as i64;
                        journal_event("user/message", seq, json!({ "turn": seq }))
                    })
                    .collect(),
                ..Default::default()
            }
        }

        fn send(&mut self, page: &HistoryPage) -> Result<Value, String> {
            self.calls.push(page.clone());
            let cursor = self
                .events
                .last()
                .and_then(|event| event["seq"].as_i64())
                .unwrap_or(-1);
            let past = format!(
                "session/page/bad: session page through seq {} is past cursor {}",
                page.through_seq, cursor
            );
            if page.through_seq > cursor {
                if self.opaque_cursor {
                    // 文案里留着 "past cursor"（主干的 `when` 过滤靠它），但那个数不是数字
                    // ⇒ `parse_past_cursor_seq` 回 None ⇒ 对半兜底。
                    return Err(format!(
                        "session/page/bad: session page through seq {} is past cursor <unset>",
                        page.through_seq
                    ));
                }
                return Err(past);
            }
            if self.refuse_after.is_some_and(|n| self.calls.len() > n) {
                return Err(past);
            }
            let mut hits: Vec<Value> = self
                .events
                .iter()
                .filter(|event| event["seq"].as_i64().unwrap_or_default() <= page.through_seq)
                .cloned()
                .collect();
            if self.page_size > 0 && hits.len() > self.page_size {
                hits.drain(..hits.len() - self.page_size);
            }
            let has_more = hits
                .first()
                .is_some_and(|event| event["seq"].as_i64().unwrap_or_default() > 0);
            Ok(json!({
                "records": hits.into_iter().map(record).collect::<Vec<_>>(),
                "hasMore": has_more,
            }))
        }
    }

    #[test]
    fn history_page_args_carry_exactly_the_keys_the_trunk_sends() {
        // 主干 `LoadSessionHistoryAsync` 的翻页请求：`{request:{address:{kind,sessionId},
        // throughSeq,maxMessages}}`，一个键不多一个键不少。
        assert_eq!(
            HistoryPage::page("s-1001", 88).to_args(),
            json!({ "request": {
                "address": { "kind": "session", "sessionId": "s-1001" },
                "throughSeq": 88,
                "maxMessages": 5000,
            }})
        );
        // 主干 `ProbeJournalCursorAsync` 的探测：同一层 address，但**不带** maxMessages。
        assert_eq!(
            HistoryPage::probe("s-1001").to_args(),
            json!({ "request": {
                "address": { "kind": "session", "sessionId": "s-1001" },
                "throughSeq": 1 << 30,
            }})
        );
        // 两个常量的数值口径照主干 `HistoryPageMessages` / `JournalProbeSeq`。
        assert_eq!((HISTORY_PAGE_MESSAGES, JOURNAL_PROBE_SEQ), (5000, 1 << 30));
        assert_eq!(
            HistoryPage::page("s-1", 0),
            HistoryPage {
                session_id: "s-1".to_string(),
                through_seq: 0,
                max_messages: Some(HISTORY_PAGE_MESSAGES),
            }
        );
    }

    #[test]
    fn past_cursor_message_yields_the_real_cursor() {
        // 内核原文：`"session page through seq <n> is past cursor <sourceCursor>"`。
        assert_eq!(
            parse_past_cursor_seq("session page through seq 1073741824 is past cursor 117"),
            Some(117)
        );
        // 空日志的游标就是 -1（主干 `cursor >= -1` 那道门放过它）。
        assert_eq!(parse_past_cursor_seq("… is past cursor -1"), Some(-1));
        assert_eq!(parse_past_cursor_seq("… is past cursor 12\n"), Some(12));
        // 取**最后**一处（主干 `LastIndexOf`）：前面还嵌着一段别的文案也不能读错。
        assert_eq!(
            parse_past_cursor_seq("is past cursor 1, retry said: is past cursor 42"),
            Some(42)
        );
        // 解不出的一律 None ⇒ 上层回落到对半探测（内核文案改版也不会崩）。
        assert_eq!(parse_past_cursor_seq("session page through seq 5 is past the end"), None);
        assert_eq!(parse_past_cursor_seq("… is past cursor abc"), None);
        assert_eq!(parse_past_cursor_seq(""), None);
        assert!(is_past_cursor("session/page/bad: … is past cursor 3"));
        assert!(!is_past_cursor("gateway/unauthorized: 401"));
    }

    #[test]
    fn page_records_peel_the_envelope_and_bad_rows_are_skipped() {
        let turn_start = journal_event("turn/start", 3, json!({ "turn": 1 }));
        let page = json!({
            "records": [
                record(turn_start.clone()),
                json!({ "type": "event" }),
                json!({ "type": "event", "event": "not-an-object" }),
                json!({ "type": "assistant-stream", "frame": {} }),
                record(journal_event("user/message", 4, json!({ "turn": 1 }))),
            ],
            "hasMore": true,
        });
        let events = page_events(&page);
        assert_eq!(events.len(), 2, "坏记录整条跳过：不炸，也不牵连同页其余");
        assert_eq!(events[0], turn_start);
        assert_eq!(
            page_first_seq(&page),
            Some(3),
            "翻页那把刀取 records[0].event.seq（页内升序 ⇒ 首条即本页最早）"
        );
        assert!(page_has_more(&page));
        // 缺键 / 型不对都不能 panic（主干那边是 TryGetProperty + ValueKind）。
        assert!(!page_has_more(&json!({})));
        assert!(page_events(&json!({ "records": "nope" })).is_empty());
        assert_eq!(page_first_seq(&json!({ "records": [] })), None);
        assert_eq!(page_first_seq(&json!({"records":[{"event": {"seq": "7"}}]})), None);
    }

    #[test]
    fn history_walk_pages_from_the_probed_cursor_back_to_seq_zero() {
        let mut journal = FakeJournal::with_seq(0, 5);
        journal.page_size = 2;
        let history = walk_history("s-1001", &mut |page| journal.send(page)).expect("拉历史不该报错");
        let seqs: Vec<i64> = history
            .events
            .iter()
            .map(|event| event["seq"].as_i64().unwrap_or_default())
            .collect();
        assert_eq!(seqs, vec![0, 1, 2, 3, 4], "落地必须是时间序（旧→新），页序要倒回来");
        // 探测（1<<30 越界，游标从错误串里拿）→ through 4 → through 2 → through 0 收手。
        let throughs: Vec<i64> = journal.calls.iter().map(|call| call.through_seq).collect();
        assert_eq!(throughs, vec![JOURNAL_PROBE_SEQ, 4, 2, 0]);
        assert_eq!(journal.calls[0], HistoryPage::probe("s-1001"));
        assert_eq!(journal.calls[1].max_messages, Some(HISTORY_PAGE_MESSAGES));
        assert_eq!(journal.calls[1].session_id, "s-1001");
    }

    #[test]
    fn history_walk_stops_at_a_past_cursor_hit_mid_paging() {
        // 翻到一半 journal 说「这个游标不存在」（主干 :4591 的那一支是**正常收手**）：
        // 已经收到的页照样折成时间序交回去，一行都不该丢。
        let mut journal = FakeJournal::with_seq(0, 10);
        journal.page_size = 4;
        journal.refuse_after = Some(2);
        let history = walk_history("s-1", &mut |page| journal.send(page)).expect("越界收手不算失败");
        let seqs: Vec<i64> = history
            .events
            .iter()
            .map(|event| event["seq"].as_i64().unwrap_or_default())
            .collect();
        assert_eq!(seqs, vec![6, 7, 8, 9], "只有已收到的那一页留下");
        assert_eq!(journal.calls.len(), 3, "第三发被拒之后不该再试第四发");
    }

    #[test]
    fn history_walk_reports_a_hard_failure_and_survives_an_empty_journal() {
        // 会话不存在这类错误：主干整段 catch 掉、什么都不画，分叉把它回给调用方去写日志。
        let error = walk_history("nope", &mut |_| Err("session/not-found: session 不存在".to_string()))
            .expect_err("非越界错误要原样冒出去");
        assert!(error.contains("session/not-found"), "{error}");
        // 空日志：游标 -1 ⇒ 连一页都不发（`while through >= 0` 直接不进）。
        let mut empty = FakeJournal::default();
        let history = walk_history("s-2", &mut |page| empty.send(page)).expect("空 journal 不是错误");
        assert!(history.events.is_empty());
        assert_eq!(history.durable_title, "");
        assert_eq!(empty.calls.len(), 1, "探测那一发就该问出来");
    }

    #[test]
    fn opaque_past_cursor_message_falls_back_to_bisection() {
        let mut journal = FakeJournal::with_seq(0, 3);
        journal.opaque_cursor = true;
        let history = walk_history("s-1", &mut |page| journal.send(page)).expect("兜底也得拉到");
        assert_eq!(history.events.len(), 3);
        let probes = journal
            .calls
            .iter()
            .filter(|call| call.max_messages.is_none())
            .count();
        assert!(
            (16..=18).contains(&probes),
            "对半探测就是最坏 ~16 次往返：{probes}"
        );
        assert_eq!(
            journal.calls.last().expect("有请求").through_seq,
            2,
            "夹出来的最后一个 throughSeq 就是 journal 尾 seq"
        );
    }

    #[test]
    fn durable_title_is_the_latest_one_in_the_journal() {
        let mut journal = FakeJournal::with_seq(0, 2);
        journal.events.push(journal_event(
            "session/title",
            2,
            json!({ "turn": 1, "title": "" }),
        ));
        journal.events.push(journal_event(
            "session/title",
            3,
            json!({ "turn": 1, "title": "旧名" }),
        ));
        journal.events.push(journal_event(
            "session/title",
            4,
            json!({ "turn": 2, "title": "新名" }),
        ));
        journal.page_size = 2;
        let history = walk_history("s-1", &mut |page| journal.send(page)).expect("三页链");
        assert_eq!(
            history.durable_title, "新名",
            "空标题不算数、按 seq 取最新（页是多页收到的）"
        );
    }

    #[test]
    fn event_coords_mirror_the_roles_the_bubbles_carry() {
        assert_eq!(
            event_coords(&journal_event("user/message", 4, json!({ "turn": 1 }))),
            vec![BubbleCoord("user".to_string(), 1, 4)]
        );
        // 一条 assistant/message 可以占两格：reasoning 与正文共用信封 seq。
        assert_eq!(
            event_coords(&journal_event(
                "assistant/message",
                27,
                json!({ "turn": 1, "message": { "content": [
                    { "type": "text", "text": "答案" },
                    { "type": "reasoning", "text": "先拆开" },
                ] } })
            )),
            vec![
                BubbleCoord("assistant".to_string(), 1, 27),
                BubbleCoord("reasoning".to_string(), 1, 27),
            ]
        );
        // 空 reasoning 段不占格（分叉那边 `!reasoning.is_empty()` 才 push）。
        assert_eq!(
            event_coords(&journal_event(
                "assistant/message",
                27,
                json!({ "turn": 1, "message": { "content": [
                    { "type": "reasoning", "text": "" },
                ] } })
            )),
            vec![BubbleCoord("assistant".to_string(), 1, 27)]
        );
        // 合成行与不进气泡的帧都不参与重叠判定。
        for kind in ["system/message", "assistant/attempt", "turn/start", "turn/end", "session/title"] {
            assert!(
                event_coords(&journal_event(kind, 5, json!({ "turn": 1 }))).is_empty(),
                "{kind} 的 seq 不是 journal 坐标"
            );
        }
        // 交付物行只有真画出文件才有坐标。
        assert!(
            event_coords(&journal_event(
                "deliverables/presented",
                8,
                json!({ "turn": 1, "files": [{ "description": "没路径" }] })
            ))
            .is_empty()
        );
        assert_eq!(
            event_coords(&journal_event(
                "deliverables/presented",
                8,
                json!({ "turn": 1, "files": [{ "path": "a.md" }] })
            )),
            vec![BubbleCoord("deliverable".to_string(), 1, 8)]
        );
    }

    #[test]
    fn overlapping_history_rows_lose_to_the_live_bubbles() {
        let events = vec![
            journal_event("user/message", 4, json!({ "turn": 1 })),
            journal_event(
                "assistant/message",
                27,
                json!({ "turn": 1, "message": { "content": [
                    { "type": "text", "text": "答案" },
                    { "type": "reasoning", "text": "先拆开" },
                ] } }),
            ),
            journal_event("tool/call", 5, json!({ "turn": 1 })),
            journal_event("system/message", 6, json!({ "turn": 1 })),
        ];
        // 在途已经画过：那一问（同坐标）+ 那条答案的正文格（reasoning 格还空着）。
        let rendered = vec![
            BubbleCoord("user".to_string(), 1, 4),
            BubbleCoord("assistant".to_string(), 1, 27),
        ];
        let kept: Vec<i64> = drop_overlapped(&events, &rendered)
            .iter()
            .map(|event| event["seq"].as_i64().unwrap_or_default())
            .collect();
        assert_eq!(
            kept,
            vec![27, 5, 6],
            "live 赢：整格都在图上才丢；reasoning 那格还空着就得留下整条"
        );
        // 刚清完屏（切会话那一步）= 全量回放。
        assert_eq!(drop_overlapped(&events, &[]).len(), 4);
    }

    // ==================== 缺口 #84-A：两级 launcher ====================

    /// 一次性的临时沙箱根。**真机安全阀**：#84-B 的所有搬迁测试只准在这里面跑，
    /// 绝不拿 `%LOCALAPPDATA%` / `%USERPROFILE%\Documents` 当 root 去 Move/Create/Delete。
    /// `Drop` 里只删自己建的那颗（名字带 pid + 递增序号，撞不上别人的目录），失败也吞。
    struct Sandbox {
        root: PathBuf,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let root = std::env::temp_dir().join(format!(
                "blade2-fg2-{name}-{}-{seq}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("建 #84 测试沙箱根目录");
            Self { root }
        }

        fn dir(&self) -> &Path {
            &self.root
        }

        /// 在沙箱里建一颗目录。
        fn make_dir(&self, tail: &str) -> PathBuf {
            let dir = self.root.join(tail);
            std::fs::create_dir_all(&dir).expect("建沙箱子目录");
            dir
        }

        /// 在沙箱里建一颗文件（顺带把父目录补齐）。
        fn make_file(&self, tail: &str, body: &str) -> PathBuf {
            let file = self.root.join(tail);
            if let Some(parent) = file.parent() {
                std::fs::create_dir_all(parent).expect("建沙箱文件父目录");
            }
            std::fs::write(&file, body).expect("写沙箱文件");
            file
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn p(text: &str) -> PathBuf {
        PathBuf::from(text)
    }

    /// §3.1 点名的五例（规格要 5 例，这里按"任一缺失整级跳过"拆成六条钉）。
    #[test]
    fn decide_launcher_five_candidate_cases() {
        let (node, bin_js, cmd) = (p(r"K:\node.exe"), p(r"K:\dsh\lib\bin.js"), p(r"A:\npm\dsh.cmd"));
        // ① 双内置在 ⇒ 第 1 级。
        assert_eq!(
            decide_launcher(Some(node.clone()), Some(bin_js.clone()), Some(cmd.clone())),
            Launcher::Bundled { node: node.clone(), bin_js: bin_js.clone() }
        );
        // ② 只 node 在 ⇒ 内置整级跳过（主干是两个 `File.Exists` 的与，**不许半用**）。
        assert_eq!(
            decide_launcher(Some(node.clone()), None, Some(cmd.clone())),
            Launcher::NpmDshCmd(cmd.clone())
        );
        // ③ 只 bin_js 在 ⇒ 同上。
        assert_eq!(
            decide_launcher(None, Some(bin_js.clone()), Some(cmd.clone())),
            Launcher::NpmDshCmd(cmd.clone())
        );
        // ④ 只 dsh.cmd 在 ⇒ 第 2 级。
        assert_eq!(decide_launcher(None, None, Some(cmd.clone())), Launcher::NpmDshCmd(cmd));
        // ⑤ 全缺 ⇒ Missing（主干 `return null`，一个进程都不 spawn）。
        assert_eq!(decide_launcher(None, None, None), Launcher::Missing);
    }

    /// 候选顺序：两级都命中时必须内置赢（主干 `ResolveLauncher` 第 1 级先 return）。
    #[test]
    fn decide_launcher_bundled_beats_dsh_cmd_when_both_present() {
        let chosen = decide_launcher(
            Some(p(r"K:\node.exe")),
            Some(p(r"K:\dsh\lib\bin.js")),
            Some(p(r"A:\npm\dsh.cmd")),
        );
        assert!(matches!(chosen, Launcher::Bundled { .. }), "内置齐全时不许落到 dsh.cmd: {chosen:?}");
    }

    /// **保真陷阱①**（§1.7-1）：`is_bundled` 只吃内置那两个 `File.Exists`，**不吃实际选中哪一级**。
    /// 分叉若改成"按实际级别给文案"就不是行为一致了 —— 这条测试就是拿来拦这个"顺手改对"的。
    #[test]
    fn is_bundled_reads_the_two_probes_not_the_chosen_level() {
        let node = Some(p(r"K:\node.exe"));
        let bin_js: Option<PathBuf> = None;
        let cmd = Some(p(r"A:\npm\dsh.cmd"));
        let chosen = decide_launcher(node.clone(), bin_js.clone(), cmd);
        // 本次**实际走**了第 2 级……
        assert!(matches!(chosen, Launcher::NpmDshCmd(_)));
        // ……但 `is_bundled` 仍是 false ⇒ 主干文案给的是那句措辞并不准确的
        // 「未找到内置内核；请安装 npm 版 dsh 或重新安装 Blade²」。刻意照抄，别改对。
        assert!(!is_bundled(&node, &bin_js));
        // 两颗齐全 ⇒ true（此时失败主干说「内核启动失败」）。
        assert!(is_bundled(&node, &Some(p(r"K:\dsh\lib\bin.js"))));
        // 一颗都没有 ⇒ false。
        assert!(!is_bundled(&None, &None));
    }

    /// §3.2：内置级的 argv —— 尾部四 token、路径含空格也**不**自己加引号（交给 std 转义，
    /// 与主干 .NET 的 `"{binJs}" …` 等效），且 `args_verbatim = false`。
    #[test]
    fn bundled_argv_keeps_shared_tail_and_leaves_quoting_to_std() {
        let bin_js = r"C:\Program Files\Blade2\Kernel\dsh\lib\bin.js";
        let plan = launcher_argv(&Launcher::Bundled {
            node: p(r"C:\Program Files\Blade2\Kernel\node.exe"),
            bin_js: p(bin_js),
        })
        .expect("内置级必须有 argv");
        assert_eq!(plan.exe, p(r"C:\Program Files\Blade2\Kernel\node.exe"));
        assert!(!plan.args_verbatim, "内置级走正常 argv 列表，不是原样串");
        assert_eq!(plan.args[0], bin_js, "不许自己再裹一层引号");
        assert_eq!(&plan.args[1..], KERNEL_ARG_TAIL, "尾部必须与主干两级共用那一串");
    }

    /// §1.1 点名的坑：第 2 级在 **Roaming**（`SpecialFolder.ApplicationData` = `%APPDATA%`），
    /// 不是 `%LOCALAPPDATA%`。拼错一个词就永远找不到，故单独钉。
    #[test]
    fn npm_dsh_cmd_path_is_roaming_appdata() {
        assert_eq!(
            npm_dsh_cmd_path(Path::new(r"C:\Users\me\AppData\Roaming")),
            p(r"C:\Users\me\AppData\Roaming\npm\dsh.cmd")
        );
        let built = npm_dsh_cmd_path(Path::new(r"C:\Users\me\AppData\Roaming"));
        assert!(!built.to_string_lossy().contains("Local"), "绝不能落到 LOCALAPPDATA");
    }

    /// **保真陷阱②**（§3.2 + §4-①）：回退级那一串嵌套引号必须**逐字节**是主干 `DshKernelHost.cs:61`
    /// 的形状，且标成原样传递（`raw_arg`）。若退化成 `Command::args`，std 会把 `"` 转义成 `\"`，
    /// cmd.exe 不认，含空格路径必拆错。
    #[test]
    fn dsh_cmd_arguments_are_mainline_verbatim_with_spaces() {
        let dsh = r"C:\Program Files\x\npm\dsh.cmd";
        let plan = launcher_argv(&Launcher::NpmDshCmd(p(dsh))).expect("回退级必须有 argv");
        assert_eq!(plan.exe, p("cmd.exe"), "主干 `:61` 写的就是字面量 cmd.exe");
        assert!(plan.args_verbatim, "这一串必须原样交给 cmd.exe，不许走列表转义");
        assert_eq!(
            plan.args,
            vec![r#"/c ""C:\Program Files\x\npm\dsh.cmd" web --no-open --port 0""#.to_string()],
            "外层一对让 cmd /c 把整串当一个命令，内层一对裹路径"
        );
        assert_eq!(npm_dsh_cmd_arguments(Path::new(dsh)), plan.args[0]);
    }

    /// 两级"参数完全一致"（`docs/DESIGN.zh.md:25`）的另一种钉法：同一串尾部 token。
    #[test]
    fn both_levels_carry_the_identical_kernel_tail() {
        let dsh = r"C:\Users\me\AppData\Roaming\npm\dsh.cmd";
        let fallback = launcher_argv(&Launcher::NpmDshCmd(p(dsh))).expect("回退级");
        let bundled = launcher_argv(&Launcher::Bundled {
            node: p(r"K:\node.exe"),
            bin_js: p(r"K\dsh\lib\bin.js"),
        })
        .expect("内置级");
        let tail = KERNEL_ARG_TAIL.join(" ");
        assert!(fallback.args[0].ends_with(&format!("{tail}\"")), "回退级尾部三…四个 token");
        assert_eq!(bundled.args[1..].join(" "), tail);
    }

    /// §4-① 里唯一**不必真机也能定死**的那半截：主干交给 `CreateProcessW` 的 `lpCommandLine`
    /// 形状（.NET 是 `FileName + " " + Arguments` 原样拼接）。分叉侧靠 `raw_arg` 出同一串。
    /// （最终串仍受 cmd.exe 自身解析影响，那一层见 `tmp/fg2-cmdline.txt` 的逐字节比对。）
    #[test]
    fn fallback_command_line_matches_mainline_string_byte_for_byte() {
        let dsh = r"C:\Program Files\Test\Roaming\npm\dsh.cmd";
        let mainline = concat!(
            "cmd.exe /c \"",
            "\"",
            "C:\\Program Files\\Test\\Roaming\\npm\\dsh.cmd",
            "\" web --no-open --port 0\"",
        );
        assert_eq!(
            launcher_command_line(&Launcher::NpmDshCmd(p(dsh))).expect("回退级有线"),
            mainline
        );
    }

    /// 第 3 态：主干 `return null` ⇒ `StartAsync` 立刻返回、**不 spawn 任何东西**。
    #[test]
    fn missing_launcher_produces_no_spawn_at_all() {
        assert!(launcher_argv(&Launcher::Missing).is_none());
        assert!(launcher_command_line(&Launcher::Missing).is_none());
    }

    /// §3.3：env 注入四例（目录在/不在 × PATH 有/无）。两个关键点：
    /// ① `DSH_HOME` 两级都给、无条件；② `Kernel\bin` **不存在时不注入 PATH**
    /// （主干 `PrependPath` 那句 `return`），且原 PATH 为空时只写新目录。
    #[test]
    fn build_env_covers_prepend_exists_times_path_present() {
        let home = Path::new(r"C:\Users\me\AppData\Local\Blade2");
        let bin = Path::new(r"C:\app\Kernel\bin");
        let home_str = r"DSH_HOME";

        // ① 目录在 + PATH 有 ⇒ 新目录前置、分号接原值。
        let both = build_env(Some(home), Some(bin), true, Some(r"C:\Windows"));
        assert_eq!(both[0], (home_str.to_string(), r"C:\Users\me\AppData\Local\Blade2".to_string()));
        assert_eq!(both[1], ("PATH".to_string(), r"C:\app\Kernel\bin;C:\Windows".to_string()));

        // ② 目录在 + PATH 无 ⇒ 只写新目录（主干 `string.IsNullOrEmpty(current) ? dir : …`）。
        let no_path = build_env(Some(home), Some(bin), true, None);
        assert_eq!(no_path[1], ("PATH".to_string(), r"C:\app\Kernel\bin".to_string()));
        // 空串与原 PATH 缺席同形（主干那颗也判 `IsNullOrEmpty`）。
        assert_eq!(build_env(Some(home), Some(bin), true, Some("")), no_path);

        // ③④ 目录不在 ⇒ **一个 PATH 键都不加**（两种 PATH 形态各钉一次）。
        for path_env in [Some(r"C:\Windows"), None] {
            let without = build_env(Some(home), Some(bin), false, path_env);
            assert_eq!(
                without,
                vec![(home_str.to_string(), r"C:\Users\me\AppData\Local\Blade2".to_string())],
                "Kernel\\bin 不存在时不许往内核 PATH 里塞一条不存在的路径"
            );
        }
        // 自测口那种：两颗都没有 ⇒ 空 env（对齐分叉既有 `BLADE2_KERNEL_EXE` 形态）。
        assert!(build_env(None, None, false, Some("x")).is_empty());
    }

    // ==================== 缺口 #84-B：Code2 → Blade2 搬迁 ====================

    /// §3.5 的四例。**只有**「新不在 && 老在」动盘；尤其「两者都在」= 保新、旧目录一个字不动。
    #[test]
    fn plan_data_home_only_moves_when_new_absent_and_legacy_present() {
        let legacy = Path::new(r"R:\Code2");
        let home = Path::new(r"R:\Blade2");
        assert_eq!(
            plan_data_home(legacy, home, true, false),
            DataHomePlan::Move { legacy: legacy.to_path_buf(), home: home.to_path_buf() }
        );
        // 新旧都在 ⇒ 保新，不合并、不比时间戳、不删源。
        assert_eq!(plan_data_home(legacy, home, true, true), DataHomePlan::Keep);
        // 老不在、新在 ⇒ 正常已迁机器。
        assert_eq!(plan_data_home(legacy, home, false, true), DataHomePlan::Keep);
        // 两者都不在 ⇒ 全新机器。
        assert_eq!(plan_data_home(legacy, home, false, false), DataHomePlan::Keep);
    }

    /// 真搬一次：沙箱里造 `Code2` 带嵌套文件，跑 `migrate_data_home` ⇒ 变成 `Blade2`、内容还在。
    #[test]
    fn migrate_data_home_moves_code2_to_blade2_in_sandbox() {
        let sandbox = Sandbox::new("move");
        sandbox.make_dir(LEGACY_DATA_HOME_DIR);
        sandbox.make_file("Code2\\sessions\\note.txt", "会话正文");

        let home = migrate_data_home(sandbox.dir());

        assert_eq!(home, sandbox.dir().join(DATA_HOME_DIR));
        assert!(!sandbox.dir().join(LEGACY_DATA_HOME_DIR).exists(), "搬完老目录必须消失（是改名不是复制）");
        assert!(
            std::fs::read(home.join("sessions").join("note.txt")).is_ok_and(|body| body == "会话正文".as_bytes()),
            "搬迁不许丢内容"
        );
    }

    /// 幂等：搬完再跑、以及「新目录已在 + 老目录也在」两种情形，都**不得报错、不得重复移动**。
    #[test]
    fn migrate_data_home_is_idempotent_across_repeated_calls() {
        let sandbox = Sandbox::new("idempotent");
        sandbox.make_dir(LEGACY_DATA_HOME_DIR);
        sandbox.make_file("Code2\\shell.json", "{}");

        let first = migrate_data_home(sandbox.dir());
        assert!(first.is_dir());
        let after_first = std::fs::read_dir(sandbox.dir()).expect("列沙箱根").count();

        // 再跑 N 次：一个字都不该动，也不该 panic。
        for _ in 0..3 {
            assert_eq!(migrate_data_home(sandbox.dir()), first, "返回值必须稳定");
        }
        assert_eq!(
            std::fs::read_dir(sandbox.dir()).expect("列沙箱根").count(),
            after_first,
            "重复调用不得再产生/再吃掉任何目录"
        );
        assert!(!sandbox.dir().join(LEGACY_DATA_HOME_DIR).exists(), "更不许出现第二次移动");
        // 纯函数侧同一件事：Move 成功后把 legacy 置为不存在 ⇒ Keep（§3.5 要的幂等断言）。
        let home = first.clone();
        let legacy = sandbox.dir().join(LEGACY_DATA_HOME_DIR);
        assert_eq!(plan_data_home(&legacy, &home, false, true), DataHomePlan::Keep);
    }

    /// 冲突策略（§2.3）：新旧都在 ⇒ **保新**，旧目录连一个字节都不许动（不合并、不覆盖、不删）。
    #[test]
    fn migrate_data_home_keeps_new_dir_and_leaves_legacy_untouched() {
        let sandbox = Sandbox::new("conflict");
        sandbox.make_dir(LEGACY_DATA_HOME_DIR);
        sandbox.make_file("Code2\\shell.json", "旧的");
        sandbox.make_dir(DATA_HOME_DIR);
        sandbox.make_file("Blade2\\shell.json", "新的");

        let home = migrate_data_home(sandbox.dir());

        assert_eq!(std::fs::read_to_string(home.join("shell.json")).expect("读新"), "新的");
        assert_eq!(
            std::fs::read_to_string(sandbox.dir().join("Code2").join("shell.json")).expect("读旧"),
            "旧的",
            "旧目录必须原样躺在盘上（主干宁可留孤儿 Code2 也不覆盖新数据）"
        );
    }

    /// `catch {}` 语义（§2.3 + 任务书要的「失败后仍能启动」）：让 `fs::rename` 真失败一次 ⇒
    /// 本函数不 panic、照样给出可用的数据家路径，且**绝不删源**（盘上至少还剩一份完整数据），
    /// 下次启动还会再试。
    ///
    /// 为什么注入要下两刀：**实测**只拿一颗同名文件堵住目标是搬不动的 —— Windows 的
    /// `std::fs::rename` 走 `MoveFileExW(.., MOVEFILE_REPLACE_EXISTING)`，会把那颗文件**直接换成目录**
    /// （第一版测试就是这么挂的）。所以 ① 把目标设成只读（ReplaceExisting 撞只读 ⇒ 拒绝），
    /// ② 再在源目录里开一个句柄（= 主干注释点名的「旧目录被占用」真机形态）。
    #[test]
    fn migrate_data_home_still_starts_when_the_move_fails() {
        let sandbox = Sandbox::new("move-fails");
        sandbox.make_dir(LEGACY_DATA_HOME_DIR);
        let inside = sandbox.make_file("Code2\\shell.json", "旧数据");

        let blocker = sandbox.make_file(DATA_HOME_DIR, "占位");
        let mut perms = std::fs::metadata(&blocker).expect("读占位文件属性").permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&blocker, perms).expect("把占位目标设成只读");
        let held = std::fs::File::open(&inside).expect("占住源目录里的文件");

        let home = migrate_data_home(sandbox.dir()); // 关键：这一发**不许 panic、不许返回 Err**

        let legacy_dir = sandbox.dir().join(LEGACY_DATA_HOME_DIR);
        assert_eq!(home, sandbox.dir().join(DATA_HOME_DIR), "搬迁失败也要照原路径给出去（静默继续）");
        assert!(blocker.is_file(), "移动没有得逞（本次注入确实注入上了）");
        assert!(legacy_dir.is_dir(), "失败不许删源 —— 盘上必须仍有一份完整数据");
        assert_eq!(
            std::fs::read_to_string(legacy_dir.join("shell.json")).expect("读旧数据"),
            "旧数据",
            "旧数据一个字节都不许少"
        );
        // 主干 getter 惰性重算、无"已失败"记录 ⇒ 下次启动仍会判成 Move 再试一次。
        assert_eq!(
            plan_data_home(&legacy_dir, &home, legacy_dir.is_dir(), home.is_dir()),
            DataHomePlan::Move { legacy: legacy_dir.clone(), home: home.clone() }
        );

        drop(held);
        // 收尾：清掉只读位，好让沙箱的 `Drop` 删得干净。
        if let Ok(mut perms) = std::fs::metadata(&blocker).map(|meta| meta.permissions()) {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(&blocker, perms);
        }
    }

    /// §3.5 的顺序要求：`BLADE2_DSH_HOME` 保留、但**排在搬迁之后**，不许拿它绕过搬迁。
    #[test]
    fn env_override_does_not_bypass_the_migration() {
        let sandbox = Sandbox::new("env-order");
        sandbox.make_dir(LEGACY_DATA_HOME_DIR);
        sandbox.make_file("Code2\\shell.json", "{}");
        let elsewhere = sandbox.make_dir("elsewhere");

        let chosen = resolve_dsh_home_with(sandbox.dir(), Some(&elsewhere));

        assert_eq!(chosen, elsewhere, "env 覆盖排在搬迁之后、仍然赢");
        // 关键在副作用这一半：即便返回值被 env 顶掉了，盘上的搬迁**照样发生**。
        assert!(!sandbox.dir().join(LEGACY_DATA_HOME_DIR).exists(), "env 不许让搬迁变成空操作");
        assert!(sandbox.dir().join(DATA_HOME_DIR).is_dir());
    }

    /// 分叉既有的 `LOCALAPPDATA` 兜底（主干没有这一条，见函数注释）：拿不到 ⇒ 根退化成 `.`，
    /// 于是数据家变成相对的 `./Blade2`。
    #[test]
    fn data_home_root_falls_back_to_relative_when_localappdata_missing() {
        assert_eq!(data_home_root(None), p("."));
        assert_eq!(
            data_home_root(Some(Path::new(r"C:\Users\me\AppData\Local"))),
            p(r"C:\Users\me\AppData\Local")
        );
        // 根 + 目录名 = 数据家；老 Code2 / 新 Blade2（主干 `MainWindow.xaml.cs:8576-8577`）。
        assert_eq!((DATA_HOME_DIR, LEGACY_DATA_HOME_DIR), ("Blade2", "Code2"));
        assert_eq!(data_home_root(None).join(DATA_HOME_DIR), Path::new(".").join(DATA_HOME_DIR));
        // 沙箱里把这条兜底走通：`.` 根拿不到 LOCALAPPDATA 时的形态，搬迁名照样对。
        let sandbox = Sandbox::new("relative-root");
        sandbox.make_dir(LEGACY_DATA_HOME_DIR);
        assert!(migrate_data_home(sandbox.dir()).ends_with(DATA_HOME_DIR));
    }

    /// 离线逐字节比对用的**字符串**打印（`tmp/fg2-cmdline.txt` 的产物来源）。
    /// **一个进程都不启动**：只算字符串，不 `spawn`、不 `Command::status()`、不碰 `Kernel\` 下任何东西。
    ///
    /// 主干侧那一串是**从 `DshKernelHost.cs:61` 的 C# 字面量逐字符誊写**下来的独立 oracle
    /// （`$"/c \"\"{dshCmd}\" web --no-open --port 0\""` → 下面的 raw string），刻意**不**复用
    /// [`npm_dsh_cmd_arguments`] —— 拿自己的函数验自己的函数等于没验。
    /// §4-① 说这一层"必须真机核对才能宣称对齐"：这里能定死的只有「两侧构造出的字符串逐字节相同」
    /// +「分叉侧走 `raw_arg` 原样传递」这两条；`cmd.exe` 自己怎么再解析那条 `lpCommandLine`，
    /// 仍是读码读不出来的另一半（不许拿本条冒充那一半）。
    #[test]
    fn dump_cmdline_shapes_for_offline_byte_compare() {
        let app_data = r"C:\Users\Admin\AppData\Roaming";
        let dsh = npm_dsh_cmd_path(Path::new(app_data));
        let dsh_text = dsh.display().to_string();

        // ---- 主干侧（从 C# 字面量誊写；`.` 处即 `{dshCmd}` 的插值位）----
        let mainline_arguments = format!(r#"/c ""{dsh_text}" web --no-open --port 0""#);
        // .NET 交给 CreateProcessW 的是 `fileName + " " + arguments`。
        let mainline_line = format!("cmd.exe {mainline_arguments}");

        // ---- 分叉侧（真进 `raw_arg` 的那一串）----
        let plan = launcher_argv(&Launcher::NpmDshCmd(dsh.clone())).expect("回退级必须有 argv");
        let fork_line = launcher_command_line(&Launcher::NpmDshCmd(dsh)).expect("回退级必须有命令行");

        // ---- 仓库里唯一那条真机观测（`DELIVERY-NOTES-0.7.4.md:148`），**整串手抄、不做任何插值** ----
        let observed =
            r#"cmd.exe /c ""C:\Users\Admin\AppData\Roaming\npm\dsh.cmd" web --no-open --port 0""#;

        println!("=== fg2 #84-A cmd.exe 命令行逐字节比对（纯字符串，未启动任何进程）===");
        println!("产出方式: cargo test --offline --lib dump_cmdline_shapes -- --nocapture");
        println!("三条串: [A] 主干 C# 誊写  [B] 分叉生产代码算出  [C] 仓库真机观测手抄");
        println!("APPDATA 取样      : {app_data}");
        println!();
        println!("[A] 主干侧 DshKernelHost.cs:57-61");
        println!("    fileName      : cmd.exe");
        println!("    arguments     : {mainline_arguments}");
        println!("    lpCommandLine : {mainline_line}");
        println!("    字节数        : {}", mainline_line.len());
        println!();
        println!("[B] 分叉侧 launcher_command_line(NpmDshCmd) -> CommandExt::raw_arg");
        println!("    exe           : {}", plan.exe.display());
        println!("    args[0]       : {}", plan.args[0]);
        println!("    args_verbatim : {}", plan.args_verbatim);
        println!("    lpCommandLine : {fork_line}");
        println!("    字节数        : {}", fork_line.len());
        println!();
        println!("[C] 仓库真机观测 DELIVERY-NOTES-0.7.4.md:148（整串手抄，未经任何代码加工）");
        println!("    lpCommandLine : {observed}");
        println!("    字节数        : {}", observed.len());
        println!();
        println!("hexdump [A] 主干 : {}", hexdump(&mainline_line));
        println!("hexdump [B] 分叉 : {}", hexdump(&fork_line));
        println!("hexdump [C] 观测 : {}", hexdump(&observed));
        println!(
            "A vs B 是否逐字节相同 : {}",
            if hexdump(&mainline_line) == hexdump(&fork_line) { "相同" } else { "有差异" }
        );
        println!(
            "A vs C 是否逐字节相同 : {}",
            if hexdump(&mainline_line) == hexdump(&observed) { "相同" } else { "有差异" }
        );
        println!();
        println!("[D] 内置级对照组（机制不同，别拿它当逐字节证据）");
        println!(
            "    lpCommandLine : {}",
            launcher_command_line(&Launcher::Bundled {
                node: p(r"C:\app\Kernel\node.exe"),
                bin_js: p(r"C:\app\Kernel\dsh\lib\bin.js"),
            })
            .expect("内置级有线")
        );
        println!(
            "    ^ 内置级分叉走 Command::args 的列表转义，主干走 Arguments 原样串；\n      两家的等效性在 argv 那一层，不在这一串字节层。"
        );
        println!();
        println!(concat!(
            "结论与边界:\n",
            "  + 已定死: 分叉侧交给 raw_arg 的串 == 主干 Arguments 串 == 仓库真机观测（80 字节逐字节相同）。\n",
            "  + 已定死: raw_arg 在 rustc 1.95.0 稳定可用（规格 4-(1) 没能确认的那一点），且 args 只有一颗，\n",
            "            所以 join(\" \") 即原串，std 不会再插反斜杠转义。\n",
            "  - 未定死: cmd.exe 拿到这串之后的自我解析结果（真正落进 GetCommandLineW 后它怎么拆参数）。\n",
            "            本任务禁起进程，故这一半仍未验；规格 4-(1) 要求真机核对，未做，不拿上面的相等冒充。"
        ));

        assert_eq!(fork_line, mainline_line, "分叉侧与主干侧必须逐字节相同");
        assert_eq!(fork_line, observed, "与仓库里那条真机观测也须逐字节相同");
        assert!(plan.args_verbatim, "这一串必须原样传递，否则 std 会把内嵌引号转义成 \\\"");
        assert_eq!(plan.args.len(), 1, "原样串只许一颗，`join(\" \")` 才等于它本身");
    }

    /// 把字符串摊成 `63 6D 64 …` 的十六进制，逐字节比对的证据用它，不靠肉眼看引号。
    fn hexdump(text: &str) -> String {
        text.as_bytes()
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    // ---------------- #74 `$events` 交互通道 ----------------

    fn waterfall(event: &str, event_id: &str, request: Value) -> EventsFrame {
        EventsFrame::Waterfall {
            event: event.to_string(),
            event_id: event_id.to_string(),
            agent_id: Some("s-1".to_string()),
            request,
        }
    }

    fn ready_frame(client_id: &str) -> EventsFrame {
        EventsFrame::Ready {
            client_id: client_id.to_string(),
        }
    }

    fn keys_of(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .map(|record| record.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    /// `$events` 只认自己那张四型表；控制面/工作区流的帧型一律不认（先认流再认帧型）。
    #[test]
    fn events_frame_table_reads_only_the_four_types_the_gateway_pushes() {
        for kind in EVENTS_FRAME_TYPES {
            assert_eq!(events_frame_type(&json!({ "type": kind })), Some(kind));
        }
        for frame in [
            json!({ "type": "baseline", "value": { "items": [] } }),
            json!({ "type": "upsert" }),
            json!({ "type": "projection" }),
            json!({ "kind": "ready" }),
            json!({ "type": 1 }),
            Value::Null,
        ] {
            assert_eq!(
                events_frame_type(&frame),
                None,
                "这一帧不该被认成 $events 帧: {frame}"
            );
        }
        // 必填键坏掉 ⇒ 整帧不认：半个载荷绝不能拿去回帧。
        assert_eq!(parse_events_frame(&json!({ "type": "ready" })), None);
        assert_eq!(
            parse_events_frame(&json!({ "type": "ready", "clientId": "" })),
            None
        );
        assert_eq!(parse_events_frame(&json!({ "type": "cancel" })), None);
        assert_eq!(parse_events_frame(&json!({ "type": "emit" })), None);
        assert_eq!(
            parse_events_frame(&json!({ "type": "waterfall", "event": APPROVAL_EVENT })),
            None
        );
        assert_eq!(
            parse_events_frame(&json!({
                "type": "waterfall", "event": APPROVAL_EVENT, "eventId": "e-1",
                "agentId": "s-1", "request": { "toolName": "bash" }
            })),
            Some(EventsFrame::Waterfall {
                event: APPROVAL_EVENT.to_string(),
                event_id: "e-1".to_string(),
                agent_id: Some("s-1".to_string()),
                request: json!({ "toolName": "bash" }),
            })
        );
        // `agentId` 与 `request` 都可缺席（网关只硬要求 event/eventId）：缺席落成 None/Null。
        let bare = parse_events_frame(
            &json!({ "type": "waterfall", "event": QUESTION_EVENT, "eventId": "e-2" }),
        )
        .expect("event/eventId 齐备即成立");
        assert_eq!(
            bare,
            EventsFrame::Waterfall {
                event: QUESTION_EVENT.to_string(),
                event_id: "e-2".to_string(),
                agent_id: None,
                request: Value::Null,
            }
        );
        assert_eq!(bare.kind(), "waterfall");
    }

    /// 登记层（`EVENTS_SUBSCRIPTIONS` 是唯一事实源）：表上 10 发名字都要被认出来，
    /// 且**只在自己那一支上**被认出 —— 主干那两条 `if` 各查各的注册表
    /// （`DshRpcClient.cs:590-611` 只喂 `emit`、`:618-628` 只喂 `waterfall`），串门即漏。
    #[test]
    fn every_registered_event_name_resolves_on_its_own_channel_only() {
        for name in EVENTS_SUBSCRIPTIONS {
            let emitted = json!({ "type": "emit", "event": name, "args": ["s-1", true] });
            let asked = json!({
                "type": "waterfall", "event": name, "eventId": "e-1", "request": {},
            });
            let wants_waterfall = matches!(name, APPROVAL_EVENT | QUESTION_EVENT);
            let (right, wrong) = if wants_waterfall {
                (&asked, &emitted)
            } else {
                (&emitted, &asked)
            };
            assert_eq!(
                events_subscription(right),
                Some(name),
                "登记表里这一发认不出: {name}"
            );
            assert_eq!(
                events_subscription(wrong),
                None,
                "{name} 串门了：它只属于 {} 那一支",
                if wants_waterfall { "waterfall" } else { "emit" }
            );
            // 认名字不改帧型：解出来的帧 `kind()` 仍是四型表上的值，且访问器与自由函数同结论
            // —— 下一刀只解析一次帧，走的是 `EventsFrame::subscription` 这一条。
            let parsed = parse_events_frame(right).expect("登记过的帧必然可解析");
            assert!(EVENTS_FRAME_TYPES.contains(&parsed.kind()));
            assert_eq!(parsed.subscription(), Some(name));
        }
        // 表 10 条、无重复 ⇒ 下一刀按它分流时不会有两条臂抢同一发名字。
        assert_eq!(EVENTS_SUBSCRIPTIONS.len(), 10);
        let unique: std::collections::BTreeSet<&'static str> =
            EVENTS_SUBSCRIPTIONS.into_iter().collect();
        assert_eq!(unique.len(), EVENTS_SUBSCRIPTIONS.len());
    }

    /// 名字比对逐字进行（主干那是 `Dictionary<string,…>` 的 `TryGetValue`）：
    /// 换大小写、加命名空间前缀、尾巴多个空格、换个后缀一律认不出。
    #[test]
    fn registered_names_match_verbatim_not_by_prefix_or_case() {
        for noise in [
            "Commands/Change",
            "COMMANDS/CHANGE",
            "dsh/commands/change",
            "commands/change ",
            "commands/change-all",
            "Api-Session/Status",
            "api-session/Added",
            "cordis/request-run-all",
        ] {
            assert_eq!(
                events_subscription(&json!({ "type": "emit", "event": noise, "args": [] })),
                None,
                "{noise} 不该被认成登记名"
            );
        }
    }

    /// 没登记过的 `emit` 名字 ⇒ `None`（下一刀据此决定「未登记帧」要不要出声）。
    /// ⚠ `api-session/title` 是既有夹具里的负样本（`session_status_event` 那一条）：
    ///   它**故意不在表上** —— 改名走 `session/title` 那条流，主干 `$events` 上没有这一发。
    #[test]
    fn unregistered_event_names_and_broken_payloads_are_not_recognised() {
        for event in [
            "api-session/title",
            "session/status",
            "turn/end",
            "assistant/live-chunk",
            "",
        ] {
            assert_eq!(
                events_subscription(
                    &json!({ "type": "emit", "event": event, "args": ["s-1", true] })
                ),
                None,
                "{event} 没登记过"
            );
        }
        // 必填键坏掉 ⇒ 整帧不认（与 `parse_events_frame` 同口径，不许 panic、不许半个载荷）。
        for frame in [
            json!({ "type": "emit", "args": [] }),
            json!({ "type": "emit", "event": 7 }),
            json!({ "type": "emit", "event": "" }),
            json!({ "type": "waterfall", "event": APPROVAL_EVENT }),
            Value::Null,
        ] {
            assert_eq!(events_subscription(&frame), None, "这一帧不该认出名: {frame}");
        }
    }

    /// 帧型判别与事件名判别**互不干扰**：认得名字救不了坏帧型，认得帧型也不白送名字。
    #[test]
    fn frame_type_and_event_name_are_two_independent_predicates() {
        let wrong_type = json!({ "type": "baseline", "event": "api-session/added" });
        assert_eq!(events_frame_type(&wrong_type), None);
        assert_eq!(events_subscription(&wrong_type), None);

        // `ready` 帧上就算贴了登记名也不认（R2 同口径：ready 只带 clientId）。
        let ready = json!({ "type": "ready", "clientId": "c-1", "event": "api-session/added" });
        assert_eq!(events_frame_type(&ready), Some("ready"));
        assert_eq!(parse_events_frame(&ready).map(|f| f.kind()), Some("ready"));
        assert_eq!(events_subscription(&ready), None);
        assert_eq!(
            events_subscription(&json!({ "type": "cancel", "eventId": "e-1" })),
            None
        );

        // 已登记那一发（`api-session/status`）解析体逐字未动：帧型、载荷、名字三样同时成立。
        let status = json!({
            "type": "emit", "event": "api-session/status", "args": ["s-1", true],
        });
        assert_eq!(events_frame_type(&status), Some("emit"));
        assert_eq!(events_subscription(&status), Some("api-session/status"));
        assert_eq!(
            parse_events_frame(&status)
                .as_ref()
                .and_then(EventsFrame::status_payload),
            Some(("s-1".to_string(), true))
        );
    }

    /// R2：clientId 只来自 `ready` 那一帧。emit/waterfall 上同名的键一概不作数
    /// —— 拿它回帧，网关回的是 `identifies no active event stream`（探针实测过的那条）。
    #[test]
    fn client_id_is_taken_only_from_the_ready_frame() {
        let mut ledger = InteractionLedger::new();
        assert!(!ledger.is_ready());
        for frame in [
            json!({
                "type": "emit", "event": "api-session/status", "args": ["s-1", true],
                "clientId": "poison-from-emit"
            }),
            json!({
                "type": "waterfall", "event": APPROVAL_EVENT, "eventId": "e-1",
                "agentId": "s-1", "clientId": "poison-from-waterfall",
                "request": { "toolName": "bash", "reason": "escalate sandbox to danger-full-access: 装包" }
            }),
        ] {
            let parsed = parse_events_frame(&frame).expect("形状合法");
            let delta = ledger.apply(0, &parsed);
            assert!(!delta.ready, "非 ready 帧不该报 ready: {frame}");
            assert!(!ledger.is_ready(), "非 ready 帧不得写下 clientId");
            assert_eq!(ledger.client_id(), None);
        }
        // ready 未到时没有回帧出口，错误文案逐字照主干（上层据此走「解锁 + 提示重试」）。
        let error = ledger
            .approval_result("e-1", ApprovalDecision::AllowedOnce)
            .unwrap_err();
        assert_eq!(error, "事件连接已失效，请等待重新连接后再回答。");
        // 但这一帧不许丢：主干那 10 秒等的就是 ready，卡要等得起（U2 的缓冲口径）。
        assert!(ledger.active_approval().is_some(), "ready 之前到的 waterfall 要留在台账里");
        assert!(!ledger.mark_submitting("e-1"), "没 ready 就不许上提交锁");

        let delta = ledger.apply(0, &ready_frame("c-1"));
        assert!(delta.ready && ledger.is_ready());
        assert_eq!(ledger.client_id(), Some("c-1"));
        assert!(ledger.mark_submitting("e-1"));
        assert!(!ledger.mark_submitting("e-2"), "已有一张在提交 ⇒ 第二发不许抢锁");
        assert_eq!(ledger.submitting(), Some("e-1"));
    }

    /// 回帧体逐字对齐网关 `parseRemoteEventResult`：恰好三键、outcome 三型各一例、
    /// `error` 的键集合；多一个键（requestId/stack）当场拒在出口，不到网关才回报错。
    #[test]
    fn event_result_frames_carry_exactly_the_validated_wire_shape() {
        let mut ledger = InteractionLedger::new();
        ledger.apply(0, &ready_frame("c-9"));
        ledger.apply(
            0,
            &waterfall(APPROVAL_EVENT, "e-a", json!({ "toolName": "bash" })),
        );

        let answers = json!([
            { "id": "q-1", "selected": ["甲"] },
            { "id": "q-2", "selected": [], "custom": "换个说法" }
        ]);
        let cases: Vec<(Value, Value)> = vec![
            // 审批「允许一次」：value 是**字符串**。
            (
                approval_outcome(ApprovalDecision::AllowedOnce),
                json!({ "kind": "result", "value": "allowed-once" }),
            ),
            (
                approval_outcome(ApprovalDecision::Rejected),
                json!({ "kind": "result", "value": "rejected" }),
            ),
            // 提问「确认执行」：value 是 `{answers:[…]}`。
            (
                question_outcome(answers.clone()),
                json!({ "kind": "result", "value": { "answers": answers.clone() } }),
            ),
            // 撤卡伴随的回帧（R5）：`next` 与「放弃整组」的 `rejected`。
            (wf_next(), json!({ "kind": "next" })),
            (
                wf_ask_cancelled(),
                json!({
                    "kind": "rejected",
                    "error": {
                        "name": "UserQuestionError",
                        "message": "the user cancelled ask_user_question",
                        "code": "ASK_CANCELLED"
                    }
                }),
            ),
        ];
        for (outcome, expected) in cases {
            let request = ledger
                .result_request("e-a", outcome.clone())
                .expect("合法 outcome");
            assert_eq!(request.client_id, "c-9");
            assert_eq!(request.event_id, "e-a");
            assert_eq!(request.outcome, expected);
            assert_eq!(keys_of(&request.to_args()), ["clientId", "eventId", "outcome"]);
            assert_eq!(request.to_args()["outcome"], expected);
            assert_eq!(outcome_error(&expected), None, "{expected} 该过网关校验");
        }
        assert_eq!(EventResultRequest {
            client_id: "c".into(),
            event_id: "e".into(),
            outcome: wf_next(),
        }
        .to_args()
        .pointer("/outcome/kind")
        .and_then(Value::as_str), Some("next"));

        // 网关硬拒的那几型：多键、错键、第三种审批字面量。
        assert!(
            ledger.result_request("e-a", json!({ "kind": "next", "value": 1 }))
                .is_err(),
            "next 不许带 value"
        );
        assert!(
            ledger
                .result_request(
                    "e-a",
                    json!({ "kind": "result", "value": "yes", "requestId": "r-1" })
                )
                .is_err(),
            "outcome 多一个 requestId 就拒（R1）"
        );
        assert!(
            ledger
                .result_request(
                    "e-a",
                    wf_rejected("UserQuestionError", "boom", None)
                )
                .is_ok_and(|request| {
                    keys_of(&request.outcome["error"]) == ["message", "name"]
                }),
            "code 缺席时不得冒出这个键"
        );
        let with_stack = json!({
            "kind": "rejected",
            "error": { "name": "E", "message": "m", "stack": "at foo" }
        });
        assert!(outcome_error(&with_stack).is_some(), "error 带 stack 必被拒");
        assert!(
            outcome_error(&json!({ "kind": "result" })).is_none(),
            "value 整个缺席也是合法 result（网关按 hasOwn 判）"
        );
        assert!(
            ledger.result_request("", wf_next()).is_err(),
            "空 eventId 到不了网关"
        );
        assert!(
            ledger.result_request("  ", wf_next()).is_err(),
            "空白 eventId 一样拒"
        );
        assert_eq!(APPROVAL_OUTCOMES, ["allowed-once", "rejected"]);
        assert_eq!(
            APPROVAL_OUTCOMES
                .into_iter()
                .filter_map(ApprovalDecision::parse)
                .count(),
            2,
            "两颗钮之外不许有第三种字面量"
        );
        assert_eq!(ApprovalDecision::parse("allowed"), None);
        assert_eq!(ApprovalDecision::parse("always"), None);
        assert_eq!(ApprovalDecision::parse("cancelled"), None);
    }

    /// 重连后到新一发 `ready` 之间可能**一个帧都没有**，那段空窗里旧 `clientId` 必须已作废，
    /// 否则回帧侧拿旧 socket 的串去答会被内核判 `identifies no active event stream`。
    #[test]
    fn drop_stream_voids_the_client_id_without_waiting_for_a_frame() {
        let mut ledger = InteractionLedger::new();
        ledger.apply(0, &ready_frame("cid-0"));
        ledger.apply(
            0,
            &waterfall(APPROVAL_EVENT, "e-1", json!({ "toolName": "bash" })),
        );
        assert!(ledger.mark_submitting("e-1"));
        assert_eq!(ledger.client_id(), Some("cid-0"));

        let delta = ledger.drop_stream(1);
        assert!(delta.generation_bumped);
        assert!(delta.unlocked, "提交锁要随代次解，否则卡永远禁用");
        assert_eq!(ledger.client_id(), None);
        assert_eq!(ledger.generation(), 1);
        assert!(
            ledger.active_approval().is_some(),
            "队列内容不清：内核会把未答帧按原 eventId 重推"
        );
        assert_eq!(
            ledger.drop_stream(1),
            LedgerDelta::default(),
            "同代次重复调用必须无副作用"
        );
    }

    /// R3：同一 eventId 在**新代次**仍算待答（网关向新 client 重推未答帧），
    /// 在**同代次**内被去重；旧代次的迟到帧整帧作废。
    #[test]
    fn repushed_event_id_survives_a_new_generation_but_dedupes_within_one() {
        let mut ledger = InteractionLedger::new();
        let frame = waterfall(
            APPROVAL_EVENT,
            "e-7",
            json!({ "toolName": "bash", "reason": "escalate sandbox to read-write: 装依赖" }),
        );
        ledger.apply(0, &ready_frame("gen-0"));
        let first = ledger.apply(0, &frame);
        assert_eq!(first.enqueued, Some(PendingKind::Approval));
        assert!(first.active_changed && !first.dropped.is_some());
        assert_eq!(ledger.generation(), 0);
        assert!(ledger.mark_submitting("e-7"));

        // 同代次重推：去重，不排第二张卡。
        let dup = ledger.apply(0, &frame);
        assert_eq!(dup.dropped, Some(FrameDrop::Duplicate("e-7".to_string())));
        assert_eq!(ledger.queued(PendingKind::Approval), 0);
        assert_eq!(ledger.active_approval().map(|item| item.event_id.as_str()), Some("e-7"));

        // 代次推进（`invalidate_mux` / 遇 end 重开）：旧 clientId 作废、提交锁解、队列内容留着。
        let bump = ledger.apply(1, &frame);
        assert!(bump.generation_bumped);
        assert!(bump.unlocked, "在途的提交锁必须随代次一起解，否则卡永远禁用");
        assert!(!bump.ready);
        assert_eq!(ledger.generation(), 1);
        assert!(!ledger.is_ready(), "旧 clientId 与新 socket 同生死（R2）");
        assert!(bump.active_changed, "重推要让这张卡重新亮起来");
        assert_eq!(
            ledger
                .active_approval()
                .map(|item| (item.event_id.clone(), item.generation)),
            Some(("e-7".to_string(), 1)),
            "重推只续代次，不排第二份"
        );
        assert!(!ledger.is_pending("no-such-event"));

        // 旧代次的迟到帧（上一根 socket 的残留批次）整帧丢。
        let stale = ledger.apply(0, &ready_frame("gen-0-again"));
        assert_eq!(stale.dropped, Some(FrameDrop::Stale(0)));
        assert!(!ledger.is_ready(), "旧代的 ready 更不许把 clientId 写回来");
    }

    /// 「取消先于请求到达」：cancel 落一颗同代次墓碑，后到的同 eventId 请求不再进 pending。
    #[test]
    fn cancel_arriving_first_leaves_a_tombstone() {
        let mut ledger = InteractionLedger::new();
        ledger.apply(0, &ready_frame("c-1"));
        let request = waterfall(APPROVAL_EVENT, "e-3", json!({ "toolName": "bash" }));

        let early = ledger.apply(0, &EventsFrame::Cancel { event_id: "e-3".into() });
        assert!(
            early.cancelled.is_empty(),
            "还没进台账，没东西可摘： {:?}",
            early
        );
        assert!(!ledger.is_pending("e-3"));

        let late = ledger.apply(0, &request);
        assert_eq!(late.dropped, Some(FrameDrop::Cancelled("e-3".to_string())));
        assert!(ledger.active_approval().is_none(), "被取消的帧不许再点亮");
        assert!(
            ledger
                .result_request("e-3", approval_outcome(ApprovalDecision::AllowedOnce))
                .is_err(),
            "不在台账里的 eventId 不该被 UI 拿去回帧"
        );

        // 新代次的重推不受旧墓碑影响：内核只重推未答帧，那一发必须还能点亮。
        let again = ledger.apply(1, &request);
        assert_eq!(again.enqueued, Some(PendingKind::Approval));
        assert!(again.active_changed);
        assert!(ledger.is_pending("e-3"));

        // 正卡在屏时收到取消：这张收掉、下一条顶上。
        ledger.apply(
            1,
            &waterfall(APPROVAL_EVENT, "e-4", json!({ "toolName": "git" })),
        );
        assert_eq!(
            ledger.active_approval().map(|item| item.event_id.as_str()),
            Some("e-3"),
            "第二条只进队列，不抢在显的那张"
        );
        assert_eq!(ledger.queued(PendingKind::Approval), 1);
        let cancelled = ledger.apply(1, &EventsFrame::Cancel { event_id: "e-3".into() });
        assert_eq!(cancelled.cancelled, ["e-3".to_string()]);
        assert!(cancelled.active_changed);
        assert_eq!(
            ledger.active_approval().map(|item| item.event_id.as_str()),
            Some("e-4"),
            "队列头顶上"
        );
        assert_eq!(ledger.queued(PendingKind::Approval), 0);

        // 结算之后内核还会补一帧 cancel（`finishRemoteEvent` 对每个收过帧的 client 都推）：
        // 只留墓碑，不该再动卡片状态。
        let settled = ledger.settle("e-4");
        assert!(settled.active_changed);
        assert!(ledger.active_approval().is_none());
        let trailing = ledger.apply(1, &EventsFrame::Cancel { event_id: "e-4".into() });
        assert!(
            trailing.cancelled.is_empty() && !trailing.active_changed,
            "补推的取消无卡可收: {trailing:?}"
        );
    }

    /// 提交结算：出账 + 解锁 + 下一条顶上；`badge` 给状态条那枚标记（approval 最高）。
    #[test]
    fn settle_unlocks_promotes_and_badges_by_priority() {
        let mut ledger = InteractionLedger::new();
        ledger.apply(0, &ready_frame("c-1"));
        ledger.apply(0, &waterfall(APPROVAL_EVENT, "e-a", json!({})));
        ledger.apply(
            0,
            &waterfall(
                QUESTION_EVENT,
                "e-q",
                json!({ "questions": [{ "id": "plan-review", "question": "按这版计划执行?" }] }),
            ),
        );
        assert!(ledger.mark_submitting("e-a"));
        let delta = ledger.settle("e-a");
        assert!(delta.unlocked && delta.active_changed);
        assert_eq!(ledger.submitting(), None);
        assert!(ledger.active_approval().is_none());

        let question = ledger.active_question().expect("提问卡在屏");
        assert_eq!(question.kind, PendingKind::Question);
        assert!(question.is_plan_review());
        assert_eq!(ledger.badge(), Some(Badge::PlanReview));
        assert!(
            ledger.settle("e-q").active_changed,
            "结算后要收卡"
        );
        assert_eq!(ledger.badge(), None, "台账空了就不该有待交互标记");

        ledger.apply(
            0,
            &waterfall(
                QUESTION_EVENT,
                "e-q2",
                json!({ "questions": [{ "id": "q-1", "question": "选哪个" }] }),
            ),
        );
        assert_eq!(ledger.badge(), Some(Badge::Question));
        ledger.apply(0, &waterfall(APPROVAL_EVENT, "e-b", json!({})));
        assert_eq!(
            ledger.badge(),
            Some(Badge::Approval),
            "approval(3) > plan-review(2) > question(1)"
        );
        assert_eq!(
            ledger
                .active_approval()
                .map(|item| (item.tool_name(), item.reason())),
            Some(("", "")),
            "内核没给的键一律读成空串，不 panic"
        );
    }

    /// answers 汇总照主干 `SubmitQuestionSetAsync`：跳过题只回空 `selected`；
    /// 单选 + 有补充说明 ⇒ `selected` 置空只留 `custom`；多选两者都回。
    /// 读的是 `multiSelect`，不是工具入参那个 `multi_select`（R4）。
    #[test]
    fn question_answers_follow_the_mainline_submit_drafts_rules() {
        let questions = [
            json!({ "id": "q-1", "question": "单选" }),
            json!({ "id": "q-2", "question": "多选", "multiSelect": true }),
            json!({ "id": "q-3", "question": "会被跳过" }),
            json!({ "id": "q-4", "question": "只读 multiSelect", "multi_select": true }),
        ];
        let drafts = [
            QuestionDraft {
                selected: vec!["甲".into()],
                custom: "  顺手补一句  ".into(),
                skipped: false,
            },
            QuestionDraft {
                selected: vec!["乙".into(), "丙".into()],
                custom: "".into(),
                skipped: false,
            },
            QuestionDraft {
                selected: vec!["别信这个".into()],
                custom: "不用了".into(),
                skipped: true,
            },
            QuestionDraft {
                selected: vec!["丁".into()],
                custom: "补充".into(),
                skipped: false,
            },
        ];
        assert_eq!(
            question_answers(&questions, &drafts),
            json!([
                { "id": "q-1", "selected": [], "custom": "顺手补一句" },
                { "id": "q-2", "selected": ["乙", "丙"] },
                { "id": "q-3", "selected": [] },
                { "id": "q-4", "selected": [], "custom": "补充" },
            ]),
            "单选+custom 置空 selected；多选两者都回；跳过的题连 custom 键都不许有"
        );
        assert!(is_multi_select(&questions[1]));
        assert!(
            !is_multi_select(&questions[3]),
            "q-4 只带工具入参那个 multi_select：读错键就会把它当多选、置空规则失效"
        );
        // 草稿缺位（题数对不上）按空草稿处理，不 panic。
        assert_eq!(
            question_answers(&questions, &[]),
            json!([
                { "id": "q-1", "selected": [] },
                { "id": "q-2", "selected": [] },
                { "id": "q-3", "selected": [] },
                { "id": "q-4", "selected": [] }
            ])
        );
    }
}

// ==================== 消息详情旁路表（#93 后续片 · 采集层，纯逻辑无 IO） ====================
//
// 主干那边是**六张旁路表**（`MainWindow.MessageDetails.cs:100-105`：`_messageDetails` /
// `_modelRetries` / `_compactions` / `_workflowRuns` / `_workflowBubbleRun` /
// `_maxTokensBubbles`），它们不画进 transcript，只从 journal 帧里默默采集，清表时机在
// `ResetMessageDomainState`（`MainWindow.MessageDetails.cs:117-130`，唯一调用点
// `MainWindow.xaml.cs:2590` 的换会话/清空那一串副作用）。分叉这边原先一张都没有
// （只有 `src/main.rs:3811-3814` 那颗吞帧计数）。
//
// 本片只落 `_messageDetails` 那一张的**采集层**：类型 + `note_*` 入口 + 清表入口 + 单测，
// **不碰 UI、不碰 main.rs**。其余五张要么带 `DispatcherQueueTimer`（`MessageDetails.cs:624-649`）
// 要么纯卡片态，归后一片。
//
// 采集面是**逐条核对主干原文**得到的，不是按事件名猜：`NoteMessageDomainEvent`
// （`MainWindow.MessageDetails.cs:135-190`，`RenderEventCore` 的唯一钩子在
// `MainWindow.xaml.cs:5995`）十七案里，真正往 `MessageDetailState` 上写字段的只有三案
// （全文件 `DetailState(` 的写入点实测 :215 / :255 / :274 / :318 / :494；:1204 与 :1277
// 是卡片消费侧）——
//   · `system/message`  `:141` → `HandleSystemMessage` `:215-221`
//   · `user/message`    `:143` → `HandleUserMessage` `:255-258`（中继）/ `:274-293`（召回）
//                        / `:318-322`（注入）/ `:331-340`（普通用户的附加块 + @ 引用）
//   · `request/context` `:145` → `NoteRequestContext` `:487-498`
// 外加主路径 `MainWindow.xaml.cs:6142` 建完用户气泡才调的 `AttachPendingUserDetail`
// （`MessageDetails.cs:348-356`）。
// `turn/end`（:148）/ `assistant/attempt`（:151）/ `tool/call`（:181）/ `tool/result`（:184）
// 对本表是 no-op（它们写的是 `_maxTokensBubbles` / `_workflowRuns` / 气泡自身字段）。

/// 寻址键。主干用 `ChatBubble` 的**对象身份**当键（`MessageDetails.cs:100`），分叉没有
/// 逐气泡对象身份 ⇒ 一律按值寻址，值由 main.rs 那一侧算好后喂进来（见 tmp/md1-report.md
/// 的接线清单）：能拿到气泡键就用 [`DetailsKey::bubble`]（与 `src/main.rs` 的 `bubble_key`
/// 同构），否则用帧坐标 [`DetailsKey::frame`] / [`DetailsKey::seq_index`]。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DetailsKey(String);

impl DetailsKey {
    /// 分叉气泡表的行键：`role-turn-seq`，与 `src/main.rs:709` 的 `bubble_key` 逐字同构。
    #[must_use]
    pub fn bubble(role: &str, turn: i64, seq: i64) -> Self {
        Self(format!("{role}-{turn}-{seq}"))
    }

    /// 内核报了 `message.id` 时用它（主干那颗 `MessageId` 就是同一枚值，`MainWindow.xaml.cs:6243`）。
    #[must_use]
    pub fn message_id(message_id: &str) -> Self {
        Self(format!("msg-{message_id}"))
    }

    /// 帧坐标兜底：这一帧还没落进气泡表（或压根不打算画）时按 (信封 seq, turn) 记。
    #[must_use]
    pub fn frame(seq: i64, turn: i64) -> Self {
        Self(format!("frame-{seq}-{turn}"))
    }

    /// (seq, index) 档：同一帧里多条并列记录（内容块 / 引用）各自占一格时用。
    #[must_use]
    pub fn seq_index(seq: i64, index: usize) -> Self {
        Self(format!("seq-{seq}-{index}"))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 一条 `source.references[]` 的结构化留档（分叉加项）。
///
/// 为什么要加：主干把那三个数字直接拼死进 zh 文案（`MessageDetails.cs:289-292`
/// 「保留 {0} 条 · 省略 {1} 条」+「已截断」），EN 档靠 `DetFormat` 现拼。分叉的 i18n 表是
/// 「中文键 → 译文」的查表口径（`src/i18n.rs:1697` 的 `l` / `:1710` 的 `lf`），拼好的串
/// 查不到 ⇒ 英文界面会露中文。故本表**照抄主干的拼装结果**（parity 优先）同时把结构化
/// 那份留在 [`MessageDetails::recalls`] 里，UI 侧要本地化就读它。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionRecall {
    pub session_id: String,
    pub label: String,
    pub retained: i64,
    pub omitted: i64,
    pub truncated: bool,
}

/// 一条消息的 Details 数据 = 主干 `MessageDetailState`（`MessageDetails.cs:42-53`）九个字段，
/// 外加 flyout 从**气泡自身**读的四项（`MessageId` / `ToolName` / `DurationMs` / `Tokens`，
/// 消费侧 `:1477-1480`）——分叉把它们并进同一行，UI 只取一处（谁喂：[`DetailsLedger::note_row_meta`]）。
///
/// 全部 `Option` / 空默认：`None` = 「这类帧没采到」，与主干「空串/`null` ⇒ `Field()` 整段收起」
/// 同口径（`:1451-1453` 的 `if (value.Length == 0) return;`）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageDetails {
    /// 主干 `MessageDetailState.Model`（`:49`，写于 `:496`）。
    pub model: Option<String>,
    /// 主干 `Provider`（`:48`，写于 `:495`）。
    pub provider: Option<String>,
    /// 主干 `ContextWindow`（`:50`，写于 `:497`，**只收 > 0 的**）。
    pub context_window: Option<i64>,
    /// 气泡侧字段（主干 `ChatBubble.DurationMs`，来源 `MainWindow.xaml.cs:6092`）。
    pub duration_ms: Option<i64>,
    /// 气泡侧字段（主干 `ChatBubble.Tokens`，来源 `MainWindow.xaml.cs:6256`）。
    pub tokens: Option<i64>,
    /// 气泡侧字段（主干 `ChatBubble.MessageId`，来源 `MainWindow.xaml.cs:6243`）。
    pub message_id: Option<String>,
    /// 气泡侧字段（主干 `ChatBubble.ToolName`，消费侧 `:1480`）。
    pub tool_name: Option<String>,
    /// 主干 `SystemPromptUpdate`（`:51`）：`system/message` 案里 `!first`（`:216`），
    /// `request/context` 案里显式置 `true`（`:498`）。
    pub system_prompt_update: Option<bool>,
    /// 主干 `SystemPromptText`（`:52`，写于 `:217`；消费侧 `:1284` 用它判可展开）。
    pub system_prompt_text: Option<String>,
    /// 主干 `RelaySessionId`（`:47`，写于 `:256`）。
    pub relay_session_id: Option<String>,
    /// 主干 `ExtraBlocks`（`:44`，`(block.type || "block", block.GetRawText())`，`:335-338`）。
    pub extra_blocks: Vec<(String, String)>,
    /// 主干 `ContextEntries`（`:45`，五处写入：`:218` / `:257` / `:289` / `:319` / 三个 Collect*）。
    pub context_entries: Vec<(String, String)>,
    /// 主干 `References`（`:46`，`(sessionId, label || sessionId)`，`:287`；@ 提及走待认领桶）。
    pub references: Vec<(String, String)>,
    /// 分叉加项（见 [`SessionRecall`] 的说明）。
    pub recalls: Vec<SessionRecall>,
    /// 分叉加项：主干 `CollectInstructionChanges`（`:430-450`）把 action 翻成 zh 标签后只留
    /// `(path, 标签)`，这里另存 `(path, 原始 action 词)`（`remove` / `set` / 其余原词，
    /// `baseline == true` 且非 remove 时记 `loaded`）。
    pub instruction_changes: Vec<(String, String)>,
    /// 分叉加项（#148 刀1 · DetText 第五族）：主干 `CollectCatalogEntries`
    /// （`MainWindow.MessageDetails.cs:452-468`）把 `shown - 8` 那发计数**只拼进文案**
    /// （本文件 [`collect_catalog_entries`] 尾部那一枚 `…还有 N 条`），表上没有可算的数位 ⇒
    /// 撤烤字之前先把数落表。`None` = 没溢出（主干 `if (shown > 8)` 不成立），
    /// 与「这类帧没采到」的既有口径同侧（见结构体文档注释）。权威：[`catalog_overflow_count`]。
    pub catalog_overflow: Option<i64>,
}

impl MessageDetails {
    /// 一行里什么都没有（主干那边等价于 `DetailState` 刚 new 出来的空壳）。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.model.is_none()
            && self.provider.is_none()
            && self.context_window.is_none()
            && self.duration_ms.is_none()
            && self.tokens.is_none()
            && self.message_id.is_none()
            && self.tool_name.is_none()
            && self.system_prompt_update.is_none()
            && self.system_prompt_text.is_none()
            && self.relay_session_id.is_none()
            && self.extra_blocks.is_empty()
            && self.context_entries.is_empty()
            && self.references.is_empty()
            && self.recalls.is_empty()
            && self.instruction_changes.is_empty()
    }
}

/// 采集回执：main.rs 只按这几个布尔决定要不要重画那一行。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DetailsDelta {
    /// 本帧落了哪一行（`None` = 本帧对本表没写）。
    pub wrote: Option<DetailsKey>,
    /// 待认领桶（附加块 / @ 引用）动了：还没有对应气泡行，下一发用户行落地时结算。
    pub pending_changed: bool,
}

impl DetailsDelta {
    /// 本表有没有被这帧动过。
    #[must_use]
    pub fn any(&self) -> bool {
        self.wrote.is_some() || self.pending_changed
    }
}

/// 气泡自身那几项的投递口（主干不放旁路表，分叉并表 ⇒ 给 main.rs 一个显式入口）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowMeta {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub duration_ms: Option<i64>,
    pub tokens: Option<i64>,
    pub message_id: Option<String>,
    pub tool_name: Option<String>,
}

/// 主干 `ModelRetryState`（`MessageDetails.cs:55-70`）那族「模型重试」旁路状态的等价物：
/// `_modelRetries`（`Dictionary<string,ModelRetryState>`，key = `retryId`，`:101`）里的一行。
/// 字段集与 trunk class 逐个对上，只把主干那三枚 UI/线程件（`ChatBubble Bubble` /
/// `DispatcherQueueTimer Timer` / `TextBlock StatusText`）换成一枚不透明的气泡寻址键
/// [`ModelRetryState::bubble_key`]——分叉没有逐气泡对象身份（`bubble_key` = `role-turn-seq`
/// 是它唯一的身份，见 main.rs `fn bubble_key`）；Timer/StatusText 是倒计时与重绘的载体，
/// 分叉渲染每帧现读本表 ⇒ 不落进状态（倒计时那一发是运行时挂账，见 rt1 报告 §7）。
#[derive(Clone, Debug)]
pub struct ModelRetryState {
    /// `RetryId`：帧里 `retryId` 为空 ⇒ 主干回落 `"retry-" + envTime + "-" + turn`（`:564-565`）。
    pub retry_id: String,
    /// `Retry`：`retry` 存在**且**是数字才取，否则 1（`:566`）。
    pub retry: i64,
    /// `MaxRetries`：`data` 是对象**且** `maxRetries` 是数字才取，否则 0（`:567-569`）。
    pub max_retries: i64,
    /// `DelayMs`：`delayMs` 是数字才取，否则 0（`:570`）。
    pub delay_ms: i64,
    /// `Mode`：空 ⇒ `"normal"`（`:571-572`）。渲染侧 `mode != "normal"` ⇒ 上限位显示 `∞`。
    pub mode: String,
    /// `FailureMessage` / `FailureCode`：`failure` 是对象才读两串，否则空（`:573-578`）。
    pub failure_message: String,
    pub failure_code: String,
    /// `State`：`scheduled | started | cancelled`。采集案恒写 `scheduled`（`:585`），
    /// `retry-started` 改写 `started`（`:619`），取消钮改写 `cancelled`（`:674`）。
    pub state: String,
    /// `DeadlineMs`：`now + delay`（`:586`）。wall-clock 由调用点喂进来，本函数保持纯、可离线测。
    pub deadline_ms: i64,
    /// 主干 `Bubble`（`ChatBubble` 引用）的等价物：这一发气泡的寻址键；空串 = 还没建气泡。
    pub bubble_key: String,
}

/// `_messageDetails` 那一张：按 [`DetailsKey`] 存 [`MessageDetails`]，加主干的两只待认领桶
/// （`_pendingUserExtras` / `_pendingUserRefs`，`MessageDetails.cs:344-345`）与那颗
/// `RunStats.SystemPromptSeen`（`:200-202`，本表用它判「系统提示词」还是「系统提示词更新」）。
///
/// #104：同族旁路表 `_maxTokensBubbles`（`MessageDetails.cs:105`）也寄在本结构上——它是主干
/// `NoteMessageDomainEvent` 那一族的另一张表，与 `rows`（=`_messageDetails`）**互不相干**：
/// `turn/end` / `assistant/attempt` 对 `rows` 恒为 no-op（见本文件 `:5229-5230` 那条既有口径），
/// 只动 [`DetailsLedger::max_tokens_turns`]。所以这两型帧的 [`Self::note_event`] 回执永远是
/// `DetailsDelta::default()`，「本表没写」这句真话不许被第二张表污染。
#[derive(Clone, Debug, Default)]
pub struct DetailsLedger {
    rows: BTreeMap<DetailsKey, MessageDetails>,
    pending_extra_blocks: Vec<(String, String)>,
    pending_references: Vec<(String, String)>,
    system_prompt_seen: bool,
    /// 主干 `_maxTokensBubbles`（`Dictionary<ChatBubble,bool>`，`:105`）的等价物。
    /// 主干那颗字典的**唯一**可读语义是 `AppendMaxTokensRow` 那道「同轮只出一条」的门
    /// （`:537-540`：遍历键，命中 `b.Turn == turn` 即 `return`），值恒 `true` 从不被读 ⇒
    /// 分叉没有逐气泡对象身份，折成「出过截断警示行的轮次」集合即可，`BTreeSet` 只为可测有序。
    max_tokens_turns: BTreeSet<i64>,
    /// 主干 `_modelRetries`（`Dictionary<string,ModelRetryState>`，key = `retryId`，`:101`）的
    /// 等价物。#111：模型重试那一族旁路表。用 `Vec` 而不是 `BTreeMap` 是为了留住**插入顺序**——
    /// 渲染侧那颗卡的兜底 `st ??= _modelRetries.Values.LastOrDefault()`（`:969`）取的是「最后
    /// 插入的一个」，`Dictionary.Values` 在 .NET 里增删前按插入序枚举 ⇒ `Vec` 是它的逐字等价，
    /// 换成按键排序的 `BTreeMap` 就把这条兜底改歪了。按 `retry_id` 找行、按 `bubble_key` 渲染，
    /// 都是线性扫（重试条数极小，与主干 `foreach` 找 bubble 同一量级）。
    model_retries: Vec<ModelRetryState>,
}

impl DetailsLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 清表（主干 `ResetMessageDomainState`，`MessageDetails.cs:117-130`）：换会话/清空时
    /// 整张旁路表连同待认领桶一并作废。分叉挂在 `Msg::Select` / `Msg::Branched` 那两处
    /// `bubbles.clear()` 旁边（`src/main.rs:7000` / `:7053`）。
    /// #104：主干 `:128` 那句 `_maxTokensBubbles.Clear()` 也在这一发里 —— 同一次清表、同一个入口。
    /// #111：主干 `:119-124` 那句「遍历 `_modelRetries.Values` 停 Timer + `_modelRetries.Clear()`」
    /// 也在同一发里 —— 分叉无 Timer 可停，整表清空等价。
    pub fn reset(&mut self) {
        self.rows.clear();
        self.pending_extra_blocks.clear();
        self.pending_references.clear();
        self.system_prompt_seen = false;
        self.max_tokens_turns.clear();
        self.model_retries.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn keys(&self) -> Vec<DetailsKey> {
        self.rows.keys().cloned().collect()
    }

    /// 消费侧读口（主干 `ShowMessageDetailsFlyout` 的 `_messageDetails.TryGetValue`，`:1446`）。
    #[must_use]
    pub fn get(&self, key: &DetailsKey) -> Option<&MessageDetails> {
        self.rows.get(key)
    }

    /// 待认领桶（只读，给日志/断言用）。
    #[must_use]
    pub fn pending_extra_blocks(&self) -> &[(String, String)] {
        &self.pending_extra_blocks
    }

    #[must_use]
    pub fn pending_references(&self) -> &[(String, String)] {
        &self.pending_references
    }

    #[must_use]
    pub fn system_prompt_seen(&self) -> bool {
        self.system_prompt_seen
    }

    /// 主干那颗 `SystemPromptSeen` 存在 `RunStats` 里、**按会话 id** 存（`:200-202`），
    /// 比旁路表活得久（`ResetMessageDomainState` 不清 RunStats）⇒ 分叉切完会话要用
    /// `src/main.rs:3808` 的 `system_prompt_seen: Vec<String>` 把它补回来。
    pub fn set_system_prompt_seen(&mut self, seen: bool) {
        self.system_prompt_seen = seen;
    }

    fn entry(&mut self, key: &DetailsKey) -> &mut MessageDetails {
        self.rows.entry(key.clone()).or_default()
    }

    /// 采集总入口（主干 `NoteMessageDomainEvent`，`:135-190`）：按 `event["type"]` 分流到
    /// 三案，认不出的名字**一律不脏表**（主干那三案之外对本表都是 no-op）。
    /// `row` = main.rs 为这一帧画的那一行气泡键；`None` = 没画/还不知道，注入案会自取帧坐标。
    ///
    /// #104：`turn/end` / `assistant/attempt` 两案走 [`Self::note_max_tokens`]。它们的
    /// `DetailsDelta` 回执**恒为 default**，这不是漏做：主干那两案对 `_messageDetails`
    /// 就是 no-op（`MessageDetails.cs:148-153` 两案都 `return false` 且不碰 `_messageDetails`），
    /// 写的是**同族另一张表** `_maxTokensBubbles`。要不要落那一行，看返回的 `bool`。
    pub fn note_event(&mut self, event: &Value, row: Option<&DetailsKey>) -> DetailsDelta {
        match event["type"].as_str().unwrap_or_default() {
            "system/message" => {
                let key = row.cloned().unwrap_or_else(|| Self::frame_key(event));
                self.note_system_message(event, &key)
            }
            "user/message" => self.note_user_message(event, row),
            "request/context" => self.note_request_context(event, row),
            // 主干 `:148-153` 两案：记完 _maxTokensBubbles 这笔账，`return false` 放行原路。
            "turn/end" | "assistant/attempt" => {
                self.note_max_tokens(event, event["data"]["turn"].as_i64().unwrap_or(0));
                DetailsDelta::default()
            }
            _ => DetailsDelta::default(),
        }
    }

    /// 主干 `_maxTokensBubbles`（`:105`）的折叠 + `AppendMaxTokensRow`（`:534-552`）那道
    /// 「同轮只出一条」的门（`:537-540`）。返回 `true` = **本轮第一次**命中截断 ⇒ main.rs
    /// 该落那一行警示卡；`false` = 这一帧不是截断形状，或本轮已经出过（重复回放不算新行）。
    ///
    /// 两型帧的判别键**逐字照主干**，含主干自己那处不对称：
    /// · `turn/end`（`NoteTurnEndReason` `:501-511`）读 `data.reason.kind == "max-tokens"`；
    /// · `assistant/attempt`（`NoteAttemptMaxTokens` `:513-530`）读 `data.stream[]`，每枚
    ///   `piece` 的 **`piece.type == "finish"`** 与 **`piece.reason.kind == "max-tokens"`**
    ///   —— 不套 `chunk`。主干另一条臂（那颗 ⚠ 失败气泡，`MainWindow.xaml.cs:6185-6186`）
    ///   读的才是 `piece.chunk.type` / `.chunk.reason.kind`；两臂路径本来就不同，分叉**不统一**
    ///   它们（统一就是改主干）。
    /// 一帧里多条命中也只落一行：主干 `:521-529` 的 foreach 会重复调 `AppendMaxTokensRow`，
    /// 第二次就被 `:539` 的门挡掉 ⇒ 这里同样靠 turn 集合天然幂等。
    pub fn note_max_tokens(&mut self, event: &Value, turn: i64) -> bool {
        let hit = match event["type"].as_str().unwrap_or_default() {
            "turn/end" => {
                let reason = &event["data"]["reason"];
                reason.is_object() && reason["kind"].as_str() == Some("max-tokens")
            }
            "assistant/attempt" => event["data"]["stream"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|piece| {
                    piece["type"].as_str() == Some("finish")
                        && piece["reason"].is_object()
                        && piece["reason"]["kind"].as_str() == Some("max-tokens")
                }),
            _ => false,
        };
        if !hit {
            return false;
        }
        // 主干 `:537-540`：同轮已有 ⇒ 整发丢弃（连表都不再写，等价于 insert 返回 false）。
        self.max_tokens_turns.insert(turn)
    }

    /// 渲染侧的门：这一轮该不该有那张截断警示卡。
    #[must_use]
    pub fn has_max_tokens(&self, turn: i64) -> bool {
        self.max_tokens_turns.contains(&turn)
    }

    /// 出过截断警示行的轮次（有序，给日志/断言用）。
    #[must_use]
    pub fn max_tokens_turns(&self) -> Vec<i64> {
        self.max_tokens_turns.iter().copied().collect()
    }

    // ---------------- #111 模型重试（主干 `_modelRetries`） ----------------

    fn retry_mut(&mut self, retry_id: &str) -> Option<&mut ModelRetryState> {
        self.model_retries.iter_mut().find(|st| st.retry_id == retry_id)
    }

    /// 主干 `HandleLlmRetry`（`:562-611`）的折叠：把一帧 `llm/retry` 折进 `_modelRetries`。
    /// `now_ms` = 采集那刻的 wall-clock（主干 `DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()`，
    /// `:586`），由调用点喂进来保纯。返回 `true` = **这一 retryId 第一次见 ⇒ main.rs 该建那一行
    /// 气泡**（主干 `:594` `if (st.Bubble is null)` 那一支，随后把 `bubble_key` 记进行里）；
    /// 返回 `false` = 已见过 ⇒ 只原地更新字段（主干走 `RepaintBubble`，分叉渲染每帧现读 ⇒ 无需动气泡）。
    ///
    /// 判据逐字照主干，含主干自己的回落与「必须同时是数字」两约束（见 [`ModelRetryState`] 字段注）。
    pub fn note_llm_retry(
        &mut self,
        event: &Value,
        turn: i64,
        env_time: i64,
        now_ms: i64,
        bubble_key: &str,
    ) -> bool {
        let data = &event["data"];
        let mut retry_id = data["retryId"].as_str().unwrap_or_default().to_string();
        if retry_id.is_empty() {
            retry_id = format!("retry-{env_time}-{turn}");
        }
        let number = |key: &str| data.get(key).filter(|v| v.is_number());
        let retry = number("retry").map_or(1, |v| v.as_i64().unwrap_or(1));
        let max_retries = number("maxRetries").map_or(0, |v| v.as_i64().unwrap_or(0));
        let delay_ms = number("delayMs").map_or(0, |v| v.as_i64().unwrap_or(0));
        let mut mode = data["mode"].as_str().unwrap_or_default().to_string();
        if mode.is_empty() {
            mode = "normal".to_string();
        }
        let (failure_message, failure_code) = match data.get("failure") {
            Some(failure) if failure.is_object() => (
                failure["message"].as_str().unwrap_or_default().to_string(),
                failure["code"].as_str().unwrap_or_default().to_string(),
            ),
            _ => (String::new(), String::new()),
        };

        // 主干 `:581-585` AddOrUpdate：见过的行原字段覆写（RetryId 不动），没见过的建一行。
        let deadline_ms = now_ms + delay_ms;
        if let Some(st) = self.retry_mut(&retry_id) {
            st.retry = retry;
            st.max_retries = max_retries;
            st.delay_ms = delay_ms;
            st.mode = mode;
            st.failure_message = failure_message;
            st.failure_code = failure_code;
            st.state = "scheduled".to_string();
            st.deadline_ms = deadline_ms;
            // 主干 `:594` `st.Bubble is null` ⇒ 分叉用 `bubble_key` 空串当同一判据：
            // 已有气泡（哪怕上一帧建的）⇒ 不再建新行。
            return st.bubble_key.is_empty();
        }
        self.model_retries.push(ModelRetryState {
            retry_id,
            retry,
            max_retries,
            delay_ms,
            mode,
            failure_message,
            failure_code,
            state: "scheduled".to_string(),
            deadline_ms,
            // 主干 `:595-604` 新建气泡时把 `ChatBubble` 引用存进 `st.Bubble`；分叉同一发把
            // 调用点（main.rs）算好的气泡寻址键存进 `bubble_key`，渲染侧按它对号。
            bubble_key: bubble_key.to_string(),
        });
        true
    }

    /// 主干 `HandleLlmRetryStarted`（`:614-622`）：`retry-started` 帧把对应行改成 `started`。
    /// 与采集案不同，这里 **retryId 空 ⇒ 直接 return**（主干 `:616` `retryId.Length == 0 ||
    /// !TryGetValue ⇒ return`，没有 envTime 回落）；找不到行也 return。返回 `true` = 改到了行。
    pub fn note_llm_retry_started(&mut self, event: &Value) -> bool {
        let retry_id = event["data"]["retryId"].as_str().unwrap_or_default();
        if retry_id.is_empty() {
            return false;
        }
        if let Some(st) = self.retry_mut(retry_id) {
            st.state = "started".to_string();
            return true;
        }
        false
    }

    /// 主干 `CancelModelRetry`（`:673+`）里「改状态」那一半：按气泡键找到行、压成 `cancelled`。
    /// （`_ = StopActiveRunAsync()` 那半边在 main.rs 的 reducer 里接既有 [`Shell::cancel_run`]。）
    pub fn cancel_model_retry(&mut self, bubble_key: &str) -> bool {
        if let Some(st) = self
            .model_retries
            .iter_mut()
            .find(|st| st.bubble_key == bubble_key)
        {
            st.state = "cancelled".to_string();
            return true;
        }
        false
    }

    /// 渲染侧读口（主干 `BuildRetryCard` `:966-969`）：先按气泡键找行，找不到回落「最后插入的一个」
    /// （主干 `st ??= _modelRetries.Values.LastOrDefault()`）。逐字照抄主干那处自带兜底。
    #[must_use]
    pub fn retry_for_bubble(&self, bubble_key: &str) -> Option<&ModelRetryState> {
        self.model_retries
            .iter()
            .find(|st| st.bubble_key == bubble_key)
            .or_else(|| self.model_retries.last())
    }

    /// 已登记的重试 `retryId`（按插入序，给日志/断言用）。
    #[must_use]
    pub fn model_retry_ids(&self) -> Vec<String> {
        self.model_retries.iter().map(|st| st.retry_id.clone()).collect()
    }


    fn frame_key(event: &Value) -> DetailsKey {
        DetailsKey::frame(event["seq"].as_i64().unwrap_or(0), event["data"]["turn"].as_i64().unwrap_or(0))
    }

    /// `system/message`（主干 `HandleSystemMessage` `:194-223`）：同会话第一条报「系统提示词」，
    /// 之后都是「系统提示词更新」——那颗 first 位由本表的 `system_prompt_seen` 记（见
    /// [`DetailsLedger::set_system_prompt_seen`] 的存活期差异）。
    pub fn note_system_message(&mut self, event: &Value, key: &DetailsKey) -> DetailsDelta {
        let data = &event["data"];
        if !data.is_object() {
            return DetailsDelta::default();
        }
        let body = extract_message_text(data, Some("message"));
        let update = self.system_prompt_seen;
        self.system_prompt_seen = true;
        let label = if update { "系统提示词更新" } else { "系统提示词" };
        let row = self.entry(key);
        row.system_prompt_update = Some(update);
        row.system_prompt_text = Some(body.clone());
        // 主干那颗气泡只有一条 ContextEntries（:218-220）⇒ 整行按这一帧重建，重放天然幂等。
        row.context_entries = vec![(label.to_string(), body)];
        DetailsDelta {
            wrote: Some(key.clone()),
            pending_changed: false,
        }
    }

    /// `user/message`（主干 `HandleUserMessage` `:227-342`）。四条分支照原文口径：
    /// · `source.kind` 缺省或 `"user"` ⇒ 普通提问：非 text/image 块进待认领桶（`:331-339`）、
    ///   正文里的 `dsh-session:` 进引用桶（`:340`），**都要等 `attach_pending_to` 才落表**；
    /// · `agent-message` + `form:relay` ⇒ 中继（`:242-260`）；
    /// · `session-reference` + `form:recall` ⇒ 跨会话召回（`:261-301`）；
    /// · 其余 `kind != "user"` ⇒ 上下文注入（`:303-324`）。
    pub fn note_user_message(&mut self, event: &Value, row: Option<&DetailsKey>) -> DetailsDelta {
        let data = &event["data"];
        let Value::Array(content) = &data["content"] else {
            return DetailsDelta::default(); // 主干 :229-234 的同款前置判据
        };
        let source = &data["source"];
        let kind = string_field(source, "kind");
        if source.is_object() && !kind.is_empty() && kind != "user" {
            let key = match row {
                Some(key) => key.clone(),
                None => Self::frame_key(event),
            };
            let form = string_field(source, "form");
            if kind == "agent-message" && form == "relay" {
                let sender = string_field(source, "senderSessionId");
                let body = extract_message_text(data, None);
                let row = self.entry(&key);
                if !sender.is_empty() {
                    row.relay_session_id = Some(sender);
                }
                // 主干那颗中继气泡只有这一条 ContextEntries（:257）⇒ 整行按帧重建。
                row.context_entries = vec![(det_zh("relay").to_string(), body)];
                return DetailsDelta {
                    wrote: Some(key),
                    pending_changed: false,
                };
            }
            if kind == "session-reference" && form == "recall" {
                let mut entries = Vec::new();
                let mut references = Vec::new();
                let mut recalls = Vec::new();
                if let Some(items) = source["references"].as_array() {
                    for item in items {
                        let label = string_field(item, "label");
                        let session_id = string_field(item, "sessionId");
                        if label.is_empty() && session_id.is_empty() {
                            continue; // 主干 :285 的两个都空就不登记
                        }
                        let recall = SessionRecall {
                            retained: number_of(item, "retainedMessages"),
                            omitted: number_of(item, "omittedMessages"),
                            truncated: item["truncated"].as_bool() == Some(true),
                            session_id: session_id.clone(),
                            label: label.clone(),
                        };
                        let shown = if label.is_empty() { session_id.clone() } else { label.clone() };
                        let mut text = det_zh("recall_counts")
                            .replace("{0}", &recall.retained.to_string())
                            .replace("{1}", &recall.omitted.to_string());
                        if recall.truncated {
                            text.push_str(&format!(" · {}", det_zh("recall_truncated")));
                        }
                        references.push((session_id, shown.clone()));
                        entries.push((det_zh("recall_title").replace("{0}", &shown), text));
                        recalls.push(recall);
                    }
                }
                let row = self.entry(&key);
                row.context_entries = entries;
                row.references = references;
                row.recalls = recalls;
                return DetailsDelta {
                    wrote: Some(key),
                    pending_changed: false,
                };
            }
            // 其余注入：标签取 plugin || path || label || kind（主干 :304-307）
            let mut label = string_field(source, "plugin");
            if label.is_empty() {
                label = string_field(source, "path");
            }
            if label.is_empty() {
                label = string_field(source, "label");
            }
            if label.is_empty() {
                label = kind.clone();
            }
            let body = extract_content_text(&data["content"]);
            let changes = collect_instruction_changes(source);
            let mut entries = vec![(label, body)];
            let mut instruction_changes = Vec::new();
            for (path, action, action_label) in changes {
                entries.push((path.clone(), action_label));
                instruction_changes.push((path, action));
            }
            entries.extend(collect_catalog_entries(source));
            entries.extend(collect_snapshot_sections(source));
            // 注入行的 ContextEntries 全数来自这一帧（主干 :319-322 三段 Collect*）
            // ⇒ 整行按帧重建，重放不翻倍。
            let row = self.entry(&key);
            row.context_entries = entries;
            row.instruction_changes = instruction_changes;
            // 溢出数位与上面那行的文案同源（主干 `:466` 的 `shown - 8`）：整行按帧重建 ⇒ 这里
            // 无条件覆写，`shown <= 8` 时落 `None`，重放不残留上一帧的数位。
            row.catalog_overflow = catalog_overflow_count(source);
            return DetailsDelta {
                wrote: Some(key),
                pending_changed: false,
            };
        }

        // 普通用户消息：只登记待认领的两桶，落不落表由 `attach_pending_to` 决定。
        let mut pending_changed = false;
        for block in content {
            let block_type = string_field(block, "type");
            if block_type == "text" || block_type == "image" {
                continue; // 主干 :334
            }
            let label = if block_type.is_empty() { "block" } else { &block_type };
            let json = block.to_string();
            if !self
                .pending_extra_blocks
                .iter()
                .any(|(l, j)| l == label && *j == json)
            {
                self.pending_extra_blocks.push((label.to_string(), json));
                pending_changed = true;
            }
        }
        let text = join_text_blocks(content);
        for (session_id, label) in collect_mention_refs(&text) {
            if !self
                .pending_references
                .iter()
                .any(|(s, l)| *s == session_id && *l == label)
            {
                self.pending_references.push((session_id, label));
                pending_changed = true;
            }
        }
        let mut wrote = None;
        if let Some(key) = row {
            // 主干 xaml.cs:6142 建完气泡才补登（`AttachPendingUserDetail` :348-356）。
            if self.attach_pending_to(key) {
                wrote = Some(key.clone());
            }
        }
        DetailsDelta { wrote, pending_changed }
    }

    /// 主干 `AttachPendingUserDetail`（`:348-356`）：把两只待认领桶并到那一行并清空。
    /// 返回是否真的动了表（两桶都空 ⇒ 不动，与主干那句提前 return 同）。
    pub fn attach_pending_to(&mut self, key: &DetailsKey) -> bool {
        if self.pending_extra_blocks.is_empty() && self.pending_references.is_empty() {
            return false;
        }
        let extras = std::mem::take(&mut self.pending_extra_blocks);
        let refs = std::mem::take(&mut self.pending_references);
        let row = self.entry(key);
        for (label, json) in extras {
            push_unique_pair(&mut row.extra_blocks, label, json);
        }
        for (session_id, label) in refs {
            push_unique_pair(&mut row.references, session_id, label);
        }
        true
    }

    /// `request/context`（主干 `NoteRequestContext` `:483-499`）：**last-wins、空值不覆盖**，
    /// 锚 = 最近一条用户气泡（`LastUserBubble()`），没有就取 transcript 末行；
    /// 两者都没有 ⇒ 整帧丢弃（`:493` 的 `if (anchor is null) return;`）。
    /// 故锚必须由拿着气泡表的 main.rs 算好喂进来，本表不自存锚。
    pub fn note_request_context(&mut self, event: &Value, anchor: Option<&DetailsKey>) -> DetailsDelta {
        let data = &event["data"];
        if !data.is_object() {
            return DetailsDelta::default();
        }
        let Some(key) = anchor else {
            return DetailsDelta::default();
        };
        let provider = string_field(data, "provider");
        let model = string_field(data, "model");
        let window = number_of(data, "contextWindow");
        let update = string_field(data, "systemPromptUpdate");
        let row = self.entry(key);
        if !provider.is_empty() {
            row.provider = Some(provider);
        }
        if !model.is_empty() {
            row.model = Some(model);
        }
        if window > 0 {
            row.context_window = Some(window);
        }
        if !update.is_empty() {
            row.system_prompt_update = Some(true);
        }
        DetailsDelta {
            wrote: Some(key.clone()),
            pending_changed: false,
        }
    }

    /// 气泡自身那四项的投递口（见 [`RowMeta`]）。语义与 [`DetailsLedger::note_request_context`]
    /// 一致：只覆盖「非空 / 正数」的新值，重复喂同一份 ⇒ 幂等。
    pub fn note_row_meta(&mut self, key: &DetailsKey, meta: &RowMeta) -> DetailsDelta {
        let row = self.entry(key);
        if let Some(value) = &meta.model {
            if !value.is_empty() {
                row.model = Some(value.clone());
            }
        }
        if let Some(value) = &meta.provider {
            if !value.is_empty() {
                row.provider = Some(value.clone());
            }
        }
        if let Some(value) = meta.duration_ms {
            if value > 0 {
                row.duration_ms = Some(value);
            }
        }
        if let Some(value) = meta.tokens {
            if value > 0 {
                row.tokens = Some(value);
            }
        }
        if let Some(value) = &meta.message_id {
            if !value.is_empty() {
                row.message_id = Some(value.clone());
            }
        }
        if let Some(value) = &meta.tool_name {
            if !value.is_empty() {
                row.tool_name = Some(value.clone());
            }
        }
        DetailsDelta {
            wrote: Some(key.clone()),
            pending_changed: false,
        }
    }
}

/// 主干 `Str(record, key)`：读不到 / 非字符串 ⇒ 空串（即本仓既有的 [`string_field`]）。
fn number_of(record: &Value, key: &str) -> i64 {
    // 主干 `TryGetProperty(...) && ValueKind == Number ? GetInt64()`：非数 / 读不到 ⇒ 0
    // （`contextWindow` 那案再按 `> 0` 过一遍，`MessageDetails.cs:497`）。
    record[key].as_i64().unwrap_or(0)
}

/// 主干 `ExtractContentText`（`:874-886`）：text/reasoning 取 `text`，其余块取**原始 JSON**，
/// 用 `\n` 串起来。偏差备案：主干 `GetRawText()` 原样保留键序与空白，serde 的
/// `to_string()` 会把对象键按字典序重排并压掉空白。
fn extract_content_text(content: &Value) -> String {
    let Some(items) = content.as_array() else {
        return String::new();
    };
    items
        .iter()
        .map(|block| {
            let block_type = string_field(block, "type");
            if block_type == "text" || block_type == "reasoning" {
                string_field(block, "text")
            } else {
                block.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 主干 `ExtractMessageText(data, messageKey)`（`:863-872`）：`messageKey` 非空 ⇒ 先下钻
/// 到那一层（不是对象就直接空串），空 ⇒ 就地取 `content`。
fn extract_message_text(data: &Value, message_key: Option<&str>) -> String {
    let holder = match message_key {
        Some(key) => {
            let inner = &data[key];
            if !inner.is_object() {
                return String::new();
            }
            inner
        }
        None => data,
    };
    extract_content_text(&holder["content"])
}

/// 主干 `:328-330` 那句 `string.Join("\n", content.Where(type == "text").Select(text))`。
fn join_text_blocks(content: &[Value]) -> String {
    content
        .iter()
        .filter(|block| string_field(block, "type") == "text")
        .map(|block| string_field(block, "text"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 主干是 `List.Add`（`:289` / `:352-353`），分叉这边 journal 帧**可重放**（补拉、轮询兜底、
/// 切回会话都会带回同一批），故待认领桶与 attach 一律「同一对值只登记一次」⇒ 重放幂等。
fn push_unique_pair(list: &mut Vec<(String, String)>, first: String, second: String) {
    if !list.iter().any(|(a, b)| *a == first && *b == second) {
        list.push((first, second));
    }
}

/// 主干 `CollectInstructionChanges`（`:430-450`）：`source.changes[]` 的 `path` 为标签，
/// `action` 翻成 zh 档标签；`source.baseline == true` 且非 remove ⇒ 一律「已载入」。
/// 返回 `(path, 原始 action 词, 已翻好的标签)`，标签那份照抄主干 zh 原文（`DetText` 的 zh 档）；
/// 原始词那份进 [`MessageDetails::instruction_changes`] 给 UI 侧本地化用。
fn collect_instruction_changes(source: &Value) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let Value::Array(items) = &source["changes"] else {
        return out;
    };
    let baseline = source["baseline"].as_bool() == Some(true);
    for change in items {
        let path = string_field(change, "path");
        if path.is_empty() {
            continue;
        }
        let action = string_field(change, "action");
        let loaded = baseline && action != "remove";
        let label = if loaded {
            det_zh("action_loaded")
        } else {
            match action.as_str() {
                "remove" => det_zh("action_remove"),
                "set" => det_zh("action_set"),
                _ => det_zh("action_other"),
            }
        };
        let kind = if loaded { "loaded" } else { action.as_str() };
        out.push((path, kind.to_string(), label.to_string()));
    }
    out
}

/// 主干 `CollectCatalogEntries`（`:452-468`）：`source.entries[]` 的 `(name, description)`，
/// **只登记前 8 条**，第 9 条起补一行「…还有 N 条」。照抄主干 :466 原样：那一行的 title 是
/// **没套参数的模板**「…还有 {0} 条」（主干渲染出来就是这个），text 才是格式化好的。
fn collect_catalog_entries(source: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Value::Array(items) = &source["entries"] else {
        return out;
    };
    let mut shown = 0;
    for entry in items {
        let name = string_field(entry, "name");
        if name.is_empty() {
            continue;
        }
        shown += 1;
        if shown <= 8 {
            out.push((name, string_field(entry, "description")));
        }
    }
    if shown > 8 {
        out.push((
            det_zh("catalog_more").to_string(),
            det_zh("catalog_more").replace("{0}", &(shown - 8).to_string()),
        ));
    }
    out
}

/// 主干 `DetText` / `DetFormat` 那一族（定义 `MainWindow.MessageDetails.cs:26-37`，消息详情
/// 面板的调用点 `:257` / `:290-292` / `:440-442` / `:446` / `:466` / `:473`）的 **zh/en 模板对**
/// 货架（#148 刀1 · DetText 第五族）。形制抄本文件在架先例 [`trajectory_step_label`]：
/// **只出模板、不出渲染结果**——i18n 归 `src/i18n.rs`，那 10 枚已登记进它的 `INLINE_ONLY`
/// 名单（判据：英文权威在调用点那一行，不该进 EN 表）。
///
/// · `key` 一律是**小写下划线 ASCII 判别串**，不许用中文当键（那是把中文当键二次散布）；
/// · 未知 `key` 回落 `None`：主干 `_` 那一支兜底的是**具体文案**（`已更新`），不是「任意键都出词」；
/// · `系统提示词` / `系统提示词更新` 两枚**不在货架上**——主干把它们登记进了 `ShellEnglish`
///   ⇒ 分叉走 `Catalog::l` 字典档，再上一档就是造出主干没有的双重真相；
/// · `{0}` / `{1}` 是**位序槽**（与上游 `{count}` / `{retained}` 那类具名槽无关），按序喂 `btf`；
/// · 分隔符 ` · ` 属拼装层（主干 `:292` 是现拼的），不进文案 ⇒ `recall_truncated` 只出 `已截断`。
#[must_use]
pub fn det_text_pair(key: &str) -> Option<(&'static str, &'static str)> {
    match key {
        // 主干 :257（zh 权威只认主干这一行：上游 `message.context.relay.from` 是另一串文案）
        "relay" => Some(("跨会话中继", "Session relay")),
        // 主干 :290（title 位：实参 = `label` 非空则 label、否则 sessionId）
        "recall_title" => Some(("跨会话召回 · {0}", "Session recall · {0}")),
        // 主干 :291（text 位的两枚数位）
        "recall_counts" => Some(("保留 {0} 条 · 省略 {1} 条", "{0} kept · {1} omitted")),
        // 主干 :292（截断后缀，前面那枚 ` · ` 由拼装层给）
        "recall_truncated" => Some(("已截断", "truncated")),
        // 主干 :446（`loaded` 那一支）
        "action_loaded" => Some(("已载入", "loaded")),
        // 主干 :440 / :441 / :442（:442 是 `_` 兜底那一支，最常走）
        "action_remove" => Some(("已移除", "removed")),
        "action_set" => Some(("已新增", "added")),
        "action_other" => Some(("已更新", "updated")),
        // 主干 :466 一行两枚同串 ⇒ title 与 text 共用这一臂（#11 与 #12 不分臂）
        "catalog_more" => Some(("…还有 {0} 条", "… {0} more")),
        // 主干 :473（title 位是空串，别「纠正」）
        "snapshot_supersedes" => Some(("取代先前的快照", "Supersedes earlier snapshots")),
        _ => None,
    }
}

/// 采集层取 **zh 档模板** 的唯一入口（#148 刀2 · DetText 第五族）：本带那 11 处烤字（12 行）
/// 全部收敛到 [`det_text_pair`] 一处，本函数是它在本带的投影。
/// 缺臂按编程期错误处理 ⇒ 摊在调用栈上，**不许**在这里退回第二份中文真相（那是本族要拆的东西）。
#[must_use]
fn det_zh(key: &str) -> &'static str {
    det_text_pair(key).expect("DetText 第五族货架缺臂").0
}

/// 主干 `CollectCatalogEntries`（`:452-468`）里 `shown - 8` 那一发计数的**落表面**（#148 刀1）。
/// 今天 [`collect_catalog_entries`] 只把它拼进文案（「…还有 N 条」的 text 位），表上没有数位 ⇒
/// 撤烤字那天 UI 要能自己算出 N。**判据与上面那圈逐字同**：只数 `name` 非空的条目，
/// 前 8 枚之后才算溢出；`None` = 没溢出（`shown <= 8`）。
/// 这里刻意**不改** [`collect_catalog_entries`] 的返回型——类型改动会连带打红消费侧的编译面，
/// 那属刀 3（撤烤字）那一批，不属本片。
#[must_use]
fn catalog_overflow_count(source: &Value) -> Option<i64> {
    let Value::Array(items) = &source["entries"] else {
        return None;
    };
    let shown = items
        .iter()
        .filter(|entry| !string_field(entry, "name").is_empty())
        .count() as i64;
    (shown > 8).then_some(shown - 8)
}

/// 主干 `CollectSnapshotSections`（`:470-481`）：先塞一行空标题的「取代先前的快照」，
/// 再按 `source.sections[]` 塞 `(name, text)`。
fn collect_snapshot_sections(source: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Value::Array(items) = &source["sections"] else {
        return out;
    };
    out.push((String::new(), det_zh("snapshot_supersedes").to_string()));
    for section in items {
        let name = string_field(section, "name");
        if name.is_empty() {
            continue;
        }
        out.push((name, string_field(section, "text")));
    }
    out
}

/// 主干 `CollectMentionReferences`（`:358-390`）：扫正文里的 `dsh-session:`，label 向前找
/// `@[label](`，找不到就用整串 URI 当 label。
/// 偏差备案：主干 `char.IsLetterOrDigit` 认 Unicode 字母，本仓按 ASCII 字母数字 + `-_/+=` 收
/// （URI 本体是 URL-safe base64，落不到差异上）。
fn collect_mention_refs(text: &str) -> Vec<(String, String)> {
    const SCHEME: &str = "dsh-session:";
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut from = 0usize;
    while let Some(offset) = text[from..].find(SCHEME) {
        let uri_start = from + offset;
        let mut uri_end = uri_start + SCHEME.len();
        while uri_end < bytes.len()
            && (bytes[uri_end].is_ascii_alphanumeric()
                || matches!(bytes[uri_end], b'-' | b'_' | b'+' | b'/' | b'='))
        {
            uri_end += 1;
        }
        let uri = &text[uri_start..uri_end];
        let mut label = String::new();
        if uri_start >= 2 {
            let head = &text[..uri_start];
            if let Some(close) = head.rfind(']') {
                if let Some(open) = head[..close].rfind('@') {
                    if head.as_bytes().get(close + 1) == Some(&b'(') {
                        label = head[open + 2..close].to_string();
                    }
                }
            }
        }
        if label.is_empty() {
            label = uri.to_string();
        }
        out.push((decode_session_mention(uri), label));
        from = uri_end;
    }
    out
}

/// 主干 `DecodeSessionMention`（`:392-428`）：URL-safe base64（`-`/`_` 换回 `+`/`/`、按
/// `len % 4` 补位）解出来若是 `{"sessionId":…}` 就取那一枚，否则取解出的串；
/// 解码/解析任何一步失败 ⇒ 退回原始 payload。
fn decode_session_mention(uri: &str) -> String {
    const SCHEME: &str = "dsh-session:";
    let Some(payload) = uri.strip_prefix(SCHEME) else {
        return String::new();
    };
    let Some(bytes) = b64_urlsafe_decode(payload) else {
        return payload.to_string();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return payload.to_string();
    };
    if text.starts_with('{') {
        match serde_json::from_str::<Value>(&text) {
            Ok(parsed) => {
                if parsed.is_object() {
                    if let Some(id) = parsed["sessionId"].as_str() {
                        if !id.is_empty() {
                            return id.to_string();
                        }
                    }
                }
            }
            // 主干 :403 的 JsonDocument.Parse 抛 ⇒ 整段 catch ⇒ 退回 payload（不是退回 text）。
            Err(_) => return payload.to_string(),
        }
    }
    if !text.is_empty() && !text.contains('\0') {
        text
    } else {
        payload.to_string()
    }
}

/// `Convert.FromBase64String` 的对应物：非法字符 / 长度不合法 ⇒ `None`（主干那边是抛 ⇒ catch）。
fn b64_urlsafe_decode(payload: &str) -> Option<Vec<u8>> {
    let mapped: String = payload
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    let padded = match mapped.len() % 4 {
        0 => mapped,
        2 => format!("{mapped}=="),
        3 => format!("{mapped}="),
        _ => mapped, // 主干 `default: return t;` 把 len%4 == 1 也原样送进去（那边必抛）
    };
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for c in padded.chars() {
        if c == '=' {
            break;
        }
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    if bits >= 6 || padded.len() % 4 != 0 {
        return None; // 余下 4 bit 以上的垃圾 = 主干那句 FormatException
    }
    Some(out)
}

#[cfg(test)]
mod message_details_tests {
    use super::*;

    /// 命中 `system/message`：第一帧「系统提示词」、同会话第二帧「系统提示词更新」，
    /// 且重复喂同一帧不会把 context_entries 追加成两条。
    #[test]
    fn system_message_writes_prompt_text_and_update_flag() {
        let mut ledger = DetailsLedger::new();
        let frame = json!({
            "type": "system/message",
            "seq": 3,
            "time": 1_700_000_000_000i64,
            "data": { "turn": 0, "message": { "content": [ { "type": "text", "text": "你是助手" } ] } }
        });
        let first = DetailsKey::bubble("tool", 0, 3);
        let delta = ledger.note_event(&frame, Some(&first));
        assert_eq!(delta.wrote.as_ref(), Some(&first));
        let row = ledger.get(&first).expect("第一帧必须落表");
        assert_eq!(row.system_prompt_update, Some(false));
        assert_eq!(row.system_prompt_text.as_deref(), Some("你是助手"));
        assert_eq!(row.context_entries, vec![("系统提示词".to_string(), "你是助手".to_string())]);

        // 重复帧幂等：覆盖而非追加。
        ledger.note_event(&frame, Some(&first));
        let row = ledger.get(&first).unwrap();
        assert_eq!(row.context_entries.len(), 1, "同一帧重放不许再追加一条");

        // 同会话第二帧（换一行）⇒ 更新档。
        let second = DetailsKey::bubble("tool", 0, 9);
        ledger.note_event(&frame, Some(&second));
        let row = ledger.get(&second).unwrap();
        assert_eq!(row.system_prompt_update, Some(true));
        assert_eq!(row.context_entries[0].0, "系统提示词更新");
        assert!(ledger.system_prompt_seen());
    }

    /// 中继 + 召回两类注入各自写对自己那几项（主干 :255-258 / :274-293）。
    #[test]
    fn relay_and_recall_frames_fill_their_own_fields() {
        let mut ledger = DetailsLedger::new();
        let relay = json!({
            "type": "user/message",
            "seq": 11,
            "data": {
                "turn": 2,
                "content": [ { "type": "text", "text": "中继过来的正文" } ],
                "source": { "kind": "agent-message", "form": "relay", "senderSessionId": "sess-a" }
            }
        });
        let key = DetailsKey::bubble("tool", 2, 11);
        assert_eq!(
            ledger.note_event(&relay, Some(&key)).wrote.as_ref(),
            Some(&key)
        );
        let row = ledger.get(&key).unwrap();
        assert_eq!(row.relay_session_id.as_deref(), Some("sess-a"));
        assert_eq!(
            row.context_entries,
            vec![("跨会话中继".to_string(), "中继过来的正文".to_string())]
        );
        // 重放不翻倍
        ledger.note_event(&relay, Some(&key));
        assert_eq!(ledger.get(&key).unwrap().context_entries.len(), 1);

        let recall = json!({
            "type": "user/message",
            "seq": 12,
            "data": {
                "turn": 2,
                "content": [],
                "source": {
                    "kind": "session-reference",
                    "form": "recall",
                    "references": [
                        { "sessionId": "sess-b", "label": "上周那次", "retainedMessages": 4, "omittedMessages": 9, "truncated": true },
                        { "sessionId": "", "label": "", "retainedMessages": 1 }
                    ]
                }
            }
        });
        let recall_key = DetailsKey::frame(12, 2); // row 不给 ⇒ 帧坐标自取
        let delta = ledger.note_event(&recall, None);
        assert_eq!(delta.wrote, Some(recall_key.clone()));
        let row = ledger.get(&recall_key).unwrap();
        assert_eq!(
            row.references,
            vec![("sess-b".to_string(), "上周那次".to_string())],
            "两个字段都空的引用不登记（主干 :285）"
        );
        assert_eq!(
            row.context_entries,
            vec![(
                "跨会话召回 · 上周那次".to_string(),
                "保留 4 条 · 省略 9 条 · 已截断".to_string()
            )]
        );
        assert_eq!(
            row.recalls,
            vec![SessionRecall {
                session_id: "sess-b".to_string(),
                label: "上周那次".to_string(),
                retained: 4,
                omitted: 9,
                truncated: true,
            }]
        );
        ledger.note_event(&recall, Some(&recall_key));
        assert_eq!(ledger.get(&recall_key).unwrap().context_entries.len(), 1);
    }

    /// 其余注入：标签优先级、changes / entries（前 8 条 + 「…还有 N 条」）/ sections 三段。
    #[test]
    fn context_injection_collects_changes_catalog_and_snapshot() {
        let mut ledger = DetailsLedger::new();
        let entries: Vec<Value> = (0..10)
            .map(|at| json!({ "name": format!("技能{at}"), "description": format!("说明{at}") }))
            .collect();
        let frame = json!({
            "type": "user/message",
            "seq": 20,
            "data": {
                "turn": 3,
                "content": [ { "type": "text", "text": "注入的正文" } ],
                "source": {
                    "kind": "runtime-context",
                    "plugin": "",
                    "path": "AGENTS.md",
                    "changes": [
                        { "path": "AGENTS.md", "action": "set" },
                        { "path": "notes.md", "action": "remove" },
                        { "path": "", "action": "set" }
                    ],
                    "entries": entries,
                    "sections": [ { "name": "工作区", "text": "快照正文" }, { "name": "", "text": "被丢掉" } ]
                }
            }
        });
        let key = DetailsKey::bubble("tool", 3, 20);
        ledger.note_event(&frame, Some(&key));
        let row = ledger.get(&key).unwrap();
        let titles: Vec<&str> = row.context_entries.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(titles[0], "AGENTS.md", "标签要按 plugin || path || label || kind 取");
        assert_eq!(row.context_entries[0].1, "注入的正文");
        // changes 两条：set ⇒「已新增」、remove ⇒「已移除」；path 空的整条丢掉（主干 :437）
        assert_eq!(
            row.context_entries[1],
            ("AGENTS.md".to_string(), "已新增".to_string()),
            "同标题的两条不许互相吞掉（主干是 List.Add）"
        );
        assert_eq!(row.context_entries[2], ("notes.md".to_string(), "已移除".to_string()));
        assert!(
            !titles.iter().any(|t| *t == "技能9"),
            "catalog 只登记前 8 条"
        );
        assert!(titles.contains(&"…还有 {0} 条"), "第 9 条起补一行「…还有 N 条」");
        assert_eq!(
            row.context_entries
                .iter()
                .find(|(t, _)| t == "…还有 {0} 条")
                .map(|(_, x)| x.as_str()),
            Some("…还有 2 条")
        );
        assert_eq!(
            row.instruction_changes,
            vec![
                ("AGENTS.md".to_string(), "set".to_string()),
                ("notes.md".to_string(), "remove".to_string()),
            ]
        );
        assert_eq!(
            row.context_entries
                .iter()
                .find(|(t, _)| t.is_empty())
                .map(|(_, x)| x.as_str()),
            Some("取代先前的快照")
        );
        assert_eq!(
            row.context_entries
                .iter()
                .find(|(t, _)| t == "工作区")
                .map(|(_, x)| x.as_str()),
            Some("快照正文")
        );
        // 重放同帧：整行按帧重建 ⇒ 一条不多一条不少
        let before = ledger.get(&key).unwrap().clone();
        ledger.note_event(&frame, Some(&key));
        assert_eq!(ledger.get(&key).unwrap(), &before, "重复帧必须覆盖而非追加");
    }

    /// 普通用户消息：附加块与 @ 引用先进待认领桶，画了行才结算；空桶不脏表。
    #[test]
    fn plain_user_message_defers_extras_and_mentions() {
        let mut ledger = DetailsLedger::new();
        let payload = {
            let raw = "{\"sessionId\":\"sess-x\"}".to_string();
            let mut out = String::new();
            // 手写一遍 URL-safe base64（不引 base64 crate：Cargo.toml 只有 serde_json + windows-reactor）
            const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let bytes = raw.as_bytes();
            for chunk in bytes.chunks(3) {
                let b0 = chunk[0] as u32;
                let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
                let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
                let n = (b0 << 16) | (b1 << 8) | b2;
                out.push(TABLE[((n >> 18) & 63) as usize] as char);
                out.push(TABLE[((n >> 12) & 63) as usize] as char);
                out.push(TABLE[((n >> 6) & 63) as usize] as char);
                out.push(TABLE[(n & 63) as usize] as char);
            }
            // 尾块不满三字节时上面多写的那几位是补零，得换成 '=' 补位（主干 PadBase64 的反向）
            let rem = bytes.len() % 3;
            let keep = if rem == 0 { out.len() } else { out.len() - (3 - rem) };
            out.truncate(keep);
            out.replace('+', "-").replace('/', "_").to_string()
        };
        // 独立 oracle（不依赖上面那台编码机）：base64("sess-y") = "c2Vzcy15"，
        // 主干 `DecodeSessionMention`（:411）对非 JSON 的解出串直接返回它本身。
        assert_eq!(decode_session_mention("dsh-session:c2Vzcy15"), "sess-y");
        // 认不出 scheme / 解不动 ⇒ 退回 payload（主干 :395 的空串 / :413 的 catch）。
        assert_eq!(decode_session_mention("dsh-fs:c2Vzcy15"), "");
        assert_eq!(decode_session_mention("dsh-session:!!"), "!!");
        let frame = json!({
            "type": "user/message",
            "seq": 31,
            "data": {
                "turn": 4,
                "source": { "kind": "user" },
                "content": [
                    { "type": "text", "text": format!("看下 @[上周那次](dsh-session:{payload}) 和裸 dsh-session:abc") },
                    { "type": "file", "path": "a.txt" },
                    { "type": "image", "attachment": { "attachmentId": "att-1" } },
                    { "no_type": 1 }
                ]
            }
        });

        // 还没画行 ⇒ 一只气泡都不该出现
        let delta = ledger.note_event(&frame, None);
        assert_eq!(delta.wrote, None);
        assert!(delta.pending_changed);
        assert!(ledger.is_empty(), "没画行就不许脏表");
        assert_eq!(ledger.pending_extra_blocks().len(), 2, "file + 那块没 type 的（label 落 \"block\"）");
        assert_eq!(
            ledger.pending_references().to_vec(),
            vec![
                ("sess-x".to_string(), "上周那次".to_string()),
                ("abc".to_string(), "上周那次".to_string()),
            ],
            "sessionId 走 base64 里的 JSON；第二条裸 URI 的 label 被**上一条**的 @[label]( 认领 \
             —— 主干 :378-384 的 LastIndexOf 就是回溯整串的，这个怪癖照抄不修"
        );

        // 画了行 ⇒ 桶结算进表；重放同帧不再翻倍
        let key = DetailsKey::bubble("user", 4, 31);
        let attached = ledger.note_event(&frame, Some(&key));
        assert_eq!(attached.wrote.as_ref(), Some(&key));
        assert!(ledger.pending_extra_blocks().is_empty());
        let row = ledger.get(&key).unwrap();
        assert_eq!(row.extra_blocks.len(), 2);
        assert_eq!(row.extra_blocks[0].0, "file");
        assert_eq!(row.extra_blocks[1].0, "block");
        assert_eq!(row.references.len(), 2);
        assert_eq!(row.model, None, "普通用户消息不碰元数据那几项");

        ledger.note_event(&frame, Some(&key));
        assert_eq!(ledger.get(&key).unwrap().extra_blocks.len(), 2);
        assert_eq!(ledger.get(&key).unwrap().references.len(), 2);
        assert!(ledger.pending_extra_blocks().is_empty());
    }

    /// `request/context`：last-wins、空值不覆盖、window 只收 > 0、无锚整帧丢弃。
    #[test]
    fn request_context_is_last_wins_and_needs_an_anchor() {
        let mut ledger = DetailsLedger::new();
        let frame = json!({
            "type": "request/context",
            "seq": 41,
            "data": { "provider": "anthropic", "model": "claude-x", "contextWindow": 200000, "systemPromptUpdate": "in-history" }
        });
        // 主干 :493：锚为 null ⇒ 整帧丢，什么都不写
        assert_eq!(ledger.note_event(&frame, None), DetailsDelta::default());
        assert!(ledger.is_empty());

        let key = DetailsKey::bubble("user", 5, 40);
        ledger.note_event(&frame, Some(&key));
        let row = ledger.get(&key).unwrap();
        assert_eq!(row.provider.as_deref(), Some("anthropic"));
        assert_eq!(row.model.as_deref(), Some("claude-x"));
        assert_eq!(row.context_window, Some(200000));
        assert_eq!(row.system_prompt_update, Some(true));

        // 空 provider/model 不许把已采到的抹掉（主干 :495-496 的 `Length > 0` 判据），
        // contextWindow: 0 同理（:497 的 `> 0`）。
        ledger.note_event(
            &json!({ "type": "request/context", "seq": 42, "data": { "provider": "", "model": "claude-y", "contextWindow": 0 } }),
            Some(&key),
        );
        let row = ledger.get(&key).unwrap();
        assert_eq!(row.provider.as_deref(), Some("anthropic"));
        assert_eq!(row.model.as_deref(), Some("claude-y"), "非空新值 last-wins");
        assert_eq!(row.context_window, Some(200000));

        // 气泡侧四项走 note_row_meta：正数才收，重复喂幂等。
        ledger.note_row_meta(
            &key,
            &RowMeta {
                duration_ms: Some(1200),
                tokens: Some(3456),
                message_id: Some("m-1".to_string()),
                tool_name: Some("read_file".to_string()),
                ..Default::default()
            },
        );
        ledger.note_row_meta(
            &key,
            &RowMeta {
                duration_ms: Some(0),
                tokens: Some(-1),
                message_id: Some(String::new()),
                ..Default::default()
            },
        );
        let row = ledger.get(&key).unwrap();
        assert_eq!((row.duration_ms, row.tokens), (Some(1200), Some(3456)));
        assert_eq!(row.message_id.as_deref(), Some("m-1"));
        assert_eq!(row.tool_name.as_deref(), Some("read_file"));
    }

    /// 与本表无关的帧一律不脏表：名字认不出、data 缺、content 不是数组、注入帧没有 source。
    #[test]
    fn unrelated_and_malformed_frames_leave_the_ledger_clean() {
        let mut ledger = DetailsLedger::new();
        let frames = [
            json!({"type": "turn/end", "seq": 1, "data": {"reason": {"kind": "max-tokens"}}}),
            json!({"type": "assistant/attempt", "seq": 2, "data": {"stream": []}}),
            json!({"type": "tool/call", "seq": 3, "data": {"toolName": "read"}}),
            json!({"type": "llm/retry", "seq": 4, "data": {"retryId": "r-1"}}),
            json!({"seq": 5, "data": {}}),
            json!({"type": 7}),
            Value::Null,
            json!({"type": "user/message", "seq": 6, "data": {"content": "不是数组"}}),
            json!({"type": "user/message", "seq": 7}),
            json!({"type": "request/context", "seq": 8}),
            json!({"type": "system/message", "seq": 9}),
            json!({"type": "user/message", "seq": 10, "data": {"content": [], "source": {"kind": "user"}}}),
        ];
        for frame in &frames {
            let delta = ledger.note_event(frame, Some(&DetailsKey::bubble("user", 1, 1)));
            assert!(
                !delta.any(),
                "{frame} 与本表无关（主干对本表只有三案），却回了 {delta:?}"
            );
        }
        assert!(
            ledger.is_empty(),
            "认不出的帧一律不建表行：{:?}",
            ledger.keys()
        );
        assert!(ledger.pending_extra_blocks().is_empty());
        assert!(ledger.pending_references().is_empty());
        assert!(!ledger.system_prompt_seen());
    }

    /// 清表：切会话/清空后旧行读不到、待认领桶作废、`system_prompt_seen` 归零。
    #[test]
    fn reset_drops_every_row_and_pending_bucket() {
        let mut ledger = DetailsLedger::new();
        let key = DetailsKey::bubble("user", 1, 1);
        ledger.note_event(
            &json!({"type": "system/message", "seq": 1, "data": {"message": {"content": [{"type": "text", "text": "旧提示词"}]}}}),
            Some(&key),
        );
        ledger.note_event(
            &json!({"type": "user/message", "seq": 2, "data": {"content": [{"type": "file", "path": "x"}]}}),
            None,
        );
        assert_eq!(ledger.len(), 1);
        assert!(!ledger.pending_extra_blocks().is_empty());

        ledger.reset();
        assert!(ledger.get(&key).is_none(), "清表后旧值必须读不到");
        assert!(ledger.get(&DetailsKey::message_id("m-1")).is_none());
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
        assert!(ledger.keys().is_empty());
        assert!(ledger.pending_extra_blocks().is_empty());
        assert!(ledger.pending_references().is_empty());
        assert!(!ledger.system_prompt_seen());

        // 清表后重新采：又是「第一条系统提示词」
        ledger.note_event(
            &json!({"type": "system/message", "seq": 3, "data": {"message": {"content": [{"type": "text", "text": "新提示词"}]}}}),
            Some(&key),
        );
        assert_eq!(ledger.get(&key).unwrap().system_prompt_update, Some(false));

        // 主干那颗标记活在 RunStats 里，切会话后由 main.rs 补回来（:200-202 vs :117-130）
        ledger.set_system_prompt_seen(true);
        let other = DetailsKey::bubble("tool", 0, 4);
        ledger.note_event(
            &json!({"type": "system/message", "seq": 4, "data": {"message": {"content": [{"type": "text", "text": "回放里的更新"}]}}}),
            Some(&other),
        );
        assert_eq!(ledger.get(&other).unwrap().system_prompt_update, Some(true));
    }

    /// 键的四种寻址口径彼此不撞，且 `MessageDetails::is_empty` 只在全空时才真。
    #[test]
    fn keys_are_distinct_across_addressing_modes() {
        assert_eq!(DetailsKey::bubble("user", 4, 31).as_str(), "user-4-31");
        assert_ne!(DetailsKey::bubble("user", 4, 31), DetailsKey::message_id("4-31"));
        assert_ne!(DetailsKey::frame(1, 2), DetailsKey::seq_index(1, 2));
        assert_ne!(DetailsKey::message_id("x"), DetailsKey::seq_index(0, 0));
        let mut row = MessageDetails::default();
        assert!(row.is_empty(), "默认壳 = 什么都没采到");
        row.references.push(("s".to_string(), "l".to_string()));
        assert!(!row.is_empty());
        row.references.clear();
        assert!(row.is_empty());
        let ledger = DetailsLedger::new();
        assert!(ledger.get(&DetailsKey::message_id("x")).is_none());
        assert!(ledger.get(&DetailsKey::seq_index(3, 1)).is_none());
    }

    /// 刀1（#148 · DetText 第五族）的货架自证：**十枚臂一枚不许少**，且每枚的 zh 必须与本文件
    /// 今天仍在拼的那 12 行烤字**逐字相同** —— 这是刀3 敢撤烤字的唯一凭据。
    /// 判据来源：主干 `MainWindow.MessageDetails.cs:257` / `:290-292` / `:440-442` / `:446` /
    /// `:466` / `:473`（`DetText` 第五族那一族的调用点，本轮逐字复到）。
    /// 全枚等号形制，一枚 `>=` 下界都没有。
    #[test]
    fn det_shelf_pairs_match_the_copy_this_kernel_still_bakes_in_place() {
        const SHELF: &[(&str, &str, &str)] = &[
            ("relay", "跨会话中继", "Session relay"),
            ("recall_title", "跨会话召回 · {0}", "Session recall · {0}"),
            ("recall_counts", "保留 {0} 条 · 省略 {1} 条", "{0} kept · {1} omitted"),
            ("recall_truncated", "已截断", "truncated"),
            ("action_loaded", "已载入", "loaded"),
            ("action_remove", "已移除", "removed"),
            ("action_set", "已新增", "added"),
            ("action_other", "已更新", "updated"),
            ("catalog_more", "…还有 {0} 条", "… {0} more"),
            ("snapshot_supersedes", "取代先前的快照", "Supersedes earlier snapshots"),
        ];
        assert_eq!(
            SHELF.len(),
            10,
            "刀1 定案 13 枚 = 10 臂（#11/#12 同臂、#1/#2 不上架、#5 只出文案不出分隔符）"
        );
        for (key, zh, en) in SHELF {
            assert_eq!(det_text_pair(key), Some((*zh, *en)), "货架臂漂了：{key}");
        }
        // 兜底必须是 None：撤字那天消费侧只有拿到 None 才走「原样回落」，不会被货架的 `_` 吃掉。
        for junk in [
            "",
            "RECALL_TITLE",
            "recall",
            "已截断",
            "system_prompt",
            "system_prompt_update",
        ] {
            assert_eq!(det_text_pair(junk), None, "未知 key 不许兜底成某一臂：{junk}");
        }
        // #1/#2 走字典档（主干登记进 `ShellEnglish`）⇒ 同串不得再上货架；中文当键同样是坏形制。
        assert_eq!(det_text_pair("系统提示词"), None, "中文当 key = 把中文当键二次散布");

        // —— 货架 zh ⇄ 分叉今天拼出来的烤字，逐字对齐（三条注入档各钉一枚）——
        let mut ledger = DetailsLedger::new();
        let relay = json!({
            "type": "user/message",
            "seq": 41,
            "data": {
                "turn": 5,
                "content": [ { "type": "text", "text": "中继正文" } ],
                "source": { "kind": "agent-message", "form": "relay", "senderSessionId": "sess-r" }
            }
        });
        let relay_key = DetailsKey::bubble("tool", 5, 41);
        ledger.note_event(&relay, Some(&relay_key));
        let row = ledger.get(&relay_key).unwrap();
        assert_eq!(
            row.context_entries[0].0,
            det_text_pair("relay").expect("货架缺 relay 臂").0,
            "拼装用的 zh 与货架 zh 不是同一串 ⇒ 刀3 撤字会改渲染"
        );

        let recall = json!({
            "type": "user/message",
            "seq": 42,
            "data": {
                "turn": 5,
                "content": [],
                "source": {
                    "kind": "session-reference",
                    "form": "recall",
                    "references": [
                        { "sessionId": "sess-b", "label": "上周那次", "retainedMessages": 4, "omittedMessages": 9, "truncated": true }
                    ]
                }
            }
        });
        let recall_key = DetailsKey::bubble("tool", 5, 42);
        ledger.note_event(&recall, Some(&recall_key));
        let row = ledger.get(&recall_key).unwrap();
        let (zh_title, en_title) = det_text_pair("recall_title").expect("货架缺 recall_title 臂");
        let (zh_counts, en_counts) = det_text_pair("recall_counts").expect("货架缺 recall_counts 臂");
        let (zh_trunc, en_trunc) =
            det_text_pair("recall_truncated").expect("货架缺 recall_truncated 臂");
        assert_eq!(
            row.context_entries[0].0,
            zh_title.replace("{0}", "上周那次"),
            "title 位 = 货架模板套上 label"
        );
        assert_eq!(
            row.context_entries[0].1,
            format!(
                "{} · {zh_trunc}",
                zh_counts.replace("{0}", "4").replace("{1}", "9")
            ),
            "text 位 = 货架模板 + 现拼分隔符 + 截断文案（主干 :291-292）"
        );
        assert_eq!(
            format!(
                "{} · {en_trunc}",
                en_counts.replace("{0}", "4").replace("{1}", "9")
            ),
            "4 kept · 9 omitted · truncated",
            "en 模板 + 分隔符必须能拼出主干 :292 那一串（刀2 接线的前置凭据）"
        );
        assert_eq!(
            en_title.replace("{0}", "sess-b"),
            "Session recall · sess-b",
            "label 空 ⇒ 主干回落 sessionId；本帧有 label ⇒ 模板槽位仍按序"
        );
        assert!(
            !zh_trunc.contains('·') && !en_trunc.contains('·'),
            "分隔符不许进文案（主干 :292 的 ` · ` 是现拼的）"
        );

        let injected = json!({
            "type": "user/message",
            "seq": 43,
            "data": {
                "turn": 5,
                "content": [ { "type": "text", "text": "注入正文" } ],
                "source": {
                    "kind": "runtime-context",
                    "plugin": "",
                    "path": "AGENTS.md",
                    "changes": [ { "path": "a.md", "action": "set" }, { "path": "b.md", "action": "remove" }, { "path": "c.md", "action": "other" } ],
                    "entries": (0..10).map(|at| json!({ "name": format!("条{at}"), "description": "d" })).collect::<Vec<Value>>(),
                    "sections": [ { "name": "", "text": "快照正文" } ]
                }
            }
        });
        let injected_key = DetailsKey::bubble("tool", 5, 43);
        ledger.note_event(&injected, Some(&injected_key));
        let row = ledger.get(&injected_key).unwrap();
        // #7/#8/#9/#10：action 标签的 zh 必须与货架同源。
        let labels: Vec<&str> = row
            .context_entries
            .iter()
            .skip(1)
            .take(3)
            .map(|(_, x)| x.as_str())
            .collect();
        assert_eq!(
            labels,
            vec![
                det_text_pair("action_set").unwrap().0,
                det_text_pair("action_remove").unwrap().0,
                det_text_pair("action_other").unwrap().0,
            ],
            "changes 三档标签与货架臂漂了（主干 :440-442）"
        );
        // #11/#12：title 是**没套参的模板**、text 是套过参的，两者共用一枚臂。
        let more_zh = det_text_pair("catalog_more").unwrap().0;
        assert_eq!(
            row.context_entries
                .iter()
                .find(|(t, _)| t.as_str() == more_zh)
                .map(|(_, x)| x.as_str()),
            Some("…还有 2 条"),
            "第 9 条起那一行的 title/text 与货架 catalog_more 臂漂了（主干 :466）"
        );
        // #13：title 位是空串，别「纠正」。
        assert_eq!(
            row.context_entries
                .iter()
                .find(|(t, _)| t.is_empty())
                .map(|(_, x)| x.as_str()),
            Some(det_text_pair("snapshot_supersedes").unwrap().0),
            "取代先前的快照那一行住在 text 位（主干 :473）"
        );

        // —— 刀1 新字段：溢出数位落表（#12 的可算面）——
        assert_eq!(
            row.catalog_overflow,
            Some(2),
            "10 条 catalog ⇒ 溢出 2 条（与上面那枚「…还有 2 条」同源）"
        );
        assert_eq!(
            catalog_overflow_count(&json!({"entries": (0..8).map(|_| json!({"name": "n"})).collect::<Vec<Value>>() })),
            None,
            "shown <= 8 不算溢出（主干 :466 的 `if (shown > 8)`）"
        );
        assert_eq!(
            catalog_overflow_count(&json!({
                "entries": [
                    { "name": "a" }, { "name": "" }, { "name": "b" }, { "name": "c" },
                    { "name": "d" }, { "name": "e" }, { "name": "f" }, { "name": "g" },
                    { "name": "h" }, { "name": "i" }
                ]
            })),
            Some(1),
            "口径与 collect_catalog_entries 逐字同：`name` 空的条目不数（10 项里 9 枚有名 ⇒ 溢出 1）"
        );
        assert_eq!(
            catalog_overflow_count(&json!({ "noEntries": 1 })),
            None,
            "没有 entries 数组就不是注入档"
        );

        // 整行按帧重建 ⇒ 数位跟着覆写：同键重放小帧不得留下上一帧的 2。
        let smaller = json!({
            "type": "user/message",
            "seq": 43,
            "data": {
                "turn": 5,
                "content": [ { "type": "text", "text": "注入正文二" } ],
                "source": {
                    "kind": "runtime-context",
                    "path": "AGENTS.md",
                    "entries": [ { "name": "x", "description": "d" } ],
                    "sections": []
                }
            }
        });
        ledger.note_event(&smaller, Some(&injected_key));
        assert_eq!(
            ledger.get(&injected_key).unwrap().catalog_overflow,
            None,
            "重放不翻倍、也不残留：溢出位随帧覆写（同 row.context_entries 的形制）"
        );
    }

    /// 刀2 两把新针共用的**靶窗口**：`[带首, 货架 doc) ∪ [helper 锚, 测试 mod 之前)`。
    /// 挖掉「货架那一格」= 货架臂本体**合法持有**这批字面串，不挖就是把闸做成恒红的假判据；
    /// 尾部止于测试 mod 之前 = 本 mod 那十臂台账（`SHELF`）也不当靶。
    /// 四枚锚的唯一性本轮实测；找不到直接 panic ⇒ 绝不让针静默空转（反向针无靶 = 假绿）。
    fn message_details_production_band() -> String {
        const SELF: &str =
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));
        let band_start = SELF
            .find("// ==================== 消息详情旁路表")
            .expect("带首锚漂了 ⇒ 本针会静默空转");
        let shelf_start = SELF
            .find(concat!("/// 主干 `DetText` / `", "DetFormat` 那一族"))
            .expect("货架 doc 锚漂了");
        let helper_start = SELF
            .find(concat!("fn catalog_", "overflow_count"))
            .expect("helper 锚漂了");
        let band_end = SELF
            .find(concat!("#[cfg(", "test)]\nmod message_", "details_tests"))
            .expect("带尾锚漂了");
        assert!(
            band_start < shelf_start && shelf_start < helper_start && helper_start < band_end,
            "四枚锚的行序错了 ⇒ 窗口挖反"
        );
        format!(
            "{}{}",
            &SELF[band_start..shelf_start],
            &SELF[helper_start..band_end]
        )
    }

    /// 刀2（#148 DetText 第五族）：采集层**不许再烤死**那批 zh 文案 —— 本带生产码只许经
    /// [`det_text_pair`] 取模板。三把针：① 烤死形归零 / ② 读者枚数等号 / ③ 唯一例外仍在。
    #[test]
    fn the_message_details_band_bakes_in_no_copy_that_the_shelf_already_owns() {
        let production = message_details_production_band();
        assert!(!production.is_empty(), "窗口开空了 ⇒ 下面三把针全是假绿");

        // ① 引号裹形的 zh 文案只许活在货架里。尺取「引号裹形」而非裸串：本带 doc 用「」引述
        //    同一批文案（`已载入`/`…还有 {0} 条`/`取代先前的快照` 那几处），裸串会把注释数进去 ⇒ 假红。
        for baked in [
            concat!("\"", "跨会话中继", "\""),
            concat!("\"", "保留 {0} 条 · 省略 {1} 条", "\""),
            concat!("\"", "已截断", "\""),
            concat!("\"", "跨会话召回 · {0}", "\""),
            concat!("\"", "已载入", "\""),
            concat!("\"", "已移除", "\""),
            concat!("\"", "已新增", "\""),
            concat!("\"", "已更新", "\""),
            concat!("\"", "…还有 {0} 条", "\""),
            concat!("\"", "取代先前的快照", "\""),
        ] {
            assert!(
                !production.contains(baked),
                "采集层还自己烤着这一枚 ⇒ 货架被绕过：{baked}"
            );
        }

        // ② 每枚 key 真被生产码读到：**枚数等号 = 11**（12 行折成 11 处调用语句，因为
        //    `recall_counts` 那一处一句占三行）。helper 自身那一枚落在被挖掉的货架格里 ⇒
        //    这里数到的只可能是调用点。多 = 有人绕 helper 又烤字；少 = 漏改。
        assert_eq!(
            production.matches("det_zh(").count(),
            11,
            "采集层的货架读者必须是 11 处（= 12 行），一枚不多不少"
        );

        // ③ 唯一一枚「不许上架」的例外必须**还在原地烤着**：主干把这两枚登记进了 `ShellEnglish`
        //    ⇒ 分叉走字典档，顺手接进货架 = 造出主干没有的第二重真相。
        assert_eq!(
            production.matches(concat!("\"", "系统提示词更新", "\"")).count(),
            1,
            "系统提示词那两枚属字典档（主干 MainWindow.xaml.cs:1197-1198），不属货架"
        );
    }

    /// 反向禁针：**英文真相只许在货架那一格**。分叉生产码今天零枚 EN 文案（本轮实测），
    /// 刀2 之后也必须保持零 —— 否则就是把主干 `DetText` 的 en 档抄成了分叉的第二本表。
    /// ⚠ 靶同 ①②③ 那把窗口：整份文件里英文真相合法住在货架臂（`det_text_pair`）与本 mod 的
    /// `SHELF` 台账两处，按整文件禁 = 一落码即红 = 假判据。
    #[test]
    fn no_inline_english_copy_leaks_into_the_message_details_band() {
        let production = message_details_production_band();
        // 反向 needle 一律 `concat!` 拆词：本测试自己的文字就在本文件里，整名落纸即恒假（假绿）。
        for en_truth in [
            concat!("Session ", "relay"),
            concat!("Session ", "recall"),
            concat!("{0} kept", " · {1} omitted"),
            concat!("Supersedes earlier ", "snapshots"),
            concat!("… {0} ", "more"),
        ] {
            assert!(
                !production.contains(en_truth),
                "本带的生产码里出现了第二份英文真相：{en_truth}"
            );
        }
    }
}

// ==================== 工作区与目录选择器（#75 · 请求构造层 + 响应解析，纯函数无 IO） ====================
//
// 本片只做「构造 + 校验 + 解析」，UI 接线留给下一个 main.rs 写者（接法见
// `rust/tmp/ws1-report.md` §7）。九发的权威形状**不是**照抄主干 C# 的读法，而是照抄内核自己
// 生成的 typert 描述符（主干目录下只读，行号按 `Kernel/dsh/node_modules/@deepseek-ai/
// dsh-api-workspace-controller/lib/typert.host.js` 实测）：
//
//   描述符 parameters 的 `wire` 字段 ⇒ 网关 `assertExactArguments`
//   （`dsh-api-gateway/lib/index.js:1040-1052`）按**逐个 wire 名**卡外层 args：
//   少必填 ⇒ `missing "x"`、多未知键 ⇒ `unexpected "y"`，真码 `gateway/arguments-invalid`，
//   落进假内核既有的 `bad_args` 口径。这就是「抄错一层内核就拒」的那道关。
//
//     workspace/rename              parameters = [{wire:'request'}]      请求 = {request:{workspaceId,title}}
//     workspace/delete              parameters = [{wire:'request'}]      请求 = {request:{workspaceId}}
//     workspace/create              parameters = [{wire:'request'}]      请求 = {request:{path}}
//     workspace/insertBefore        parameters = [{wire:'request'}]      请求 = {request:{workspaceId,beforeWorkspaceId?}}
//     workspace/archiveSession      parameters = [{wire:'request'}]      请求 = {request:{sessionId}}
//     workspace/insertSessionBefore parameters = [{wire:'request'}]      请求 = {request:{workspaceId,sessionId,beforeSessionId?}}
//     directoryPicker/pick          parameters = []                      请求 = {}            ← 零颗 wire 字段，多一个键就红
//     directoryPicker/list          parameters = [{wire:'path',acceptsUndefined:true}]  请求 = {} 或 {path}
//     directoryPicker/createDirectory parameters = [{wire:'path'},{wire:'name'}]        请求 = {path,name} ← 平铺，无 request
//
//   回执（同文件的 `*_result$schema`，typert.host.js:4-118）：
//     WorkspaceView = {workspaceId,path,title,sessionIds,createdAt,updatedAt} 六颗全必填；
//     rename / insertSessionBefore → {workspace}；create → {workspace,created}；
//     delete → {deleted: 字面量 true}；insertBefore → {workspaceIds}；archiveSession → {archivedSessionIds}；
//     pick → string | null（null = 操作员按了取消）；createDirectory → string（新目录绝对路径）；
//     list → {path,home,crumbs[{name,path,hidden}],entries[{name,path,hidden}],truncated} 五颗全必填。
//
//   运行期错误码（`dsh-api-workspace-controller/lib/index.js`，主干按 code 分支的就这几条）：
//     rename 空标题（trim 后）→ `gateway/bad-request`；重名 → `workspace/name-conflict`；
//     工作区不存在 → `workspace/not-found`；create 路径不是目录 → `workspace/invalid-path`；
//     insertBefore 任一颗 id 不在序里 → `workspace/not-found`；insertSessionBefore 挪不动 →
//     `workspace/move-invalid`；archiveSession 会话不认识 → `session/not-found`；
//     能力不配对不上 → `directory-picker/unavailable`（主干 :3990 就 `Contains` 这一串）；
//     浏览读不到 → `directory-picker/unreadable`、撞名 → `directory-picker/exists`、
//     建不出来 → `directory-picker/create-failed`；createDirectory 的 name 不是单段 →
//     `gateway/bad-request`（`index.js:349-351` 那条 refine：非空且不是 `.`/`..` 且不含 `/\\`）。

/// 一发 unary RPC 的产物：方法名 + 进 `payload.args` 的那棵树。
///
/// 刻意做成「结构体而不是元组」：调用点写成 `kernel.call(call.method, call.args)` 读得通，
/// 而且 `RpcCall` 可以整颗进断言（`tests/ipc.rs` 逐字段比 JSON 树用）。
#[derive(Clone, Debug, PartialEq)]
pub struct RpcCall {
    pub method: &'static str,
    pub args: Value,
}

impl RpcCall {
    /// 九发共同的出口。`args` 一律是**外层**那一层（`{request:…}` 或平铺），不是 request 本身。
    pub fn new(method: &'static str, args: Value) -> Self {
        Self { method, args }
    }

    /// 喂给 `Kernel::call` 的形态（`pub fn call(&mut self, method: &str, args: Value)`）。
    pub fn into_parts(self) -> (&'static str, Value) {
        (self.method, self.args)
    }
}

/// 九发的方法名。集中一处，`tests/ipc.rs` 与假内核桩都按这组常量对表，改不漏。
pub const WORKSPACE_RENAME: &str = "workspace/rename";
pub const WORKSPACE_DELETE: &str = "workspace/delete";
pub const WORKSPACE_CREATE: &str = "workspace/create";
pub const WORKSPACE_INSERT_BEFORE: &str = "workspace/insertBefore";
pub const WORKSPACE_ARCHIVE_SESSION: &str = "workspace/archiveSession";
pub const WORKSPACE_INSERT_SESSION_BEFORE: &str = "workspace/insertSessionBefore";
pub const DIRECTORY_PICKER_PICK: &str = "directoryPicker/pick";
pub const DIRECTORY_PICKER_LIST: &str = "directoryPicker/list";
pub const DIRECTORY_PICKER_CREATE_DIRECTORY: &str = "directoryPicker/createDirectory";

/// 六颗必填、顺序无所谓的一层 `request` 包裹（主干那六发全是这一型）。
fn wrapped(request: Value) -> Value {
    json!({ "request": request })
}

/// 「空串视同未选」这一判沿用 [`session_create_location`] 的口径：内核描述符里 `before*` 是
/// `optional`，主干 `MoveWorkspaceAsync` 末位时给的就是 `null`（键整个不存在）而不是空串。
fn anchor(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|text| !text.is_empty()).map(str::to_string)
}

/// `workspace/rename`（主干 `MainWindow.xaml.cs:3828`）：`{request:{workspaceId,title}}`。
/// 标题**不在这里** trim —— 主干是 `box.Text.Trim()` 之后才传，trim 属于 UI 侧的取值动作；
/// 构造层原样送出，才测得出「传了空白标题会不会被内核拒」（答案：`gateway/bad-request`）。
pub fn workspace_rename(workspace_id: &str, title: &str) -> RpcCall {
    RpcCall::new(
        WORKSPACE_RENAME,
        wrapped(json!({ "workspaceId": workspace_id, "title": title })),
    )
}

/// `workspace/delete`（`:3858`）：`{request:{workspaceId}}`。回执 `{deleted:true}`。
pub fn workspace_delete(workspace_id: &str) -> RpcCall {
    RpcCall::new(WORKSPACE_DELETE, wrapped(json!({ "workspaceId": workspace_id })))
}

/// `workspace/create`（`:3951`）：`{request:{path}}`。
/// 0.7.1 的实测教训写在主干注释里：裸 `{path}` 会被拒成 arguments-invalid，
/// 「新建工作区」因此从未工作过 ⇒ 这层 `request` 包裹是本发最容易抄错的一处。
pub fn workspace_create(path: &str) -> RpcCall {
    RpcCall::new(WORKSPACE_CREATE, wrapped(json!({ "path": path })))
}

/// `workspace/insertBefore`（`:4231`，`args` 是变量 ⇒ 拼法在 `:4228-4230`）：
/// 有锚点 ⇒ `{request:{workspaceId,beforeWorkspaceId}}`，无锚点（追加到尾部）⇒ `beforeWorkspaceId`
/// 整颗键不存在。回执 `{workspaceIds}` = 改完之后的**全序**。
pub fn workspace_insert_before(workspace_id: &str, before_workspace_id: Option<&str>) -> RpcCall {
    let mut request = json!({ "workspaceId": workspace_id });
    if let Some(before) = anchor(before_workspace_id) {
        request["beforeWorkspaceId"] = json!(before);
    }
    RpcCall::new(WORKSPACE_INSERT_BEFORE, wrapped(request))
}

/// `workspace/archiveSession`（`:4248`）：`{request:{sessionId}}`。
/// 回执 `{archivedSessionIds}` = 归档集的**全量**（与 follow 流的 `archived` 帧同一条口径）。
pub fn workspace_archive_session(session_id: &str) -> RpcCall {
    RpcCall::new(WORKSPACE_ARCHIVE_SESSION, wrapped(json!({ "sessionId": session_id })))
}

/// `workspace/insertSessionBefore`（`:4266`）：`{request:{workspaceId,sessionId}}`。
/// 主干那处**从不**带 `beforeSessionId`（会话拖到指定位置 = 进目标工作区的尾部）；
/// 内核描述符这颗是 optional，所以这里按 `Option` 留着，`None` ⇒ 键不存在。
/// 回执 `{workspace}` = 目标工作区的新视图（含新的 `sessionIds` 顺序）。
pub fn workspace_insert_session_before(
    workspace_id: &str,
    session_id: &str,
    before_session_id: Option<&str>,
) -> RpcCall {
    let mut request = json!({ "workspaceId": workspace_id, "sessionId": session_id });
    if let Some(before) = anchor(before_session_id) {
        request["beforeSessionId"] = json!(before);
    }
    RpcCall::new(WORKSPACE_INSERT_SESSION_BEFORE, wrapped(request))
}

/// `directoryPicker/pick`（`:3919`）：**零颗** wire 字段（描述符 `parameters: []`）。
/// 主干传的是 `new { }`，不是 `new { request = new { } }` —— 多包一层就是 `unexpected "request"`。
pub fn directory_picker_pick() -> RpcCall {
    RpcCall::new(DIRECTORY_PICKER_PICK, json!({}))
}

/// `directoryPicker/list` 的两种形状（`:3986` 空参 = 内核默认起点、`:4102` 带 `path`）：
/// 描述符只有一颗 `path` 且 `acceptsUndefined: true` ⇒ `{}` 与 `{path}` 都合法，
/// `{path: null}` **不**合法（codec 是 `union(undefined,string)`，null 过不了）。
pub fn directory_picker_list(path: Option<&str>) -> RpcCall {
    let mut args = json!({});
    if let Some(path) = path.filter(|path| !path.is_empty()) {
        args["path"] = json!(path);
    }
    RpcCall::new(DIRECTORY_PICKER_LIST, args)
}

/// `directoryPicker/createDirectory`（`:4139`）：`{path,name}` **平铺**，无 `request` 包裹。
/// 平铺是本发唯一正确的形状（两颗独立 wire 参数），照 workspace 那族裹一层就必拒。
pub fn directory_picker_create_directory(path: &str, name: &str) -> RpcCall {
    RpcCall::new(DIRECTORY_PICKER_CREATE_DIRECTORY, json!({ "path": path, "name": name }))
}

/// 形状不合内核描述符时的判词：`内核没给`（键不存在）与 `形状不对`（键在但类型错）
/// 要分得开，两者都**不 panic**，一律 `Err(String)`（`bad-response: ` 前缀 = 本文件既有口径，
/// 见 `Kernel::create_session`）。
fn shape_missing(field: &str, from: &str) -> String {
    format!("bad-response: {from} 里没有 {field} 这个键。")
}

fn shape_off_type(field: &str, from: &str) -> String {
    format!("bad-response: {from} 的 {field} 不是预期的类型。")
}

fn req_str(record: &Value, field: &str, from: &str) -> Result<String, String> {
    match record.get(field) {
        None => Err(shape_missing(field, from)),
        Some(value) => value.as_str().map(str::to_string).ok_or_else(|| shape_off_type(field, from)),
    }
}

fn req_bool(record: &Value, field: &str, from: &str) -> Result<bool, String> {
    match record.get(field) {
        None => Err(shape_missing(field, from)),
        Some(value) => value.as_bool().ok_or_else(|| shape_off_type(field, from)),
    }
}

fn req_str_list(record: &Value, field: &str, from: &str) -> Result<Vec<String>, String> {
    match record.get(field) {
        None => Err(shape_missing(field, from)),
        Some(value) => {
            let items = value.as_array().ok_or_else(|| shape_off_type(field, from))?;
            Ok(items
                .iter()
                .map(|item| item.as_str().map(str::to_string).unwrap_or_default())
                .collect())
        }
    }
}

/// `WorkspaceView`（内核回执与 follow 流 `upsert` 帧共用同一颗对象，六颗必填）。
///
/// 与既有的 [`Workspace`] 分开是两个理由：那张表是 follow 流的**投影**（只读四颗、缺键宽容），
/// 这一张是 unary 回执的**校验**（六颗必填，缺一颗就是内核版本对不上）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceView {
    pub id: String,
    pub path: String,
    pub title: String,
    pub session_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkspaceView {
    /// 校验一颗 View 对象本身（`value.workspace` 那一层）。
    pub fn parse(value: &Value) -> Result<Self, String> {
        const FROM: &str = "workspace 视图";
        Ok(Self {
            id: req_str(value, "workspaceId", FROM)?,
            path: req_str(value, "path", FROM)?,
            title: req_str(value, "title", FROM)?,
            session_ids: req_str_list(value, "sessionIds", FROM)?,
            created_at: req_str(value, "createdAt", FROM)?,
            updated_at: req_str(value, "updatedAt", FROM)?,
        })
    }

    /// 折进左栏那张表（`WorkspaceTree` 的元素形状）。`createdAt/updatedAt` 树里不留：
    /// 主干的 `WorkspaceVm` 同样只有 id/path/title 三颗，排序靠 follow 的 `order` 帧。
    pub fn to_workspace(&self) -> Workspace {
        Workspace {
            id: self.id.clone(),
            path: self.path.clone(),
            title: self.title.clone(),
            session_ids: self.session_ids.clone(),
        }
    }
}

/// `{workspace: …}` 这一型回执（rename / insertSessionBefore 两发共用）。
pub fn parse_workspace_value(value: &Value) -> Result<WorkspaceView, String> {
    let view = value
        .get("workspace")
        .ok_or_else(|| shape_missing("workspace", "工作区回执"))?;
    WorkspaceView::parse(view)
}

/// `workspace/create` 的回执 `{workspace, created}`：`created=false` = 该路径早就注册过，
/// 内核只是把它解析出来（`resolveByPath`），主干据此照样选中新工作区。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceCreated {
    pub view: WorkspaceView,
    pub created: bool,
}

pub fn parse_workspace_created(value: &Value) -> Result<WorkspaceCreated, String> {
    Ok(WorkspaceCreated {
        view: parse_workspace_value(value)?,
        created: req_bool(value, "created", "workspace/create 回执")?,
    })
}

/// `workspace/delete` 的回执 `{deleted: true}`（描述符是**字面量** true，不是布尔）。
pub fn parse_workspace_deleted(value: &Value) -> Result<bool, String> {
    req_bool(value, "deleted", "workspace/delete 回执")
}

/// `workspace/insertBefore` 的回执 `{workspaceIds}`：改完之后的全序。
pub fn parse_workspace_ids(value: &Value) -> Result<Vec<String>, String> {
    req_str_list(value, "workspaceIds", "workspace/insertBefore 回执")
}

/// `workspace/archiveSession` 的回执 `{archivedSessionIds}`：归档集全量。
pub fn parse_archived_session_ids(value: &Value) -> Result<Vec<String>, String> {
    req_str_list(value, "archivedSessionIds", "workspace/archiveSession 回执")
}

/// `directoryPicker/pick` 的两态。内核描述符是 `union(null,string)`：
/// `null` = 操作员在原生对话框里按了取消（主干 :3921 的 `is { Length: > 0 }` 同一判据 ——
/// 空串也当取消处理，不能拿它去覆盖用户已经手输的路径）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickOutcome {
    Picked(String),
    Cancelled,
}

pub fn parse_pick(value: &Value) -> PickOutcome {
    match value.as_str() {
        Some(path) if !path.is_empty() => PickOutcome::Picked(path.to_string()),
        _ => PickOutcome::Cancelled,
    }
}

/// `directoryPicker/createDirectory` 的回执：**裸字符串**（新目录的绝对路径），不是对象。
pub fn parse_created_directory(value: &Value) -> Result<String, String> {
    value
        .as_str()
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "bad-response: 新目录回执不是非空字符串。".to_string())
}

/// `list` 的 `crumbs[]` 与 `entries[]` 的元素形状（三颗必填；描述符里两族逐字同形）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub hidden: bool,
}

/// `DirectoryListing`（`@deepseek-ai/dsh-host-directory-picker#DirectoryListing`）：
/// 五颗全必填。注意主干**从不读** `home`（`:4008` 之后只用 path/crumbs/entries/truncated），
/// 但描述符要求它必须在 ⇒ 解析照样必填，缺了就是内核没给。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirectoryListing {
    pub path: String,
    pub home: String,
    pub crumbs: Vec<DirectoryEntry>,
    pub entries: Vec<DirectoryEntry>,
    pub truncated: bool,
}

fn req_entry_list(value: &Value, field: &str, from: &str) -> Result<Vec<DirectoryEntry>, String> {
    let Some(items) = value.get(field) else {
        return Err(shape_missing(field, from));
    };
    let items = items.as_array().ok_or_else(|| shape_off_type(field, from))?;
    items
        .iter()
        .map(|item| {
            Ok(DirectoryEntry {
                name: req_str(item, "name", field)?,
                path: req_str(item, "path", field)?,
                hidden: req_bool(item, "hidden", field)?,
            })
        })
        .collect()
}

impl DirectoryListing {
    pub fn parse(value: &Value) -> Result<Self, String> {
        const FROM: &str = "directoryPicker/list 回执";
        Ok(Self {
            path: req_str(value, "path", FROM)?,
            home: req_str(value, "home", FROM)?,
            crumbs: req_entry_list(value, "crumbs", FROM)?,
            entries: req_entry_list(value, "entries", FROM)?,
            truncated: req_bool(value, "truncated", FROM)?,
        })
    }

    /// 主干 `RenderDirectoryListing` 对**空 path 的行**是「跳过」而不是报错（`:4025-4027` 面包屑、
    /// `:4076-4078` 目录项）；内核不会发空 path，这一道是主干的自卫。渲染侧走这两个方法，
    /// 别在解析里悄悄丢行 —— 丢了就再也看不出「内核发了颗空行」。
    pub fn visible_entries(&self) -> Vec<&DirectoryEntry> {
        self.entries
            .iter()
            .filter(|entry| !entry.path.is_empty())
            .collect()
    }

    pub fn visible_crumbs(&self) -> Vec<&DirectoryEntry> {
        self.crumbs
            .iter()
            .filter(|crumb| !crumb.path.is_empty())
            .collect()
    }
}

// ==================== #104 `_maxTokensBubbles` 折叠（主干 `MessageDetails.cs:105/501-552`） ====================
// 本 mod 追加在**文件末尾**：本仓的 `include_str!` 源码锁按「起始锚 → 结束锚的首次出现」开窗，
// 插在被打锁的函数之前会抢掉结束锚点（既有规矩，见 src/main.rs 尾部那条同名注释）。

#[cfg(test)]
mod max_tokens_ledger_tests {
    use super::*;

    fn turn_end(turn: i64, kind: &str) -> Value {
        json!({"type": "turn/end", "seq": 7, "time": 900, "data": {"turn": turn, "reason": {"kind": kind}}})
    }

    fn attempt(pieces: Value) -> Value {
        json!({"type": "assistant/attempt", "seq": 8, "time": 900, "data": {"turn": 3, "stream": pieces}})
    }

    /// 主干 `NoteTurnEndReason`（`:501-511`）：`data.reason.kind == "max-tokens"` 才落行，
    /// 且 `AppendMaxTokensRow` 的「同轮只出一条」门（`:537-540`）压住重复回放。
    #[test]
    fn turn_end_marks_the_turn_once_per_turn() {
        let mut ledger = DetailsLedger::new();
        assert!(
            ledger.note_max_tokens(&turn_end(3, "max-tokens"), 3),
            "主干 `:506` 认这个 kind ⇒ 本轮第一次命中必须给行"
        );
        assert!(
            !ledger.note_max_tokens(&turn_end(3, "max-tokens"), 3),
            "同轮第二条撞主干 `:539` 那道 `return` ⇒ 不许再落一行"
        );
        assert!(
            ledger.note_max_tokens(&turn_end(4, "max-tokens"), 4),
            "下一轮是独立的一行（主干的门按 `b.Turn == turn` 比）"
        );
        assert_eq!(ledger.max_tokens_turns(), vec![3, 4]);
        assert!(ledger.has_max_tokens(3) && ledger.has_max_tokens(4) && !ledger.has_max_tokens(5));
    }

    /// 非 max-tokens 的收尾理由一律不出行：`end_turn` / 缺 reason / reason 不是对象 / data 缺。
    #[test]
    fn other_turn_end_reasons_never_mark() {
        let mut ledger = DetailsLedger::new();
        assert!(!ledger.note_max_tokens(&turn_end(3, "end_turn"), 3));
        assert!(!ledger.note_max_tokens(
            &json!({"type": "turn/end", "data": {"turn": 3}}),
            3
        ));
        assert!(!ledger.note_max_tokens(
            &json!({"type": "turn/end", "data": {"turn": 3, "reason": "max-tokens"}}),
            3
        ));
        assert!(!ledger.note_max_tokens(&json!({"type": "turn/end"}), 0));
        assert!(ledger.max_tokens_turns().is_empty());
    }

    /// 主干 `NoteAttemptMaxTokens`（`:513-530`）读的是 **`piece.type` / `piece.reason.kind`**，
    /// 不套 `chunk`。同一份数据挪进 `chunk` 里主干这一案就读不到（另一案 `MainWindow.xaml.cs:6185`
    /// 才读 `chunk`），分叉照抄这个不对称 —— 这条用例存在的意义就是拦住「顺手统一两条臂」。
    #[test]
    fn attempt_hits_on_piece_type_and_not_on_chunk_nested_shape() {
        let mut ledger = DetailsLedger::new();
        assert!(
            ledger.note_max_tokens(&attempt(json!([{"type": "finish", "reason": {"kind": "max-tokens"}}])), 3),
            "主干 `:523/:525` 的两判据都在 piece 本层"
        );
        let mut nested = DetailsLedger::new();
        assert!(
            !nested.note_max_tokens(
                &attempt(json!([{"chunk": {"type": "finish", "reason": {"kind": "max-tokens"}}}])),
                3
            ),
            "套进 chunk 就不是主干这一案的形状（那是 ⚠ 失败气泡那条臂的路径）"
        );
        assert!(nested.max_tokens_turns().is_empty());
    }

    /// `stream` 形状细节：非 finish 的块、kind 不是 max-tokens、`stream` 不是数组、整帧缺 data
    /// 都不出行；一帧里多条命中只出行一次（主干 foreach 第二次被同轮的门挡掉）。
    #[test]
    fn attempt_shapes_that_do_not_produce_a_row() {
        let mut ledger = DetailsLedger::new();
        let misses = [
            attempt(json!([{"type": "text-delta", "reason": {"kind": "max-tokens"}}])),
            attempt(json!([{"type": "finish", "reason": {"kind": "error"}}])),
            attempt(json!([{"type": "finish"}])),
            attempt(json!([{"type": "finish", "reason": "max-tokens"}])),
            json!({"type": "assistant/attempt", "data": {"turn": 9, "stream": "不是数组"}}),
            json!({"type": "assistant/attempt", "data": {}}),
            json!({"type": "assistant/attempt"}),
        ];
        for frame in &misses {
            assert!(
                !ledger.note_max_tokens(frame, 9),
                "{frame} 不该出行（主干两处判据没同时中）"
            );
        }
        let doubled = attempt(json!([
            {"type": "finish", "reason": {"kind": "max-tokens"}},
            {"type": "finish", "reason": {"kind": "max-tokens"}},
        ]));
        assert!(ledger.note_max_tokens(&doubled, 9), "一帧内两条命中 ⇒ 第一条出行");
        assert!(!ledger.note_max_tokens(&doubled, 9), "整帧重放不许出行第二条");
        assert_eq!(ledger.max_tokens_turns(), vec![9]);
    }

    /// 与本表（`_messageDetails`）互不相干：这两型帧写的是同族另一张表，
    /// `rows` / 待认领桶 / `system_prompt_seen` 一律不动，`note_event` 的回执仍是 default。
    /// 主干依据 `MessageDetails.cs:148-153`（两案都 `return false` 且不碰 `_messageDetails`）。
    ///
    /// **两型帧各占一个独立轮次**（3 只由 `turn/end` 喂、4 只由 `assistant/attempt` 喂）：
    /// 若把两型喂进同一轮，`note_event` 少路由一案也会被同一轮的另一案补上，用例照样绿
    /// ⇒ 那是假绿（本仓记过的第四种，变异检验 M8 当场把它抓出来过）。
    #[test]
    fn max_tokens_frames_leave_message_details_untouched_but_do_mark_the_turn() {
        let mut ledger = DetailsLedger::new();
        let mut attempt_frame = attempt(json!([{"type": "finish", "reason": {"kind": "max-tokens"}}]));
        attempt_frame["data"]["turn"] = json!(4);
        for (turn, frame) in [(3_i64, turn_end(3, "max-tokens")), (4, attempt_frame)] {
            let delta = ledger.note_event(&frame, Some(&DetailsKey::bubble("user", 3, 1)));
            assert!(!delta.any(), "{frame} 对 _messageDetails 是 no-op，却回了 {delta:?}");
            assert!(
                !ledger.has_max_tokens(turn + 1),
                "串轮了：{turn} 轮的帧不该把 {turn}+1 轮也标上"
            );
            assert!(
                ledger.has_max_tokens(turn),
                "{frame} 走 note_event 没折进 _maxTokensBubbles ⇒ 分派臂少了一案"
            );
        }
        assert_eq!(ledger.max_tokens_turns(), vec![3, 4]);
        assert!(ledger.is_empty());
        assert!(ledger.pending_extra_blocks().is_empty());
        assert!(ledger.pending_references().is_empty());
        assert!(!ledger.system_prompt_seen());
    }

    /// 认不出的名字不脏第二张表：九类吞帧与 `tool/call` 之类一律不参与截断门。
    #[test]
    fn unrelated_frames_never_mark_a_turn() {
        let mut ledger = DetailsLedger::new();
        for frame in [
            json!({"type": "llm/retry", "data": {"turn": 3, "reason": {"kind": "max-tokens"}}}),
            json!({"type": "compaction/end", "data": {"turn": 3}}),
            json!({"type": "tool/call", "data": {"turn": 3}}),
            json!({"type": "request/context", "data": {"turn": 3}}),
            json!({"seq": 1}),
            json!({"type": 7}),
            Value::Null,
        ] {
            assert!(!ledger.note_max_tokens(&frame, 3), "{frame} 与截断警示无关");
        }
        assert!(ledger.max_tokens_turns().is_empty());
    }

    /// 清空点：主干 `MessageDetails.cs:128` 那句 `_maxTokensBubbles.Clear()` 与 `:124-127`
    /// 同在一发 `ResetMessageDomainState` 里 ⇒ 分叉也必须跟着 `reset()` 一起清，
    /// 且清完同一轮还能再出（换会话后回放新的 journal）。
    #[test]
    fn reset_clears_the_max_tokens_turns_alongside_the_rows() {
        let mut ledger = DetailsLedger::new();
        ledger.note_event(
            &json!({"type": "system/message", "seq": 1, "data": {"message": {"content": [{"type": "text", "text": "提示词"}]}}}),
            Some(&DetailsKey::bubble("tool", 1, 1)),
        );
        assert!(ledger.note_max_tokens(&turn_end(6, "max-tokens"), 6));
        assert!(!ledger.is_empty() && ledger.has_max_tokens(6));
        ledger.reset();
        assert!(ledger.is_empty());
        assert!(
            ledger.max_tokens_turns().is_empty(),
            "换会话/清空后旧轮的截断账必须作废（主干 `:128`）"
        );
        assert!(
            ledger.note_max_tokens(&turn_end(6, "max-tokens"), 6),
            "清完同一轮要能重新出行，否则回放丢行"
        );
    }
}

#[cfg(test)]
mod model_retry_ledger_tests {
    use super::*;

    fn retry(data: Value) -> Value {
        json!({"type": "llm/retry", "seq": 5, "time": 1000, "data": data})
    }

    /// 主干 `HandleLlmRetry`（`:562-586`）逐字段：`retryId` 空 ⇒ 回落 `retry-{envTime}-{turn}`；
    /// `retry` 只认「存在且是数字」否则 1；`maxRetries`/`delayMs` 只认数字否则 0；`mode` 空 ⇒
    /// "normal"；`failure` 是对象才读两串。`state` 恒 `scheduled`、`deadline = now + delay`。
    #[test]
    fn note_llm_retry_folds_every_mainline_field_quirk() {
        let mut ledger = DetailsLedger::new();
        // 回落 retryId + retry=非数字⇒1 + mode 空⇒normal + 无 failure⇒空。
        assert!(
            ledger.note_llm_retry(&retry(json!({"retry": "3"})), 7, 1000, 500, ""),
            "首见这一 retryId ⇒ 该建一行"
        );
        let st = ledger.retry_for_bubble("").expect("回落行没建起来");
        assert_eq!(st.retry_id, "retry-1000-7", "主干 `:564-565` 的回落串");
        assert_eq!(st.retry, 1, "`retry` 是字符串不是数字 ⇒ 落回 1（`:566`）");
        assert_eq!(st.max_retries, 0, "缺 maxRetries ⇒ 0（`:567-569`）");
        assert_eq!(st.delay_ms, 0, "缺 delayMs ⇒ 0（`:570`）");
        assert_eq!(st.mode, "normal", "mode 空 ⇒ normal（`:571-572`）");
        assert!(st.failure_message.is_empty() && st.failure_code.is_empty());
        assert_eq!(st.state, "scheduled");
        assert_eq!(st.deadline_ms, 500, "now(500)+delay(0)");

        // 显式 retryId + 全是数字 + mode 非 normal + failure 对象。
        let mut two = DetailsLedger::new();
        assert!(two.note_llm_retry(
            &retry(json!({
                "retryId": "r-9", "retry": 3, "maxRetries": 5, "delayMs": 2500,
                "mode": "endless", "failure": {"message": "boom", "code": "E500"}
            })),
            7,
            1000,
            800,
            "",
        ));
        let st = two.retry_for_bubble("").expect("显式行没建起来");
        assert_eq!(st.retry_id, "r-9");
        assert_eq!((st.retry, st.max_retries, st.delay_ms, st.mode.as_str()), (3, 5, 2500, "endless"));
        assert_eq!(st.failure_message, "boom");
        assert_eq!(st.failure_code, "E500");
        assert_eq!(st.deadline_ms, 3300, "800 + 2500");
    }

    /// `retry` 是 0 / 负数也照取（主干只判 `ValueKind == Number`，不判正负）。
    #[test]
    fn retry_zero_is_a_number_and_beats_the_default() {
        let mut ledger = DetailsLedger::new();
        ledger.note_llm_retry(&retry(json!({"retry": 0})), 1, 1, 0, "");
        assert_eq!(ledger.retry_for_bubble("").unwrap().retry, 0, "0 是合法数字，不落回 1");
    }

    /// 主干 `:581-585` AddOrUpdate + `:594` `Bubble is null` 那一支：同 `retryId` 第二帧只更新
    /// 字段、不再建新行（返回 false）；字段被覆写、`state` 复位 scheduled。
    #[test]
    fn repeat_of_same_retry_id_updates_in_place_without_new_bubble() {
        let mut ledger = DetailsLedger::new();
        assert!(ledger.note_llm_retry(&retry(json!({"retryId": "r", "retry": 1})), 2, 1, 0, "k-1"), "首帧建行");
        // 第二帧：同 retryId ⇒ 已有气泡 ⇒ 返回 false（不建新行），字段覆写。
        assert!(
            !ledger.note_llm_retry(&retry(json!({"retryId": "r", "retry": 2, "maxRetries": 9})), 2, 1, 50, "ignored"),
            "同 retryId 已有气泡 ⇒ 主干走 RepaintBubble，分叉不再建新行"
        );
        assert_eq!(ledger.model_retry_ids(), vec!["r".to_string()], "不该多出第二行");
        let st = ledger.retry_for_bubble("k-1").unwrap();
        assert_eq!((st.retry, st.max_retries), (2, 9), "AddOrUpdate 覆写没生效");
        assert_eq!(st.bubble_key, "k-1", "回填的键被第二帧冲掉了");
    }

    /// 两个不同 `retryId` 各建一行；`retry_for_bubble` 找不到匹配键时回落「最后插入的一个」
    /// （主干 `:969` `st ??= _modelRetries.Values.LastOrDefault()`，逐字照抄的兜底）。
    #[test]
    fn retry_for_bubble_falls_back_to_the_last_inserted() {
        let mut ledger = DetailsLedger::new();
        ledger.note_llm_retry(&retry(json!({"retryId": "a"})), 1, 1, 0, "ka");
        ledger.note_llm_retry(&retry(json!({"retryId": "b"})), 1, 2, 0, "kb");
        assert_eq!(ledger.model_retry_ids(), vec!["a".to_string(), "b".to_string()], "插入序");
        assert_eq!(ledger.retry_for_bubble("ka").unwrap().retry_id, "a", "命中自己那行");
        assert_eq!(ledger.retry_for_bubble("kb").unwrap().retry_id, "b");
        assert_eq!(
            ledger.retry_for_bubble("不存在").unwrap().retry_id,
            "b",
            "找不到 ⇒ 主干那发 LastOrDefault（最后一个插入的），不是按键序",
        );
    }

    /// 主干 `HandleLlmRetryStarted`（`:614-622`）：`retry-started` 把行改 `started`。
    /// 与采集案不同——这里 `retryId` 空 ⇒ 直接 return（**没有** envTime 回落）；找不到行也 return。
    #[test]
    fn retry_started_marks_found_rows_only_and_never_synthesizes_an_id() {
        let mut ledger = DetailsLedger::new();
        // 空 retryId ⇒ 主干 `:616` 直接 return，绝不回落建/找 `retry-...`。
        assert!(!ledger.note_llm_retry_started(&json!({"type": "llm/retry-started", "data": {}})));
        assert!(ledger.model_retry_ids().is_empty(), "started 案不建行");
        // 未知 retryId ⇒ 找不到 ⇒ false。
        assert!(!ledger.note_llm_retry_started(&json!({"type":"llm/retry-started","data":{"retryId":"ghost"}})));
        // 命中 ⇒ 改 started。
        ledger.note_llm_retry(&retry(json!({"retryId": "r"})), 1, 1, 0, "k");
        assert!(ledger.note_llm_retry_started(&json!({"type":"llm/retry-started","data":{"retryId":"r"}})));
        assert_eq!(ledger.retry_for_bubble("k").unwrap().state, "started");
    }

    /// 主干 `CancelModelRetry`（`:673+`）改状态那一半：按气泡键找到行压 `cancelled`；找不到不动。
    #[test]
    fn cancel_flips_the_matching_row_to_cancelled() {
        let mut ledger = DetailsLedger::new();
        ledger.note_llm_retry(&retry(json!({"retryId": "r"})), 1, 1, 0, "k");
        assert!(!ledger.cancel_model_retry("别的键"), "找不到 ⇒ 什么都不改");
        assert_eq!(ledger.retry_for_bubble("k").unwrap().state, "scheduled");
        assert!(ledger.cancel_model_retry("k"));
        assert_eq!(ledger.retry_for_bubble("k").unwrap().state, "cancelled");
    }

    /// 主干 `ResetMessageDomainState`（`:119-124`）里 `_modelRetries.Clear()` 那一发 ⇒
    /// 分叉 `reset()` 一并清；清完同 retryId 能重新建行（换会话回放）。
    #[test]
    fn reset_clears_model_retries_alongside_the_rows() {
        let mut ledger = DetailsLedger::new();
        assert!(ledger.note_llm_retry(&retry(json!({"retryId": "r"})), 1, 1, 0, "k"));
        ledger.reset();
        assert!(ledger.model_retry_ids().is_empty(), "换会话旧重试账必须作废（主干 `:124`）");
        assert!(
            ledger.note_llm_retry(&retry(json!({"retryId": "r"})), 1, 1, 0, "k2"),
            "清完同一 retryId 要能重新建行"
        );
    }
}

#[cfg(test)]
mod usage_stats_tests {
    use super::*;

    /// 一页 `session/page` 的回包（records 按 seq 升序 = 内核真实形状）。
    fn page(records: Vec<Value>, has_more: bool) -> Value {
        json!({ "records": records, "hasMore": has_more })
    }

    /// 一条 journal 记录：`{type:"event", event:{seq,time,type,data}}`。
    fn record(seq: i64, kind: &str, time: i64, data: Value) -> Value {
        json!({ "type": "event", "event": { "seq": seq, "time": time, "type": kind, "data": data } })
    }

    fn usage(tokens: i64) -> Value {
        json!({ "usage": { "totalTokens": tokens } })
    }

    fn sourced(tokens: i64, provider: &str, model: &str) -> Value {
        json!({
            "usage": { "totalTokens": tokens },
            "message": { "source": { "provider": provider, "model": model } },
        })
    }

    /// 一条带模型出处的 `assistant/message`。
    fn message(seq: i64, time: i64, tokens: i64, provider: &str, model: &str) -> Value {
        record(seq, "assistant/message", time, sourced(tokens, provider, model))
    }

    /// 一张内存页表（不出网）：按 `throughSeq` 命中，顺带把每发请求的 JSON 记下来。
    #[derive(Default)]
    struct Desk {
        pages: Vec<(i64, Value)>,
        sent: Vec<Value>,
    }

    impl Desk {
        fn send(&mut self, request: &HistoryPage) -> Result<Value, String> {
            self.sent.push(request.to_args());
            self.pages
                .iter()
                .find(|(through, _)| *through == request.through_seq)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| format!("session/page/bad: past cursor {}", request.through_seq))
        }
    }

    /// 单页走查（`hasMore:false` ⇒ 一发收手），拿回来的是那个会话的累加器。
    fn feed(records: Vec<Value>) -> SessionScan {
        let mut desk = Desk {
            pages: vec![(9, page(records, false))],
            sent: Vec::new(),
        };
        let scan = scan_session_usage("s", 0, 9, 0, &mut |request| desk.send(request));
        assert_eq!(desk.sent.len(), 1, "这一组用例只该发一发请求");
        scan
    }

    const TODAY: i64 = 20_720; // 2026-09-24（UTC 口径的日序号，用 `date -u` 核过）

    #[test]
    fn token_of_prefers_total_then_sums_four_buckets_truncating() {
        assert_eq!(token_of(&json!({"totalTokens": 90.5})), 90, "主干 (long)GetDouble ⇒ 先截后返回");
        assert_eq!(
            token_of(&json!({"totalTokens": 0, "inputTokens": 1.9, "outputTokens": 2, "cacheReadTokens": 3, "cacheWriteTokens": 4})),
            10,
            "totalTokens==0 ⇒ 走 4 桶（1 截自 1.9）",
        );
        assert_eq!(token_of(&json!({})), 0);
        assert_eq!(token_of(&json!({"totalTokens": -5, "inputTokens": 7})), 7, "负 totalTokens 不算『有值』（主干 v>0 前置）");
        assert_eq!(token_of(&json!({"inputTokens": "3", "outputTokens": 1})), 1, "字符串数字不是数值 ⇒ 不计");
    }

    #[test]
    fn day_index_and_format_match_mainline_local_daykey() {
        assert_eq!(format_day(0), "1970-01-01");
        assert_eq!(format_day(-1), "1969-12-31", "div_euclid：1970 前不落进 0 号格");
        for (day, text) in [
            (11_017, "2000-03-01"),
            (19_782, "2024-02-29"),
            (20_714, "2026-09-18"),
            (TODAY, "2026-09-24"),
            (47_846, "2100-12-31"),
        ] {
            assert_eq!(format_day(day), text, "日序 {day} 的显示形态");
        }
        // 同一时刻，UTC 与东八区差一天：主干 `ToLocalTime()` 的分界就在 UTC 16:00。
        let utc_four_pm = TODAY * DAY_MS + 16 * 3600 * 1000;
        assert_eq!(day_index(utc_four_pm, 0), TODAY);
        assert_eq!(day_index(utc_four_pm, 8 * 3600), TODAY + 1);
        assert_eq!(format_day(day_index(utc_four_pm, 8 * 3600)), "2026-09-25");
        assert_eq!(day_index(0, 0), 0, "time 缺失（主干按 0 算）落 1970-01-01");
    }

    #[test]
    fn ledger_through_skips_blank_and_unusable_cursor_but_keeps_seq_zero() {
        assert_eq!(ledger_through(true, Some(9)), None, "blank ⇒ 跳过并进 sessions_skipped");
        assert_eq!(ledger_through(false, None), None, "无游标 ⇒ 主干按 -1 处理");
        assert_eq!(ledger_through(false, Some(-1)), None);
        assert_eq!(ledger_through(false, Some(0)), Some(0), "0 是合法游标，判成 skip 就少扫一个会话");
        assert_eq!(ledger_through(false, Some(7)), Some(7));
    }

    #[test]
    fn stats_page_request_is_the_mainline_five_hundred_row_walk() {
        let mut desk = Desk {
            pages: vec![(9, page(Vec::new(), false))],
            sent: Vec::new(),
        };
        scan_session_usage("s-1", 0, 9, 0, &mut |request| desk.send(request));
        assert_eq!(
            desk.sent,
            vec![json!({"request":{"address":{"kind":"session","sessionId":"s-1"},"throughSeq":9,"maxMessages":500}})],
            "逐字节：500 不是内核默认的 50；外层 request 包裹与 kind=session 都在",
        );
    }

    #[test]
    fn paging_steps_back_from_the_first_record_seq() {
        let mut desk = Desk {
            pages: vec![
                (
                    9,
                    page(
                        vec![
                            message(6, TODAY * DAY_MS, 10, "p", "a"),
                            message(8, TODAY * DAY_MS + 1000, 20, "p", "a"),
                        ],
                        true,
                    ),
                ),
                (
                    5,
                    page(
                        vec![
                            message(2, (TODAY - 1) * DAY_MS, 5, "p", "a"),
                            message(3, (TODAY - 1) * DAY_MS + 50, 5, "p", "b"),
                        ],
                        false,
                    ),
                ),
            ],
            sent: Vec::new(),
        };
        let scan = scan_session_usage("s", 0, 9, 0, &mut |request| desk.send(request));
        assert_eq!(
            desk.sent
                .iter()
                .map(|args| args["request"]["throughSeq"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            vec![9, 5],
            "下一页 = 本页第一条事件的 seq - 1",
        );
        assert_eq!(scan.usage_messages, 4);
        assert_eq!(
            scan.entries,
            vec![
                (TODAY, "p/a".to_string(), 30),
                (TODAY - 1, "p/a".to_string(), 5),
                (TODAY - 1, "p/b".to_string(), 5),
            ],
            "明细表按**收到序**（内核新→旧翻页），同日同模型当场合并；日序由 `absorb` 插进升序日表",
        );
        assert_eq!(scan.usage().span_ms, (TODAY * DAY_MS + 1000) - ((TODAY - 1) * DAY_MS));
    }

    #[test]
    fn walk_stops_on_empty_page_unreadable_seq_and_transport_error() {
        // 空 records ⇒ 一发收手（主干 `count == 0 break`），哪怕 `hasMore:true`。
        let mut desk = Desk {
            pages: vec![(5, page(Vec::new(), true))],
            sent: Vec::new(),
        };
        let scan = scan_session_usage("s", 0, 5, 0, &mut |request| desk.send(request));
        assert_eq!(desk.sent.len(), 1);
        assert_eq!(scan.usage().span_ms, 0);
        assert!(!scan.pages_capped());

        // 页非空、hasMore:true，但解不出翻页刀 ⇒ 收手，不许原地重复拉同一页。
        let mut desk = Desk {
            pages: vec![(
                5,
                page(vec![json!({"type":"event","event":{"time":1,"type":"noop"}})], true),
            )],
            sent: Vec::new(),
        };
        scan_session_usage("s", 0, 5, 0, &mut |request| desk.send(request));
        assert_eq!(desk.sent.len(), 1, "翻页刀读不出 ⇒ 主干 break，不是死循环");

        // 第二发报错 ⇒ 保留第一发已聚合的部分（主干 `catch (DshRpcException) break`）。
        let mut desk = Desk {
            pages: vec![(9, page(vec![message(9, TODAY * DAY_MS, 7, "p", "a")], true))],
            sent: Vec::new(),
        };
        let scan = scan_session_usage("s", 0, 9, 0, &mut |request| desk.send(request));
        assert_eq!(desk.sent.len(), 2, "第二发（through=8）报错前已发出");
        assert_eq!(scan.usage_messages, 1, "部分计入不许整段丢");
    }

    #[test]
    fn forty_page_cap_marks_the_session_as_partially_counted() {
        let mut sends = 0;
        let scan = scan_session_usage("s", 0, 1000, 0, &mut |request| {
            sends += 1;
            // 恒有下一页、恒非空，翻页刀一路往回走 ⇒ 只有 40 页上限收得住它。
            let through = request.through_seq;
            Ok(page(
                vec![message(through - 1, through * 10, 1, "p", "a")],
                true,
            ))
        });
        assert_eq!(sends, STATS_MAX_PAGES as usize, "主干 maxPages = 40");
        assert!(scan.pages_capped());
        let mut aggregate = UsageAggregate::default();
        aggregate.absorb(&scan);
        assert_eq!(
            (aggregate.sessions_scanned, aggregate.pages_capped),
            (1, 1),
            "走查数与『部分计入』是两件事",
        );
    }

    #[test]
    fn talk_ms_only_counts_forward_turns_and_invalidates_after_end() {
        let scan = feed(vec![
            record(1, "turn/start", 1000, Value::Null),
            record(2, "turn/end", 3500, Value::Null), // +2500
            record(3, "turn/end", 4000, Value::Null), // 没有起点 ⇒ 不计
            record(4, "turn/start", 6000, Value::Null),
            record(5, "turn/end", 5000, Value::Null), // 时间倒挂 ⇒ 不计，且起点照样作废
            record(6, "turn/start", 0, Value::Null),  // 起点 0 ⇒ 主干 `> 0` 前置挡住
            record(7, "turn/end", 700, Value::Null),
        ]);
        assert_eq!(scan.usage().talk_ms, 2500, "只有第一条正向 turn 配对成功");
        assert_eq!(scan.usage().span_ms, 6000 - 700, "span 是存活窗口（time>0 的首末差）");
    }

    #[test]
    fn only_positive_usage_from_assistant_messages_gets_booked() {
        let scan = feed(vec![
            record(1, "user/message", 1000, usage(999)), // 非 assistant ⇒ 不入账
            record(2, "assistant/message", 1100, json!({})), // 无 usage ⇒ 弃
            record(3, "assistant/message", 1200, json!({"usage": 5})), // usage 非对象 ⇒ 弃
            record(4, "assistant/message", 1300, usage(0)), // 0 token ⇒ 弃，连条数都不涨
            record(5, "assistant/message", 1400, usage(10)),
        ]);
        assert_eq!(scan.usage_messages, 1, "四条里只有一条真入账");
        assert_eq!(scan.entries, vec![(0, UNLABELED_MODEL.to_string(), 10)]);
        assert_eq!(model_of(&sourced(1, "openai", "gpt-x")), "openai/gpt-x");
        assert_eq!(model_of(&sourced(1, "", "gpt-x")), "gpt-x");
        assert_eq!(
            model_of(&sourced(1, "openai", "")),
            UNLABELED_MODEL,
            "model 空 ⇒ 兜底，provider 不参与拼接"
        );
        assert_eq!(model_of(&json!({})), UNLABELED_MODEL);
    }

    #[test]
    fn absorb_keeps_days_ascending_and_merges_same_day_models() {
        let mut aggregate = UsageAggregate::default();
        // 晚的一天先来（内核就是从新往旧翻页），日表必须仍按日升序。
        aggregate.absorb(&feed(vec![
            message(2, TODAY * DAY_MS, 30, "openai", "a"),
            message(3, TODAY * DAY_MS + 10, 10, "x", "b"),
            message(4, TODAY * DAY_MS + 20, 5, "openai", "a"),
        ]));
        aggregate.absorb(&feed(vec![message(1, (TODAY - 2) * DAY_MS, 5, "openai", "a")]));
        assert_eq!(
            aggregate.days.iter().map(|day| day.day).collect::<Vec<_>>(),
            vec![TODAY - 2, TODAY]
        );
        assert_eq!(aggregate.days[1].tokens, 45);
        assert_eq!(
            aggregate.days[1].models,
            vec![("openai/a".to_string(), 35), ("x/b".to_string(), 10)],
            "模型按首现序，不是字典序（主干是 Dictionary 枚举序）"
        );
        assert_eq!((aggregate.sessions_scanned, aggregate.usage_messages), (2, 4));
        assert_eq!(aggregate.sessions.len(), 2, "空会话也留汇总行（主干 `_statsSessionUsage` 无条件 Add）");
        aggregate.skip_session();
        assert_eq!(aggregate.sessions_skipped, 1);
    }

    #[test]
    fn kpi_five_tuple_matches_mainline_semantics() {
        let mut aggregate = UsageAggregate::default();
        aggregate.absorb(&feed(vec![message(1, TODAY * DAY_MS, 40, "p", "a")]));
        aggregate.absorb(&feed(vec![message(1, (TODAY - 1) * DAY_MS, 50, "p", "a")]));
        aggregate.absorb(&feed(vec![message(1, (TODAY - 3) * DAY_MS, 50, "p", "b")]));
        aggregate.sessions.push(SessionUsage {
            session_id: "s".into(),
            updated_at: 0,
            span_ms: 99,
            talk_ms: 7_200_000,
        });
        let kpis = aggregate.kpis(TODAY);
        assert_eq!(kpis.total_tokens, 140, "累计 = 全历史 Σ");
        assert_eq!((kpis.peak_tokens, kpis.peak_day), (50, Some(TODAY - 3)), "并列取**最早**那天（主干 v > peak 严格大于）");
        assert_eq!(kpis.longest_talk_ms, 7_200_000, "第三枚 KPI 取各会话 talk_ms 最大，不是 span");
        assert_eq!((kpis.current_streak, kpis.longest_streak), (2, 2), "今+昨连着 2 天；断一天后 t-3 单独 1 天，最长来自那段 2");
        assert_eq!(kpis.active_days, 3);
        assert_eq!((kpis.usage_messages, kpis.session_count), (3, 4));
    }

    #[test]
    fn streak_start_anchor_moves_from_yesterday_when_today_is_empty() {
        let mut aggregate = UsageAggregate::default();
        aggregate.absorb(&feed(vec![
            message(1, (TODAY - 1) * DAY_MS, 1, "p", "a"),
            message(2, (TODAY - 2) * DAY_MS, 1, "p", "a"),
        ]));
        assert_eq!(aggregate.streaks(TODAY), (2, 2), "今天没用量 ⇒ 从昨天往回数（主干 cursor 那一支）");
        assert_eq!(aggregate.streaks(TODAY - 3), (0, 2), "游标落在断档处 ⇒ 当前连续 0，最长不变");
    }

    #[test]
    fn range_only_touches_the_two_charts_and_orders_models_by_tokens_desc() {
        let mut aggregate = UsageAggregate::default();
        aggregate.absorb(&feed(vec![
            message(1, TODAY * DAY_MS, 10, "p", "small"),
            message(2, TODAY * DAY_MS, 70, "p", "big"),
            message(3, (TODAY - 9) * DAY_MS, 999, "p", "out-of-range"),
        ]));
        assert_eq!(
            aggregate
                .model_totals(TODAY, 7)
                .iter()
                .map(|(model, _)| model.as_str())
                .collect::<Vec<_>>(),
            vec!["p/big", "p/small"],
            "降序，且 7 天窗口外那条根本不进清单",
        );
        assert_eq!(aggregate.range_total(TODAY, 7), 80);
        assert_eq!(aggregate.total(), 1079, "KPI 的『累计』仍是全历史 —— 时间范围不许动它");
        assert_eq!(
            aggregate.day_series(TODAY, 7).iter().filter(|(_, v)| *v == 0).count(),
            6,
            "7 格里只有今天有值，其余给 0 占位"
        );
        let mut tie = UsageAggregate::default();
        tie.absorb(&feed(vec![
            message(1, TODAY * DAY_MS, 10, "p", "a"),
            message(2, TODAY * DAY_MS, 10, "p", "b"),
        ]));
        assert_eq!(
            tie.model_totals(TODAY, 7)
                .iter()
                .map(|(model, _)| model.as_str())
                .collect::<Vec<_>>(),
            vec!["p/a", "p/b"],
            "同量并列保首现序（主干 LINQ 稳定排序）",
        );
        assert_eq!(aggregate.model_totals(TODAY, 0).len(), 0, "0 天档 = 主干的空列表");
    }

    #[test]
    fn empty_aggregate_yields_the_dashes_not_a_number() {
        let aggregate = UsageAggregate::default();
        let kpis = aggregate.kpis(TODAY);
        assert_eq!(kpis.total_tokens, 0, "无数据 ⇒ 渲染侧要画破折号，这里不许冒出个假数");
        assert_eq!(aggregate.peak(), None);
        assert_eq!(aggregate.streaks(TODAY), (0, 0));
        assert!(aggregate.model_totals(TODAY, 7).is_empty());
        assert_eq!(aggregate.range_total(TODAY, 7), 0);
        assert_eq!(aggregate.day_series(TODAY, 7).len(), 7);
        assert_eq!(aggregate.longest_talk_ms(), 0);
        assert_eq!(
            source_note_tails(&Catalog::load("zh", None), &kpis),
            (String::new(), String::new())
        );
    }

    /// 反向哨兵：**有**用量时，驱动无数据档（破折号 / 占位句）的判据必须全部退场。
    #[test]
    fn booked_usage_flips_every_no_data_predicate() {
        let mut aggregate = UsageAggregate::default();
        aggregate.absorb(&feed(vec![message(1, TODAY * DAY_MS, 12_345, "p", "a")]));
        let kpis = aggregate.kpis(TODAY);
        assert!(kpis.total_tokens > 0 && kpis.peak_tokens > 0);
        assert!(kpis.peak_day.is_some() && kpis.longest_streak > 0 && kpis.current_streak > 0);
        assert_eq!(kpis.usage_messages, 1);
        assert_eq!(aggregate.model_totals(TODAY, 7), vec![("p/a".to_string(), 12_345)]);
        assert_eq!(aggregate.range_total(TODAY, 7), 12_345);
    }

    #[test]
    fn source_note_tails_are_verbatim_mainline_strings_and_empty_when_zero() {
        let zh = Catalog::load("zh", None);
        let en = Catalog::load("en", None);
        let base = UsageKpis {
            sessions_scanned: 36,
            usage_messages: 412,
            ..UsageKpis::default()
        };
        assert_eq!(
            source_note_tails(&zh, &base),
            (String::new(), String::new()),
            "计数为 0 ⇒ 两半都是空串，不是「跳过 0 个」",
        );
        let skipped = UsageKpis {
            sessions_skipped: 42,
            ..base
        };
        assert_eq!(source_note_tails(&zh, &skipped).0, "，跳过 42 个空会话");
        assert_eq!(source_note_tails(&zh, &skipped).1, "", "跳过句不许顺带把上限句也带出来");
        assert_eq!(
            source_note_tails(&en, &skipped).0,
            ", 42 empty sessions skipped",
            "英文档必须同键查表，不许把中文带进英文界面",
        );
        let capped = UsageKpis {
            pages_capped: 2,
            ..base
        };
        assert_eq!(
            source_note_tails(&zh, &capped).1,
            "；2 个超长会话触到分页上限，其数据为部分计入"
        );
        assert_eq!(source_note_tails(&zh, &capped).0, "");
        let both = UsageKpis {
            sessions_skipped: 1,
            pages_capped: 2,
            ..base
        };
        let (first, second) = source_note_tails(&zh, &both);
        assert_eq!(
            first + &second,
            "，跳过 1 个空会话；2 个超长会话触到分页上限，其数据为部分计入"
        );
    }

    #[test]
    fn collect_usage_stats_walks_the_ledger_in_mainline_order() {
        let ledger = json!({"items": [
            {"sessionId": "", "blank": false},
            {"sessionId": "a", "blank": true, "updatedAt": 1},
            {"sessionId": "c", "updatedAt": 7},
            {"sessionId": "d", "updatedAt": 9, "projections": {"asOfSeq": 4}},
        ]});
        let mut sent: Vec<(String, Value)> = Vec::new();
        let aggregate = collect_usage_stats(
            &mut |method, args| {
                sent.push((method.to_string(), args.clone()));
                if method == "session/list" {
                    return Ok(ledger.clone());
                }
                Ok(page(vec![message(3, TODAY * DAY_MS, 12, "p", "a")], false))
            },
            0,
        )
        .expect("台账够用就该出聚合结果");
        assert_eq!(
            sent[0],
            ("session/list".to_string(), json!({ "_request": {} })),
            "台账那一发的包裹形状（主干 :15044 就是这两个键）",
        );
        assert_eq!(sent.len(), 2, "四行里只有一行够格走查 ⇒ 只发一发 session/page");
        assert_eq!(
            sent[1].1["request"]["throughSeq"],
            json!(4),
            "起点 = session/list 带的 projections.asOfSeq，不是 0 也不是探测出来的",
        );
        assert_eq!(sent[1].0, "session/page");
        let kpis = aggregate.kpis(TODAY);
        assert_eq!(
            (kpis.sessions_scanned, kpis.sessions_skipped),
            (1, 2),
            "四行里 1 扫 2 跳（blank + 无游标），空 sessionId 那行两个计数都不许动",
        );
        assert_eq!((kpis.total_tokens, kpis.usage_messages), (12, 1));
        assert_eq!(kpis.active_days, 1);
    }

    #[test]
    fn no_ledger_and_transport_failures_map_to_the_two_mainline_banners() {
        let mut calls = 0;
        let error = collect_usage_stats(
            &mut |method, _| {
                calls += 1;
                assert_eq!(method, "session/list", "台账没回清单之前不该有第二发");
                Ok(json!({ "items": 5 }))
            },
            0,
        )
        .expect_err("`items` 不是数组 = 主干 Error 横幅那一档");
        assert_eq!(error, UsageError::NoLedger);
        assert_eq!(calls, 1, "判据失败就到此为止，一发 page 都不许补");

        let error = collect_usage_stats(&mut |_, _| Err("connection/lost: 管道断了".to_string()), 0)
            .expect_err("RPC 抛异常 = 主干 Warning 横幅那一档");
        assert_eq!(
            error,
            UsageError::Failed("connection/lost: 管道断了".to_string()),
            "主干把 ex.Message 原样塞进横幅，分叉不许改写",
        );
    }

    #[test]
    fn mid_walk_page_failure_keeps_the_partial_aggregate() {
        let ledger = json!({"items": [
            {"sessionId": "d", "updatedAt": 1, "projections": {"asOfSeq": 6}},
            {"sessionId": "e", "updatedAt": 2, "projections": {"asOfSeq": 6}},
        ]});
        let mut pages = 0;
        let aggregate = collect_usage_stats(
            &mut |method, args| {
                if method == "session/list" {
                    return Ok(ledger.clone());
                }
                pages += 1;
                if args["request"]["address"]["sessionId"] == "d" {
                    return Ok(page(vec![message(6, TODAY * DAY_MS, 5, "p", "a")], false));
                }
                Err("session/notFound: 会话被删了".to_string())
            },
            0,
        )
        .expect("单会话失败不是整趟失败");
        assert_eq!(pages, 2, "第二行也要试（主干是逐会话 catch）");
        assert_eq!(
            (aggregate.sessions_scanned, aggregate.usage_messages),
            (2, 1),
            "报错那行仍算「走查过」，只是没数据进来 —— 已聚合的部分不许整段丢",
        );
        assert_eq!(aggregate.kpis(TODAY).total_tokens, 5);
        assert_eq!(aggregate.sessions.len(), 2);
    }
}

// ==================== #135 家 A/B：`step/start` / `step/end` 两臂的折叠本体 ====================
// 逐条对表主干 `MainWindow.RunStats.cs`（366 行那份）与 `MainWindow.Trajectory.cs`（家 B）。
// 本 mod 追加在**文件末尾**，规矩同 7740 那条：本仓的 `include_str!` 源码锁按「起点锚 →
// 终点锚的首次出现」开窗，插在被打锁的函数之前会抢掉终点锚。

/// 主干 `NumToString`（`RunStats.cs:197-200`）：`data[name]` **只有是数字**才算数，折成
/// `ToString("0")`（不变文化整数串）；缺键 / 字符串 / null 一律回 `None`。
///
/// 这颗是 #135 最容易假绿的一格（母本 §6 地雷 2）：`src/main.rs` 的共用入口把缺 turn 折成了
/// `0`（`event["data"]["turn"].as_i64().unwrap_or(0)`），照那个值折下去，桩 `--step=3` 那发
/// 孤立 `step/end`（`data` 里压根没有 turn）会凭白 `turns++` + `steps++`，而主干
/// `:136` 的 `if (turn is not null)` 挡住的正是**整段入账**。⇒ 折叠层自己按 Option 再读一次，
/// 不接壳侧那颗已经塌成 0 的坐标。
fn kw1_num_key(data: &Value, name: &str) -> Option<i64> {
    data.get(name)
        .and_then(Value::as_f64)
        .map(|number| number.round() as i64)
}

/// 主干 `RunStats.cs:72` 的 `time`：`TryGetProperty("time")` 且 `ValueKind == Number` 才
/// `(long)GetDouble()`（向零截断），否则 0。缺 `time` 的帧不是「0 时刻」而是「这一格读不出」，
/// 但主干确实把两种情况合成同一个 0 ⇒ 分叉同形（`llm_ms` 那几道 `> 0` 前置就是靠它挡倒挂）。
fn kw1_event_time(event: &Value) -> i64 {
    event
        .get("time")
        .and_then(Value::as_f64)
        .map(|number| number as i64)
        .unwrap_or(0)
}

/// 主干 `UsageField`（`RunStats.cs:191-195`）：`data.usage` 得是对象、该键得是数字、
/// 且 `GetDouble() >= 0` 才要（负数按「没有」，不是按 0）。
fn kw1_usage_field(data: &Value, name: &str) -> Option<i64> {
    data.get("usage")
        .filter(|usage| usage.is_object())?
        .get(name)
        .and_then(Value::as_f64)
        .filter(|number| *number >= 0.0)
        .map(|number| number as i64)
}

/// 主干 `FirstTokenFromStream`（`RunStats.cs:174-189`）：`data.stream` 是数组时，**第一个**
/// 「对象 + 带数字 `time`」的块就是首 token 时刻；后面的块不看。
fn kw1_first_token_from_stream(data: &Value) -> Option<i64> {
    data.get("stream")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|block| {
            block
                .as_object()
                .and_then(|piece| piece.get("time"))
                .and_then(Value::as_f64)
        })
        .next()
        .map(|number| number as i64)
}

/// 主干 `Str(data, "callId")`（`MainWindow.xaml.cs:9712`）：读不出来即空串。
fn kw1_str(data: &Value, name: &str) -> String {
    data.get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// 台账 #135 家 A：主干 `RunStatsState`（`MainWindow.RunStats.cs:27-47`）的**壳侧折叠**等价物
/// （不是内核投影 —— 内核那份是 `settings/describe` 的 `sessionStats`，见 [`SessionStats`]，
/// 两值在档 0 就是两份真相，登记在 kw1 报告 §5，这里不替它对齐）。
///
/// 字段公有 = 主干那 16 格 `public` 的逐字镜像；唯一例外是主干 `SystemPromptSeen`（`:42`）：
/// 它由 `MainWindow.xaml.cs:6095-6096` 在 `turn/end` 那案里写，**不在** `TrackRunStats` 的
/// switch 内 ⇒ 本结构不建模它（分叉那颗台账早就在 `Shell::system_prompt_seen`）。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunStepFold {
    /// 主干 `Turns`（`:29`）：只在 `step/end` 且「轮变了」才 +1（`:138-142`）。
    pub turns: i64,
    /// 主干 `Steps`（`:30`）：**只**在 `step/end` +1（`:143`），`step/start` 一格都不许碰。
    pub steps: i64,
    /// 主干 `LastTurn`（`:31`，`string?`）：上一发计过数的轮；`None` = 还没有。
    pub last_turn: Option<i64>,
    /// 主干 `LlmMs`（`:32`）：`assistant/message` 关步时按 `time - OpenStepStartMs` 累加。
    pub llm_ms: i64,
    /// 主干 `ToolMs`（`:33`）：`tool/result` 与配对的 `tool/call` 的时差。
    pub tool_ms: i64,
    /// 主干 `TtftMs` / `TtftSteps`（`:34-35`）：首 token 时差与其样本数。
    pub ttft_ms: i64,
    pub ttft_steps: i64,
    /// 主干 `DecodeMs` / `DecodeTokens`（`:36-37`）：首 token 到收尾那段与其产出。
    pub decode_ms: i64,
    pub decode_tokens: i64,
    /// 主干 `OutputTokens` / `TotalTokens` / `CacheReadTokens` / `InputTokens`（`:38-41`）。
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cache_read_tokens: i64,
    pub input_tokens: i64,
    /// 主干 `OpenStepTurn`（`:43`，`string?`）：**开步闸**的本体。`None` = 没有开着的步。
    pub open_step_turn: Option<i64>,
    /// 主干 `OpenStepStartMs`（`:44`）：开步那一刻的 `time`（关步后才清闸的是 turn，起点不清）。
    pub open_step_start_ms: i64,
    /// 主干 `FirstTokenMs`（`:45`，`long?`）：本轮首 token，`assistant/attempt` 置**一次**。
    pub first_token_ms: Option<i64>,
    /// 主干 `PendingCalls`（`:46`，`callId → 发出时刻`）：线性表留插入序，与分叉
    /// `Shell::mutations` 同一口径。
    pub pending_calls: Vec<(String, i64)>,
}

impl RunStepFold {
    /// 主干 `TrackRunStats`（`RunStats.cs:62-153`）的 switch 本体。
    /// 回 `true` = 主干尾部那句无条件 `UpdateRunStatsStrip()`（`:150`）该跑一发；
    /// `default: return`（`:147-148`「未跟踪事件不刷新条」）与 `data` 不是对象
    /// （`:67` 的前置门）都回 `false` = 条保持上次值。
    ///
    /// 主干整段 `catch (Exception) { }`（`:152`）在这层不需要对应物：这里的每一次读都是
    /// `Option` 判据，抛不出来。
    pub fn note_event(&mut self, event: &Value) -> bool {
        let Some(data) = event.get("data").filter(|data| data.is_object()) else {
            return false; // 主干 :67 `data.ValueKind != Object` ⇒ 整格不折、也不刷条
        };
        let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
        let time = kw1_event_time(event);
        let turn = kw1_num_key(data, "turn");
        match kind {
            "step/start" => {
                // 主干 :77-81。三条赋值**无条件**，包括把 turn 塌成 `None` 那一支（缺 turn 的
                // step/start 就是「把闸关掉」）。last-wins：连续两发只留后发的起点，主干
                // 没有「已经开着就不许再开」的闸 ⇒ 别顺手加（母本 §6 地雷 3）。
                self.open_step_turn = turn;
                self.open_step_start_ms = time;
                self.first_token_ms = None;
            }
            "assistant/attempt" => {
                // 主干 :82-87：`OpenStepTurn is not null && == turn && FirstTokenMs is null`
                // ⇒ 首 token 每步**只记一次**（第二发 attempt 不改它）。
                if self.open_step_turn.is_some() && self.open_step_turn == turn
                    && self.first_token_ms.is_none()
                {
                    self.first_token_ms = kw1_first_token_from_stream(data);
                }
            }
            "assistant/message" => {
                // 主干 :88-113：整段都在「开着的步且同轮」那道闸里，收尾 `OpenStepTurn = null`。
                if self.open_step_turn.is_some() && self.open_step_turn == turn {
                    if time > self.open_step_start_ms && self.open_step_start_ms > 0 {
                        self.llm_ms += time - self.open_step_start_ms;
                    }
                    let first = self.first_token_ms.or_else(|| kw1_first_token_from_stream(data));
                    let output = kw1_usage_field(data, "outputTokens");
                    if let Some(first) = first {
                        if first > self.open_step_start_ms && self.open_step_start_ms > 0 {
                            self.ttft_ms += first - self.open_step_start_ms;
                            self.ttft_steps += 1;
                        }
                        if let (Some(output), true) = (output, time > first) {
                            self.decode_ms += time - first;
                            self.decode_tokens += output;
                        }
                    }
                    self.open_step_turn = None;
                }
                // 主干 :112 在闸**外**：usage 四桶与开步闸无关，关不关步都落账。
                self.accumulate_usage(data);
            }
            "tool/call" => {
                // 主干 :114-120：`callId` 读得出来且 `time > 0` 才登记（重复 callId 后发覆盖）。
                let call_id = kw1_str(data, "callId");
                if !call_id.is_empty() && time > 0 {
                    match self
                        .pending_calls
                        .iter_mut()
                        .find(|(id, _)| *id == call_id)
                    {
                        Some(slot) => slot.1 = time,
                        None => self.pending_calls.push((call_id, time)),
                    }
                }
            }
            "tool/result" => {
                // 主干 :121-134：结果信封的 callId 在 `data.message.source.callId`（不是
                // `data.callId`）；配上了且不倒挂才计 `ToolMs`，然后**无条件**摘登记。
                let source = data
                    .get("message")
                    .filter(|message| message.is_object())
                    .and_then(|message| message.get("source"))
                    .filter(|source| source.is_object());
                if let Some(source) = source {
                    let call_id = kw1_str(source, "callId");
                    if let Some((_, dispatched)) = self
                        .pending_calls
                        .iter()
                        .find(|(id, _)| *id == call_id)
                        .cloned()
                    {
                        if !call_id.is_empty() && time > dispatched {
                            self.tool_ms += time - dispatched;
                        }
                        self.pending_calls.retain(|(id, _)| *id != call_id);
                    }
                }
            }
            "step/end" => {
                // 主干 :135-146。两处「主干就这么松」的口径，都是分叉容易顺手修歪的：
                // ① `turn is not null` 才入账 ⇒ **没有配对闸**，孤立 `step/end`（桩档 3）照计；
                // ② `Turns++` 只在轮号变了时才加 ⇒ 同轮两步（桩档 2）`turns` 加 1、`steps` 加 2；
                // ③ 尾部 `OpenStepTurn = null` 在 `if` **外面**：缺 turn 的那一发不开口计账，
                //    但照样把闸关掉。
                if turn.is_some() {
                    if self.last_turn != turn {
                        self.turns += 1;
                        self.last_turn = turn;
                    }
                    self.steps += 1;
                }
                self.open_step_turn = None;
            }
            _ => return false, // 主干 :147-148 `default: return`
        }
        true
    }

    /// 主干 `AccumulateUsage`（`:156-171`）：`usage` 不是对象整格跳过；`totalTokens` 缺失时
    /// 按 `input + output + cacheRead + cacheWrite` 口径补（与统计页一致）。
    fn accumulate_usage(&mut self, data: &Value) {
        if !data.get("usage").is_some_and(|usage| usage.is_object()) {
            return;
        }
        let total = kw1_usage_field(data, "totalTokens");
        let input = kw1_usage_field(data, "inputTokens");
        let output = kw1_usage_field(data, "outputTokens");
        let cache_read = kw1_usage_field(data, "cacheReadTokens");
        let cache_write = kw1_usage_field(data, "cacheWriteTokens");
        self.total_tokens += total.unwrap_or_else(|| {
            input.unwrap_or(0) + output.unwrap_or(0) + cache_read.unwrap_or(0) + cache_write.unwrap_or(0)
        });
        self.cache_read_tokens += cache_read.unwrap_or(0);
        self.input_tokens += input.unwrap_or(0);
        self.output_tokens += output.unwrap_or(0);
    }

    /// 主干 `UpdateRunStatsStrip`（`:224-228`）那颗可见性判据的纯函数版：
    /// 「无状态或 `Steps <= 0` ⇒ Collapsed」。**桩 `--step=4`（只发 start 不发 end）演的
    /// 就是这一格**：`llm_ms` 已经偷偷累计，条却必须整条收起 ⇒ 别拿 `llm_ms > 0` 当可见判据。
    #[must_use]
    pub fn strip_visible(&self) -> bool {
        self.steps > 0
    }
}

/// 主干 `_runStats`（`RunStats.cs:49`，按会话 id 分槽的字典）的等价物。
/// 用 `Vec` 而非 `HashMap`：分叉既有旁路表（`Shell::feedback` / `system_prompt_seen`）同口径，
/// 且枚举序 = 首现序，测试可直接比对。
#[derive(Clone, Debug, Default)]
pub struct RunStatsLedger {
    rows: Vec<(String, RunStepFold)>,
}

impl RunStatsLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 主干 `TrackRunStats` 的整段（含 `:66-70` 那两道前置：`sid` 空、`data` 不是对象）。
    /// 那两道门在主干是走在 `GetOrCreateRunStats`（`:71`）**之前**的 ⇒ 一帧坏 data 连台账格都
    /// 不建（`fold(sid)` 仍是 `None`）；未跟踪的名字才建格不折账。回 `true` = 该刷条。
    /// 分叉 reactor 每 tick 读一次返回值置脏即可，不必逐帧重绘。
    pub fn note(&mut self, session: &str, event: &Value) -> bool {
        if session.is_empty() || !event.get("data").is_some_and(|data| data.is_object()) {
            return false; // 主干 :67 `string.IsNullOrEmpty(sid) || data.ValueKind != Object`
        }
        self.entry(session).note_event(event)
    }

    /// 主干 `GetOrCreateRunStats`（`:51-59`）。
    fn entry(&mut self, session: &str) -> &mut RunStepFold {
        if let Some(index) = self.rows.iter().position(|(id, _)| id == session) {
            return &mut self.rows[index].1;
        }
        self.rows.push((session.to_string(), RunStepFold::default()));
        &mut self.rows.last_mut().expect("刚 push 过").1
    }

    #[must_use]
    pub fn fold(&self, session: &str) -> Option<&RunStepFold> {
        self.rows.iter().find(|(id, _)| id == session).map(|(_, row)| row)
    }

    /// 主干 `ResetRunStats`（`:203-211`）里**Remove 当前 sid** 那半边（`UpdateRunStatsStrip()`
    /// 那半边由调用方的重绘接管）。回 `true` = 确实摘掉了一格（清错会话当场可辨）。
    /// 主干「先清零、再由随后的完整 transcript 回放重建」是家 A 防回放翻倍的**唯一**手段
    /// （家 B 用的是 seq 去重，两套策略别统一）⇒ 清闸点漏一次，重放一遍 `steps` 就翻倍。
    pub fn forget(&mut self, session: &str) -> bool {
        let before = self.rows.len();
        self.rows.retain(|(id, _)| id != session);
        self.rows.len() != before
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// 该会话的步数读数（无该格 = 0），给「两路 steps 相等」这类判据用。
    #[must_use]
    pub fn steps(&self, session: &str) -> i64 {
        self.fold(session).map_or(0, |row| row.steps)
    }
}

/// 台账 #135 家 B：主干 `TrajectoryTurnTiming`（`MainWindow.Trajectory.cs:45-71`）里**步那三格**
/// 的等价物。账本行、按轮计时表与右侧面板本体是 #108 的宿主，这里只落折叠状态。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrajectoryStepFold {
    /// 主干 `Steps`（`:51`）：`step/end` 无条件 +1（`:334-337`）—— **不**计轮，
    /// 轮数只在 `turn/start`/`turn/end` 那两案改（`:217-223`）。
    pub steps: i64,
    /// 主干 `OpenStepKey`（`:63`，`string?`）：键是 **turn 的不变文化数字串**，不是 `(turn,step)`。
    pub open_step_key: Option<String>,
    /// 主干 `OpenStepStartMs`（`:64`）/ `FirstTokenMs`（`:65`）。
    pub open_step_start_ms: i64,
    pub first_token_ms: Option<i64>,
}

/// 主干 `_trajectories` 那一格（`TrajectoryState` `:93-98`：Entries / Turns / SeenSeqs）里
/// #135 用得上的两样：按轮的步折叠 + **seq 去重集**。
#[derive(Clone, Debug, Default)]
pub struct TrajectoryFold {
    turns: Vec<(i64, TrajectoryStepFold)>,
    seen_seqs: Vec<i64>,
}

impl TrajectoryFold {
    /// 主干 `TrajectoryObserve:164`（`if (envSeq > 0 && !st.SeenSeqs.Add(envSeq)) return;`）
    /// 加 `FoldTrajectoryTiming:196-203` 的 `turn <= 0 return`，再加 step 两臂
    /// （`:224-228`、`:334-337`）。
    /// 回 `true` = 这一帧进了折叠（主干同一处还要往账本里 Add 一行，宿主 = #108）。
    pub fn note_event(&mut self, event: &Value) -> bool {
        let seq = event.get("seq").and_then(Value::as_f64).map(|v| v as i64).unwrap_or(0);
        if seq > 0 {
            if self.seen_seqs.contains(&seq) {
                return false; // 页回放 + follow 双流：同一发只折一次
            }
            self.seen_seqs.push(seq);
        }
        let Some(data) = event.get("data").filter(|data| data.is_object()) else {
            return false;
        };
        // 家 B 的 turn 与家 A **不同型**：主干 `:160-161` 用 `TryGetInt32 ? number : 0` 把它
        // 塌成 int，再在 `:203` 用 `turn <= 0 return` 整帧丢弃 ⇒ `turn: 0` 的 step/end 家 B 不计、
        // 家 A 却照计（那边 `turn is not null` 对 0 成立）。两家的这道 asymmetry 由单测钉住，
        // 谁「顺手统一」谁改主干。
        let turn = kw1_num_key(data, "turn").unwrap_or(0);
        if turn <= 0 {
            return false;
        }
        let row = self.entry(turn);
        match event.get("type").and_then(Value::as_str).unwrap_or_default() {
            "step/start" => {
                row.open_step_key = Some(turn.to_string());
                row.open_step_start_ms = kw1_event_time(event);
                row.first_token_ms = None;
            }
            "step/end" => {
                row.steps += 1;
                row.open_step_key = None;
            }
            _ => {}
        }
        true
    }

    fn entry(&mut self, turn: i64) -> &mut TrajectoryStepFold {
        if let Some(index) = self.turns.iter().position(|(id, _)| *id == turn) {
            return &mut self.turns[index].1;
        }
        self.turns.push((turn, TrajectoryStepFold::default()));
        &mut self.turns.last_mut().expect("刚 push 过").1
    }

    #[must_use]
    pub fn steps(&self, turn: i64) -> i64 {
        self.turns
            .iter()
            .find(|(id, _)| *id == turn)
            .map_or(0, |(_, row)| row.steps)
    }

    #[must_use]
    pub fn open_step_key(&self, turn: i64) -> Option<&str> {
        self.turns
            .iter()
            .find(|(id, _)| *id == turn)
            .and_then(|(_, row)| row.open_step_key.as_deref())
    }
}

/// 家 B 的按会话分槽（主干 `TrajectoryState` 字典 `:100` + `TrimTrajectorySessions` 的上限 8
/// 那一档留给 #108：分叉这里不落面板，就没有「丢掉非当前会话账本」那条路径）。
#[derive(Clone, Debug, Default)]
pub struct TrajectoryLedger {
    rows: Vec<(String, TrajectoryFold)>,
}

impl TrajectoryLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 同 [`RunStatsLedger::note`]：空 sid 整格不建（主干 `TrajectoryObserve:155-158` 也是先取
    /// sid 再折），回 `true` = 这一帧进了折叠。
    pub fn note(&mut self, session: &str, event: &Value) -> bool {
        if session.is_empty() {
            return false;
        }
        if let Some(index) = self.rows.iter().position(|(id, _)| id == session) {
            return self.rows[index].1.note_event(event);
        }
        self.rows.push((session.to_string(), TrajectoryFold::default()));
        self.rows.last_mut().expect("刚 push 过").1.note_event(event)
    }

    #[must_use]
    pub fn fold(&self, session: &str) -> Option<&TrajectoryFold> {
        self.rows.iter().find(|(id, _)| id == session).map(|(_, row)| row)
    }

    /// 主干 `TrimTrajectorySessions`（`:178-186`「丢掉非当前会话的账本」）里 #135 用得上的
    /// 那半边：换屏时把**正要离开**那一格摘掉（家 B 防回放翻倍靠 seq 去重，这颗是防跨会话串账）。
    pub fn forget(&mut self, session: &str) -> bool {
        let before = self.rows.len();
        self.rows.retain(|(id, _)| id != session);
        self.rows.len() != before
    }

    #[must_use]
    pub fn steps(&self, session: &str, turn: i64) -> i64 {
        self.fold(session).map_or(0, |row| row.steps(turn))
    }
}

/// 主干 `TrajectoryKindOf`（`Trajectory.cs:400-408`）的账本**型标**分桶，整表照抄：
/// `step/start` / `step/end` 与 `turn/start` / `turn/end` 同落 `"step"` 桶 ⇒ 这正是
/// step/* **不许**进 `CHAT_DOMAIN_EVENTS`（吞帧表）的理由：主干会给它画一行账本。
#[must_use]
pub fn trajectory_kind_of(event_type: &str) -> &'static str {
    match event_type {
        "user/message" => "user",
        "assistant/message" | "assistant/attempt" | "assistant/live-chunk" => "assistant",
        "tool/call" | "tool/result" | "deliverables/presented" => "tool",
        "turn/start" | "turn/end" | "step/start" | "step/end" => "step",
        "system/message" | "request/header" | "request/context" | "session/title" => "system",
        _ => "other",
    }
}

/// 主干 `TrajectoryLabel`（`:441-448`）里 step 那两格的 **zh/en 模板对**（`{0}` = `TurnOrDash`）。
/// 宿主（右侧轨迹面板 #108）拿它去喂 `Catalog::dtf`：这两族主干压根没进 `ShellEnglish`，
/// 走 `l`/`lf` 那套通用 EN 表在非中文界面必露中文（`i18n.rs:1744-1757` 的既有理由）。
/// 这里只出模板、不出渲染结果：i18n 归 `src/i18n.rs`，本轮禁改，也不该在纯折叠层建第二份文案。
#[must_use]
pub fn trajectory_step_label(event_type: &str) -> Option<(&'static str, &'static str)> {
    match event_type {
        "step/start" => Some(("步骤开始 · 第 {0} 轮", "Step start · Turn {0}")),
        "step/end" => Some(("步骤结束 · 第 {0} 轮", "Step end · Turn {0}")),
        _ => None,
    }
}

// ==================== #135 折叠判据（母本 §6 判据 1 的十格 + 家 B 两臂 + 型标） ====================
#[cfg(test)]
mod step_fold_tests {
    use super::*;

    /// 一帧 `$events` waterfall（`{type, seq, time, data}`）。`data` 由用例自己给，
    /// 因为「缺 turn」这一格（判据 9 与地雷 2）必须能构造出**没有该键**的对象。
    fn frame(kind: &str, seq: i64, time: i64, data: Value) -> Value {
        json!({"type": kind, "seq": seq, "time": time, "data": data})
    }

    fn start(turn: i64, time: i64) -> Value {
        frame("step/start", 1, time, json!({"turn": turn, "step": 1}))
    }

    fn end(turn: i64, time: i64) -> Value {
        frame("step/end", 2, time, json!({"turn": turn, "step": 1}))
    }

    fn message(turn: i64, time: i64, output: i64) -> Value {
        frame(
            "assistant/message",
            3,
            time,
            json!({"turn": turn, "usage": {"outputTokens": output}}),
        )
    }

    fn attempt(turn: i64, time: i64, first_token: i64) -> Value {
        frame(
            "assistant/attempt",
            4,
            time,
            json!({"turn": turn, "stream": [{"time": first_token, "type": "text-delta"}]}),
        )
    }

    /// 判据 1：一轮完整的开步→关步→计步，`llm_ms` 由开步起点折出。
    #[test]
    fn one_open_step_books_llm_ms_and_counts_one_turn_one_step() {
        let mut fold = RunStepFold::default();
        assert!(fold.note_event(&start(1, 100)));
        assert!(fold.note_event(&message(1, 900, 100)));
        assert!(fold.note_event(&end(1, 950)));
        assert_eq!((fold.llm_ms, fold.steps, fold.turns), (800, 1, 1));
        assert_eq!(fold.open_step_turn, None, "关步那发（:110）没把闸合上");
        assert_eq!(fold.last_turn, Some(1));
        assert!(fold.strip_visible(), "有步 ⇒ 条可见（主干 :224-228）");
    }

    /// 判据 2：**主干没有配对闸**。孤立 `step/end`（桩 `--step=3` 的负形）照样计轮计步。
    /// 分叉一旦「顺手」加配对闸就是改主干，这条当场翻红。
    #[test]
    fn an_orphan_step_end_counts_because_the_trunk_has_no_pairing_gate() {
        let mut fold = RunStepFold::default();
        assert!(fold.note_event(&end(1, 950)));
        assert_eq!((fold.steps, fold.turns), (1, 1));
        assert_eq!(fold.llm_ms, 0, "没开过步就没有 llm 时长，但计数不受影响");
    }

    /// 判据 3：两发 `step/start` 无 end ⇒ **last-wins 覆盖起点**，且 `steps` 恒 0
    /// （桩 `--step=4`：`llm_ms` 已偷偷累计、条却 Collapsed 的那一档）。
    #[test]
    fn a_second_start_overwrites_the_origin_and_steps_stay_zero() {
        let mut fold = RunStepFold::default();
        fold.note_event(&start(1, 100));
        fold.note_event(&frame("step/start", 9, 700, json!({"turn": 1, "step": 2})));
        assert!(fold.note_event(&message(1, 900, 10)));
        assert_eq!(fold.llm_ms, 200, "起点必须是后发的 700，不是首发 100");
        assert_eq!(fold.steps, 0, "steps 只在 step/end 涨（地雷 1）");
        assert!(!fold.strip_visible(), "Steps<=0 ⇒ 条 Collapsed，哪怕 llm_ms 已经非零");
    }

    /// 判据 4：没有开着的步 ⇒ `assistant/message` 一格时长都不许入账（`:89` 那道闸）。
    #[test]
    fn a_message_without_an_open_step_moves_no_timing() {
        let mut fold = RunStepFold::default();
        assert!(fold.note_event(&message(1, 900, 100)));
        assert_eq!(
            (fold.llm_ms, fold.ttft_ms, fold.ttft_steps, fold.decode_ms),
            (0, 0, 0, 0)
        );
        assert_eq!(fold.open_step_turn, None);
        // usage 四桶在闸**外**（主干 :112）：这一格照样落账，别把整案一起闸掉。
        assert_eq!(fold.output_tokens, 100);
    }

    /// 判据 5：首 token **只记一次**（`:83` 的 `FirstTokenMs is null`）⇒ 第二发 attempt 不改它。
    #[test]
    fn the_first_token_is_recorded_once_per_open_step() {
        let mut fold = RunStepFold::default();
        fold.note_event(&start(1, 100));
        fold.note_event(&attempt(1, 200, 200));
        fold.note_event(&attempt(1, 300, 300));
        assert_eq!(fold.first_token_ms, Some(200), "后发的 attempt 不许把首 token 推后");
        assert!(fold.note_event(&message(1, 900, 100)));
        assert_eq!((fold.ttft_ms, fold.ttft_steps), (100, 1), "TTFT 样本数必须是 1，不是 2");
        assert_eq!((fold.decode_ms, fold.decode_tokens), (700, 100));
    }

    /// 判据 6：两轮各一步 ⇒ 轮步同数（`RunStats.cs:138-142` 的「轮变化才计轮」）。
    #[test]
    fn two_turns_count_two_turns_and_two_steps() {
        let mut fold = RunStepFold::default();
        for turn in 1..=2 {
            fold.note_event(&start(turn, turn * 100));
            fold.note_event(&end(turn, turn * 100 + 50));
        }
        assert_eq!((fold.turns, fold.steps), (2, 2));
    }

    /// 判据 7（桩 `--step=2` 的内核真形）：同轮两步 ⇒ `turns` 只加 1、`steps` 加 2。
    #[test]
    fn two_steps_in_one_turn_add_one_turn_and_two_steps() {
        let mut fold = RunStepFold::default();
        for step in 1..=2 {
            fold.note_event(&frame("step/start", step as i64, 100 * step, json!({"turn": 1, "step": step})));
            fold.note_event(&frame("step/end", step as i64 + 8, 100 * step + 50, json!({"turn": 1, "step": step})));
        }
        assert_eq!((fold.turns, fold.steps), (1, 2), "轮号没变就不许再计轮");
    }

    /// 判据 8：时间倒挂与起点为 0 都被两道前置挡住（`:91`、`:99`、`:104`）。
    #[test]
    fn a_zero_or_reversed_clock_adds_no_duration() {
        let mut fold = RunStepFold::default();
        fold.note_event(&start(1, 0));
        fold.note_event(&message(1, 500, 100));
        assert_eq!((fold.llm_ms, fold.ttft_ms, fold.decode_ms), (0, 0, 0));

        let mut reversed = RunStepFold::default();
        reversed.note_event(&start(1, 900));
        reversed.note_event(&message(1, 500, 100));
        assert_eq!(reversed.llm_ms, 0, "time > start 那一半也得在");
        assert_eq!(reversed.open_step_turn, None, "关步不看时钟对不对，闸照样合");
    }

    /// 判据 9 + 地雷 2：`data` 里**没有 turn** ⇒ 家 A 整段入账被 `:136` 挡住（0/0），
    /// 但 `:145` 那句 `OpenStepTurn = null` 在 `if` 外面 ⇒ 关步仍然生效。
    #[test]
    fn a_step_end_without_a_turn_books_nothing_yet_still_closes_the_gate() {
        let mut fold = RunStepFold::default();
        fold.note_event(&start(1, 100));
        assert_eq!(fold.open_step_turn, Some(1));
        assert!(fold.note_event(&frame("step/end", 7, 900, json!({"step": 1}))));
        assert_eq!((fold.turns, fold.steps), (0, 0), "缺 turn 被折成 0 就会凭白 +1 步");
        assert_eq!(fold.open_step_turn, None);
        // 同一条帧在「turn 是 0」那一支反而**要**入账：主干判据是 `is not null`，不是 `> 0`。
        let mut zero = RunStepFold::default();
        assert!(zero.note_event(&frame("step/end", 7, 900, json!({"turn": 0, "step": 1}))));
        assert_eq!((zero.turns, zero.steps), (1, 1));
        assert_eq!(zero.last_turn, Some(0), "主干把 turn 0 当正常轮号存进 LastTurn");
    }

    /// 判据 10：清闸只 Remove 当前会话（`:203-211`），别的会话读数一格不动。
    #[test]
    fn forgetting_a_session_clears_only_that_session() {
        let mut ledger = RunStatsLedger::new();
        for turn in 1..=2 {
            ledger.note("A", &start(turn, turn * 100));
            ledger.note("A", &end(turn, turn * 100 + 50));
            ledger.note("B", &start(turn, turn * 100));
            ledger.note("B", &end(turn, turn * 100 + 50));
        }
        assert_eq!(ledger.steps("A"), 2);
        assert!(ledger.forget("A"));
        assert!(ledger.fold("A").is_none(), "清闸没生效 ⇒ 回放一次就翻倍");
        assert_eq!(ledger.steps("B"), 2, "主干只 Remove 当前 sid ⇒ 另一条不许受影响");
        assert!(!ledger.forget("A"), "再清一次是空操作（分叉靠这个返回值辨「清错会话」）");
        assert!(!ledger.note("", &start(1, 100)), "空 sid ⇒ 主干 :67 整段不折");
        assert!(!ledger.note("C", &json!({"type": "step/end"})), "data 不是对象也整段不折");
        assert!(ledger.fold("C").is_none());
    }

    /// 未跟踪的名字 ⇒ 回 `false`（主干 `:147-148` `default: return`，「未跟踪事件不刷新条」）。
    #[test]
    fn untracked_names_ask_for_no_repaint() {
        let mut fold = RunStepFold::default();
        assert!(!fold.note_event(&frame("turn/start", 1, 100, json!({"turn": 1}))));
        assert!(!fold.note_event(&frame("user/message", 1, 100, json!({"turn": 1}))));
        assert!(!fold.note_event(&json!({"type": 7})), "type 读不出来也归未跟踪");
    }

    /// `tool/call` → `tool/result` 配对（`:114-134`）：callId 在**结果信封的
    /// `data.message.source.callId`**，不在 `data.callId`；配不上不倒挂都不计，摘登记无条件。
    #[test]
    fn paired_tool_calls_accumulate_tool_ms() {
        let mut fold = RunStepFold::default();
        assert!(fold.note_event(&frame("tool/call", 5, 200, json!({"turn": 1, "callId": "c1"}))));
        assert_eq!(fold.pending_calls, vec![("c1".to_string(), 200)]);
        assert!(fold.note_event(&frame(
            "tool/result",
            6,
            700,
            json!({"turn": 1, "message": {"source": {"callId": "c1"}}})
        )));
        assert_eq!(fold.tool_ms, 500);
        assert!(fold.pending_calls.is_empty(), "结果到了就该摘登记");
        // 没配对上的结果（桩外来的、或重复回放的第二发）不加时长、也不报错。
        fold.note_event(&frame("tool/result", 7, 900, json!({"message": {"source": {"callId": "c1"}}})));
        assert_eq!(fold.tool_ms, 500);
        // `time > 0` 才登记（:116）；usage 的负数按「没有」（:193）。
        fold.note_event(&frame("tool/call", 8, 0, json!({"callId": "c2"})));
        assert!(fold.pending_calls.iter().all(|(id, _)| id != "c2"));
        fold.note_event(&frame(
            "assistant/message",
            9,
            900,
            json!({"turn": 1, "usage": {"outputTokens": -5, "inputTokens": 7}}),
        ));
        assert_eq!((fold.output_tokens, fold.input_tokens), (0, 7));
    }

    /// 家 B 两臂（`Trajectory.cs:224-228`、`:334-337`）：**计步不计轮**，
    /// 开步键是 turn 的不变文化数字串（不是 `(turn,step)`），且**按 seq 去重**不翻倍。
    #[test]
    fn trajectory_step_arms_fold_per_turn_and_dedup_by_seq() {
        let mut ledger = TrajectoryLedger::new();
        assert!(ledger.note("A", &start(1, 100)));
        assert_eq!(ledger.fold("A").map_or(0, |f| f.steps(1)), 0, "steps 只在 end 涨");
        // 回放翻倍闸（`TrajectoryObserve:164`）：同一 seq 的第二发**整帧**不折
        // ⇒ 页回放 + follow 双流不重复计数（与家 A 的「先清零再重建」是两套策略，别统一）。
        assert!(
            !ledger.note("A", &start(1, 100)),
            "同 seq 的第二发必须被去重闸挡下"
        );
        assert_eq!(ledger.fold("A").and_then(|f| f.open_step_key(1)), Some("1"));
        ledger.note("A", &frame("step/end", 11, 150, json!({"turn": 1, "step": 1})));
        ledger.note("A", &frame("step/end", 12, 250, json!({"turn": 2, "step": 1})));
        assert_eq!(
            (ledger.steps("A", 1), ledger.steps("A", 2)),
            (1, 1),
            "按轮分格：主干那颗表就是 `Dictionary<int, TrajectoryTurnTiming>`"
        );
        // 去重闸对**已折过的那一发 `step/end`** 同样生效：整页重放不涨计数。
        let before = ledger.steps("A", 1);
        assert!(
            !ledger.note("A", &frame("step/end", 11, 150, json!({"turn": 1, "step": 1}))),
            "同 seq 的重放必须被去重闸挡下"
        );
        assert_eq!(ledger.steps("A", 1), before);
        // 与家 A 的**不对称**（`:160-163` 折 int、`:203` 的 `turn <= 0 return`）：
        // 缺 turn / turn 为 0 的 step/end 家 B 整帧丢弃，家 A 对 turn 0 却照计。
        let mut no_turn = TrajectoryLedger::new();
        assert!(!no_turn.note("A", &frame("step/end", 77, 900, json!({"step": 1}))));
        assert!(!no_turn.note("A", &frame("step/end", 78, 900, json!({"turn": 0, "step": 1}))));
        assert_eq!(no_turn.steps("A", 0), 0);
        assert!(!no_turn.note("", &start(1, 100)), "空 sid 整格不建");
    }

    /// 型标与文案模板（`:405` / `:445`、`:447`）：step 族与 turn 族同落 `"step"` 桶
    /// ⇒ 这就是 step/* **不许**进吞帧表的理由（主干会给它画一行账本）。
    #[test]
    fn the_kind_bucket_and_the_label_templates_match_the_trunk() {
        assert_eq!(trajectory_kind_of("step/start"), "step");
        assert_eq!(trajectory_kind_of("step/end"), "step");
        assert_eq!(trajectory_kind_of("turn/start"), "step");
        assert_eq!(trajectory_kind_of("turn/end"), "step");
        assert_eq!(trajectory_kind_of("step/end"), "step");
        assert_eq!(trajectory_kind_of("assistant/live-chunk"), "assistant");
        assert_eq!(trajectory_kind_of("tool/result"), "tool");
        assert_eq!(trajectory_kind_of("session/title"), "system");
        assert_eq!(trajectory_kind_of("whatever-else"), "other");
        assert_eq!(
            trajectory_step_label("step/start"),
            Some(("步骤开始 · 第 {0} 轮", "Step start · Turn {0}"))
        );
        assert_eq!(
            trajectory_step_label("step/end"),
            Some(("步骤结束 · 第 {0} 轮", "Step end · Turn {0}"))
        );
        assert!(trajectory_step_label("turn/start").is_none(), "轮那两格不是本片的文案（#108）");
    }
}

// ==================== #57 容量圈：真弧的**弦近似**几何（宿主渲染侧唯一需要的纯算子） ====================
//
// 主干那颗弧是 `Path` + `ArcSegment`（`MainWindow.ContextMeter.cs:193-207`、`MainWindow.xaml:1318`）。
// 本机 `windows-reactor-0.100.0` 的公开形状面只有 `Rectangle` / `Ellipse` / `Line` / `PathIcon`
// （`src/generated.rs` 现测：`Ellipse:3045`、`Line:3102`、`Canvas:5174`），**没有** `Path`、
// 没有 `PathGeometry` / `ArcSegment` / `Geometry` ⇒ 真弧画不出来。
//
// 于是宿主只能把 `RingFace::Arc` 那一段折成 N 枚 `Line` 弦。这里只出**坐标**，不碰 `View`：
// 渲染侧在 `main.rs` 的 `Shell::context_meter_cell` 里把这些线段挂进 `Canvas`。
// 为什么住在 `kernel.rs` 而不是 `contextmeter.rs`：算子层是本轮的**只读**面（它描述几何、
// 不描述近似），而近似是宿主形态的产物；本仓纯几何只有这一层能放（`layout.rs` / `theme.rs`
// 都不归本片写）。
//
// 端点公式与主干 `CM:197-203` 逐字同源：12 点起、顺时针、`(center + r·sin θ, center − r·cos θ)`，
// 扫过角走算子层那一颗 `contextmeter::sweep_radians`（`CM:194` 的 `p × π / 50`）——
// 不在这里重算，免得第二份真相。

/// 弦近似的默认段数：主干那枚 viewBox 是 14×14 的 12 点刻度盘，折 12 段 ⇒ 每段 30°，
/// 与「12 点顺时针」的几何同一套分度（近似误差在 6 DIP 半径上 < 0.23 DIP）。
pub const MW3_RING_CHORD_SEGMENTS: usize = 12;

/// 圆周上一点：`θ` 从 12 点起顺时针计（弧度）。主干 `CM:201-203` 的端点公式。
#[must_use]
pub fn mw3_ring_point(angle_radians: f64) -> (f64, f64) {
    (
        crate::contextmeter::RING_CENTER + crate::contextmeter::RING_RADIUS * angle_radians.sin(),
        crate::contextmeter::RING_CENTER - crate::contextmeter::RING_RADIUS * angle_radians.cos(),
    )
}

/// percent → N 枚弦段 `[x1, y1, x2, y2]`（14×14 视图坐标）。
///
/// 三条与主干 `SetContextMeterRing` 同形的收起判据：
/// · `percent <= 0` ⇒ 空表（主干 `CM:187` 的 `p is > 0 and < 100` 不成立 ⇒ 弧 `Collapsed`，
///   只剩轨道那枚 `Ellipse`）；
/// · `percent >= 100` ⇒ **同样**空表（主干 `CM:189` 走另一颗整圆 `Ellipse`，因为「弧起终重合」
///   是退化几何 —— 弦近似要是把 100% 折成一圈，第一枚与最后一枚端点重合，就是那个不确定形态）；
/// · `segments == 0` ⇒ 空表（不除零）。
///
/// 中间点一律从**扫过角**推，不从上一段的终点累加 ⇒ 误差不随段数累积，末点恒等
/// `contextmeter::face_of(percent)` 给的 `ArcGeometry.end_*`。
#[must_use]
pub fn mw3_ring_chords(percent: i32, segments: usize) -> Vec<[f64; 4]> {
    let sweep = crate::contextmeter::sweep_radians(percent);
    if sweep <= 0.0 || percent >= crate::contextmeter::FULL_PERCENT || segments == 0 {
        return Vec::new();
    }
    let step = sweep / segments as f64;
    (0..segments)
        .map(|i| {
            let (x1, y1) = mw3_ring_point(step * i as f64);
            let (x2, y2) = mw3_ring_point(step * (i + 1) as f64);
            [x1, y1, x2, y2]
        })
        .collect()
}

#[cfg(test)]
mod mw3_ring_geometry_tests {
    use super::*;
    use crate::contextmeter::{RING_CENTER, RING_RADIUS, face_of};

    /// 判据①：只有 `RingFace::Arc` 那一档出弦，`TrackOnly` / `Full` 都出空表
    /// ⇒ 「缺分母 ⇒ 画空而不是一圈红」这条硬闸在几何层也有对应物（0% 与 100% 都不画弧）。
    #[test]
    fn only_the_partial_face_emits_chords() {
        assert!(mw3_ring_chords(0, MW3_RING_CHORD_SEGMENTS).is_empty(), "0% = 只剩轨道");
        assert!(mw3_ring_chords(-7, MW3_RING_CHORD_SEGMENTS).is_empty());
        assert!(mw3_ring_chords(100, MW3_RING_CHORD_SEGMENTS).is_empty(), "满值走整圆");
        assert!(mw3_ring_chords(4000, MW3_RING_CHORD_SEGMENTS).is_empty());
        assert_eq!(mw3_ring_chords(50, 0).len(), 0, "段数 0 不许除零");
        assert_eq!(mw3_ring_chords(50, 3).len(), 3);
        assert_eq!(mw3_ring_chords(50, MW3_RING_CHORD_SEGMENTS).len(), 12);
    }

    /// 判据②：首段起点恒在 12 点，末段终点与算子层 `face_of` 给的真弧终点**逐字同值**
    /// ⇒ 近似只改「中间怎么走」，不改两端在哪。
    #[test]
    fn the_endpoints_match_the_operator_layer_arc() {
        for percent in [1, 12, 38, 50, 51, 99] {
            let chords = mw3_ring_chords(percent, MW3_RING_CHORD_SEGMENTS);
            let first = chords.first().expect("非空");
            assert!((first[0] - RING_CENTER).abs() < 1e-12, "1% 起点的 x 该在圆心");
            assert!((first[1] - (RING_CENTER - RING_RADIUS)).abs() < 1e-12, "起点在 12 点");
            let last = chords.last().expect("非空");
            let (gx, gy) = ring_face_end(percent);
            assert!(
                (last[2] - gx).abs() < 1e-9 && (last[3] - gy).abs() < 1e-9,
                "{percent}% 的弦末点没落在真弧终点上：{last:?} vs {gx},{gy}"
            );
        }
    }

    /// 判据③：每一枚端点都**在圆上**（离圆心恰好一个半径）⇒ 近似是内接折线，不会鼓出盒子。
    /// 主干注释自己钉的就是这条（`CM:150-152`「描边带落在 5..7，不出盒」）。
    #[test]
    fn every_chord_endpoint_sits_on_the_ring_radius() {
        for percent in [1, 25, 50, 75, 99] {
            for segment in mw3_ring_chords(percent, MW3_RING_CHORD_SEGMENTS) {
                for (x, y) in [(segment[0], segment[1]), (segment[2], segment[3])] {
                    let dx = x - RING_CENTER;
                    let dy = y - RING_CENTER;
                    assert!(
                        (dx.hypot(dy) - RING_RADIUS).abs() < 1e-9,
                        "{percent}% 上有点离圆心 {dx:?},{dy:?} ≠ 半径"
                    );
                }
            }
        }
    }

    /// 判据④：顺时针 —— 扫过角越大，x 先增后减、y 单调增到 6 点后继续绕。
    /// 钉两个已知方位：25% 的末点在 3 点、50% 的末点在 6 点（主干 `CM:201-203` 同判据）。
    #[test]
    fn the_chords_sweep_clockwise() {
        let quarter = mw3_ring_chords(25, MW3_RING_CHORD_SEGMENTS).last().expect("非空").to_vec();
        assert!((quarter[2] - (RING_CENTER + RING_RADIUS)).abs() < 1e-9, "25% → 3 点");
        assert!((quarter[3] - RING_CENTER).abs() < 1e-9);
        let half = mw3_ring_chords(50, MW3_RING_CHORD_SEGMENTS).last().expect("非空").to_vec();
        assert!((half[2] - RING_CENTER).abs() < 1e-9, "50% → 6 点");
        assert!((half[3] - (RING_CENTER + RING_RADIUS)).abs() < 1e-9);
    }

    /// 只在本文件里用的适配器：把算子层的弧档拆成（终点 x, 终点 y），
    /// 免得测试为了拿终点去 import 一整枚举。非弧档直接 panic（判据②只喂 1..=99）。
    fn ring_face_end(percent: i32) -> (f64, f64) {
        match face_of(percent) {
            crate::contextmeter::RingFace::Arc(g) => (g.end_x, g.end_y),
            other => panic!("非弧档：{other:?}"),
        }
    }
}

// =====================================================================================
// 台账 #144 · `dynamicCordisRunner/*` 的**客户端层**（KW2 · 批 B1）
//
// 这一节只有两件事：**args 构造**（主干发出那一发时长什么样）与 **inventory 回帧投影**
// （主干读那一发时怎么折）。发出时机、面板/审批卡宿主、i18n 全在 B3（`main.rs`）——
// 本文件不碰它们，也不碰 `main.rs`。
//
// 真值来源（本轮逐字回源码复核，行号 = 现工作树）：
//   · 主干 args：`MainWindow.Cordis.cs:202 / 324-332 / 345 / 352 / 358 / 364 / 371 / 396`
//   · 主干回帧读法：`MainWindow.Cordis.cs:233-295`（`ParseCordisRow`）＋ `:204-218`（数组前置门、排序）
//   · 主干事件 → 审批记录：`MainWindow.Cordis.cs:114-151`（取格 `:118/119/131-136`、闸门 `:126`）
//   · 主干对账谓词：`MainWindow.Cordis.cs:298-317`（`StillPending` 在 `:300-301`）
//   · 取串口径：`MainWindow.Capabilities.cs:114-116` `CapabilityString` —— 容器不是对象 / 键不存在 /
//     键不是串，三态一律塌成**空串**（不是 null、不抛）
//   · 键集闸：`@deepseek-ai/dsh-api-gateway/lib/types/index.js:795-822` `assertExactArguments` ——
//     按**键名集合**比对（`Reflect.ownKeys`），键序无关；多一颗 `unexpected`、少一颗 `missing` 都拒
//   · 可选标记：`@deepseek-ai/dsh-cordis-host-runner/lib/typert.remote-client.js:18-60`
//     （inventory 行的 `currentPackageId`/`nextPackageId`/`activeRun`/`latestRun` 四颗 `.optional()`；
//     `latestRun` 内的 `approvalRequestId`/`requiresApproval`/`error` 三颗 `.optional()`；
//     `packages[]` 的五颗**全必填**）；`runHostHalf` 的 `requestId` 是
//     `z.union([z.literal(null), string])`（`:126`）且那颗 descriptor（`:583-592`）没给
//     `acceptsUndefined` —— 整座 descriptor 文件里 `acceptsUndefined` **0 命中** ⇒ 本族八发的
//     每一颗键都不可省（`assertExactArguments` 的 `acceptsMissing` 集合是空表）
//     ⇒ **键必须在、值可为 null**
//   · 省略式产帧：`@deepseek-ai/dsh-cordis-host-runner/lib/index.js:1943-1956` ——
//     `...plugin.currentPackageId === void 0 ? {} : { currentPackageId: … }`、`activeRun`/`latestRun`
//     同型 ⇒ 不适用时**整颗键不存在**，内核从不写 `"activeRun": null`
//   · 信封与 null 的序列化：`Dsh/DshRpcClient.cs:88-96` 拼 `{type,rpcId,method,payload:{args}}`，
//     `JsonSerializer.Serialize(envelope)` **无** `JsonSerializerOptions`（全文件零处
//     `DefaultIgnoreCondition`）⇒ C# 的 `string? requestId = null` 会**字面写出** `"requestId":null`
//
// 「主干不发的那几面」：`MainWindow.Cordis.cs` 里另有四颗包装（`:383 / :405 / :412 / :419`）
// **除定义行外全域零调用方**（`invoke` / `resolveInspectQuery` / `reportClientGuardFailure` /
// `reportRenderFailure`），且 `:379-404 / :411 / :418` 的注释明文禁止桌面壳伪造它们。
// 本节**不**为它们建常量、构造器或任何臂 —— 判据由 `kw2_cordis_tests` 里的源码反向锁钉住。
// =====================================================================================

/// `dynamicCordisRunner/inventory`（主干 `MainWindow.Cordis.cs:202`，`args` 是 `new { }`）。
pub const CORDIS_INVENTORY: &str = "dynamicCordisRunner/inventory";
/// `dynamicCordisRunner/runHostHalf`（`:324-332`）。
pub const CORDIS_RUN_HOST_HALF: &str = "dynamicCordisRunner/runHostHalf";
/// `dynamicCordisRunner/resolveRequestRun`（`:345`）。
pub const CORDIS_RESOLVE_REQUEST_RUN: &str = "dynamicCordisRunner/resolveRequestRun";
/// `dynamicCordisRunner/settleUserRun`（`:352`）。
pub const CORDIS_SETTLE_USER_RUN: &str = "dynamicCordisRunner/settleUserRun";
/// `dynamicCordisRunner/stopFromPanel`（`:358`）。
pub const CORDIS_STOP_FROM_PANEL: &str = "dynamicCordisRunner/stopFromPanel";
/// `dynamicCordisRunner/undefineFromPanel`（`:364`）。
pub const CORDIS_UNDEFINE_FROM_PANEL: &str = "dynamicCordisRunner/undefineFromPanel";
/// `dynamicCordisRunner/getClientCode`（`:371`）。
pub const CORDIS_GET_CLIENT_CODE: &str = "dynamicCordisRunner/getClientCode";
/// `dynamicCordisRunner/syncInspectManifest`（`:396`）。
///
/// **构造器在册、发信路径不接**：主干唯一的调用方是 boot 那一发 `_ = SyncCordisInspectManifestAsync(…)`
/// （`:109`，由 `AttachCordisEvents` 发），它既丢弃 Task、包装内又 `catch{}` 吞异常（`:397`）、
/// 回帧还是 `z.literal(null)`（`typert.remote-client.js:191`）无人读 ⇒ 接不接都不产生可观测差异。
/// B3 若照抄 boot，就把这一发顺带发掉；不照抄也不算缺。
pub const CORDIS_SYNC_INSPECT_MANIFEST: &str = "dynamicCordisRunner/syncInspectManifest";

/// 八发的名单（与上面八颗常量一字不差）。给 `tests/ipc.rs` 与本文件的反向锁当基数：
/// **恰八颗**，且不含主干零调用方的那四面。
pub const CORDIS_METHODS: [&str; 8] = [
    CORDIS_INVENTORY,
    CORDIS_RUN_HOST_HALF,
    CORDIS_RESOLVE_REQUEST_RUN,
    CORDIS_SETTLE_USER_RUN,
    CORDIS_STOP_FROM_PANEL,
    CORDIS_UNDEFINE_FROM_PANEL,
    CORDIS_GET_CLIENT_CODE,
    CORDIS_SYNC_INSPECT_MANIFEST,
];

/// 主干 `CordisPluginRow.Status` 里唯一有**行为后果**的那一格（`:282/:301/:691/:740`）：
/// 待审 ⇒ 排序进前段、审批卡不收、`CordisRunStop` 禁能。值本身是主干 `:282` 的字面量。
pub const CORDIS_STATUS_AWAITING: &str = "awaiting-approval";

/// 主干 `CapabilityString`（`MainWindow.Capabilities.cs:114-116`）。
/// 三态（容器非对象 / 缺键 / 键非串）全塌成空串；**不抛**。
fn kw2_capability_string(container: &Value, key: &str) -> String {
    container
        .as_object()
        .and_then(|record| record.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// 主干 `TryGetProperty(k, out v) && v.ValueKind == JsonValueKind.True`（`:255/256/263/264/119`）：
/// **只有字面 `true` 算真**。缺键 / `false` / `null` / `"true"` / `1` 全算假 —— 这三态里任何一态
/// 被写成「补默认」都会把主干的省略式投影读成实值。
fn kw2_is_literal_true(container: &Value, key: &str) -> bool {
    container
        .as_object()
        .and_then(|record| record.get(key))
        .is_some_and(|value| value == &Value::Bool(true))
}

/// 主干 `:276-277` 那一型的可空读法：`TryGetProperty && ValueKind == String ? GetString() : null`。
/// 与 [`kw2_capability_string`] 的分别是**本体的**：缺键 ⇒ `None`（「这格不适用」），不是 `Some("")`。
fn kw2_optional_string(container: &Value, key: &str) -> Option<String> {
    container
        .as_object()
        .and_then(|record| record.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// 主干 `ParseCordisRow:244-266` 里那**两段赋值体**（同一段在两个 `if` 里各写一遍，`:252-256`
/// 与 `:260-264` 逐字相同）：整颗覆盖，无论文本长短、无论文本是否为空。
fn kw2_take_package(row: &mut CordisRow, pkg: &Value, pid: &str) {
    row.package_id = pid.to_string();
    row.name = kw2_capability_string(pkg, "name");
    row.purpose = kw2_capability_string(pkg, "purpose");
    row.has_client_half = kw2_is_literal_true(pkg, "hasClientHalf");
    row.has_host_half = kw2_is_literal_true(pkg, "hasHostHalf");
}

/// inventory 的一行 = 主干 `CordisPluginRow`（`MainWindow.Cordis.cs:50-67`）的逐字镜像。
///
/// 字段序、缺省值、可空性三样都跟主干：
/// · `plugin_id`/`agent_id`/`name`/`purpose`/`package_id`/`current_package_id`/`next_package_id`
///   是 `String`（主干 `= ""` 的 `string`）；
/// · `active_run_id`/`active_package_id`/`approval_request_id`/`latest_error` 是 `Option<String>`
///   （主干 `string?`）—— **缺键 ⇒ `None`，不是 `Some("")`**；
/// · `status` 缺省 `"idle"`（主干 `:64` 的字段初值）。
///
/// 一处**必须记住**的主干 quirk：`activeRun` 键在且是对象、但里面 `pluginRunId` 缺/非串时，
/// 主干拿到的是**空串而非 null**（`:270` 走 `CapabilityString`），而 `:287/:290` 的判据是
/// `is not null` ⇒ 照样折成 `"running"`。所以这里刻意用 `Option<String>`（`Some("")` ≠ `None`），
/// 不许「顺手」把空串归并成 `None`。UI 侧 `:881` 那句 `ActiveRunId is {Length:>0}` 是**另一道**判据，
/// 空串在那里是假 —— 收起/禁能的活归 B3，本层只保证两态分得开。
#[derive(Clone, Debug, PartialEq)]
pub struct CordisRow {
    /// 主干 `PluginId`（`:52`；取值 `:237`）。
    pub plugin_id: String,
    /// 主干 `AgentId`（`:53`；`:238`）。
    pub agent_id: String,
    /// 主干 `Name`（`:54`；`:253/261`，只由挑中的那颗 package 写）。
    pub name: String,
    /// 主干 `Purpose`（`:55`；`:254/262`）。
    pub purpose: String,
    /// 主干 `PackageId`（`:56`；`:252/260`）—— 展示用「当前包」，挑选规则见 [`parse_cordis_row`]。
    pub package_id: String,
    /// 主干 `CurrentPackageId`（`:57`；`:239`）。
    pub current_package_id: String,
    /// 主干 `NextPackageId`（`:58`；`:240`）。
    pub next_package_id: String,
    /// 主干 `HasClientHalf`（`:59`；`:255/263`，只认字面 `true`）。
    pub has_client_half: bool,
    /// 主干 `HasHostHalf`（`:60`；`:256/264`）。
    pub has_host_half: bool,
    /// 主干 `ActiveRunId`（`:61`，`string?`；`:270`）。
    pub active_run_id: Option<String>,
    /// 主干 `ActivePackageId`（`:62`；`:271`）。
    pub active_package_id: Option<String>,
    /// 主干 `Status`（`:64`，初值 `"idle"`；归并 `:280-293`）。
    pub status: String,
    /// 主干 `ApprovalRequestId`（`:65`，`string?`；`:276-277`）。
    pub approval_request_id: Option<String>,
    /// 主干 `LatestError`（`:66`，`string?`；`:278-279`）。
    pub latest_error: Option<String>,
}

impl Default for CordisRow {
    /// 手写而非派生：主干 `Status` 的初值是 `"idle"`（`:64`）而不是空串。
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            agent_id: String::new(),
            name: String::new(),
            purpose: String::new(),
            package_id: String::new(),
            current_package_id: String::new(),
            next_package_id: String::new(),
            has_client_half: false,
            has_host_half: false,
            active_run_id: None,
            active_package_id: None,
            status: "idle".to_string(),
            approval_request_id: None,
            latest_error: None,
        }
    }
}

impl CordisRow {
    /// 主干那颗 run 按钮算 `mode` 的式子（`:758`，行内审批 `:811` 是同一个式子的第二次抄写）：
    /// `CurrentPackageId.Length > 0 && PackageId != CurrentPackageId ? "update" : "run"`。
    /// 判据刻意用「挑中的展示包 ≠ 当前包」而不是「有 nextPackageId」，两者在有历史包时不等价。
    #[must_use]
    pub fn run_mode(&self) -> &'static str {
        if !self.current_package_id.is_empty() && self.package_id != self.current_package_id {
            "update"
        } else {
            "run"
        }
    }

    /// 主干 `:739` `bool running = row.Status is "running" or "client-pending";` —— 那颗
    /// `CordisRunStop` 走 stop 分支的判据（**不**含 `waiting`/`starting-host`：那两态若没活动
    /// run 会停在原值上，见 [`parse_cordis_row`] 的归并）。
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.status == "running" || self.status == "client-pending"
    }

    /// 主干 `:740` `awaiting = row.Status == "awaiting-approval"`（`:745` 用它禁能 run/stop）。
    #[must_use]
    pub fn is_awaiting(&self) -> bool {
        self.status == CORDIS_STATUS_AWAITING
    }
}

/// inventory 单行投影 = 主干 `ParseCordisRow`（`MainWindow.Cordis.cs:233-295`）的直译。
///
/// 省略式（本层唯一容易写错的一条）：内核那四颗 `.optional()` 键在「不适用」时**整颗不存在**
/// （`index.js:1943-1956`），主干因此全程用 `TryGetProperty` 读（`:242/255/256/268/273/276/278`），
/// 缺键的落点是**字段初值**（`""` 或 `null`）。所以：
/// · 缺 `activeRun` ⇒ `active_run_id == None`（不是 `Some("")`）⇒ 不进 `"running"` 归并；
/// · 缺 `latestRun` ⇒ `approval_request_id`/`latest_error` 保持 `None`、`status` 保持初值，
///   只有活动 run 才把状态抬成 `"running"`（`:290-293`）；
/// · 缺 `packages`（或非数组）⇒ `name`/`purpose`/`package_id`/两颗 half 全保持初值（`:242` 前置门）。
///
/// **记录级偏差**（主干畸形回帧会抛、分叉不能 panic，两条都写进单测钉住）：
/// 主干 `item` 或 `packages[]` 元素不是对象时，那颗裸 `TryGetProperty`（`:242/255/268/273`）
/// 抛 `InvalidOperationException` → 被 `RefreshCordisInventoryAsync` 的 `catch (Exception)`（`:227-230`）
/// 吞掉 ⇒ **整批行**丢弃、`_cordisRows` 保持上一次的表。这里读不出的一律按缺键折（该行仍会产出，
/// 内容全初值），既不 panic 也不清空。差异只在「内核回帧畸形」时可见，而那种回帧本就被
/// `typert` 的结果 codec 挡在网关外（`:18-60`）⇒ 分叉不追那份「整批消失」的静默行为。
#[must_use]
pub fn parse_cordis_row(item: &Value) -> CordisRow {
    let mut row = CordisRow {
        plugin_id: kw2_capability_string(item, "pluginId"),
        agent_id: kw2_capability_string(item, "agentId"),
        current_package_id: kw2_capability_string(item, "currentPackageId"),
        next_package_id: kw2_capability_string(item, "nextPackageId"),
        ..Default::default()
    };
    // 主干 :242-267 `packages` 数组（缺键或非数组 ⇒ 整段跳过，`PackageId` 等保持初值）。
    if let Some(packages) = item
        .as_object()
        .and_then(|record| record.get("packages"))
        .and_then(Value::as_array)
    {
        for pkg in packages {
            let pid = kw2_capability_string(pkg, "packageId");
            // :248-250「默认展示 current → next → 首个」：
            // ① 还没挑中过（`PackageId` 空串）→ 先占住这颗，**包括 `packageId` 本身是空串**的脏包
            //    （那时 ① 对后续每一颗都继续成立 ⇒ 实际留的是**最后一颗**，quirk 由单测钉住）；
            // ② 有 current 且命中 → 换过去；③ 无 current 但有 next 且命中 → 换过去。
            if row.package_id.is_empty()
                || (!row.current_package_id.is_empty() && pid == row.current_package_id)
                || (row.current_package_id.is_empty()
                    && !row.next_package_id.is_empty()
                    && pid == row.next_package_id)
            {
                kw2_take_package(&mut row, pkg, &pid);
            }
            // :258-265 主干的**第二道**同轮 `if`：上一段把 `name` 落成空串时，同一颗包再读一遍
            // （结果不变，但「空 name 也算挑中了」这条得留着 —— 它使 `PackageId` 可以停在空串上）。
            if row.name.is_empty() {
                kw2_take_package(&mut row, pkg, &pid);
            }
        }
    }
    // :268-272 `activeRun`：键在**且是对象**才写两颗 Option（值走 CapabilityString ⇒ 可为 `Some("")`）。
    if let Some(active) = item
        .as_object()
        .and_then(|record| record.get("activeRun"))
        .filter(|value| value.is_object())
    {
        row.active_run_id = Some(kw2_capability_string(active, "pluginRunId"));
        row.active_package_id = Some(kw2_capability_string(active, "packageId"));
    }
    if let Some(latest) = item
        .as_object()
        .and_then(|record| record.get("latestRun"))
        .filter(|value| value.is_object())
    {
        // :275-279
        let status = kw2_capability_string(latest, "status");
        row.approval_request_id = kw2_optional_string(latest, "approvalRequestId");
        if let Some(error) = latest.get("error").filter(|value| value.is_object()) {
            row.latest_error = Some(kw2_capability_string(error, "message"));
        }
        // :280-288 状态归并（六臂 + 默认臂）。四型「还在路上」的：有活动 run 才算 running，
        // 否则**原样**留（主干 `: status` ⇒ 拿到的是 `waiting` 这类未收口的原值）。
        row.status = match status.as_str() {
            "awaiting-approval" => "awaiting-approval".to_string(),
            "running" | "waiting" | "starting-host" | "client-pending" => {
                if row.active_run_id.is_some() {
                    "running".to_string()
                } else {
                    status
                }
            }
            "failed" => "failed".to_string(),
            "rejected" | "cancelled" | "stopped" => "stopped".to_string(),
            // :287 默认臂：内核 `status` 是九元 union（`typert:38`），主干不为未知值开洞，
            // 只看「有没有活动 run」决定 running / idle。
            _ => {
                if row.active_run_id.is_some() {
                    "running".to_string()
                } else {
                    "idle".to_string()
                }
            }
        };
    } else if row.active_run_id.is_some() {
        // :290-293 没有 latestRun 却有活动 run ⇒ running。
        row.status = "running".to_string();
    }
    row
}

/// inventory 整帧投影 = 主干 `RefreshCordisInventoryAsync:204-218`：**前置门 + 逐行 + 排序**。
/// 回帧不是数组（`null` / 对象 / 数字 / 脏型）⇒ 零行，与主干 `:204` 那句
/// `if (value.ValueKind == JsonValueKind.Array)` 完全一致：既不报错也不补一行占位。
/// 读 `Kernel::call` 的 `Ok(Value)`（它已经剥到 `result.value`，见 `kernel.rs` 的 `pub fn call`）。
#[must_use]
pub fn parse_cordis_rows(value: &Value) -> Vec<CordisRow> {
    let mut rows = Vec::new();
    if let Some(items) = value.as_array() {
        rows.extend(items.iter().map(parse_cordis_row));
    }
    sort_cordis_rows(&mut rows);
    rows
}

/// 主干 `:212-218` 的排序：待审优先，同档按 `PluginId` 的**序数**比较。
/// `string.CompareOrdinal` 逐 UTF-16 code unit 比、Rust `str` 的 `Ord` 逐 UTF-8 字节比：
/// 两种编码都是 code point 保序的 ⇒ 对合法字符串等价（代理对不改变相对序）。
/// 主干用的是 `List.Sort`（不稳定），这里是稳定排序：只在**两行 `PluginId` 相同**时结果才可能不同，
/// 而 `pluginId` 是内核 registry 的键（`index.js:1940` `registry.all().map(…)`）⇒ 一行一键，不追。
pub fn sort_cordis_rows(rows: &mut [CordisRow]) {
    rows.sort_by(|a, b| {
        let aa = u8::from(!a.is_awaiting());
        let bb = u8::from(!b.is_awaiting());
        aa.cmp(&bb).then_with(|| a.plugin_id.as_str().cmp(b.plugin_id.as_str()))
    });
}

/// 主干 `ReconcileCordisApprovals:300-301` 那颗局部函数 `StillPending` 的等价物：
/// **两个条件同真**（有同一 `ApprovalRequestId` 的行 **且** 该行仍在 `awaiting-approval`）。
/// 主干拿它收掉「已批准/已撤回但队列里还留着」的审批卡（`:303-314`）；收放动作本身在 B3。
#[must_use]
pub fn cordis_still_pending(rows: &[CordisRow], request_id: &str) -> bool {
    rows.iter()
        .any(|row| row.approval_request_id.as_deref() == Some(request_id) && row.is_awaiting())
}

/// 一条待审的 `cordis/request-run` = 主干 `CordisPendingApproval`（`MainWindow.Cordis.cs:70-79`）。
/// 它是 `runHostHalf` 审批路的**唯一**参数来源（`:571-573` 逐字取的这六格）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CordisPendingApproval {
    /// 主干 `RequestId`（`:72`；事件取格 `:118`）。
    pub request_id: String,
    /// 主干 `AgentId`（`:73`；`:131`）。
    pub agent_id: String,
    /// 主干 `PluginId`（`:74`；`:132`）。
    pub plugin_id: String,
    /// 主干 `PackageId`（`:75`；`:133`）。
    pub package_id: String,
    /// 主干 `Mode`（`:76`，初值 `"run"`；`:134` 只在事件里的 `mode` **非空**时才覆盖）。
    pub mode: String,
    /// 主干 `Name`（`:77`；`:135`）。
    pub name: String,
    /// 主干 `Purpose`（`:78`；`:136`）。
    pub purpose: String,
}

/// 主干 `OnCordisRequestRunAsync:118-137` 的**纯**部分：从一帧 `cordis/request-run` 折出待审记录。
/// 回 `None` = 主干 `:126` 那句 `if (!requires || requestId.Length == 0) return;`（不建卡）。
/// · `requiresApproval` 必须是**字面 true**（`:119` 的 `ValueKind == True`）；缺键/`null`/串/`1` 全假。
/// · `mode` 不做白名单：主干 `:134` 是「非空就用它」，非法值（既非 run 也非 update）会一路带到
///   `runHostHalf` 被网关的 `mode` codec 拒掉（`typert:125`）⇒ 这里不提前夹，夹了就是发明行为。
/// · 去重（`_cordisSeenRequests` `:127`）与入队/弹卡是**状态**，在 B3。
#[must_use]
pub fn cordis_pending_from_event(event: &Value) -> Option<CordisPendingApproval> {
    let requires = kw2_is_literal_true(event, "requiresApproval");
    let request_id = kw2_capability_string(event, "requestId");
    if !requires || request_id.is_empty() {
        return None;
    }
    let mode = kw2_capability_string(event, "mode");
    Some(CordisPendingApproval {
        request_id,
        agent_id: kw2_capability_string(event, "agentId"),
        plugin_id: kw2_capability_string(event, "pluginId"),
        package_id: kw2_capability_string(event, "packageId"),
        mode: if mode.is_empty() { "run".to_string() } else { mode },
        name: kw2_capability_string(event, "name"),
        purpose: kw2_capability_string(event, "purpose"),
    })
}

/// `runHostHalf` 回帧（`typert.remote-client.js:128-140`：`{ok:true,…}` ∪ `{ok:false,message}`）
/// 的**成功判据** = 主干 `:586-587`（面板 `:760-761`、行内审批 `:813-814` 是同一式的三次抄写）：
/// 回帧是对象 **且** `ok` 是字面 `true`。`ok` 缺 / 为 `null` / 为串 `\"true\"` 全算失败。
#[must_use]
pub fn cordis_host_half_ok(reply: &Value) -> bool {
    kw2_is_literal_true(reply, "ok")
}

/// 主干 `CapabilityString(started, "pluginRunId")`（`:602`，另有 `:770`、`:820`）—— 喂给随后的
/// `resolveRequestRun` / `settleUserRun`。读不出来 ⇒ **空串**（主干会照样把空串当 pluginRunId 发回去，
/// 网关在 `typert:109` 那颗 `z.string()` 上放行它）；这里不替主干兜底。
#[must_use]
pub fn cordis_host_half_plugin_run_id(reply: &Value) -> String {
    kw2_capability_string(reply, "pluginRunId")
}

/// 主干 `CapabilityString(started, "message")`（`:591`、`:764`、`:830`）：失败原因上屏用。
/// 非对象回帧 ⇒ 空串；调用方自己决定空串时显示什么（主干 `:597` 夹 `"host-half-failed"`、
/// `:765` 夹「运行失败」文案，两处的兜底**不同** ⇒ 属宿主，不留在这层）。
#[must_use]
pub fn cordis_host_half_message(reply: &Value) -> String {
    kw2_capability_string(reply, "message")
}

/// `resolveRequestRun` / `settleUserRun` 的第二颗参数 `resolution`
/// （内核 `DynamicCordisRunResolution`，`typert.remote-client.js:107-118` 与 `:143-154` 同型）。
///
/// **恰好三型 = 主干实际发出去的三种形状**，键集逐字：
/// · [`Self::Activated`] → `{ok:true, pluginRunId, waitingFor:[]}`（`:604-609`、`:771-776`、`:817-822`；
///   三处全是 `Array.Empty<string>()` ⇒ 这里不留 `Option` 口子）
/// · [`Self::Rejected`] → `{ok:false, reason:"rejected"}`（`:563`、`:804`）—— **两颗键，没有 `message`**
/// · [`Self::HostHalfFailed`] → `{ok:false, reason:"host-half-failed", message}`
///   （`:577-582`、`:593-598`、`:826-831`）
///
/// 刻意**不开**第四型（Client 半边失败那条 reason）：主干 `:29-30` 与 `:551` 两处注释明文
/// 「桌面壳不可达，禁止伪造」。内核确实会**回**那个 reason（`index.js:2331-2335` 的竞态档），
/// 但那是回帧不是本壳发出的 resolution ⇒ 投影侧读它不需要、发送侧造它不允许。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CordisRunResolution {
    /// Host 半边已起，结算成功。
    Activated { plugin_run_id: String },
    /// 用户点「拒绝」。
    Rejected,
    /// Host 半边失败（抛错或 `ok:false` 都走这一型，主干 `:577-598`）。
    HostHalfFailed { message: String },
}

impl CordisRunResolution {
    /// 上面那张三行表的逐字序列化（键序无所谓：`assertExactArguments` 按键名集合比，
    /// `types/index.js:802-804` 用的是 `Reflect.ownKeys`）。
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Activated { plugin_run_id } => {
                json!({"ok": true, "pluginRunId": plugin_run_id, "waitingFor": []})
            }
            Self::Rejected => json!({"ok": false, "reason": "rejected"}),
            Self::HostHalfFailed { message } => {
                json!({"ok": false, "reason": "host-half-failed", "message": message})
            }
        }
    }
}

/// `dynamicCordisRunner/inventory`（主干 `:202`）：**零颗** wire 字段。
/// 形状是 `{}` 而不是 `{request:{}}` —— 那发 descriptor 的 `parameters` 是空表，多一颗就 `unexpected`。
#[must_use]
pub fn cordis_inventory() -> RpcCall {
    RpcCall::new(CORDIS_INVENTORY, json!({}))
}

/// `dynamicCordisRunner/runHostHalf`（主干 `:320-333`）：六颗必填，逐字对应
/// `new { agentId, pluginId, packageId, mode, requestId, approveFutureVersions }`。
///
/// `request_id` 两态**都必须发出键**：审批路给 `Some(rid)`（`:571-573`、`:809-812`），
/// 面板「运行」路给 `None` ⇒ 序列化出 `"requestId":null`（`:759` 传的就是 C# 的 `null`，
/// 而那颗 codec 是 `z.union([z.literal(null), string])`）。把 `None` 折成「键不存在」会被
/// `assertExactArguments` 拒成 `missing "requestId"` —— 这是本族最容易写错的一颗。
///
/// `mode` 由 [`CordisRow::run_mode`] 或主干 `:134` 的那道「非空即用」给出；这里不校验
/// （内核只认 `run`/`update`，越界该由网关回 `gateway/arguments-invalid`，不是壳的活）。
#[must_use]
pub fn cordis_run_host_half(
    agent_id: &str,
    plugin_id: &str,
    package_id: &str,
    mode: &str,
    request_id: Option<&str>,
    approve_future_versions: bool,
) -> RpcCall {
    RpcCall::new(
        CORDIS_RUN_HOST_HALF,
        json!({
            "agentId": agent_id,
            "pluginId": plugin_id,
            "packageId": package_id,
            "mode": mode,
            "requestId": request_id,
            "approveFutureVersions": approve_future_versions,
        }),
    )
}

/// `dynamicCordisRunner/resolveRequestRun`（主干 `:342-346`）：`{requestId, resolution}` 两颗。
/// 主干**不读回帧**（`accepted` 无人消费：`:630-634` 吞异常、直调那三处 `:563/804/817/826` 也丢弃
/// `JsonElement`）⇒ B3 侧是「await 但不消费」，别为它造错误支。
#[must_use]
pub fn cordis_resolve_request_run(request_id: &str, resolution: &CordisRunResolution) -> RpcCall {
    RpcCall::new(
        CORDIS_RESOLVE_REQUEST_RUN,
        json!({"requestId": request_id, "resolution": resolution.to_value()}),
    )
}

/// `dynamicCordisRunner/settleUserRun`（主干 `:349-353`）：`{agentId, pluginId, resolution}`。
/// 主干唯一调用点被 `:768 if (row.HasClientHalf)` 闸住，回帧整个丢弃（`await` 无接收者）。
#[must_use]
pub fn cordis_settle_user_run(
    agent_id: &str,
    plugin_id: &str,
    resolution: &CordisRunResolution,
) -> RpcCall {
    RpcCall::new(
        CORDIS_SETTLE_USER_RUN,
        json!({"agentId": agent_id, "pluginId": plugin_id, "resolution": resolution.to_value()}),
    )
}

/// `dynamicCordisRunner/stopFromPanel`（主干 `:355-359`）：`{agentId, pluginId}`。
/// 回帧丢弃（`:754` 那句没接收者）⇒ 主干看不见 `{ok:false, reason:"not-running"}` 这类失败，
/// 之后一律刷 inventory。分叉照抄那份宽容，**不许顺手修**（RD6 §A-2 第 5 条）。
#[must_use]
pub fn cordis_stop_from_panel(agent_id: &str, plugin_id: &str) -> RpcCall {
    RpcCall::new(CORDIS_STOP_FROM_PANEL, json!({"agentId": agent_id, "pluginId": plugin_id}))
}

/// `dynamicCordisRunner/undefineFromPanel`（主干 `:361-365`）：`{agentId, pluginId}`。
/// 回帧的 `wasRunning`（`typert:196` 那颗必填布尔）主干**不读**（`:862` 丢弃整个回帧）。
#[must_use]
pub fn cordis_undefine_from_panel(agent_id: &str, plugin_id: &str) -> RpcCall {
    RpcCall::new(
        CORDIS_UNDEFINE_FROM_PANEL,
        json!({"agentId": agent_id, "pluginId": plugin_id}),
    )
}

/// `dynamicCordisRunner/getClientCode`（主干 `:368-372`）：`{agentId, pluginId, pluginRunId}`。
/// 回帧的 `code`（`typert:12`）进只读文本（`:904`）；内核三条失败路径是 **throw**
/// （`index.js:1819/1821/1823` 三处 `throw new Error`）⇒ 信封 `ok:false` ⇒ `Kernel::call` 回 `Err`，
/// 面板内联显示（`:926-935`）。
/// 这一发的 `pluginRunId` 取的是**行上的活动 run**（`:903` 用 `row.ActiveRunId!`，
/// 那颗按钮在 `:881` 已按 `is {Length:>0}` 禁能）。
#[must_use]
pub fn cordis_get_client_code(
    agent_id: &str,
    plugin_id: &str,
    plugin_run_id: &str,
) -> RpcCall {
    RpcCall::new(
        CORDIS_GET_CLIENT_CODE,
        json!({"agentId": agent_id, "pluginId": plugin_id, "pluginRunId": plugin_run_id}),
    )
}

/// `dynamicCordisRunner/syncInspectManifest`（主干 `:393-398`）：`{providers}` 一颗，
/// boot 那发的实参是 `Array.Empty<object>()`（`:109`）⇒ `{"providers":[]}`。
/// 见 [`CORDIS_SYNC_INSPECT_MANIFEST`] 的「构造器在册、发信不接」说明。
#[must_use]
pub fn cordis_sync_inspect_manifest() -> RpcCall {
    RpcCall::new(CORDIS_SYNC_INSPECT_MANIFEST, json!({"providers": []}))
}

#[cfg(test)]
/// 台账 #144 · KW2 批 B1：八发 args 的逐字节钉子 ＋ inventory 省略式投影的两态 ＋
/// 主干零调用方那几面的**源码反向锁**。
///
/// 反向锁的自匹配问题（本文件内自锁 ⇒ needle 与本体的文字同一）：
/// 禁串一律用 `concat!` 把前缀与尾段拆开，说明文字里不留连续的 `dynamicCordisRunner/<四条>` 形；
/// 被锁的文件 = **本文件自己**（`include_str!` 只指 `src/kernel.rs`），绝不指 `src/main.rs`
/// （那颗文件正被并发改，锁它必然假红）。
mod kw2_cordis_tests {
    use super::*;

    /// 本文件的源码文本（反向锁的靶）。断言它**读到了东西**，否则零命中只是「文件是空的」。
    const KW2_SELF: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));

    /// 主干发出那一发的逐字 args（`MainWindow.Cordis.cs` 的实参取值），给 byte 级比对当基准。
    fn sample_agent() -> &'static str {
        "s-1001"
    }
    fn sample_plugin() -> &'static str {
        "demo-cordis"
    }
    fn sample_package() -> &'static str {
        "pkg-1"
    }

    /// 序列化后的**键名集合**（BTreeMap ⇒ 升序），多一颗少一颗都出得来。
    fn keys(args: &Value) -> Vec<&str> {
        args.as_object()
            .map(|record| record.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }

    #[test]
    fn eight_builders_match_the_mainline_argument_shape_byte_for_byte() {
        // serde_json 的 Map 是 BTreeMap ⇒ 落盘串是**键名升序**。主干 C# 写的是声明序，但
        // `assertExactArguments`（`types/index.js:802-804`）按 `Reflect.ownKeys` 比**集合** ⇒ 序无关。
        assert_eq!(cordis_inventory().args.to_string(), "{}");
        assert_eq!(cordis_inventory().method, "dynamicCordisRunner/inventory");
        assert_eq!(
            cordis_run_host_half(
                sample_agent(),
                sample_plugin(),
                sample_package(),
                "run",
                Some("req-1"),
                true
            )
            .args
            .to_string(),
            r##"{"agentId":"s-1001","approveFutureVersions":true,"mode":"run","packageId":"pkg-1","pluginId":"demo-cordis","requestId":"req-1"}"##
        );
        assert_eq!(
            cordis_resolve_request_run(
                "req-1",
                &CordisRunResolution::Activated { plugin_run_id: "run-1".to_string() }
            )
            .args
            .to_string(),
            r##"{"requestId":"req-1","resolution":{"ok":true,"pluginRunId":"run-1","waitingFor":[]}}"##
        );
        assert_eq!(
            cordis_settle_user_run(
                sample_agent(),
                sample_plugin(),
                &CordisRunResolution::Activated { plugin_run_id: "run-1".to_string() }
            )
            .args
            .to_string(),
            r##"{"agentId":"s-1001","pluginId":"demo-cordis","resolution":{"ok":true,"pluginRunId":"run-1","waitingFor":[]}}"##
        );
        assert_eq!(
            cordis_stop_from_panel(sample_agent(), sample_plugin())
                .args
                .to_string(),
            r#"{"agentId":"s-1001","pluginId":"demo-cordis"}"#
        );
        assert_eq!(
            cordis_undefine_from_panel(sample_agent(), sample_plugin())
                .args
                .to_string(),
            r#"{"agentId":"s-1001","pluginId":"demo-cordis"}"#
        );
        assert_eq!(
            cordis_get_client_code(sample_agent(), sample_plugin(), "run-1")
                .args
                .to_string(),
            r#"{"agentId":"s-1001","pluginId":"demo-cordis","pluginRunId":"run-1"}"#
        );
        assert_eq!(
            cordis_sync_inspect_manifest().args.to_string(),
            r#"{"providers":[]}"#
        );
    }

    #[test]
    fn each_builder_carries_exactly_the_mainline_key_set() {
        // 键集单独钉一遍：上一发比的是整串，改一颗键名会同时改串 ⇒ 这里给「哪几颗」的判据。
        assert_eq!(keys(&cordis_inventory().args), Vec::<&str>::new());
        assert_eq!(
            keys(
                &cordis_run_host_half(
                    sample_agent(),
                    sample_plugin(),
                    sample_package(),
                    "run",
                    None,
                    false
                )
                .args
            ),
            vec![
                "agentId",
                "approveFutureVersions",
                "mode",
                "packageId",
                "pluginId",
                "requestId"
            ]
        );
        assert_eq!(
            keys(
                &cordis_resolve_request_run("req-1", &CordisRunResolution::Rejected).args
            ),
            vec!["requestId", "resolution"]
        );
        assert_eq!(
            keys(
                &cordis_settle_user_run(sample_agent(), sample_plugin(), &CordisRunResolution::Rejected)
                    .args
            ),
            vec!["agentId", "pluginId", "resolution"]
        );
        assert_eq!(
            keys(&cordis_stop_from_panel(sample_agent(), sample_plugin()).args),
            vec!["agentId", "pluginId"]
        );
        assert_eq!(
            keys(&cordis_undefine_from_panel(sample_agent(), sample_plugin()).args),
            vec!["agentId", "pluginId"]
        );
        assert_eq!(
            keys(&cordis_get_client_code(sample_agent(), sample_plugin(), "run-1").args),
            vec!["agentId", "pluginId", "pluginRunId"]
        );
        assert_eq!(keys(&cordis_sync_inspect_manifest().args), vec!["providers"]);
    }

    #[test]
    fn panel_run_still_emits_the_request_id_key_as_literal_null() {
        // 主干 `:759` 面板「运行」传的是 C# 的 `null`；那颗 codec 只认 `null` 或串
        // （`typert.remote-client.js:126`），且 descriptor 没给 `acceptsUndefined`
        // ⇒ **省键 = `missing "requestId"`**。所以这里必须是显式 null。
        let args = cordis_run_host_half(
            sample_agent(),
            sample_plugin(),
            sample_package(),
            "update",
            None,
            false,
        )
        .args;
        assert_eq!(args["requestId"], Value::Null);
        assert!(args.as_object().is_some_and(|r| r.contains_key("requestId")));
        assert_eq!(
            args.to_string(),
            r##"{"agentId":"s-1001","approveFutureVersions":false,"mode":"update","packageId":"pkg-1","pluginId":"demo-cordis","requestId":null}"##
        );
    }

    #[test]
    fn resolution_has_exactly_three_shapes_with_the_mainline_key_sets() {
        // `rejected` 那型是**两颗**键：主干 `:563`、`:804` 两处都只 `{ok:false, reason:"rejected"}`，
        // 没有 `message`（RD6 把三型并成两型，见 kw2 报告 §纠正 3）。
        let rejected = CordisRunResolution::Rejected.to_value();
        assert_eq!(rejected.to_string(), r#"{"ok":false,"reason":"rejected"}"#);
        assert_eq!(keys(&rejected), vec!["ok", "reason"]);
        let failed = CordisRunResolution::HostHalfFailed { message: "boom".to_string() }.to_value();
        assert_eq!(
            failed.to_string(),
            r#"{"message":"boom","ok":false,"reason":"host-half-failed"}"#
        );
        assert_eq!(keys(&failed), vec!["message", "ok", "reason"]);
        let activated = CordisRunResolution::Activated { plugin_run_id: String::new() }.to_value();
        assert_eq!(
            activated.to_string(),
            r#"{"ok":true,"pluginRunId":"","waitingFor":[]}"#
        );
        assert_eq!(keys(&activated), vec!["ok", "pluginRunId", "waitingFor"]);
    }

    #[test]
    fn no_resolution_shape_fabricates_the_unreachable_client_side_reason() {
        // 主干 `MainWindow.Cordis.cs:29-30` 与 `:551`：桌面壳**禁止**伪造 Client 半边失败那条 reason。
        let needle = concat!("client-", "half-", "failed");
        for resolution in [
            CordisRunResolution::Rejected,
            CordisRunResolution::HostHalfFailed { message: "x".to_string() },
            CordisRunResolution::Activated { plugin_run_id: "r".to_string() },
        ] {
            assert!(
                !resolution.to_value().to_string().contains(needle),
                "resolution 里出现了桌面壳不可达的那条 reason：{resolution:?}"
            );
        }
    }

    #[test]
    fn absent_optional_keys_stay_absent_in_the_row() {
        // 内核「不适用 ⇒ 整颗键不存在」（`index.js:1943-1956`）。最小合法行 = 只有前三颗必填键。
        let row = parse_cordis_row(&json!({"pluginId":"p","agentId":"a","packages":[]}));
        assert_eq!(row.plugin_id, "p");
        assert_eq!(row.agent_id, "a");
        assert_eq!(row.current_package_id, ""); // 主干 `:239` CapabilityString ⇒ 空串
        assert_eq!(row.next_package_id, "");
        assert_eq!(row.package_id, "");
        assert_eq!(row.name, "");
        assert_eq!(row.purpose, "");
        assert_eq!(row.active_run_id, None, "缺 activeRun ⇒ None，不是 Some(\"\")");
        assert_eq!(row.active_package_id, None);
        assert_eq!(row.approval_request_id, None);
        assert_eq!(row.latest_error, None);
        assert_eq!(row.status, "idle");
        assert!(!row.has_client_half && !row.has_host_half);
    }

    #[test]
    fn present_keys_do_not_get_silently_defaulted() {
        // 「多字段」那一态：四颗 optional 全在 ⇒ 每颗都被读到，且 latestRun 的归并生效。
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a",
            "packages":[{"packageId":"pkg-2","name":"演示","purpose":"用","hasHostHalf":true,"hasClientHalf":true}],
            "currentPackageId":"pkg-2","nextPackageId":"pkg-3",
            "activeRun":{"pluginRunId":"run-9","packageId":"pkg-2"},
            "latestRun":{"status":"running","approvalRequestId":"req-7","error":{"message":"炸了"}}
        }));
        assert_eq!(row.current_package_id, "pkg-2");
        assert_eq!(row.next_package_id, "pkg-3");
        assert_eq!(row.package_id, "pkg-2");
        assert_eq!(row.name, "演示");
        assert!(row.has_client_half && row.has_host_half);
        assert_eq!(row.active_run_id.as_deref(), Some("run-9"));
        assert_eq!(row.active_package_id.as_deref(), Some("pkg-2"));
        assert_eq!(row.approval_request_id.as_deref(), Some("req-7"));
        assert_eq!(row.latest_error.as_deref(), Some("炸了"));
        assert_eq!(row.status, "running");
    }

    #[test]
    fn an_empty_active_run_object_is_some_empty_string_and_still_reads_as_running() {
        // 主干 `:268-270`：`activeRun` 是对象就写 `ActiveRunId`，值走 CapabilityString ⇒ **空串**；
        // `:287/:290` 的判据是 `is not null` ⇒ 空串照样把状态抬成 running。
        // 把空串归并成 None 就是给分叉少造一行面板行。
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":[],
            "latestRun":{"status":"waiting"},"activeRun":{}
        }));
        assert_eq!(row.active_run_id.as_deref(), Some(""));
        assert_eq!(row.active_package_id.as_deref(), Some(""));
        assert_eq!(row.status, "running");
        // 同一无活动 run 的世界 ⇒ 原值留在 waiting（主干 `:284` 的 `: status`）。
        let idle = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":[],
            "latestRun":{"status":"waiting"}
        }));
        assert_eq!(idle.active_run_id, None);
        assert_eq!(idle.status, "waiting");
    }

    #[test]
    fn status_merge_walks_all_six_mainline_arms() {
        // 主干 `:280-288` 那六臂 × 有无活动 run 两态。
        let cases: &[(&str, bool, &str)] = &[
            ("awaiting-approval", false, "awaiting-approval"),
            ("awaiting-approval", true, "awaiting-approval"), // 第一臂**不看** activeRun
            ("running", false, "running"),
            ("failed", true, "failed"), // failed 也不被活动 run 抬走
            ("rejected", false, "stopped"),
            ("cancelled", false, "stopped"),
            ("stopped", true, "stopped"),
            ("starting-host", false, "starting-host"),
            ("client-pending", false, "client-pending"),
            // 默认臂（`:287`）：未知值 ⇒ 有活动 run 才 running
            ("something-new", false, "idle"),
            ("something-new", true, "running"),
        ];
        for (raw, has_run, want) in cases {
            let mut item = json!({"pluginId":"p","agentId":"a","packages":[],
                                  "latestRun":{"status":raw}});
            if *has_run {
                item["activeRun"] = json!({"pluginRunId":"run-1","packageId":"pkg-1"});
            }
            let row = parse_cordis_row(&item);
            assert_eq!(row.status, *want, "latestRun.status = {raw} / activeRun = {has_run}");
        }
        // status 缺键 / 非串 ⇒ CapabilityString 空串 ⇒ 走默认臂
        let missing = parse_cordis_row(
            &json!({"pluginId":"p","agentId":"a","packages":[],"latestRun":{}}),
        );
        assert_eq!(missing.status, "idle");
    }

    #[test]
    fn package_pick_order_is_current_then_next_then_first() {
        let packages = json!([
            {"packageId":"pkg-a","name":"甲","purpose":"pa"},
            {"packageId":"pkg-b","name":"乙","purpose":"pb"},
            {"packageId":"pkg-c","name":"丙","purpose":"pc"}
        ]);
        // 有 current ⇒ 命中 current
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","currentPackageId":"pkg-c","nextPackageId":"pkg-b",
            "packages":packages
        }));
        assert_eq!((row.package_id.as_str(), row.name.as_str()), ("pkg-c", "丙"));
        // 无 current、有 next ⇒ 命中 next
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","nextPackageId":"pkg-b","packages":packages
        }));
        assert_eq!((row.package_id.as_str(), row.name.as_str()), ("pkg-b", "乙"));
        // 两棵都没有 ⇒ 首个（`PackageId` 空串那一道 ①）
        let row = parse_cordis_row(&json!({"pluginId":"p","agentId":"a","packages":packages}));
        assert_eq!((row.package_id.as_str(), row.name.as_str()), ("pkg-a", "甲"));
        // current 指向**不存在**的包：① 只有第一次成立、② 永不命中 ⇒ 首颗留住（不是「最后一颗」）
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","currentPackageId":"ghost","packages":packages
        }));
        assert_eq!((row.package_id.as_str(), row.name.as_str()), ("pkg-a", "甲"));
        // 主干 `:248` 的 ① 查的是**行上的** `PackageId`：挑中的包自己 `packageId` 缺/空 ⇒ 那道门
        // 对后面每一颗都继续成立 ⇒ 实际留的是**最后一颗**（省略式回帧下的真 quirk，钉住别修）。
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":[
                {"packageId":"","name":"头无名","purpose":"u"},
                {"packageId":"pkg-y","name":"尾有名","purpose":"u"}]
        }));
        assert_eq!((row.package_id.as_str(), row.name.as_str()), ("pkg-y", "尾有名"));
        // 主干 `:258-265` 的第二道 `if`：挑中的包 `name` 是空串 ⇒ 同一颗再读一遍（结果不变），
        // 但 `Name` 仍空 ⇒ **下一颗**接管（这一支才是「有名字的最后一颗」）。
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":[
                {"packageId":"pkg-x","name":"","purpose":"无名"},
                {"packageId":"pkg-y","name":"有名字","purpose":"有名"}]
        }));
        assert_eq!((row.package_id.as_str(), row.name.as_str()), ("pkg-y", "有名字"));
    }

    #[test]
    fn only_a_literal_true_counts_as_a_half() {
        for dirty in [
            json!({"packageId":"p1","name":"n","purpose":"u","hasClientHalf":false}),
            json!({"packageId":"p1","name":"n","purpose":"u","hasClientHalf":null}),
            json!({"packageId":"p1","name":"n","purpose":"u","hasClientHalf":"true"}),
            json!({"packageId":"p1","name":"n","purpose":"u","hasClientHalf":1}),
            json!({"packageId":"p1","name":"n","purpose":"u"}),
        ] {
            let row = parse_cordis_row(
                &json!({"pluginId":"p","agentId":"a","packages":[dirty]}),
            );
            assert!(!row.has_client_half, "{dirty} 被读成了真");
            assert!(!row.has_host_half);
            // `settleUserRun` 在主干就挂在这颗布尔上（`:768`）⇒ 误读会凭空多发一发。
        }
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a",
            "packages":[{"packageId":"p1","name":"n","purpose":"u","hasClientHalf":true,"hasHostHalf":true}]
        }));
        assert!(row.has_client_half && row.has_host_half);
    }

    #[test]
    fn approval_request_id_and_latest_error_need_both_guards() {
        // `:276-279`：`approvalRequestId` 要**在且是串**；`error` 要**在且是对象**才读 `message`。
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":[],
            "latestRun":{"approvalRequestId":null,"error":{"message":404}}
        }));
        assert_eq!(row.approval_request_id, None, "JSON null 不是串");
        assert_eq!(row.latest_error.as_deref(), Some(""));
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":[],
            "latestRun":{"approvalRequestId":"req-1","error":"字符串"}
        }));
        assert_eq!(row.approval_request_id.as_deref(), Some("req-1"));
        assert_eq!(row.latest_error, None, "error 不是对象 ⇒ 整格不读");
    }

    #[test]
    fn a_non_array_inventory_reply_projects_to_no_rows() {
        // 主干 `:204` 的前置门：不是数组就一行都不建（`Value::Null` 是 `CallOkAsync` 缺
        // `result.value` 时的默认形状，见 `kernel.rs` 的 `pub fn call`）。
        assert_eq!(parse_cordis_rows(&Value::Null), Vec::new());
        assert_eq!(parse_cordis_rows(&json!({})), Vec::new());
        assert_eq!(parse_cordis_rows(&json!({"0": "行"})), Vec::new());
        assert_eq!(parse_cordis_rows(&json!("[]")), Vec::new());
        assert_eq!(parse_cordis_rows(&json!(42)), Vec::new());
        // 空数组是**合法**的零行（桩档 1 的世界），与上面那些「一行都不该有」同结果但不同判据
        assert_eq!(parse_cordis_rows(&json!([])), Vec::new());
    }

    #[test]
    fn dirty_frames_fold_to_nothing_instead_of_panicking() {
        // 与主干的**记录级偏差**（见 `parse_cordis_row` 的注释）：那几颗裸 `TryGetProperty`
        // 在主干会抛 ⇒ `:227` 吞异常 ⇒ 整批行保持上次的表。分叉按「读不出＝缺键」折，不 panic。
        let rows = parse_cordis_rows(&json!(["不是对象", {"pluginId":"p","agentId":"a"}]));
        assert_eq!(rows.len(), 2, "脏元素被整批吞掉了 ⇒ 这是主干行为，不是本层的");
        assert_eq!(rows[1].plugin_id, "p");
        assert_eq!(rows[0].plugin_id, "");
        assert_eq!(rows[0].status, "idle");
        // packages 是对象 / 是串 ⇒ 主干 `:242` 的 ValueKind 前置门跳过整段（这一格与主干同行为）
        let row = parse_cordis_row(&json!({
            "pluginId":"p","agentId":"a","packages":{"pkg-a":{}}
        }));
        assert_eq!(row.package_id, "");
        assert_eq!(row.name, "");
    }

    #[test]
    fn rows_sort_awaiting_first_then_by_ordinal_plugin_id() {
        // 主干 `:212-218`。同档内是**序数**比较，不是区域名序（中文/大小写都按码位）。
        let rows = parse_cordis_rows(&json!([
            {"pluginId":"b-demo","agentId":"a","latestRun":{"status":"running"},
             "activeRun":{"pluginRunId":"r","packageId":"p"},"packages":[]},
            {"pluginId":"A-demo","agentId":"a","packages":[],"latestRun":{"status":"awaiting-approval"}},
            {"pluginId":"a-demo","agentId":"a","packages":[]},
            {"pluginId":"中文","agentId":"a","packages":[],"latestRun":{"status":"awaiting-approval"}},
            {"pluginId":"B-demo","agentId":"a","packages":[]}
        ]));
        let ids: Vec<&str> = rows.iter().map(|row| row.plugin_id.as_str()).collect();
        assert_eq!(ids, vec!["A-demo", "中文", "B-demo", "a-demo", "b-demo"]);
        assert!(rows[0].is_awaiting() && rows[1].is_awaiting());
        // 「码位序」而非 `collator`：'B'(0x42) < 'a'(0x61) < '中'(0x4E2D)，与 CompareOrdinal 同。
    }

    #[test]
    fn run_mode_and_running_probe_match_the_row_predicates() {
        // 主干 `:758`（mode）、`:739`（running = running ∪ client-pending）、`:740`（awaiting）。
        let mut row = CordisRow {
            plugin_id: "p".into(),
            package_id: "pkg-b".into(),
            current_package_id: "pkg-a".into(),
            ..Default::default()
        };
        assert_eq!(row.run_mode(), "update");
        row.package_id = row.current_package_id.clone();
        assert_eq!(row.run_mode(), "run");
        row.current_package_id = String::new();
        assert_eq!(row.run_mode(), "run", "没有 current 时无条件 run（主干 `> 0` 那道门）");
        for status in ["running", "client-pending"] {
            row.status = status.to_string();
            assert!(row.is_running());
            assert!(!row.is_awaiting());
        }
        for status in ["waiting", "starting-host", "awaiting-approval", "idle", "stopped"] {
            row.status = status.to_string();
            assert!(!row.is_running(), "{status} 不该走 stop 分支");
        }
        row.status = "awaiting-approval".to_string();
        assert!(row.is_awaiting());
    }

    #[test]
    fn pending_approval_needs_a_literal_true_and_a_non_empty_request_id() {
        // 主干 `:119`（`requiresApproval` 必须字面 true）＋ `:126`（空 requestId 不建卡）。
        let full = json!({
            "requestId":"req-1","requiresApproval":true,"agentId":"s-1001",
            "pluginId":"demo-cordis","packageId":"pkg-1","mode":"update",
            "name":"演示","purpose":"用"
        });
        assert_eq!(
            cordis_pending_from_event(&full),
            Some(CordisPendingApproval {
                request_id: "req-1".into(),
                agent_id: "s-1001".into(),
                plugin_id: "demo-cordis".into(),
                package_id: "pkg-1".into(),
                mode: "update".into(),
                name: "演示".into(),
                purpose: "用".into(),
            })
        );
        for dirty in [
            json!({"requestId":"req-1"}),
            json!({"requestId":"req-1","requiresApproval":false}),
            json!({"requestId":"req-1","requiresApproval":null}),
            json!({"requestId":"req-1","requiresApproval":"true"}),
            json!({"requestId":"","requiresApproval":true}),
            json!({"requiresApproval":true}),
            Value::Null,
        ] {
            assert_eq!(cordis_pending_from_event(&dirty), None, "{dirty} 建成了卡");
        }
        // `mode` 缺/空 ⇒ "run"（主干 `:76` 初值 + `:134` 的 `is {Length:>0}`）；
        // 非法 mode **原样透传**（夹它等于发明行为）。
        let row = cordis_pending_from_event(
            &json!({"requestId":"req-1","requiresApproval":true,"mode":""}),
        )
        .expect("该建卡");
        assert_eq!(row.mode, "run");
        let row = cordis_pending_from_event(
            &json!({"requestId":"req-1","requiresApproval":true,"mode":"restart"}),
        )
        .expect("该建卡");
        assert_eq!(row.mode, "restart");
        // 建出来的卡喂 runHostHalf：requestId 走 Some 那一支（`:571-573`）
        let call = cordis_run_host_half(
            &row.agent_id,
            &row.plugin_id,
            &row.package_id,
            &row.mode,
            Some(&row.request_id),
            false,
        );
        assert_eq!(call.args["requestId"], json!("req-1"));
        assert_eq!(call.args["mode"], json!("restart"));
    }

    #[test]
    fn reconcile_predicate_needs_both_the_id_and_the_awaiting_status() {
        // 主干 `:300-301`。
        let rows = parse_cordis_rows(&json!([
            {"pluginId":"p1","agentId":"a","packages":[],"latestRun":{"status":"awaiting-approval","approvalRequestId":"req-1"}},
            {"pluginId":"p2","agentId":"a","packages":[],"latestRun":{"status":"running","approvalRequestId":"req-2"}}
        ]));
        assert!(cordis_still_pending(&rows, "req-1"));
        assert!(!cordis_still_pending(&rows, "req-2"), "行已不在待审态，卡该收");
        assert!(!cordis_still_pending(&rows, "req-9"));
        assert!(!cordis_still_pending(&[], "req-1"));
        // latestRun 缺 ⇒ 连 request_id 都没有（省略式），任何请求都不算仍在待审
        let bare = parse_cordis_rows(&json!([{"pluginId":"p","agentId":"a","packages":[]}]));
        assert_eq!(bare[0].approval_request_id, None);
        assert!(!cordis_still_pending(&bare, "req-1"));
    }

    #[test]
    fn host_half_reply_probes_read_like_the_mainline_triplet() {
        // 主干那三处同式：`:586-587/760-761/813-814`（ok）、`:602/770/820`（pluginRunId）、
        // `:591/764/830`（message）。
        assert!(cordis_host_half_ok(&json!({"ok":true,"pluginRunId":"run-1"})));
        for dirty in [
            json!({}),
            json!({"ok":null}),
            json!({"ok":"true"}),
            json!({"ok":1}),
            Value::Null,
        ] {
            assert!(!cordis_host_half_ok(&dirty), "{dirty} 被读成了成功");
        }
        let good = json!({"ok":true,"pluginRunId":"run-1","startedHere":true,"waitingFor":[]});
        assert_eq!(cordis_host_half_plugin_run_id(&good), "run-1");
        assert_eq!(cordis_host_half_message(&good), "");
        let bad = json!({"ok":false,"message":"插件不存在"});
        assert_eq!(cordis_host_half_message(&bad), "插件不存在");
        assert_eq!(cordis_host_half_plugin_run_id(&bad), "");
        // 非对象回帧 ⇒ 空串（主干 `:764` 拿 `CapabilityString` 的守卫兜住）
        assert_eq!(cordis_host_half_message(&Value::Null), "");
        assert_eq!(cordis_host_half_plugin_run_id(&json!("串")), "");
    }

    #[test]
    fn the_eight_consts_are_the_mainline_wire_names_and_nothing_else() {
        assert_eq!(
            CORDIS_METHODS,
            [
                "dynamicCordisRunner/inventory",
                "dynamicCordisRunner/runHostHalf",
                "dynamicCordisRunner/resolveRequestRun",
                "dynamicCordisRunner/settleUserRun",
                "dynamicCordisRunner/stopFromPanel",
                "dynamicCordisRunner/undefineFromPanel",
                "dynamicCordisRunner/getClientCode",
                "dynamicCordisRunner/syncInspectManifest",
            ]
        );
        // 正向锁（反向锁要成立，先要保证 needle 真的进得来源码）
        assert!(KW2_SELF.len() > 20_000, "自锁没读到文件 ⇒ 零命中是假的");
        for name in CORDIS_METHODS {
            assert!(KW2_SELF.contains(name), "八发里 {name} 在本文件没有对应常量");
        }
    }

    #[test]
    fn the_four_mainline_callerless_faces_have_no_route_here() {
        // 主干那四颗只有定义、零调用方（`MainWindow.Cordis.cs:383/405/412/419`），
        // 且 `:379-404` 的注释明文禁止伪造 ⇒ 本层不建常量、不建构造器、不建臂。
        // needle 用 `concat!` 拆开，否则本测试自己的文字就会命中自己（正向恒真、反向恒假）。
        let forbidden = [
            concat!("dynamicCordisRunner/", "invoke"),
            concat!("dynamicCordisRunner/", "resolveInspectQuery"),
            concat!("dynamicCordisRunner/", "reportClientGuardFailure"),
            concat!("dynamicCordisRunner/", "reportRenderFailure"),
        ];
        for needle in forbidden {
            assert!(
                !KW2_SELF.contains(needle),
                "本文件出现了主干零调用方的那一面：{needle}"
            );
            assert!(
                !CORDIS_METHODS.contains(&needle),
                "八发名单被塞进了不该发的那一面：{needle}"
            );
        }
        // 锁不是空转：前缀与四颗尾段各自都在源码里出现过，只是**从不相邻**。
        assert!(KW2_SELF.contains("dynamicCordisRunner/"));
        for tail in [
            "invoke",
            "resolveInspectQuery",
            "reportClientGuardFailure",
            "reportRenderFailure",
        ] {
            assert!(
                KW2_SELF.contains(tail),
                "尾段 {tail} 在本文件一次都没出现 ⇒ 上面那张禁串表可能压根没编进二进制"
            );
        }
        // 八发 = 恰八颗（塞进第 9 颗时基数先变红）
        assert_eq!(CORDIS_METHODS.len(), 8);
    }
}

// ==================== RD9 表A · C5+C6：credentials 3 + settings 5 + llm 2 的构造与回执 ====================
// 本节只有两件事：进 `payload.args` 的那棵树、回帧的形状。宿主与派发在 `main.rs`（下一批），UI 一颗不碰。
// 追加在文件末尾的理由同 `:7741` 那条 —— 本仓 `include_str!` 源码锁按「起始锚 → 结束锚首次出现」开窗。
//
// **十发全平铺，无一裹 `request`**（与 `session/selectModel` 那颗相反，也与 workspace 那六发相反）：
// 两族描述符的 `invocation.kind` 都是 `direct`，`parameters[].wire` 就是顶层键名；网关
// `assertExactArguments`（`@deepseek-ai/dsh-api-gateway/lib/types/index.js`）拿 `Reflect.ownKeys(args)`
// 与 `descriptor.parameters.map(p => p.wire)` 比**集合** —— 多一颗报 `unexpected "x"`、少一颗报
// `missing "x"`，只有声明了 `acceptsUndefined: true` 的那几颗（本批 = `settings/replace|update` 的
// `expectedRevision`）允许键整个缺席。唯一带 `request` 字样的 `llm/discoverModels`，那颗 `request`
// 本身是描述符的第二颗**命名参数**（`LlmModelDiscoveryRequest`，四颗子字段全 `.optional()`），
// 不是信封层 —— 把它抄成 `{request:{settingsNs, request}}` 就是自己造第二层。
//
// 描述符出处（逐颗读过 `parameters` 与 `result` 的 schema）：
// · credentials 三发 → `@deepseek-ai/dsh-api-settings-controller#credentialsController`
// · settings 五发 → 同包的 `#settingsController`（`settings/describe` 的 result 复用
//   `@deepseek-ai/dsh-settings/types#SettingsDescribeValue`，ns 元素复用 `#SettingsNamespaceView`）
// · llm 两发 → `@deepseek-ai/dsh-llm#llm`

/// 十发的方法名（集中一处，下一批 main.rs 与 `tests/ipc.rs` 都按这组常量对表，改不漏）。
pub const CREDENTIALS_DESCRIBE: &str = "credentials/describe";
pub const CREDENTIALS_SET: &str = "credentials/set";
pub const CREDENTIALS_UNSET: &str = "credentials/unset";
pub const SETTINGS_DESCRIBE: &str = "settings/describe";
pub const SETTINGS_REPLACE: &str = "settings/replace";
pub const SETTINGS_UPDATE: &str = "settings/update";
pub const SETTINGS_CAN_OPEN_PRESET_DIR: &str = "settings/canOpenAgentPresetDirectory";
pub const SETTINGS_OPEN_PRESET_DIR: &str = "settings/openAgentPresetDirectory";
pub const LLM_LIST_CONFIGURABLE_PROVIDERS: &str = "llm/listConfigurableProviders";
pub const LLM_DISCOVER_MODELS: &str = "llm/discoverModels";

/// C5 + C6 的十发名单（基数锁用；测试按「恰十颗」与「逐颗在源码里有 const」两头钉）。
pub const RD9_C5_C6_METHODS: &[&str] = &[
    CREDENTIALS_DESCRIBE,
    CREDENTIALS_SET,
    CREDENTIALS_UNSET,
    SETTINGS_DESCRIBE,
    SETTINGS_REPLACE,
    SETTINGS_UPDATE,
    SETTINGS_CAN_OPEN_PRESET_DIR,
    SETTINGS_OPEN_PRESET_DIR,
    LLM_LIST_CONFIGURABLE_PROVIDERS,
    LLM_DISCOVER_MODELS,
];

/// `credentials/describe`（主干 `LoadProviderRowsAsync` 批量那发、`DescribeSearchCredentialsAsync`
/// 单颗那发）：描述符一颗 `refs`（`z.array(z.string())`，**无** `acceptsUndefined`）
/// ⇒ 平铺 `{refs:[…]}`。空数组是合法的，但 `refs` 这颗键必须在 —— 主干 `refs.Count > 0` 那道闸
/// 是「没引用可查就不发这一发」，不是「发 `{}`」；发 `{}` 会被拒成 `missing "refs"`。
#[must_use]
pub fn credentials_describe(refs: &[&str]) -> RpcCall {
    RpcCall::new(CREDENTIALS_DESCRIBE, json!({ "refs": refs }))
}

/// `credentials/set`（主干 `MaybeShowDeepSeekOnboardingAsync` 与模型编辑卡的三处直调）：
/// 两颗必填 `ref` / `value`，都是 `z.string()` ⇒ 平铺 `{ref, value}`。
///
/// C# 字面写的是 `new { @ref = …, value = … }`：`@` 只是 C# 的**关键字转义符**，反射出来的属性名
/// 是 `ref` ⇒ 上线键名 `ref`。抄成 `"@ref"` 会同时踩中 `unexpected "@ref"` 与 `missing "ref"`。
///
/// `value` 是密钥明文：本层**不 trim、不校验非空** —— 主干是在 `keyBox.Password.Trim()` 之后
/// 才发（取值动作属于 UI 侧，同 `workspace_rename` 的口径），空串该由内核回 `gateway/input-invalid`。
#[must_use]
pub fn credentials_set(reference: &str, value: &str) -> RpcCall {
    RpcCall::new(CREDENTIALS_SET, json!({ "ref": reference, "value": value }))
}

/// `credentials/unset`（主干 `DeleteProviderCardAsync` 的 `removesCredential` 那一支）：一颗 `ref`。
/// 主干把它排在 `settings/mutate` **之前**（先清凭据再删配置，两步都幂等），顺序属于派发侧 ⇒
/// 本层只保证这一发的形状。回执与 `set` 同为 `z.void()`（见 [`credentials_set`] 末段）。
#[must_use]
pub fn credentials_unset(reference: &str) -> RpcCall {
    RpcCall::new(CREDENTIALS_UNSET, json!({ "ref": reference }))
}

/// `settings/describe`（主干 `EnsureSettingsSnapshotAsync`）：**零颗** wire 字段
/// （描述符 `parameters: []`）⇒ `{}`。多给任何一颗（含 `{request:{}}`）都是 `unexpected`。
/// 回执是**单层**对象 `{writable, hasDocument, namespaces:[…]}`：网关信封的 `ok` 由
/// [`Kernel::call`] 剥掉，剥完就是这一棵 —— 本仓的**双层 `ok` 族是 `messageFeedback/*`**
/// （假内核 `feedback_envelope` 那处自陈），settings 这族不在其中，别为它剥第二层。
#[must_use]
pub fn settings_describe() -> RpcCall {
    RpcCall::new(SETTINGS_DESCRIBE, json!({}))
}

/// 「有 revision 才带键」那第三颗（`replace` / `update` 共用）。
///
/// 主干 `ResetSectionAsync` 的三元表达式逐字是：快照里有这颗 ns 的 revision ⇒
/// `new { ns, section, expectedRevision = rev }`；没有 ⇒ `new { ns, section }`，**键整个不存在**。
/// 依据两条：描述符 `expectedRevision` 的 codec 是 `z.union([z.undefined(), z.number()])` 且带
/// `acceptsUndefined: true`（缺席被 `assertExactArguments` 放行），而**显式 `null` 过不了**
/// 那颗 union。主干注释把这层写得很清楚：「显式 null 会被 strict codec 判为非法」。
fn with_expected_revision(mut args: Value, expected_revision: Option<f64>) -> Value {
    if let Some(revision) = expected_revision {
        args["expectedRevision"] = json!(revision);
    }
    args
}

/// `settings/replace`（主干 `ResetSectionAsync`「恢复本页默认」，一发一个 ns）：
/// 平铺 `{ns, section[, expectedRevision]}`。`section` 是
/// `z.record(z.string(), JsonValue)`；「清空用户段」那一型主干传的就是**空对象** `{}`。
///
/// 回执 = `#SettingsNamespaceView`，主干**整个丢弃**（`await` 无接收者，随后 `_settingsSnapshot = null`
/// 重拉 describe）⇒ 本层不为它造解析器（同 `cordis_resolve_request_run` 的「await 但不消费」口径）。
///
/// 在册历史：`tests/mw2_deadbuttons.rs` 正向钉着 main.rs 的
/// `count("分叉未接入 settings/replace") == 1`。那颗锁不在本文件、本轮不改 ⇒
/// 本发是「ctor 已落、派发与撤钉在下一批」。
#[must_use]
pub fn settings_replace(ns: &str, section: &Value, expected_revision: Option<f64>) -> RpcCall {
    RpcCall::new(
        SETTINGS_REPLACE,
        with_expected_revision(json!({ "ns": ns, "section": section }), expected_revision),
    )
}

/// `settings/update`（主干 `AcknowledgeWelcomeNoticeAsync` 写 `ui-onboarding` 的
/// `welcomeNoticeVersion`；`SetDefaultPresetAsync` 写 `agent-presets` 的 `default`）：
/// 平铺 `{ns, patch[, expectedRevision]}`，`patch` 与 `replace` 的 `section` 同一型
/// `z.record(z.string(), JsonValue)`。
///
/// 主干那两处调用点**都不带** `expectedRevision`（最后写入者赢），所以参数给 `None` 是常态、
/// `Some` 是给下一批留的口（描述符允许）。`patch` 的键由调用方给整棵树：默认预设那颗
/// 是 `{default: id}`（C# 写 `@default`，上线键名 `default`），本层不猜键、不拼字段。
/// 回执同 `replace` = 主干丢弃的 `#SettingsNamespaceView`。
#[must_use]
pub fn settings_update(ns: &str, patch: &Value, expected_revision: Option<f64>) -> RpcCall {
    RpcCall::new(
        SETTINGS_UPDATE,
        with_expected_revision(json!({ "ns": ns, "patch": patch }), expected_revision),
    )
}

/// `settings/canOpenAgentPresetDirectory`（主干 `RenderPresetsSectionAsync` 的能力门控）：
/// **零颗**字段 ⇒ `{}`；回执是**裸布尔**（`result` 的 schema 就是 `z.boolean()`），
/// 不是 `{canOpen:…}` 那种对象。
#[must_use]
pub fn settings_can_open_agent_preset_directory() -> RpcCall {
    RpcCall::new(SETTINGS_CAN_OPEN_PRESET_DIR, json!({}))
}

/// `settings/openAgentPresetDirectory`（主干 `OpenPresetDirectoryAsync`）：一颗
/// `agentPreset`（`z.string()`）⇒ 平铺 `{agentPreset}`，不裹 `request`。
/// 描述符另有 `cancellation: {parameter:'signal'}`（主干那发带 `lifetime.Token`）⇒
/// 分叉的 unary 通道没有取消位，这一条**留给派发侧**说明，ctor 层不造假。
#[must_use]
pub fn settings_open_agent_preset_directory(agent_preset: &str) -> RpcCall {
    RpcCall::new(SETTINGS_OPEN_PRESET_DIR, json!({ "agentPreset": agent_preset }))
}

/// `llm/listConfigurableProviders`（主干 `LoadProviderRowsAsync` 的第一发）：**零颗**字段 ⇒ `{}`。
#[must_use]
pub fn llm_list_configurable_providers() -> RpcCall {
    RpcCall::new(LLM_LIST_CONFIGURABLE_PROVIDERS, json!({}))
}

/// `llm/discoverModels` 的探针（描述符第二颗命名参数 `request` = `LlmModelDiscoveryRequest`）。
///
/// 四颗子字段在 schema 里全 `.optional()`，主干 `FetchCandidatesForEditorAsync` 用的是
/// `Dictionary<string, object?>` + 「非空才 Add」⇒ **空值 = 键不存在**，不是 `""`：
/// · `provider` 的唯一判据是「这张卡不是自定义卡」（`!editor.IsCustom`），**不查空串**；
/// · `baseURL` / `api` / `apiKey` 三颗都是 `Trim()` 后 `Length > 0` 才进表。
/// 这两道都发生在取值侧（UI），所以本层收 `Option`：`None` = 主干没 Add 那颗键。
/// 「`provider` 给了空串」是合法形状（主干就发得出来），不许在这里悄悄丢掉。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveryProbe {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub api: Option<String>,
    pub api_key: Option<String>,
}

impl DiscoveryProbe {
    /// 逐字序列化：只发 `Some` 的键，键名是 wire 的 `provider` / `baseURL` / `api` / `apiKey`
    /// （`baseURL` 的 URL 全大写是内核的拼法，抄成 `baseUrl` 就是 `unexpected "baseUrl"`）。
    #[must_use]
    pub fn to_value(&self) -> Value {
        let mut request = json!({});
        for (key, value) in [
            ("provider", self.provider.as_deref()),
            ("baseURL", self.base_url.as_deref()),
            ("api", self.api.as_deref()),
            ("apiKey", self.api_key.as_deref()),
        ] {
            if let Some(value) = value {
                request[key] = json!(value);
            }
        }
        request
    }
}

/// `llm/discoverModels`：平铺两颗 `{settingsNs, request}`。两颗都**没有** `acceptsUndefined`
/// ⇒ 即使探针四颗全空，`request` 也必须发成 `{}`（主干那发 `new Dictionary` 序列化出来
/// 就是空对象，键照在）。回执是数组，见 [`parse_discovered_models`]。
#[must_use]
pub fn llm_discover_models(settings_ns: &str, request: &DiscoveryProbe) -> RpcCall {
    RpcCall::new(
        LLM_DISCOVER_MODELS,
        json!({ "settingsNs": settings_ns, "request": request.to_value() }),
    )
}

/// 「键在**且**为 `true`」——主干那几处凭据判据的逐字形状
/// （`TryGetProperty("configured", out var c) && c.ValueKind == JsonValueKind.True`）：
/// 缺键 false、`null` false、`"true"` 字符串也 false。不额外兜底、也不报错。
fn true_flag(record: &Value, field: &str) -> bool {
    record.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn opt_str(record: &Value, field: &str) -> Option<String> {
    record.get(field).and_then(Value::as_str).map(str::to_string)
}

/// `credentials/describe` 回帧里**一格**（`Record<ref, {configured, source?, writable}>`）。
///
/// 只收主干真读的两颗：`configured`（状态点、`removesCredential` 的前半）、
/// `writable`（同一处 AND 的后半）。`source` 描述符是 optional 且**主干从不读**
/// （全仓 `MainWindow*.cs` 里凭据格上没有 `GetProperty("source")`）⇒ 不落地，
/// 需要时读 [`CredentialState`] 的宿主自己从 raw 取（同 `DirectoryListing.home` 那一族的口径）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CredentialState {
    pub configured: bool,
    pub writable: bool,
}

impl CredentialState {
    #[must_use]
    pub fn from_cell(cell: &Value) -> Self {
        Self {
            configured: true_flag(cell, "configured"),
            writable: true_flag(cell, "writable"),
        }
    }
}

/// 按名取一格，**两态分得开**（RD3 已核、本轮复核）：
/// · `None` = 请求里报了这颗 ref，回帧**整格缺席**；
/// · `Some(configured:false)` = 格子在场但内核说「没配」。
///
/// 主干两条路径对这两态的处理**不一样**，所以这层不许把它们折成一态：
/// `LoadProviderRowsAsync` 走 `TryGetProperty(key)` —— 拿不到就 `HasCredential=false`、
/// `Credential` 保持 `Undefined`，删卡时 `removesCredential` 恒 false（不碰凭据库）；
/// `DescribeSearchCredentialsAsync` 走 `EnumerateObject()` —— 只遍历在场的格，
/// 缺席的 ref **连一行都不出**（网页搜索页就没那行密钥框）。
#[must_use]
pub fn credential_state(described: &Value, reference: &str) -> Option<CredentialState> {
    described.get(reference).map(CredentialState::from_cell)
}

/// 在场格的整表（`DescribeSearchCredentialsAsync` 那一型：遍历回帧自身，不按请求表）。
/// 回帧不是对象 ⇒ 空表：主干那处的 `if (described.ValueKind == Object)` 守卫与
/// 另一处 `EnumerateObject()` 抛异常被自家 `catch (Exception) { }` 吞掉，**两条终点都是空表**。
#[must_use]
pub fn parse_credential_states(described: &Value) -> Vec<(String, CredentialState)> {
    described
        .as_object()
        .map(|record| {
            record
                .iter()
                .map(|(key, cell)| (key.clone(), CredentialState::from_cell(cell)))
                .collect()
        })
        .unwrap_or_default()
}

/// `settings/describe` 的一个 ns = 主干 `_settingsSnapshot` 那张
/// `Dictionary<string, (JsonElement Value, double Revision)>` 的元素，逐字同构：
/// 只有 `ns`（主干 `GetProperty("ns")`，缺 = 硬失败）与 `revision`（`TryGetProperty` +
/// 缺省 0）在解析期落地，`schema` / `value` / `base` / `user` / `applies` / `secrets` /
/// `writable` 主干全是**事后按名宽容取**（`TryGetProperty`）⇒ 这里保留整颗 `raw`，
/// 不预先摊平、更不替主干决定「缺 `user` 该回落成什么」。
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsNamespace {
    pub ns: String,
    pub revision: f64,
    pub raw: Value,
}

impl SettingsNamespace {
    /// 主干 `snap.Value.TryGetProperty(name, …)` 的那次按名取值。
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.raw.get(name)
    }

    /// 主干 `AppendGenericPluginNsFallback` 逐字：`!TryGetProperty("writable", out w) || w.ValueKind != False`
    /// ⇒ **键缺席算可写**，只有显式 `false` 才判不可写（三态，别写成两态）。
    #[must_use]
    pub fn is_writable(&self) -> bool {
        self.field("writable").and_then(Value::as_bool) != Some(false)
    }
}

/// `settings/describe` → ns 快照表（顺序 = 回帧 `namespaces` 的原始顺序，主干 `foreach` 就是这个序）。
pub fn parse_settings_describe(described: &Value) -> Result<Vec<SettingsNamespace>, String> {
    const FROM: &str = "settings/describe 回执";
    let namespaces = match described.get("namespaces") {
        None => return Err(shape_missing("namespaces", FROM)),
        Some(value) => value
            .as_array()
            .ok_or_else(|| shape_off_type("namespaces", FROM))?,
    };
    namespaces
        .iter()
        .map(|entry| {
            let ns = req_str(entry, "ns", FROM)?;
            let revision = match entry.get("revision") {
                None => 0.0,
                Some(value) => value
                    .as_f64()
                    .ok_or_else(|| shape_off_type("revision", FROM))?,
            };
            Ok(SettingsNamespace {
                ns,
                revision,
                raw: entry.clone(),
            })
        })
        .collect()
}

/// `canOpenAgentPresetDirectory` 的回执：主干判据逐字 `v.ValueKind == JsonValueKind.True`，
/// 且整发被 `catch (Exception)` 包成「判定失败按不可用处理」⇒ **不是 `Result`**：
/// 抛由 [`Kernel::call`] 负责，形状不合（内核回了对象之类）与 `false` 在壳侧同一个终点。
#[must_use]
pub fn parse_can_open_preset_dir(value: &Value) -> bool {
    value.as_bool().unwrap_or(false)
}

/// `openAgentPresetDirectory` 的回执（`AgentPresetDirectoryOpenValue` 的两档 union）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresetDirectoryOpen {
    /// `{opened:true}` —— 内核已经原生打开。
    Opened,
    /// `{opened:false, path}` —— 打不开时把路径交回壳显示。
    /// 描述符里 false 档的 `path` 必填，主干却按 `TryGetProperty("path")` 取
    /// （拿不到就 `GetString()` 出 `null`，文案里照样插空串）⇒ 这里留 `Option`，不替内核造路径。
    NotOpened { path: Option<String> },
}

/// 两档的判分按主干那一行：`opened` 键在且为 `true` ⇒ 已开；其余（缺键、`false`、别的类型）
/// 一律走「把路径显示出来」那一支。多余键（内核真发了 `path` 配 `opened:true`）不进判据。
#[must_use]
pub fn parse_preset_directory_open(value: &Value) -> PresetDirectoryOpen {
    if value.get("opened").and_then(Value::as_bool) == Some(true) {
        return PresetDirectoryOpen::Opened;
    }
    PresetDirectoryOpen::NotOpened {
        path: opt_str(value, "path"),
    }
}

/// `llm/listConfigurableProviders` 的一条目录项（`LlmConfigurableProvider`）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LlmConfigurableProvider {
    pub provider: String,
    pub display_name: String,
    pub settings_ns: String,
    pub settings_path: Vec<String>,
    /// 三态：`None` = 键缺席（官方根，主干据此判「不可删」），`Some(true)` = 用户声明的自定义路由。
    pub declared: Option<bool>,
    pub error: Option<String>,
}

impl LlmConfigurableProvider {
    /// 主干 `provider.Length == 0 ⇒ continue`：空 provider 的行**不进 rows**。
    /// 与 `DirectoryListing::visible_entries` 同规 —— 解析不悄悄丢行，渲染侧才看得出
    /// 「内核发了颗空行」。**计数、渲染一律走这个方法**，别拿 `Vec::len()` 当条数。
    #[must_use]
    pub fn listed(&self) -> bool {
        !self.provider.is_empty()
    }
}

/// 目录数组（主干 `foreach (var p in (…).EnumerateArray())`：回帧不是数组 = 抛，
/// 且那处只 `catch (DshRpcException)` ⇒ 这里给 `Err`，与「内核没给/形状不对就抛」同一条口径）。
pub fn parse_configurable_providers(value: &Value) -> Result<Vec<LlmConfigurableProvider>, String> {
    const FROM: &str = "llm/listConfigurableProviders 回执";
    let rows = value
        .as_array()
        .ok_or_else(|| format!("bad-response: {FROM} 不是数组。"))?;
    rows
        .iter()
        .map(|row| {
            // `provider` 是这一行的身份：主干 `?? ""` 之后立刻 `continue`，但对**非字符串**
            // 会 `GetString()` 抛 ⇒ 这里缺席/`null` 折成空串、非字符串给 Err。
            let provider = match row.get("provider") {
                None | Some(Value::Null) => String::new(),
                Some(value) => value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| shape_off_type("provider", FROM))?,
            };
            let settings_path = row
                .get("settingsPath")
                .and_then(Value::as_array)
                .map(|segments| {
                    segments
                        .iter()
                        .filter_map(|segment| segment.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let display_name = opt_str(row, "displayName").unwrap_or_else(|| provider.clone());
            Ok(LlmConfigurableProvider {
                provider,
                display_name,
                settings_ns: opt_str(row, "settingsNs").unwrap_or_default(),
                settings_path,
                declared: row.get("declared").map(|_| true_flag(row, "declared")),
                error: opt_str(row, "error"),
            })
        })
        .collect()
}

/// `llm/discoverModels` 的一条候选（`{id, name?, contextWindow?, maxTokens?}`）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveredModel {
    pub id: String,
    pub name: Option<String>,
    pub context_window: i64,
    pub max_tokens: i64,
}

impl DiscoveredModel {
    /// 主干 `id.Length == 0 ⇒ continue`，同 [`LlmConfigurableProvider::listed`] 的口径。
    #[must_use]
    pub fn listed(&self) -> bool {
        !self.id.is_empty()
    }
}

/// 候选表。回帧不是数组 ⇒ **空表**而不是报错：主干那处外面就是
/// `if (models.ValueKind == JsonValueKind.Array)` 这道守卫，非数组直接落进
/// 「该提供方没有列出任何模型，请手动添加。」那一行。
#[must_use]
pub fn parse_discovered_models(value: &Value) -> Vec<DiscoveredModel> {
    value
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| DiscoveredModel {
                    name: opt_str(row, "name"),
                    // 主干 `TryGetProperty(...) && ValueKind == Number ? GetInt64() : 0`
                    // ⇒ 缺键与非数字都 0；分数值（违约形状）取不到整数时同样 0。
                    context_window: row.get("contextWindow").and_then(Value::as_i64).unwrap_or(0),
                    max_tokens: row.get("maxTokens").and_then(Value::as_i64).unwrap_or(0),
                    id: opt_str(row, "id").unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
/// RD9 表A · C5+C6（KW3 批）：十发 args 的逐字节钉子 + 六处回执形状的两态钉子。
///
/// 全部离线断言（构造与解析都是纯函数），零派发、零 `tests/ipc.rs`；自锁只指本文件
/// （`src/main.rs` 正被 MR2 改，锁它必然假红）。
mod kw3_rd9_settings_tests {
    use super::*;

    const KW3_SELF: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));

    fn keys(args: &Value) -> Vec<String> {
        args.as_object()
            .map(|record| record.keys().cloned().collect())
            .unwrap_or_default()
    }

    #[test]
    fn ten_consts_match_rd9_table_a_and_are_the_only_ten() {
        assert_eq!(RD9_C5_C6_METHODS.len(), 10);
        for method in RD9_C5_C6_METHODS {
            assert!(
                KW3_SELF.contains(&format!("\"{method}\"")),
                "{method} 的 const 不在这份源码里"
            );
        }
        // 逐颗方法名（宿主拿 [`RpcCall::method`] 直接 `kernel.call`，这里钉住它没被写歪）
        assert_eq!(credentials_describe(&["a"]).method, "credentials/describe");
        assert_eq!(credentials_set("r", "v").method, "credentials/set");
        assert_eq!(credentials_unset("r").method, "credentials/unset");
        assert_eq!(settings_describe().method, "settings/describe");
        assert_eq!(
            settings_replace("n", &json!({}), None).method,
            "settings/replace"
        );
        assert_eq!(
            settings_update("n", &json!({}), None).method,
            "settings/update"
        );
        assert_eq!(
            settings_can_open_agent_preset_directory().method,
            "settings/canOpenAgentPresetDirectory"
        );
        assert_eq!(
            settings_open_agent_preset_directory("p").method,
            "settings/openAgentPresetDirectory"
        );
        assert_eq!(
            llm_list_configurable_providers().method,
            "llm/listConfigurableProviders"
        );
        assert_eq!(
            llm_discover_models("llm-deepseek", &DiscoveryProbe::default()).method,
            "llm/discoverModels"
        );
    }

    #[test]
    fn none_of_the_ten_wraps_its_arguments_in_a_request_envelope() {
        // 本仓已确证过的坑（`session/selectModel` 反过来**必须**裹）。这十发全是 `direct` +
        // 顶层 `wire` 名 ⇒ 裹一层就是 `unexpected "request"`。`llm/discoverModels` 的 `request`
        // 是描述符的第二颗命名参数，比的是「外层只两颗、`request` 里面还有一颗 `settingsNs`」这型错。
        for call in [
            credentials_describe(&["a"]),
            credentials_set("r", "v"),
            credentials_unset("r"),
            settings_describe(),
            settings_replace("n", &json!({}), Some(1.0)),
            settings_update("n", &json!({}), Some(1.0)),
            settings_can_open_agent_preset_directory(),
            settings_open_agent_preset_directory("p"),
            llm_list_configurable_providers(),
            llm_discover_models("ns", &DiscoveryProbe::default()),
        ] {
            assert!(
                !keys(&call.args).contains(&"request".to_string())
                    || call.method == LLM_DISCOVER_MODELS,
                "{} 又去裹 request 了：{}",
                call.method,
                call.args
            );
        }
        // 零颗的三发：`{}`，一颗都不能有
        assert_eq!(settings_describe().args.to_string(), "{}");
        assert_eq!(settings_can_open_agent_preset_directory().args.to_string(), "{}");
        assert_eq!(llm_list_configurable_providers().args.to_string(), "{}");
        // 内层也不许长出第二层
        let models = llm_discover_models("ns", &DiscoveryProbe::default()).args;
        assert_eq!(keys(&models), ["request", "settingsNs"]);
        assert_eq!(models["request"], json!({}));
        assert!(models.get("settingsNs").is_some());
    }

    #[test]
    fn credentials_builders_use_the_unescaped_ref_key_and_always_send_refs() {
        // `@ref` 是 C# 的关键字转义，上线键名是 `ref`；写成 `"@ref"` 两头都错。
        assert_eq!(
            credentials_set("DEEPSEEK_API_KEY", "sk-1").args.to_string(),
            r#"{"ref":"DEEPSEEK_API_KEY","value":"sk-1"}"#
        );
        assert_eq!(
            credentials_unset("web-search-key").args.to_string(),
            r#"{"ref":"web-search-key"}"#
        );
        assert_eq!(
            credentials_describe(&["deepseek", "web"]).args.to_string(),
            r#"{"refs":["deepseek","web"]}"#
        );
        // 空数组合法、键必须在（发 `{}` 就是 `missing "refs"`）
        assert_eq!(credentials_describe(&[]).args.to_string(), r#"{"refs":[]}"#);
        assert_eq!(keys(&credentials_describe(&[]).args), ["refs"]);
        // 明文原样送出：空串也在（trim 属于 UI 取值侧）
        assert_eq!(credentials_set("k", "").args["value"], json!(""));
    }

    #[test]
    fn expected_revision_is_three_state_on_replace_and_update() {
        let bare = settings_replace("llm-deepseek", &json!({}), None);
        assert_eq!(bare.args.to_string(), r#"{"ns":"llm-deepseek","section":{}}"#);
        let guarded = settings_replace("llm-deepseek", &json!({"a": 1}), Some(12.0));
        assert_eq!(
            keys(&guarded.args),
            ["expectedRevision", "ns", "section"] // serde_json 的 Map 是 BTreeMap ⇒ 升序
        );
        assert_eq!(guarded.args["expectedRevision"], json!(12.0));
        assert_eq!(guarded.args["section"], json!({"a": 1}));

        // 主干两处 update 调用点都不带 revision
        let update = settings_update(
            "agent-presets",
            &json!({"default": "preset-1"}),
            None,
        );
        assert_eq!(
            update.args.to_string(),
            r#"{"ns":"agent-presets","patch":{"default":"preset-1"}}"#
        );
        assert_eq!(
            settings_update("ui-onboarding", &json!({"welcomeNoticeVersion": 3}), Some(0.0))
                .args["expectedRevision"],
            json!(0.0)
        );
    }

    #[test]
    fn discovery_probe_sends_request_even_when_empty_and_keeps_empty_provider() {
        assert_eq!(
            llm_discover_models("llm-pi-ai", &DiscoveryProbe::default())
                .args
                .to_string(),
            r#"{"request":{},"settingsNs":"llm-pi-ai"}"#
        );
        let probe = DiscoveryProbe {
            provider: Some(String::new()), // 主干：非自定义卡就发这颗键，空串也发
            base_url: Some("https://gw.internal/v1".to_string()),
            api: None,
            api_key: Some("sk-probe".to_string()),
        };
        assert_eq!(
            llm_discover_models("llm-deepseek", &probe).args.to_string(),
            r#"{"request":{"apiKey":"sk-probe","baseURL":"https://gw.internal/v1","provider":""},"settingsNs":"llm-deepseek"}"#
        );
        // `baseURL` 的 URL 全大写是内核拼法
        assert_eq!(keys(&probe.to_value()), ["apiKey", "baseURL", "provider"]);
    }

    #[test]
    fn credential_describe_keeps_absent_cell_and_present_false_apart() {
        let described = json!({
            "deepseek": {"configured": true, "writable": true, "source": "user"},
            "web": {"configured": false, "writable": true},
            "locked": {"configured": false, "writable": false},
        });
        // 两态：在场且 false ≠ 整格缺席
        assert_eq!(
            credential_state(&described, "web"),
            Some(CredentialState { configured: false, writable: true })
        );
        assert_eq!(credential_state(&described, "never-reported"), None);
        assert_eq!(
            credential_state(&described, "deepseek"),
            Some(CredentialState { configured: true, writable: true })
        );
        // 删卡判据 = configured AND writable（`removesCredential` 那一支）
        let removable = |key: &str| {
            credential_state(&described, key)
                .is_some_and(|state| state.configured && state.writable)
        };
        assert!(removable("deepseek"));
        assert!(!removable("locked"));
        assert!(!removable("never-reported"));
        // 「键在且为 true」：缺键 / null / 字符串都 false，不抛
        let odd = json!({"a": {"configured": null, "writable": "true"}, "b": {}});
        assert_eq!(
            parse_credential_states(&odd),
            vec![
                ("a".to_string(), CredentialState::default()),
                ("b".to_string(), CredentialState::default()),
            ]
        );
        // 非对象回帧 = 空表（主干两处守卫的终点一致）
        assert_eq!(parse_credential_states(&json!([])), Vec::new());
    }

    #[test]
    fn settings_describe_snapshot_lands_only_ns_and_revision_and_keeps_raw() {
        let described = json!({
            "writable": true,
            "hasDocument": true,
            "namespaces": [
                {"ns": "locale", "revision": 3.0, "value": {"preference": "zh"}, "schema": null},
                {"ns": "agent-default-model", "applies": "restart", "user": {"x": 1}},
            ]
        });
        let snapshot = parse_settings_describe(&described).expect("describe 回执该解析得动");
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].ns, "locale");
        assert_eq!(snapshot[0].revision, 3.0);
        assert_eq!(snapshot[0].field("value"), Some(&json!({"preference": "zh"})));
        assert_eq!(snapshot[0].field("user"), None);
        // `revision` 缺席 = 主干的 `TryGetProperty … : 0`
        assert_eq!(snapshot[1].revision, 0.0);
        assert_eq!(snapshot[1].field("applies"), Some(&json!("restart")));
        // 未落地的键照样取得到（主干事后按名取）
        assert_eq!(
            snapshot[1].field("secrets"),
            None,
            "内核没发 secrets 时不许造空表"
        );

        for (bad, needle) in [
            (json!({}), "namespaces"),
            (json!({"namespaces": {}}), "namespaces"),
            (json!({"namespaces": [{"revision": 1}]}), "ns"),
            (
                json!({"namespaces": [{"ns": "a", "revision": "3"}]}),
                "revision",
            ),
        ] {
            let error = parse_settings_describe(&bad).expect_err("违约形状该给 Err");
            assert!(error.contains(needle), "{needle} 没出现在判词里：{error}");
            assert!(error.starts_with("bad-response:"), "判词前缀走本文件既有口径：{error}");
        }
    }

    #[test]
    fn namespace_writable_is_three_state_with_absent_key_reading_writable() {
        let entry = |raw: Value| SettingsNamespace { ns: "x".to_string(), revision: 0.0, raw };
        assert!(entry(json!({})).is_writable(), "键缺席算可写（主干那条 `||` 判据）");
        assert!(entry(json!({"writable": true})).is_writable());
        assert!(!entry(json!({"writable": false})).is_writable());
        assert!(
            entry(json!({"writable": null})).is_writable(),
            "null 不是显式 false，主干照样放行"
        );
    }

    #[test]
    fn preset_directory_receipts_are_the_two_shapes_mainline_reads() {
        assert!(parse_can_open_preset_dir(&json!(true)));
        assert!(!parse_can_open_preset_dir(&json!(false)));
        assert!(!parse_can_open_preset_dir(&json!({"canOpen": true})), "对象不是裸布尔 ⇒ 判不可开");
        assert!(!parse_can_open_preset_dir(&Value::Null));

        assert_eq!(
            parse_preset_directory_open(&json!({"opened": true})),
            PresetDirectoryOpen::Opened
        );
        assert_eq!(
            parse_preset_directory_open(&json!({"opened": false, "path": "C:/presets"})),
            PresetDirectoryOpen::NotOpened { path: Some("C:/presets".to_string()) }
        );
        // false 档缺 path：主干 `TryGetProperty` 取不到就显示空路径，不造兜底
        assert_eq!(
            parse_preset_directory_open(&json!({})),
            PresetDirectoryOpen::NotOpened { path: None }
        );
        assert_eq!(
            parse_preset_directory_open(&json!({"opened": "true"})),
            PresetDirectoryOpen::NotOpened { path: None }
        );
    }

    #[test]
    fn provider_catalog_follows_the_mainline_tolerant_lookups() {
        let rows = json!([
            {"provider": "deepseek-official", "displayName": "DeepSeek",
             "settingsNs": "llm-deepseek", "settingsPath": []},
            {"provider": "my-gateway", "settingsNs": "llm-pi-ai",
             "settingsPath": ["providers", "my-gateway"], "declared": true},
            {"provider": "declared-off", "settingsNs": "llm-pi-ai",
             "settingsPath": ["providers"], "declared": false},
            {"provider": "", "displayName": 7, "settingsPath": ["a", 5, null], "error": "boom"},
            42,
        ]);
        let parsed = parse_configurable_providers(&rows).expect("数组回执该解析得过");
        assert_eq!(parsed.len(), 5, "解析不丢行：五颗全在场");
        assert!(parsed[0].listed());
        assert_eq!(parsed[0].declared, None, "官方根没有 declared 键 = 三态里的 None");
        assert_eq!(parsed[1].declared, Some(true));
        assert_eq!(parsed[2].declared, Some(false), "declared:false 是第三态，不是 None");
        assert_eq!(parsed[1].display_name, "my-gateway", "displayName 缺席回落 provider");
        assert_eq!(parsed[1].settings_path, ["providers", "my-gateway"]);
        assert!(!parsed[3].listed(), "空 provider = 主干 continue 的那一行");
        assert_eq!(parsed[3].settings_path, ["a"], "非字符串段跳过（主干那条 if 就是这判据）");
        assert_eq!(parsed[3].error.as_deref(), Some("boom"));
        assert_eq!(parsed[3].display_name, "", "回落链落在空 provider 上");
        // 计数走 listed()，别拿 len 当条数
        assert_eq!(parsed.iter().filter(|row| row.listed()).count(), 3);

        let error = parse_configurable_providers(&json!({})).expect_err("非数组该抛");
        assert!(error.starts_with("bad-response:"), "{error}");
        assert_eq!(
            parse_configurable_providers(&json!([{"displayName": "x"}])).expect("provider 缺席合法")
                [0]
                .provider,
            ""
        );
    }

    #[test]
    fn discovered_models_non_array_is_empty_not_an_error() {
        let models = json!([
            {"id": "deepseek-chat", "name": "DeepSeek Chat", "contextWindow": 128000, "maxTokens": 8192},
            {"id": "bare"},
            {"id": "", "name": "要被主干丢掉的一行"},
            {"id": "weird", "contextWindow": "128000", "maxTokens": true},
        ]);
        let parsed = parse_discovered_models(&models);
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0].context_window, 128000);
        assert_eq!(parsed[0].max_tokens, 8192);
        assert_eq!(parsed[1].name, None);
        assert_eq!(parsed[1].context_window, 0, "缺键 = 主干那条 `: 0`");
        assert!(!parsed[2].listed());
        assert_eq!(parsed[3].context_window, 0, "非数字 = 0，不抛");
        assert_eq!(parsed[3].max_tokens, 0);
        assert_eq!(
            parsed.iter().filter(|model| model.listed()).count(),
            3
        );
        // 回帧不是数组 ⇒ 空表（主干外面那道 `ValueKind == Array` 守卫）
        assert_eq!(parse_discovered_models(&json!({"models": []})), Vec::new());
        assert_eq!(parse_discovered_models(&Value::Null), Vec::new());
    }

    #[test]
    fn unconsumed_receipts_get_no_parser_here() {
        // 主干对 `credentials/set|unset`（`z.void()`）与 `settings/replace|update`
        // （`#SettingsNamespaceView`）都是「await 但不消费」：成败只看抛不抛。
        // 本层跟着不造解析器 —— 造了就是比主干多一道判据（KW2 的 `cordis_resolve_request_run` 同规）。
        // needle 用 `concat!` 拆开，否则本测试自己的文字就会命中自己（正向恒真、反向恒假）。
        for forbidden in [
            concat!("parse_credentials_", "set_receipt"),
            concat!("parse_credentials_", "unset_receipt"),
            concat!("parse_settings_", "replace_receipt"),
            concat!("parse_settings_", "update_receipt"),
        ] {
            assert!(
                !KW3_SELF.contains(forbidden),
                "本层给主干不消费的回执造了判据：{forbidden}"
            );
        }
        // 锁不是空转：三处「不消费」的判据文字都真在本文件里（否则反向零命中只是因为没写过）
        assert!(KW3_SELF.contains("z.void()"), "void 那两发的形状说明没落进来");
        assert!(KW3_SELF.contains("await 但不消费"), "「不消费」这条口径没落进来");
        assert!(
            KW3_SELF.contains("SettingsNamespaceView"),
            "replace/update 的回执符号名没写进注释 ⇒ 下一批会以为漏了东西"
        );
    }
}

// ==================== RD9 表A · C4+C9：agentPresets 4 + sessionFeedback/record 的构造与回执 ====================
//
// 本节只有两件事：进 `payload.args` 的那棵树、`Kernel::call` 剥完**第一层** `ok` 之后剩下的那格形状。
// 宿主与派发在 `main.rs`（本批一颗不碰），UI 一颗不碰，`tests/ipc.rs` 只读复用不写。
// 追加在文件末尾的理由同上一批那条「源码锁按起始锚 → 结束锚首次出现开窗」的说明。
//
// 真值来源（本轮逐字回源码复核；按本仓纪律只写符号锚，不写主干行号）：
//   · `agentPresets/read`  = 主干 `ShowPresetCompositionAsync`（`MainWindow.SettingsExtras.cs`），
//     `CallOkAsync("agentPresets/read", new { agentPreset = id })`；回帧只读两颗：
//     `name`（`TryGetProperty` + 是串才要，否则 `null` 回落对话框标题）、
//     `content`（`TryGetProperty` + 是串才要，否则空串）。
//   · `agentPresets/copy`  = 主干 `CopyPresetAsync`（`MainWindow.xaml.cs`），
//     `new { from = fromId, id = newId, name = nameBox.Text.Trim() }`。
//   · `agentPresets/deletePreset` = 主干 `DeletePresetAsync`（同文件），`new { id }`。
//   · `agentPresets/select` = 主干 `SelectPresetAsync`（同文件）与 `StartCreatorDraftAsync`
//     （`MainWindow.SettingsExtras.cs`）**两处**，形状同一 `new { agentId = sid, agentPreset = id }`；
//     两处的差别只在**回帧消不消费**：`SelectPresetAsync` 读 `applied.GetString()`（回执是**裸串**
//     = 生效的预设 id），`StartCreatorDraftAsync` 整个丢弃且只 `catch (DshRpcException)`
//     （那一支的用途是「agent 已启动锁死 ⇒ 改建新会话」，靠的是错误码不是回帧）。
//   · `sessionFeedback/record` = 主干 `OnSessionFeedbackClick`（`MainWindow.xaml.cs`）：
//     `CallAsync`（**不是** `CallOkAsync`）+ `new { request }`，`request` 是
//     `Dictionary<string, object>` 且「非空才 `Add`」⇒ `sessionId` 必有，`text`/`category`
//     两态是**键整个不存在**（主干注释明文：带 `null` 会被内核的 zod 边界拒）。
//   · 键集闸 = `assertExactArguments`（`@deepseek-ai/dsh-api-gateway/lib/types/index.js`）按
//     `Reflect.ownKeys` 比**集合**：多一颗 `unexpected`、少一颗 `missing`，只有声明
//     `acceptsUndefined` 的那颗允许缺席。假内核那一面的镜像 = `wire_fields_error`
//     （`src/bin/fake_dsh.rs`），本轮逐颗读到的 spec 是
//     `read ["agentPreset"]` / `copy ["from","id","?name"]` / `deletePreset ["id"]` /
//     `select ["agentId","agentPreset"]` ⇒ **`copy` 的 `name` 是唯一那颗可选**（`?` 前缀），
//     其余全必填。
//
// **四发平铺、一发裹 `request`**：`agentPresets/*` 四颗的 wire 字段就是顶层键名，裹一层
// `{request:…}` 立刻 `unexpected "request"`；`sessionFeedback/record` 反过来**必须**裹
// （`CallAsync` 那一族的 `request` 是描述符的命名参数，与 `workspace/*` 那六发同型）。
// 同族可选性逐颗现测过，没有按族推：`copy.name` 可选，而 `read.agentPreset` /
// `deletePreset.id` / `select` 两颗都不可省。
//
// **C3（那族第四批想补的颗）不在本节**：主干那四颗包装除定义行外全仓零调用方，且那颗
// partial 自己的架构说明写的是「桌面壳不可达，不伪造调用」；`kw2_cordis_tests` 已按这一事实
// 下了源码反向锁。本轮复核**确认 KW2 的判据成立**，故不在此建常量/构造器（详见批报告 §3），
// 并把这一事实钉成 `kw4_rd9_tests` 里的第二道锁。

/// 五发的方法名（集中一处；下一批 `main.rs` 与假内核桩都按这组常量对表，改不漏）。
pub const AGENT_PRESETS_READ: &str = "agentPresets/read";
pub const AGENT_PRESETS_COPY: &str = "agentPresets/copy";
pub const AGENT_PRESETS_DELETE: &str = "agentPresets/deletePreset";
pub const AGENT_PRESETS_SELECT: &str = "agentPresets/select";
pub const SESSION_FEEDBACK_RECORD: &str = "sessionFeedback/record";

/// C4 + C9 的名单（基数锁用；测试按「恰五颗」与「逐颗在源码里有 const」两头钉）。
pub const RD9_C4_C9_METHODS: &[&str] = &[
    AGENT_PRESETS_COPY,
    AGENT_PRESETS_DELETE,
    AGENT_PRESETS_READ,
    AGENT_PRESETS_SELECT,
    SESSION_FEEDBACK_RECORD,
];

/// 内核 `FEEDBACK_CATEGORIES` 的七个线值，**顺序与主干 `SessionFeedbackCategories` 一致**
/// （主干那表是 `(Wire, Label)` 对，`Wire` 就是这个顺序；下拉框的呈现顺序按它，不重排）。
/// 主干传的是 `SelectedIndex >= 0 ? …Wire : null`，而 `null` 那一支走的是「不给 `category` 键」
/// ⇒ 本层用 `Option` 表达（见 [`session_feedback_record`]），不在这里造「其他」兜底值。
pub const SESSION_FEEDBACK_CATEGORIES: [&str; 7] = [
    "task-result",
    "instruction-following",
    "product-interaction",
    "service-stability",
    "resource-cost",
    "security-privacy-permission",
    "other",
];

/// `agentPresets/read`（主干 `ShowPresetCompositionAsync`）：一颗必填 `agentPreset` ⇒ 平铺
/// `{agentPreset}`。传的是预设 id（不是显示名），主干那处的 `id` 来自设置页行上的按钮 Tag。
#[must_use]
pub fn agent_presets_read(agent_preset: &str) -> RpcCall {
    RpcCall::new(AGENT_PRESETS_READ, json!({ "agentPreset": agent_preset }))
}

/// `agentPresets/copy`（主干 `CopyPresetAsync`）：平铺 `{from, id[, name]}`。
///
/// `name` 是本批**唯一**那颗可选（描述符带 `acceptsUndefined`，假内核的 spec 是 `?name`）：
/// `None` ⇒ 键整个缺席（**不是** `"name":null`，那颗 union 不放行 null）；`Some` ⇒ 键在。
/// 主干两处实参都过了 `Trim()`（`newId`、`nameBox.Text`）—— 取值动作属于 UI 侧，本层不 trim、
/// 不校验非空，`Some("")` 是合法形状（内核拿它当「没给名字」由假内核那侧回落 `from`）。
/// 回帧 `z.void()` ⇒ 主干整个丢弃（见 [`RD9_C4_C9_METHODS`] 末段的反向锁）。
#[must_use]
pub fn agent_presets_copy(from: &str, id: &str, name: Option<&str>) -> RpcCall {
    let mut args = json!({ "from": from, "id": id });
    if let Some(name) = name {
        args["name"] = json!(name);
    }
    RpcCall::new(AGENT_PRESETS_COPY, args)
}

/// `agentPresets/deletePreset`（主干 `DeletePresetAsync`）：一颗必填 `id` ⇒ 平铺 `{id}`。
/// 二次确认与「删完重拉 `agent-presets` 那一节」都在宿主；回帧同 `copy` = `z.void()` 不消费。
#[must_use]
pub fn agent_presets_delete(id: &str) -> RpcCall {
    RpcCall::new(AGENT_PRESETS_DELETE, json!({ "id": id }))
}

/// `agentPresets/select`（主干 `SelectPresetAsync` 与 `StartCreatorDraftAsync` 两处）：
/// 平铺两颗必填 `{agentId, agentPreset}`。`agentId` 就是**会话 id**（主干传
/// `Volatile.Read(ref _activeSessionId)` / 新建那发的 `sid`），不是 agent 名。
/// 常见失败是 `agent-preset/locked`（该会话的 agent 已启动 ⇒ 预设随之钉死），那是
/// [`Kernel::call`] 的 `Err` 而不是回帧里的判据。回执形状见 [`parse_agent_presets_select`]。
#[must_use]
pub fn agent_presets_select(agent_id: &str, agent_preset: &str) -> RpcCall {
    RpcCall::new(
        AGENT_PRESETS_SELECT,
        json!({ "agentId": agent_id, "agentPreset": agent_preset }),
    )
}

/// `sessionFeedback/record`（主干 `OnSessionFeedbackClick`）：**唯一裹 `request` 的那发**，
/// 形状是 `{request:{sessionId[, text][, category]}}`。
///
/// 两颗可选的落点是「键不存在」，与主干那三行 `if (…) request[…] = …` 逐字同构：
/// `None` = 主干没 `Add`（空文本 / 未选分类），`Some("")` 是合法形状但不来自主干
/// （空地判在 `Trim()` 之后，属取值侧）。`sessionId` 必填 ⇒ 恒发。
///
/// 用 `CallAsync` 而非 `CallOkAsync` 的理由（主干注释自陈）：业务失败也带信封 `ok:true`
/// —— `session-not-found` 是回帧里的第二层 `ok:false`，不是网关错误。分叉 [`Kernel::call`]
/// 只剥第一层，剥完就是 [`parse_session_feedback_record`] 吃的那格。
#[must_use]
pub fn session_feedback_record(
    session_id: &str,
    text: Option<&str>,
    category: Option<&str>,
) -> RpcCall {
    let mut request = json!({ "sessionId": session_id });
    for (key, value) in [("text", text), ("category", category)] {
        if let Some(value) = value {
            request[key] = json!(value);
        }
    }
    RpcCall::new(SESSION_FEEDBACK_RECORD, json!({ "request": request }))
}

/// `agentPresets/read` 的回执里主干真读的两格（其余 `agentPreset` / `trust` / `description`
/// 内核确实发，但那一处只渲染 `name` 与 `content` ⇒ 不落地，需要时从 raw 取）。
///
/// 两态分得开：`name` 是 `Option` —— 主干「缺键或非串 ⇒ `null`，再回落对话框标题」，
/// 标题在宿主手里，本层不猜；`content` 是 `String` —— 主干那两态（缺键 / 非串）的终点
/// 都是空串（`: ""`），折成一态无损，空串的「这文档是空的」提示属宿主文案。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentPresetComposition {
    pub name: Option<String>,
    pub content: String,
}

/// 读法逐字照主干的两段 `TryGetProperty(...) && ValueKind == String`：非对象回帧
/// （内核 `null` / 串 / 数字）在主干是**抛** `InvalidOperationException`（那颗 `catch` 只接
/// `DshRpcException` ⇒ 会漏出到调用方），这里折成全默认值 —— 一条**比主干软**的偏离，
/// 已在批报告 §备案里给主干原码与理由（与 `parse_cordis_row` 同型待决案）。
#[must_use]
pub fn parse_agent_presets_read(value: &Value) -> AgentPresetComposition {
    AgentPresetComposition {
        name: opt_str(value, "name"),
        content: opt_str(value, "content").unwrap_or_default(),
    }
}

/// `agentPresets/select` 的回执：内核回的是**生效的预设 id 那一颗裸串**（假内核那侧
/// `json!(id)`），不是 `{applied:…}` 那种对象。主干 `applied.GetString()` 两态：
/// 串 ⇒ 插进文案；**非串 ⇒ 抛**（同上一颗那条偏离）。这里给 `Option<String>`，
/// `None` = 「内核没按契约回串」，宿主决定显示什么 —— 不替它把 id 编出来。
#[must_use]
pub fn parse_agent_presets_select(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

/// `sessionFeedback/record` **剥完第一层 `ok`** 之后的那一格（= 主干 `envelope.value`）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionFeedbackReceipt {
    /// 主干 `value.ok` 是**字面 `true`** ⇒ 「已记录」。多余键不管。
    Recorded,
    /// 其余全在这一支：`ok` 缺 / `false` / `null` / 回帧不是对象（主干那两段的
    /// `ValueKind == Object` 守卫）都算失败。`code` 取 `value.error.code`（主干
    /// `Str(err, "code")`，且只认 `error` 是对象）；`None` = 主干折成空串那一型，
    /// 「空串显示成什么」属宿主文案，这里不替它归并成某个具体码。
    Rejected { code: Option<String> },
}

/// 判据逐字照主干那两段：先看 `value` 是对象**且** `ok` 是字面 `true`，否则去 `error.code`。
///
/// 一处**内核自陈与代码不合**的地方（本轮现测）：主干那段注释写的是
/// 「返回 `{ok:true, value:{recorded:true}}`」，而它自己的读法读的是 `value.ok`——
/// `recorded` 那颗键全仓没人读。这里跟**代码**不跟注释：`{recorded:true}`（没有 `ok`）
/// 判 `Rejected`，并由单测钉住，免得下一批照注释补一条 `recorded` 臂造出主干没有的行为。
#[must_use]
pub fn parse_session_feedback_record(value: &Value) -> SessionFeedbackReceipt {
    if value.is_object() && true_flag(value, "ok") {
        return SessionFeedbackReceipt::Recorded;
    }
    SessionFeedbackReceipt::Rejected {
        code: value
            .get("error")
            .filter(|error| error.is_object())
            .and_then(|error| opt_str(error, "code")),
    }
}

#[cfg(test)]
/// RD9 表A · C4+C9（KW4 批）：五发 args 的逐字节钉子 + 三处回执形状的两态钉子
/// + C3 那四颗的**第二道**源码反向锁 + 主干不消费的回执不造判据。
///
/// 全部离线断言（构造与解析都是纯函数），零派发、零 `tests/ipc.rs`；自锁只指本文件
/// （`src/main.rs` 正被 MR2 改，锁它必然假红）。
mod kw4_rd9_tests {
    use super::*;

    const KW4_SELF: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));

    fn keys(args: &Value) -> Vec<String> {
        args.as_object()
            .map(|record| record.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// serde_json 的 Map 是 BTreeMap ⇒ 落盘串是**键名升序**；网关按 `Reflect.ownKeys` 比集合 ⇒ 序无关。
    /// 这里比字节串，钉的是「一颗不多、一颗不少、值原样」。
    fn sample_id() -> &'static str {
        "my-reviewer"
    }

    #[test]
    fn the_five_consts_are_rd9_c4_c9_and_are_all_present_in_this_file() {
        assert_eq!(RD9_C4_C9_METHODS.len(), 5);
        for method in RD9_C4_C9_METHODS {
            assert!(
                KW4_SELF.contains(&format!("\"{method}\"")),
                "{method} 的 const 不在这份源码里"
            );
        }
        assert_eq!(agent_presets_read("p").method, "agentPresets/read");
        assert_eq!(agent_presets_copy("f", "t", None).method, "agentPresets/copy");
        assert_eq!(agent_presets_delete("p").method, "agentPresets/deletePreset");
        assert_eq!(
            agent_presets_select("s-1", "p").method,
            "agentPresets/select"
        );
        assert_eq!(
            session_feedback_record("s-1", None, None).method,
            "sessionFeedback/record"
        );
    }

    #[test]
    fn exactly_one_of_the_five_wraps_its_arguments_in_a_request_object() {
        // 本仓已确证过的坑（`workspace/*` 那六发反过来**必须**裹）。这四发是 `direct` +
        // 顶层 `wire` 名 ⇒ 裹一层就是 `unexpected "request"` + `missing "from"`。
        for call in [
            agent_presets_read("p"),
            agent_presets_copy("f", "t", Some("n")),
            agent_presets_delete("t"),
            agent_presets_select("s-1", "p"),
        ] {
            assert!(
                !keys(&call.args).contains(&"request".to_string()),
                "{} 又去裹 request 了：{}",
                call.method,
                call.args
            );
        }
        let record = session_feedback_record("s-1", Some("t"), Some("other")).args;
        assert_eq!(keys(&record), ["request"]);
        assert_eq!(keys(&record["request"]), ["category", "sessionId", "text"]);
        // 外层只准有 `request` 这一颗：内层的 `sessionId` 不许同时长到外面
        assert!(record.get("sessionId").is_none());
    }

    #[test]
    fn preset_read_and_delete_send_their_single_required_key() {
        assert_eq!(
            agent_presets_read(sample_id()).args.to_string(),
            r#"{"agentPreset":"my-reviewer"}"#
        );
        assert_eq!(
            agent_presets_delete(sample_id()).args.to_string(),
            r#"{"id":"my-reviewer"}"#
        );
        // 必填就是必填：空串照发（内核那边回 `agent-preset/invalid`，不是壳的活）
        assert_eq!(agent_presets_read("").args["agentPreset"], json!(""));
        assert_eq!(agent_presets_delete("").args["id"], json!(""));
    }

    #[test]
    fn preset_copy_name_is_the_only_optional_key_and_absent_is_not_null() {
        assert_eq!(
            agent_presets_copy(sample_id(), "pk-copy", Some("复盘助手")).args.to_string(),
            r#"{"from":"my-reviewer","id":"pk-copy","name":"复盘助手"}"#
        );
        let bare = agent_presets_copy(sample_id(), "pk-copy", None);
        assert_eq!(bare.args.to_string(), r#"{"from":"my-reviewer","id":"pk-copy"}"#);
        assert!(!keys(&bare.args).contains(&"name".to_string()));
        assert_ne!(
            bare.args.to_string(),
            r#"{"from":"my-reviewer","id":"pk-copy","name":null}"#,
            "`None` 被写成了显式 null —— 那颗可选键的 union 不放行 null"
        );
        // `Some("")` 合法（主干的 `Trim()` 结果就能是空串），本层不替它丢掉
        assert_eq!(
            agent_presets_copy("f", "t", Some("")).args["name"],
            json!("")
        );
    }

    #[test]
    fn preset_select_sends_session_id_as_agent_id_with_both_keys_required() {
        assert_eq!(
            agent_presets_select("s-1001", sample_id()).args.to_string(),
            r#"{"agentId":"s-1001","agentPreset":"my-reviewer"}"#
        );
        // 两处主干调用点都是这两颗、都不缺席：空串也要把键留在（缺席 = `missing`）
        let empty = agent_presets_select("", "");
        assert_eq!(keys(&empty.args), ["agentId", "agentPreset"]);
        assert_eq!(empty.args["agentId"], json!(""));
    }

    #[test]
    fn preset_read_receipt_keeps_absent_name_and_blank_content_apart() {
        let full = parse_agent_presets_read(&json!({
            "agentPreset": "my-reviewer", "trust": "user",
            "name": "复盘助手", "content": "# 复盘助手\n", "description": "d",
        }));
        assert_eq!(full.name.as_deref(), Some("复盘助手"));
        assert_eq!(full.content, "# 复盘助手\n");
        // 缺 `name` = 主干的 `null` ⇒ 回落宿主标题，本层不猜
        let blank = parse_agent_presets_read(&json!({"agentPreset": "x", "content": ""}));
        assert_eq!(blank, AgentPresetComposition { name: None, content: String::new() });
        // 非串与缺键同一终点（主干那两段守卫的落点都是默认值）
        assert_eq!(
            parse_agent_presets_read(&json!({"name": 7, "content": null})),
            AgentPresetComposition { name: None, content: String::new() }
        );
        // 派生默认值 = 非对象回帧的落点（见函数注释那条与主干的偏离）
        assert_eq!(parse_agent_presets_read(&Value::Null), Default::default());
    }

    #[test]
    fn preset_select_receipt_is_a_bare_string_not_a_wrapped_object() {
        assert_eq!(
            parse_agent_presets_select(&json!("my-reviewer")).as_deref(),
            Some("my-reviewer")
        );
        for dirty in [
            json!({"applied": "my-reviewer"}),
            json!(null),
            json!(7),
            Value::Null,
        ] {
            assert_eq!(
                parse_agent_presets_select(&dirty),
                None,
                "{dirty} 被读成了生效的预设 id"
            );
        }
        // 空串是合法回执（主干 `GetString()` 给得出空串，照样进文案）
        assert_eq!(parse_agent_presets_select(&json!("")).as_deref(), Some(""));
    }

    #[test]
    fn record_request_omits_text_and_category_entirely_when_absent() {
        assert_eq!(
            session_feedback_record("s-1", None, None).args.to_string(),
            r#"{"request":{"sessionId":"s-1"}}"#
        );
        assert_eq!(
            session_feedback_record("s-1", Some("很卡"), None).args.to_string(),
            r#"{"request":{"sessionId":"s-1","text":"很卡"}}"#
        );
        assert_eq!(
            session_feedback_record("s-1", None, Some("resource-cost")).args.to_string(),
            r#"{"request":{"category":"resource-cost","sessionId":"s-1"}}"#
        );
        // 显式 null 是主干明令禁止的形状（「带 null 会被内核 zod 边界拒」）
        let args = session_feedback_record("s-1", None, None).args;
        assert_ne!(args["request"].to_string(), r#"{"sessionId":"s-1","text":null}"#);
        assert!(args["request"].get("text").is_none());
        assert!(args["request"].get("category").is_none());
    }

    #[test]
    fn record_categories_are_the_seven_wire_values_in_kernel_order() {
        assert_eq!(
            SESSION_FEEDBACK_CATEGORIES,
            [
                "task-result",
                "instruction-following",
                "product-interaction",
                "service-stability",
                "resource-cost",
                "security-privacy-permission",
                "other",
            ]
        );
        assert_eq!(SESSION_FEEDBACK_CATEGORIES.len(), 7);
    }

    #[test]
    fn record_reads_the_second_ok_after_kernel_call_stripped_the_first() {
        // `Kernel::call` 已经只回 `result.value`（`{ok:true, value:…}` 的第一层在这里剥），
        // 所以本函数吃的是主干 `envelope.value` 那一格 —— 双层 `ok` 的第二层。
        assert_eq!(parse_session_feedback_record(&json!({"ok": true})), SessionFeedbackReceipt::Recorded);
        assert_eq!(
            parse_session_feedback_record(&json!({"ok": true, "extra": 1})),
            SessionFeedbackReceipt::Recorded,
            "多余键不进判据"
        );
        assert_eq!(
            parse_session_feedback_record(
                &json!({"ok": false, "error": {"code": "session-not-found", "sessionId": "s-1"}})
            ),
            SessionFeedbackReceipt::Rejected { code: Some("session-not-found".to_string()) }
        );
        // 「比主干软」的两态：`ok` 缺 / 非字面 true 全算失败，不抛
        for dirty in [json!({}), json!({"ok": null}), json!({"ok": "true"}), json!(true)] {
            assert_eq!(
                parse_session_feedback_record(&dirty),
                SessionFeedbackReceipt::Rejected { code: None },
                "{dirty} 被读成了已记录"
            );
        }
        // `error` 不是对象 ⇒ 主干那段的守卫给不出 code（不是「随便找个键当 code」）
        assert_eq!(
            parse_session_feedback_record(&json!({"ok": false, "error": "boom"})),
            SessionFeedbackReceipt::Rejected { code: None }
        );
    }

    #[test]
    fn record_success_is_read_from_ok_not_the_recorded_key_the_comment_promises() {
        // 主干注释写 `{ok:true, value:{recorded:true}}`，代码读的却是 `value.ok`。
        // 跟代码：只有 `recorded` 的那一格不算成功（造一条 `recorded` 臂就是发明主干没有的行为）。
        assert_eq!(
            parse_session_feedback_record(&json!({"recorded": true})),
            SessionFeedbackReceipt::Rejected { code: None }
        );
        assert!(
            !KW4_SELF.contains(concat!("value.get(\"", "recorded\")")),
            "本层给 `recorded` 开了读臂 ⇒ 与主干代码不一致"
        );
    }

    #[test]
    fn unconsumed_preset_receipts_get_no_parser_here() {
        // 主干对 `copy` / `deletePreset`（`z.void()`）与 `StartCreatorDraftAsync` 那一处
        // `select` 都是**整发丢弃**：成败只看抛不抛 ⇒ 本层跟着不造解析器（造了就是比主干
        // 多一道判据，与上一批 `settings/replace` 同规）。
        // needle 用 `concat!` 拆开，否则本测试自己的文字就会命中自己（正向恒真、反向恒假）。
        for forbidden in [
            concat!("parse_agent_presets_", "copy_receipt"),
            concat!("parse_agent_presets_", "delete_receipt"),
            concat!("parse_session_feedback_", "envelope"),
        ] {
            assert!(
                !KW4_SELF.contains(forbidden),
                "本层给主干不消费的回执造了判据：{forbidden}"
            );
        }
        // 反向锁不是空转：三处「不消费」的形状说明都真在本文件里
        assert!(KW4_SELF.contains("z.void()"), "`z.void()` 那两发的形状说明没落进来");
        assert!(KW4_SELF.contains("整发丢弃"), "「丢弃」这条口径没落进来");
        assert!(
            KW4_SELF.contains("CallAsync"),
            "`record` 走 `CallAsync` 而不是 `CallOkAsync` 这件事没写进注释"
        );
    }

    #[test]
    fn the_callerless_cordis_faces_stay_out_even_after_this_batch() {
        // 上一批的 `kw2_cordis_tests` 已按「主干零调用方 + 那颗 partial 自陈不可达」下了
        // 源码反向锁。本轮独立复核**确认该判据成立**（四颗包装的定义行即唯一出现处），
        // 于是这里补第二道锁：本批的追加段同样不许出现那四面。
        // needle 照上一批的口径用 `concat!` 拆开 —— 否则本测试自己就把锁写红了。
        let prefix = concat!("dynamicCordisRunner", "/");
        for tail in [
            "invoke",
            "resolveInspectQuery",
            "reportClientGuardFailure",
            "reportRenderFailure",
        ] {
            assert!(
                !KW4_SELF.contains(&format!("{prefix}{tail}")),
                "本文件出现了主干零调用方的那一面：{prefix}{tail}"
            );
        }
        // 基数两头：八发名单不许被这批塞成十二发；五发名单不许混进那一族的颗。
        assert_eq!(CORDIS_METHODS.len(), 8);
        for method in RD9_C4_C9_METHODS {
            assert!(
                !method.starts_with(prefix),
                "C4/C9 的名单混进了 cordis 那一族：{method}"
            );
        }
        // 锁不是空转：前缀与尾段各自都在源码里出现过，只是从不相邻。
        assert!(KW4_SELF.contains(prefix));
        assert!(KW4_SELF.contains("resolveInspectQuery"));
    }
}

// ===========================================================================
// K-1 · 能力面板两发 + 子代理三发：五颗 `RpcCall` 构造器
// ===========================================================================
//
// 为什么这一段是**追加**而不是并进上面的族：本文件里已有的那三张方法名登记表各自配着
// 一条基数断言与一族反向锁（禁串表 + 须在串表），把新发掺进去会连带改掉别族的判据 ——
// 那是别的批次的事。这里另起两张小表，见 `CAPABILITY_METHODS` / `SUBAGENTS_METHODS`。
//
// 五发的实参形状**既不平铺也不成律**，逐发照主干调用点抄，别「顺手统一」：
//   `goals/get`                   平铺 `{agentId}` —— 键名是 `agentId`，值传的是会话 id
//   `skills/list`                 裹一层 `{request:{sessionId}}`
//   `subagents/list`              平铺 `{parentSessionId}`
//   `subagents/prompt`            裹一层，且 `requestId` 由**调用方**现造（lib 不碰熵源）
//   `subagents/interruptByParent` 平铺 `{childSessionId,parentSessionId,mode}` —— child 在前
//
// 五发一律**不建 `parse_*`**：`goals/get` 与 `skills/list` 的回执消费者已经在
// `capabilities` 模块里（`parse_goal_summary` / `parse_goal_panel` / `parse_skills_list`），
// 本层再造一份就是第二真相；后三发的落点见文件末尾那把新自锁。

/// `goals/get`（主干 `MainWindow.Capabilities.cs` 的
/// `CallOkAsync("goals/get", new { agentId = sessionId }, lifetime.Token)`）：平铺一颗。
/// 键名照抄主干那颗 `agentId` —— 它的**值**是会话 id，主干自己就叫这个名字，
/// 改名成 `session_id` 会把内核描述符的键漂掉。
pub const GOALS_GET: &str = "goals/get";

/// `skills/list`（同文件 `CallOkAsync("skills/list", new { request = new { sessionId } })`）：
/// 这一发**要裹** `request`，与上一发的平铺正好相反 —— 两发在同一个文件里，形状却不通用。
pub const SKILLS_LIST: &str = "skills/list";

/// `subagents/list`（主干 `MainWindow.Subagents.cs` 的
/// `CallOkAsync("subagents/list", new { parentSessionId }, ct)`）：平铺一颗。
pub const SUBAGENTS_LIST: &str = "subagents/list";

/// `subagents/prompt`（同文件）：裹一层 `request`，内含六颗键，其中 `requestId`
/// 主干是 `$"c2-{Guid.NewGuid():N}"` ⇒ **现造在调用方**，本层不碰随机数也不为此加依赖。
pub const SUBAGENTS_PROMPT: &str = "subagents/prompt";

/// `subagents/interruptByParent`（同文件）：平铺三颗，**`childSessionId` 在前**。
pub const SUBAGENTS_INTERRUPT_BY_PARENT: &str = "subagents/interruptByParent";

/// K-1 能力面板那两发的登记表（与既有三张表互不相交，基数由本文件末尾的锁钉住）。
pub const CAPABILITY_METHODS: [&str; 2] = [GOALS_GET, SKILLS_LIST];

/// K-1 子代理那三发的登记表。
pub const SUBAGENTS_METHODS: [&str; 3] = [
    SUBAGENTS_LIST,
    SUBAGENTS_PROMPT,
    SUBAGENTS_INTERRUPT_BY_PARENT,
];

/// `goals/get`：`{agentId}`，平铺。
pub fn goals_get(agent_id: &str) -> RpcCall {
    RpcCall::new(GOALS_GET, json!({ "agentId": agent_id }))
}

/// `skills/list`：`{request:{sessionId}}`，**要裹** —— 复用本文件那颗 `wrapped` 形状的
/// 一层包裹，别手抄第二份 `{ "request": … }` 字面量。
pub fn skills_list(session_id: &str) -> RpcCall {
    RpcCall::new(SKILLS_LIST, wrapped(json!({ "sessionId": session_id })))
}

/// `subagents/list`：`{parentSessionId}`。形状的唯一真相仍在
/// `crate::subagents::list_args`，这里只是把它包成一颗可发的调用。
pub fn subagents_list(parent_session_id: &str) -> RpcCall {
    RpcCall::new(SUBAGENTS_LIST, crate::subagents::list_args(parent_session_id))
}

/// `subagents/prompt`：整颗 `request` 体由调用方拼好（含 `requestId`）再递进来 ——
/// 构造层**不搓 id**，否则一次重发就换一个 id，主干那个幂等语义当场失效。
pub fn subagents_prompt(request: &crate::subagents::PromptRequest<'_>) -> RpcCall {
    RpcCall::new(SUBAGENTS_PROMPT, request.to_args())
}

/// `subagents/interruptByParent`：形参顺序 **child 在前、parent 在后**，与
/// `subagents/prompt` 那发的 `parentSessionId, childSessionId` **相反**；抄反就是
/// 打断错对象。判据在文件末尾的锁里，不靠注释。
pub fn subagents_interrupt_by_parent(
    child_session_id: &str,
    parent_session_id: &str,
) -> RpcCall {
    RpcCall::new(
        SUBAGENTS_INTERRUPT_BY_PARENT,
        crate::subagents::interrupt_args(child_session_id, parent_session_id),
    )
}

// ===========================================================================
// L2 · `goals/<verb>` 六发变更：再六颗 `RpcCall` 构造器
// ===========================================================================
//
// 主干这一族的**方法名是拼出来的** —— 全仓唯一那一颗
// `MainWindow.Capabilities.cs:248` 的 `await rpc.CallOkAsync("goals/" + verb, args, lifetime.Token);`。
// 按整串方法名查表的台账对这种拼接结构性失明 ⇒ 六发在分叉的四层里第①层此前为零
// （名单与逐层命中表见 `tmp/g1-goals-verbs.md` §1.2/§2；`get` 不在此列，它已在上面 K-1 那节）。
// **本节只落第①层**：桩臂、main 分派、真 socket 用例各归自己那一轮，在这儿提前塞就是
// 「桩里有/生产没有」那档假完。
//
// ⚠ 动词的**单一真源是 `src/capabilities.rs:89-95` 那七枚 `VERB_*`**（主干根本没有常量段，
// 它的第二半是 `:274-280` 的按钮元组 + `var verb = entry.Item1;`）。所以本节**不再立动词表**、
// 也不写 `goals/` 前缀常量：六颗方法名各是一枚整串常量（与上面 `GOALS_GET` 同形制），
// 「方法名 == `goals/` + 那一枚 `VERB_*`」由本节末尾的锁**跨模块**钉住，不靠注释自觉。
//
// ⚠ 形状逐枚照主干 `:226-246`：三键 `{agentId, ref, request}` **全在顶层**，其中
// `request` 是 create/edit **自己的载荷键**（`:246`），不是给整发套的外壳。⇒ 上面那颗
// `wrapped()` 与 `PromptRequest::to_args()` 这类「已经裹了一层」的 helper 在这里一律**不用**，
// 再包一次就是双层（本仓栽过：见 `tmp/g1-goals-verbs.md` §5.1-1）。
//
// 六发一律**不建解析器**：主干 `:248` 是 `await` 一句到底、连返回值都不接，紧接着 `:249`
// 才 `goals/get` 回读 —— 回执在这条链上没人读（真内核六枚出口的回执形状各不相同，
// 逐枚见 `tmp/m2b-spec.md` §1.2；那是桩与用例那一轮的账）。

/// `goals/create`（主干 `:239`+`:246`，**没有** `:244` 那一颗 `ref`）：
/// `{agentId, request:{objective, maxGoalRounds?}}`。
pub const GOALS_CREATE: &str = "goals/create";

/// `goals/edit`（同文件）：六发里**唯一三键齐**的一发 —— 既带 CAS 的 `ref`，又带 `request`。
pub const GOALS_EDIT: &str = "goals/edit";

/// `goals/pause`：`{agentId, ref}`。
pub const GOALS_PAUSE: &str = "goals/pause";

/// `goals/resume`：`{agentId, ref}`。
pub const GOALS_RESUME: &str = "goals/resume";

/// `goals/complete`：`{agentId, ref}`。
pub const GOALS_COMPLETE: &str = "goals/complete";

/// `goals/clear`：`{agentId, ref}`。主干 `:251-253` 在这发之后额外收起目标条，
/// 那是宿主的事，本层只管把这一发拼对。
pub const GOALS_CLEAR: &str = "goals/clear";

/// L2 这六发变更的登记表（与 K-1 那两张同族口径：新发**不塞进**既有表，理由见 K-1 节开头
/// 「另起小表」那一段）。`goals/get` 仍留在 [`CAPABILITY_METHODS`] 里 —— 它是读，不是变更。
pub const GOALS_MUTATION_METHODS: [&str; 6] = [
    GOALS_CREATE,
    GOALS_EDIT,
    GOALS_PAUSE,
    GOALS_RESUME,
    GOALS_COMPLETE,
    GOALS_CLEAR,
];

/// 主干 `:244` 的 `args["ref"] = new { id = ..., revision = ... }`。
///
/// `revision` 吃 `&Value` 而不是 `i64`：主干那句是 `goal.GetProperty("revision").Clone()`，
/// **不校验类型**、原样回带（判据同 `capabilities.rs` 的 `GoalPanelView::ref_fields`，
/// 那两颗正是本函数的唯一原料供给者）。在这里收窄成整数就是替主干加了一道它没有的闸。
fn goal_ref(id: &str, revision: &Value) -> Value {
    json!({ "id": id, "revision": revision })
}

/// 主干 `:226-238` 的 `request` 体：`objective` 恒在（非空闸在调用方，`:228-229` 抛串），
/// `maxGoalRounds` **只在文本框非空时才有**（`:230` 的 `if (!string.IsNullOrWhiteSpace(...))`）
/// ⇒ 留空这一档是「键整个不存在」，不是 `null`、不是 `0`。
fn goal_request(objective: &str, max_goal_rounds: Option<i64>) -> Value {
    let mut request = json!({ "objective": objective });
    if let Some(rounds) = max_goal_rounds {
        request["maxGoalRounds"] = json!(rounds);
    }
    request
}

/// `{agentId, ref}` 那一型的共用拼接（pause / resume / complete / clear 四发同形，
/// 形状只此一处定义）。
fn goal_ref_call(method: &'static str, agent_id: &str, goal_id: &str, revision: &Value) -> RpcCall {
    RpcCall::new(
        method,
        json!({ "agentId": agent_id, "ref": goal_ref(goal_id, revision) }),
    )
}

/// `goals/create`：`{agentId, request}` —— 这一发**不许带 `ref`**（主干 `:240` 的
/// `if (verb != "create")` 就是把它筛掉的那道闸）。
pub fn goals_create(agent_id: &str, objective: &str, max_goal_rounds: Option<i64>) -> RpcCall {
    RpcCall::new(
        GOALS_CREATE,
        json!({ "agentId": agent_id, "request": goal_request(objective, max_goal_rounds) }),
    )
}

/// `goals/edit`：三键齐 —— `{agentId, ref, request}`。
pub fn goals_edit(
    agent_id: &str,
    goal_id: &str,
    revision: &Value,
    objective: &str,
    max_goal_rounds: Option<i64>,
) -> RpcCall {
    RpcCall::new(
        GOALS_EDIT,
        json!({
            "agentId": agent_id,
            "ref": goal_ref(goal_id, revision),
            "request": goal_request(objective, max_goal_rounds),
        }),
    )
}

/// `goals/pause`：`{agentId, ref}`。
pub fn goals_pause(agent_id: &str, goal_id: &str, revision: &Value) -> RpcCall {
    goal_ref_call(GOALS_PAUSE, agent_id, goal_id, revision)
}

/// `goals/resume`：`{agentId, ref}`。
pub fn goals_resume(agent_id: &str, goal_id: &str, revision: &Value) -> RpcCall {
    goal_ref_call(GOALS_RESUME, agent_id, goal_id, revision)
}

/// `goals/complete`：`{agentId, ref}`。
pub fn goals_complete(agent_id: &str, goal_id: &str, revision: &Value) -> RpcCall {
    goal_ref_call(GOALS_COMPLETE, agent_id, goal_id, revision)
}

/// `goals/clear`：`{agentId, ref}`。形参与上一发**逐字同序**，这六发没有 child/parent 那种
/// 反序坑（那坑在 `subagents_interrupt_by_parent`）。
pub fn goals_clear(agent_id: &str, goal_id: &str, revision: &Value) -> RpcCall {
    goal_ref_call(GOALS_CLEAR, agent_id, goal_id, revision)
}

#[cfg(test)]
mod l2_goals_ctor_tests {
    //! L2 的自锁：① 六发的实参形状逐字钉回主干那一处拼接调用点的装配段（一颗键不许多、
    //! `request` 不许双层）；② 六颗方法名与 `capabilities::VERB_*` 两头对齐 ⇒ 动词只有
    //! 一个真源；③ 这张新表小而互斥，没被塞进既有那五族；④ 主干对六发的回执一个都不读
    //! ⇒ 本层不建解析器。被搜的串一律 `concat!`/`format!` 现拼，防整名自匹配。
    use super::*;

    /// 自锁读的是**本文件**，路径写法与本文件既有那几族一致。
    const L2_SELF: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));

    /// ② 用的配对表：本层方法名 ↔ 动词单一真源。**只有这一处**把两者并排列出来，
    /// 动词本身仍是 `capabilities` 那七枚常量，本文件不重打一遍 `"create"`。
    const METHOD_VERB_PAIRS: [(&str, &str); 6] = [
        (GOALS_CREATE, crate::capabilities::VERB_CREATE),
        (GOALS_EDIT, crate::capabilities::VERB_EDIT),
        (GOALS_PAUSE, crate::capabilities::VERB_PAUSE),
        (GOALS_RESUME, crate::capabilities::VERB_RESUME),
        (GOALS_COMPLETE, crate::capabilities::VERB_COMPLETE),
        (GOALS_CLEAR, crate::capabilities::VERB_CLEAR),
    ];

    /// ① 六发的形状。
    #[test]
    fn the_six_mutation_ctors_emit_the_mainline_argument_shapes_verbatim() {
        let revision = json!(3);

        // create：两键，**没有 ref**（主干 `:240` 那道 `verb != "create"` 的闸）。
        let (method, args) = goals_create("sess-1", "接通 goals 六发", Some(4)).into_parts();
        assert_eq!(method, "goals/create");
        assert_eq!(
            args,
            json!({ "agentId": "sess-1", "request": { "objective": "接通 goals 六发", "maxGoalRounds": 4 } }),
            "create 是 {{agentId, request}} 两键，多一颗 ref 就坏"
        );

        // create 的「留空 = 键不存在」档：主干 `:230` 那个 if 不进，就不是 null、不是 0。
        let (_, args) = goals_create("sess-1", "只给目标", None).into_parts();
        assert_eq!(args["request"], json!({ "objective": "只给目标" }));
        assert!(
            args["request"].get("maxGoalRounds").is_none(),
            "留空时 maxGoalRounds 必须整个不存在"
        );

        // edit：六发里唯一三键齐的一发。
        let (method, args) = goals_edit("sess-1", "goal-1", &revision, "改目标", Some(8)).into_parts();
        assert_eq!(method, "goals/edit");
        assert_eq!(
            args,
            json!({
                "agentId": "sess-1",
                "ref": { "id": "goal-1", "revision": 3 },
                "request": { "objective": "改目标", "maxGoalRounds": 8 },
            }),
            "edit 的 request 与 ref 是**同级**两颗，不许把 ref 塞进 request"
        );

        // 其余四发同形：逐枚验，别只验一枚（抄漏一发的键就是发错方法）。
        let (method, args) = goals_pause("sess-1", "goal-1", &revision).into_parts();
        assert_eq!(method, "goals/pause");
        assert_eq!(args, json!({ "agentId": "sess-1", "ref": { "id": "goal-1", "revision": 3 } }));
        for (call, want) in [
            (goals_resume("s", "g", &revision), "goals/resume"),
            (goals_complete("s", "g", &revision), "goals/complete"),
            (goals_clear("s", "g", &revision), "goals/clear"),
        ] {
            let (method, args) = call.into_parts();
            assert_eq!(method, want);
            assert_eq!(
                args,
                json!({ "agentId": "s", "ref": { "id": "g", "revision": 3 } }),
                "{want} 的形状与 pause 那发不同 ⇒ 三键全在顶层这一律没守住"
            );
        }

        // `revision` 原样带：主干是 `.Clone()`，**不校验类型**（非数的 revision 也照发）。
        let texty = json!("3");
        let (_, args) = goals_pause("s", "g", &texty).into_parts();
        assert_eq!(args["ref"]["revision"], json!("3"), "本层偷偷替主干校了 revision 的类型");

        // 双层陷阱的反向锁：六发里没有任何一颗把整发包进第二层 `request`。
        for args in [
            goals_create("s", "o", None).into_parts().1,
            goals_edit("s", "g", &revision, "o", None).into_parts().1,
            goals_pause("s", "g", &revision).into_parts().1,
            goals_resume("s", "g", &revision).into_parts().1,
            goals_complete("s", "g", &revision).into_parts().1,
            goals_clear("s", "g", &revision).into_parts().1,
        ] {
            assert!(
                args["request"].get("request").is_none(),
                "goals 这一族被又裹了一层 `request` ⇒ 双层"
            );
        }
    }

    /// ② 方法名与动词两头对齐：`capabilities.rs` 是动词的唯一真源，本文件的六颗整串常量
    /// 必须逐枚等于 `goals/` + 那一枚 `VERB_*`（漂一个字符当场红）。
    #[test]
    fn the_six_method_names_are_the_verbs_from_the_single_source() {
        for (method, verb) in METHOD_VERB_PAIRS {
            assert_eq!(method, format!("goals/{}", verb));
            assert!(GOALS_MUTATION_METHODS.contains(&method), "{method} 没进这张表");
        }
        assert_eq!(GOALS_MUTATION_METHODS.len(), METHOD_VERB_PAIRS.len());
        // 反向：`get` 不在变更这张表里（它是读，住在 `CAPABILITY_METHODS`）。
        assert!(!GOALS_MUTATION_METHODS.contains(&GOALS_GET));
        assert!(CAPABILITY_METHODS.contains(&GOALS_GET));
    }

    /// ③ 这张新表小而互斥：六发没塞进既有那五族，既有那五族的基数也没被这一刀动了。
    #[test]
    fn the_new_mutation_table_stays_small_and_disjoint() {
        assert_eq!(GOALS_MUTATION_METHODS.len(), 6);
        assert_eq!(CAPABILITY_METHODS.len(), 2, "K-1 那张表的基数被这一刀动了");
        assert_eq!(SUBAGENTS_METHODS.len(), 3, "K-1 那张表的基数被这一刀动了");
        assert_eq!(CORDIS_METHODS.len(), 8, "既有那族的基数被这一刀动了");
        assert_eq!(RD9_C5_C6_METHODS.len(), 10, "既有那族的基数被这一刀动了");
        assert_eq!(RD9_C4_C9_METHODS.len(), 5, "既有那族的基数被这一刀动了");
        for method in GOALS_MUTATION_METHODS {
            assert!(
                !CORDIS_METHODS.contains(&method)
                    && !RD9_C5_C6_METHODS.contains(&method)
                    && !RD9_C4_C9_METHODS.contains(&method)
                    && !CAPABILITY_METHODS.contains(&method)
                    && !SUBAGENTS_METHODS.contains(&method),
                "{method} 被塞进了别族那张表"
            );
        }
        // 六颗自身互不相同（拼错一枚动词就撞另一枚，撞了这里红，而不是上线才发现）。
        let unique = GOALS_MUTATION_METHODS
            .iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), 6, "六枚 goals 方法名里有重复");
    }

    /// ④ 主干对六发都是 `await` 一句到底、**不接返回值** ⇒ 本层不建解析器。
    /// 禁名从**登记表自身**推出来（把方法名里那颗 `/` 换成 `_`，前后各裹一段解析器命名规则），
    /// 这样既不必在本文件另列一份动词表，也不会漏数某枚；这里刻意不把它拼成字面量 ——
    /// 本测试读的是全文件源码，写出一次就自匹配。
    #[test]
    fn the_six_dropped_goal_receipts_get_no_parser_here() {
        for method in GOALS_MUTATION_METHODS {
            let forbidden = format!("parse_{}_receipt", method.replace('/', "_"));
            assert!(
                !L2_SELF.contains(forbidden.as_str()),
                "本层给主干丢弃的回执造了判据：{forbidden}"
            );
        }
        // 反向锁不许空转：六发的方法名与「不读回执」这条口径都真在本文件里出现过。
        assert!(L2_SELF.contains(concat!("goals/", "clear")));
        assert!(L2_SELF.contains("连返回值都不接"), "「不建解析」的理由没落进注释 ⇒ 反向判据是空的");
    }
}

#[cfg(test)]
mod kx1_ctor_tests {
    //! K-1 的自锁：① 五发的实参形状逐字钉回主干那五个调用点；② 两张新表小而互斥，
    //! 不许被并进既有那三族；③ 主干 `await` 之后**丢弃**回执的那两发，本层不建解析器。
    use super::*;

    /// 自锁读的是**本文件**，路径写法与本文件既有那几族一致。
    const SELF_SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));

    /// 五发的形状：值必须落到**主干那颗键**上，一颗都不许多、一颗都不许多包一层。
    #[test]
    fn the_five_ctors_emit_the_mainline_argument_shapes_verbatim() {
        let (method, args) = goals_get("sess-1").into_parts();
        assert_eq!(method, "goals/get");
        assert_eq!(args, json!({ "agentId": "sess-1" }), "goals/get 是平铺，不许裹 request");

        let (method, args) = skills_list("sess-1").into_parts();
        assert_eq!(method, "skills/list");
        assert_eq!(
            args,
            json!({ "request": { "sessionId": "sess-1" } }),
            "skills/list 要裹 request，且里面只有 sessionId 一颗"
        );

        let (method, args) = subagents_list("parent-1").into_parts();
        assert_eq!(method, "subagents/list");
        assert_eq!(args, json!({ "parentSessionId": "parent-1" }));

        let request = crate::subagents::PromptRequest {
            request_id: "c2-0123456789abcdef0123456789abcdef",
            parent_session_id: "parent-1",
            child_session_id: "child-1",
            delivery: "steer",
            text: "接着跑",
        };
        let (method, args) = subagents_prompt(&request).into_parts();
        assert_eq!(method, "subagents/prompt");
        // requestId 是**传进来**的那串 ⇒ 构造层没有自己搓 id（`c2-` 前缀只是实参的形）。
        assert_eq!(args["request"]["requestId"], request.request_id);
        assert_eq!(args["request"]["parentSessionId"], "parent-1");
        assert_eq!(args["request"]["childSessionId"], "child-1");
        assert_eq!(args["request"]["content"][0]["text"], "接着跑");

        // 这一发专测「形参顺序」：两枚 id 给不同的值，抄反了就当场红。
        let (method, args) = subagents_interrupt_by_parent("child-9", "parent-9").into_parts();
        assert_eq!(method, "subagents/interruptByParent");
        assert_eq!(args["childSessionId"], "child-9", "child/parent 串位 = 打断错对象");
        assert_eq!(args["parentSessionId"], "parent-9");
        assert_eq!(args["mode"], "continuable");
        assert_eq!(
            args,
            json!({ "childSessionId": "child-9", "parentSessionId": "parent-9", "mode": "continuable" }),
            "平铺三颗，一颗不许多"
        );
    }

    /// 两张新表的基数与互斥：新发**没有**塞进既有那三族，既有那三族也没被这一刀改小。
    #[test]
    fn the_two_new_tables_stay_small_and_disjoint_from_the_existing_families() {
        assert_eq!(CAPABILITY_METHODS.len(), 2);
        assert_eq!(SUBAGENTS_METHODS.len(), 3);
        assert_eq!(CORDIS_METHODS.len(), 8, "既有那族的基数被这一刀动了");
        assert_eq!(RD9_C5_C6_METHODS.len(), 10, "既有那族的基数被这一刀动了");
        assert_eq!(RD9_C4_C9_METHODS.len(), 5, "既有那族的基数被这一刀动了");
        let fresh = CAPABILITY_METHODS
            .iter()
            .chain(SUBAGENTS_METHODS.iter())
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(fresh.len(), fresh.iter().collect::<std::collections::HashSet<_>>().len());
        for method in &fresh {
            assert!(
                !CORDIS_METHODS.contains(method)
                    && !RD9_C5_C6_METHODS.contains(method)
                    && !RD9_C4_C9_METHODS.contains(method),
                "{method} 被塞进了既有那三族"
            );
        }
    }

    /// 反向锁：主干对 `subagents/prompt` 与 `subagents/interruptByParent` 都是
    /// `await CallOkAsync(...)` 一句到底、**不接返回值** —— 回执整发丢弃 ⇒ 本层不建解析器。
    /// 两枚禁名一律 `concat!` 拆写：本测试的源码就在本文件里，整名一旦落纸，下面的反向
    /// `contains` 就会命中自己 ⇒ 判据恒假（假绿）。
    #[test]
    fn the_two_dropped_subagent_receipts_get_no_parser_here() {
        for forbidden in [
            concat!("parse_subagents_", "prompt_receipt"),
            concat!("parse_subagents_", "interrupt_receipt"),
        ] {
            assert!(
                !SELF_SRC.contains(forbidden),
                "本层给主干丢弃的回执造了判据：{forbidden}"
            );
        }
        // 反向锁不是空转：两发的方法名与「丢弃」这条口径都真在本文件里出现过（正向那三枚
        // 一律 `concat!` 拆，否则 `contains` 命中的是本测试自己写的那串字面量）。
        assert!(SELF_SRC.contains(concat!("subagents/", "prompt")));
        assert!(SELF_SRC.contains(concat!("subagents/", "interruptByParent")));
        assert!(SELF_SRC.contains("回执整发丢弃"), "「不建解析」的理由没落进注释 ⇒ 反向判据是空的");
    }
}

// ==================== B-1 · #162 第三族「恢复默认模型」的 `settings/mutate` ctor ====================
// 本节只做一件事：把主干那 7 发 `settings/mutate` 的 args 形状收成一颗 ctor + 两枚 op 辅助。
// 宿主与派发在 `main.rs`（B-2，另派），UI 一颗不碰、i18n 零新键、假内核零改动（那臂早已在架：
// `src/bin/fake_dsh.rs:2709 "settings/mutate" => return settings_write(&args, &rpc_id, state, "ops")`）。
// 追加在文件末尾的理由同 `:11195` 那条 —— 本仓 `include_str!` 源码锁按「起始锚 → 结束锚首次出现」
// 开窗，落在最后就不动任何既有窗口。
//
// 描述符出处（本轮逐字读过 `Kernel/dsh/node_modules/@deepseek-ai/dsh-api-settings-controller/lib/typert.host.js:237-282`，
// 端点在架那步的取证也出自同一处）：
//   id `@deepseek-ai/dsh-api-settings-controller#settings/mutate`、`invocation: { kind: 'direct' }`
//   ⇒ 参数**平铺**在 `payload.args` 顶层，无一裹 `request`（与本节上方那十发同规）。三颗 `parameters`：
//   · `ns`   → `z.string()`，**没有** `acceptsUndefined` ⇒ 这颗键必在；
//   · `ops`  → `z.array(z.union([ {op:'set', path: string[], value: JsonValue},
//                                  {op:'unset', path: string[]} ]))`，同样**没有** `acceptsUndefined`
//              ⇒ 键必在，且 `path` 是**数组**（抄成 `path: "models"` 就是拿 string 去撞 array）；
//   · `expectedRevision` → `z.union([z.undefined(), z.number()])` 且带 `acceptsUndefined: true`
//              ⇒ 整颗键缺席合法、**显式 `null` 非法**（与 `replace`/`update` 那颗同型，共用
//                [`with_expected_revision`]）。
//   result = `@deepseek-ai/dsh-settings/types#SettingsNamespaceView` —— 与 [`settings_replace`] /
//   [`settings_update`] 的回执同一型。
//
// 主干那 7 发**无一**递 `expectedRevision`（现测：`MainWindow.ModelExtras.cs` 与
// `MainWindow.SettingsExtras.cs` 各 0 命中；`MainWindow.xaml.cs` 的 2 命中全在 `settings/replace`
// 那族，即 `ResetSectionAsync` 的 `new { ns, section, expectedRevision = rev }`）⇒ `None` 是常态，
// `Some` 只是把描述符允许的那颗口留着给派发侧。

/// `settings/mutate` 的方法名（第 1 头：const）。
pub const SETTINGS_MUTATE: &str = "settings/mutate";

/// B-1 自开的名册（第 2 头：登记表），**只有这一发**（基数锁见 `the_new_roster_stays_alone_and_disjoint`）。
///
/// 为什么不并进 [`RD9_C5_C6_METHODS`]：那十发的基数 `== 10` 在本文件里被四处钉死
/// （`ten_consts_match_rd9_table_a_and_are_the_only_ten`、`the_new_mutation_table_stays_small_and_disjoint`、
/// `the_two_new_tables_stay_small_and_disjoint_from_the_existing_families` 各一枚，外加逐颗 const 校验），
/// 附近还有「新发不落进既有那三族」的排斥式断言 ⇒ 往里塞一颗当场红。登记法本身没改：
/// const + 名册 + `fn` 三头齐，与 `settings_update` 同一套。
pub const SETTINGS_MUTATE_METHODS: &[&str] = &[SETTINGS_MUTATE];

/// 一枚 `set` op：`{op:"set", path:[…], value:…}`（主干 `xaml.cs:13227` 的
/// `entries.Select(entry => new { op = "set", path = entry.Path, value = entry.Value })` 逐字同形）。
///
/// `value` 是 `JsonValue` 全域（`null` / 字符串 / 数 / `false` / `true` / 数组 / 对象都合法）：
/// 本层**不 trim、不判空、不折叠、不校类型** —— 取值与校验都在 UI 侧（同 [`credentials_set`] 的口径）。
#[must_use]
pub fn settings_op_set(path: &[&str], value: &Value) -> Value {
    json!({ "op": "set", "path": path, "value": value })
}

/// 一枚 `unset` op：`{op:"unset", path:[…]}` —— 描述符那一支**只声明 `op` 与 `path` 两颗**
/// （`typert.host.js:61-64`），主干五发 unset 全是这个形状 ⇒ 本层不替它补 `value: null`。
///
/// `path` 一律数组：`ModelExtras.cs:183` 是 `path = new[] { "models" }`，
/// `SettingsExtras.cs:576` 是 `path = new[] { field }`，删卡那发是 `path = row.SettingsPath`（多段）。
/// 空数组照发 `[]` —— 非空那道闸在派发侧，不在这里。
#[must_use]
pub fn settings_op_unset(path: &[&str]) -> Value {
    json!({ "op": "unset", "path": path })
}

/// `settings/mutate`（第 3 头：`fn`）：平铺 `{ns, ops[, expectedRevision]}`。
///
/// 主干 7 发调用点（本轮 grep 现测，逐字原文见 `rust/tmp/b1-report.md` §2）：
/// `MainWindow.ModelExtras.cs:180`（`foreach` 里 ⇒ **一发调用点、两趟**）与 `:186`、
/// `MainWindow.SettingsExtras.cs:573`、`MainWindow.xaml.cs:12070`、`:12152`、`:13230`、`:16640`。
///
/// `ops` 收调用方**已经拼好**的数组：那 7 发里 4 发当场写 `new[] { new { op = …, path = new[] { … } } }`，
/// 另外 3 发（`ops` 变量、`opsList.ToArray()`、`entries.Select(…)`）从投影里拼好再递 —— 递的都是整棵数组，
/// 所以本层不猜键、不拼字段（同 [`settings_update`] 的 `patch`）。
///
/// 一发**一个 ns**：`ModelExtras.cs:178` 那个 `foreach (var ns in new[] { "llm-deepseek", "llm-pi-ai" })`
/// 就是「整批不共用 ns」的实证 ⇒ 参数里没有 `ns` 数组。
///
/// 空 `ops` 照发 `"ops":[]`：`xaml.cs:12066` 的 `if (ops.Length > 0)` 是**派发侧**的闸
/// （口径同 [`credentials_describe`] 的 `refs:[]`）⇒ 本层不替它兜，也不在这里抛。
///
/// 回执 = `#SettingsNamespaceView`，主干那 7 发 `await CallOkAsync(...)` 一句到底、**不接返回值**
/// （成败只看抛不抛，随后 `_settingsSnapshot = null` 重拉 describe）⇒ 本层不建解析器。
#[must_use]
pub fn settings_mutate(ns: &str, ops: &[Value], expected_revision: Option<f64>) -> RpcCall {
    RpcCall::new(
        SETTINGS_MUTATE,
        with_expected_revision(
            json!({ "ns": ns, "ops": ops }),
            expected_revision,
        ),
    )
}

#[cfg(test)]
/// B-1（#162 第三族）自锁：① 方法串逐字；② args 顶层键集合等值（排序后比）；
/// ③ `expectedRevision` 带与不带两档各一颗；④ 主干那 7 发调用点仍在（**运行时 `std::fs` 读 `.cs`**，
/// 读不到 ⇒ eprintln + skip，不 `include_str!`）；⑤ 新名册自身基数与对既有五族的互斥；
/// ⑥ `unset` 支不长 `value`、`path` 恒为数组；⑦ 主干丢弃的回执在本层不建解析器（反向锁）。
///
/// 全部离线断言：零派发、零 UI、零 `tests/` 依赖。判据一律**等值**形制，禁 `>= N` 下界。
mod b1_settings_mutate_tests {
    use super::*;

    /// 自锁读本文件，路径写法与 `KW2_SELF` / `KW3_SELF` / `SELF_SRC` 那几族一致。
    const B1_SELF: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/kernel.rs"));

    /// 主干带 `settings/mutate` **活调用点**的文件 + 现测发数（本轮 `grep -o | wc -l` 实测就这三颗）。
    const B1_TRUNK: [(&str, usize); 3] = [
        ("MainWindow.ModelExtras.cs", 2),
        ("MainWindow.SettingsExtras.cs", 1),
        ("MainWindow.xaml.cs", 4),
    ];

    /// 运行时读主干源（**不** `include_str!`）；读不到 ⇒ 打印原因并让调用方 skip（不 panic）。
    /// 现测这三颗主干文件**全是 CRLF**（本轮实测 ModelExtras 245 / SettingsExtras 694 / xaml.cs 19021
    /// 行，各带等量 `\r`，与 `wc -l` 同值）⇒ 归一在这里是必须的，不是「顺手防漂」。
    fn b1_mainline(file: &str) -> Option<String> {
        let path = format!("{}/../{}", env!("CARGO_MANIFEST_DIR"), file);
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text.replace("\r\n", "\n")),
            Err(err) => {
                eprintln!("b1 读不到主干源 {path}（{err}）—— 本锁跳过");
                None
            }
        }
    }

    /// 锚计数：命中数**等值**于现测值，并把实得值打出来（防「窗口恰好含它」式假锁）。
    fn b1_assert_hits(src: &str, needle: &str, want: usize, label: &str) {
        let got = src.matches(needle).count();
        eprintln!("b1 锁[{label}] {needle:?} 命中 {got} 次");
        assert_eq!(got, want, "{label}：{needle:?} 命中数不是现测的 {want}（实得 {got}）");
    }

    /// 排序后的顶层键集合（等值比，不依赖 `serde_json` 的 Map 顺序）。
    fn b1_keys(args: &Value) -> Vec<String> {
        let mut keys: Vec<String> = args
            .as_object()
            .map(|record| record.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    /// ① + 主干 7 发的形状：逐发把 args 钉成**整串 JSON 逐字**比较（少一颗键、多一层信封、
    /// `path` 掉成字符串都当场红），顺带钉 method 串逐字。
    #[test]
    fn settings_mutate_emits_the_trunk_argument_shapes_verbatim() {
        // 1) ModelExtras.cs:180 —— foreach 的两个 ns 各一趟，形状同一枚
        for ns in ["llm-deepseek", "llm-pi-ai"] {
            let call = settings_mutate(ns, &[settings_op_unset(&["models"])], None);
            assert_eq!(call.method, "settings/mutate", "方法串被写歪了");
            assert_eq!(
                call.args.to_string(),
                format!(r#"{{"ns":"{ns}","ops":[{{"op":"unset","path":["models"]}}]}}"#),
                "{ns} 那一趟的形状与主干 ModelExtras.cs:180 不同"
            );
        }
        // 2) ModelExtras.cs:186 —— 同一发里两枚 unset（主干原文顺序：provider 在前、model 在后）
        assert_eq!(
            settings_mutate(
                "agent-default-model",
                &[
                    settings_op_unset(&["provider"]),
                    settings_op_unset(&["model"]),
                ],
                None,
            )
            .args
            .to_string(),
            r#"{"ns":"agent-default-model","ops":[{"op":"unset","path":["provider"]},{"op":"unset","path":["model"]}]}"#
        );
        // 3) SettingsExtras.cs:573 —— 字段级「恢复默认」：path 就是那颗字段名
        assert_eq!(
            settings_mutate("ui-settings-general", &[settings_op_unset(&["language"])], None)
                .args
                .to_string(),
            r#"{"ns":"ui-settings-general","ops":[{"op":"unset","path":["language"]}]}"#
        );
        // 4) xaml.cs:12070 —— 编辑卡保存：`new { ns = editor.SettingsNs, ops }`，
        //    ops 由 PathOps / `{op:"set", path: editor.SettingsPath, value}` 拼好
        assert_eq!(
            settings_mutate(
                "llm-pi-ai",
                &[settings_op_set(&["baseURL"], &json!("https://gw.internal/v1"))],
                None,
            )
            .args
            .to_string(),
            r#"{"ns":"llm-pi-ai","ops":[{"op":"set","path":["baseURL"],"value":"https://gw.internal/v1"}]}"#
        );
        // 5) xaml.cs:12152 —— 删卡：`ops = new object[] { new { op = "unset", path = row.SettingsPath } }`
        //    `SettingsPath` 是**多段**字符串数组，不是单段
        assert_eq!(
            settings_mutate("llm-deepseek", &[settings_op_unset(&["providers", "custom-1"])], None)
                .args
                .to_string(),
            r#"{"ns":"llm-deepseek","ops":[{"op":"unset","path":["providers","custom-1"]}]}"#
        );
        // 6) xaml.cs:13230 —— 整页保存：**一个 ns 一批多枚 set**（ns 每发一个、ops 整批一个）
        assert_eq!(
            settings_mutate(
                "ui-settings-general",
                &[
                    settings_op_set(&["language"], &json!("en")),
                    settings_op_set(&["theme"], &json!("dark")),
                ],
                None,
            )
            .args
            .to_string(),
            r#"{"ns":"ui-settings-general","ops":[{"op":"set","path":["language"],"value":"en"},{"op":"set","path":["theme"],"value":"dark"}]}"#
        );
        // 7) xaml.cs:16640 —— 默认权限档：`{op:"set", path:new[]{"defaultPreset"}, value}`
        assert_eq!(
            settings_mutate(
                "permission",
                &[settings_op_set(&["defaultPreset"], &json!("autoApprove"))],
                None,
            )
            .args
            .to_string(),
            r#"{"ns":"permission","ops":[{"op":"set","path":["defaultPreset"],"value":"autoApprove"}]}"#
        );
        // 反向：这一族的 args 从不裹 `request`（描述符 `invocation.kind` 是 `direct`）
        for call in [
            settings_mutate("n", &[settings_op_unset(&["a"])], None),
            settings_mutate("n", &[settings_op_set(&["a"], &json!(1))], Some(3.0)),
        ] {
            assert!(
                !b1_keys(&call.args).contains(&"request".to_string()),
                "settings/mutate 又去裹 request 了：{}",
                call.args
            );
        }
    }

    /// ② 顶层键集合：不带 revision 是 `["ns","ops"]`、带是 `["expectedRevision","ns","ops"]`，
    /// **等值**比较（多一颗少一颗都红）。
    #[test]
    fn the_top_level_key_set_is_exactly_ns_and_ops() {
        let bare = settings_mutate("llm-deepseek", &[settings_op_unset(&["models"])], None);
        assert_eq!(b1_keys(&bare.args), ["ns", "ops"]);
        let guarded = settings_mutate("llm-deepseek", &[settings_op_unset(&["models"])], Some(12.0));
        assert_eq!(b1_keys(&guarded.args), ["expectedRevision", "ns", "ops"]);
        // ops 里那一枚的键集合也钉住（两支各自的形状）
        assert_eq!(b1_keys(&settings_op_unset(&["models"])), ["op", "path"]);
        assert_eq!(
            b1_keys(&settings_op_set(&["models"], &json!(null))),
            ["op", "path", "value"]
        );
    }

    /// ③ `expectedRevision` 两档各一颗：`None` = **整颗键不存在**，`Some` = 数值原样。
    /// 显式 `null` 过不了描述符那颗 union（`z.union([z.undefined(), z.number()])`），
    /// 所以这一档不许长成 `"expectedRevision":null`。
    #[test]
    fn expected_revision_on_settings_mutate_is_three_state() {
        let without = settings_mutate("permission", &[settings_op_unset(&["defaultPreset"])], None);
        assert!(
            without.args.get("expectedRevision").is_none(),
            "None 那一档长出了键：{}",
            without.args
        );
        assert_eq!(
            without.args.to_string(),
            r#"{"ns":"permission","ops":[{"op":"unset","path":["defaultPreset"]}]}"#
        );
        let with = settings_mutate("permission", &[settings_op_unset(&["defaultPreset"])], Some(0.0));
        assert_eq!(with.args["expectedRevision"], json!(0.0), "revision 被折成 null 或丢了");
        assert_eq!(b1_keys(&with.args), ["expectedRevision", "ns", "ops"]);
        // 0 与「缺席」分得开（0.0 是合法 revision，不许被当成 falsy 丢掉）
        assert_ne!(
            without.args.to_string(),
            with.args.to_string(),
            "Some(0.0) 与 None 折成了同一发"
        );
    }

    /// ⑤ 新名册基数 = 1、对既有五族互斥，且既有五族的基数**一枚没动**。
    #[test]
    fn the_new_roster_stays_alone_and_disjoint() {
        assert_eq!(SETTINGS_MUTATE_METHODS.len(), 1, "B-1 这张名册只该有一发");
        assert_eq!(SETTINGS_MUTATE_METHODS, [SETTINGS_MUTATE].as_slice());
        assert_eq!(CORDIS_METHODS.len(), 8, "既有那族的基数被这一刀动了");
        assert_eq!(RD9_C5_C6_METHODS.len(), 10, "既有那族的基数被这一刀动了");
        assert_eq!(RD9_C4_C9_METHODS.len(), 5, "既有那族的基数被这一刀动了");
        assert_eq!(CAPABILITY_METHODS.len(), 2, "既有那族的基数被这一刀动了");
        assert_eq!(SUBAGENTS_METHODS.len(), 3, "既有那族的基数被这一刀动了");
        assert_eq!(GOALS_MUTATION_METHODS.len(), 6, "既有那族的基数被这一刀动了");
        assert!(
            !RD9_C5_C6_METHODS.contains(&SETTINGS_MUTATE),
            "settings/mutate 被塞进了 RD9 那十发"
        );
        assert!(
            !RD9_C4_C9_METHODS.contains(&SETTINGS_MUTATE),
            "settings/mutate 被塞进了 C4/C9 那五发"
        );
        for foreign in CORDIS_METHODS
            .iter()
            .chain(RD9_C4_C9_METHODS.iter())
            .chain(RD9_C5_C6_METHODS.iter())
            .chain(CAPABILITY_METHODS.iter())
            .chain(SUBAGENTS_METHODS.iter())
            .chain(GOALS_MUTATION_METHODS.iter())
        {
            assert_ne!(*foreign, SETTINGS_MUTATE, "{foreign} 与本发撞名");
        }
        // 三头登记齐：const 的整串真在本文件源码里（口径同 `ten_consts_match_rd9_table_a_and_are_the_only_ten`）
        for method in SETTINGS_MUTATE_METHODS {
            assert!(
                B1_SELF.contains(&format!("\"{method}\"")),
                "{method} 的 const 不在这份源码里"
            );
        }
    }

    /// ⑥ `path` 恒为**数组**、`unset` 支不长 `value`、空 ops 照发 `"ops":[]`。
    /// 这一条专治 on1 §4 的 J1 假请求（把 `path` 写成 `"models"` 字符串 = strict codec 必拒）。
    #[test]
    fn paths_stay_arrays_and_unset_never_grows_a_value_key() {
        let unset = settings_op_unset(&["models"]);
        assert_eq!(unset.to_string(), r#"{"op":"unset","path":["models"]}"#);
        assert!(unset.get("value").is_none(), "unset 支自己长出了 value：{unset}");
        assert!(unset["path"].is_array(), "path 不是数组：{unset}");
        // 空 path 照发 `[]`（非空闸在派发侧）
        assert_eq!(settings_op_unset(&[])["path"], json!([]));
        // `value` 全域原样送出：null / 0 / false / 数组 / 对象都不折叠
        for value in [json!(null), json!(0), json!(false), json!(["a"]), json!({"a": 1})] {
            assert_eq!(
                settings_op_set(&["k"], &value)["value"],
                value,
                "本层偷偷替主干折了 value：{value}"
            );
        }
        // 空 ops 整发：键照在、值是空数组（`xaml.cs:12066` 的 `ops.Length > 0` 属派发侧）
        let no_ops: [Value; 0] = [];
        let empty = settings_mutate("permission", &no_ops, None);
        assert_eq!(empty.args.to_string(), r#"{"ns":"permission","ops":[]}"#);
        assert!(empty.args["ops"].is_array());
        assert_eq!(empty.args["ops"].as_array().map(Vec::len), Some(0));
    }

    /// ⑦ 反向锁：主干 7 发都 `await` 一句到底、不接返回值 ⇒ 本层不给这发的回执建解析器。
    /// 禁名一律 `concat!` 拆词 —— 本测试读的是全文件源码，整名一旦落纸就自匹配（判据恒假）。
    #[test]
    fn the_discarded_mutate_receipt_gets_no_parser_here() {
        for forbidden in [
            concat!("parse_settings_", "mutate_receipt"),
            concat!("parse_settings_", "mutate_view"),
            concat!("parse_", "settings_mutate"),
        ] {
            assert!(
                !B1_SELF.contains(forbidden),
                "本层给主干丢弃的回执造了判据：{forbidden}"
            );
        }
        // 反向锁不许空转：方法名与本层「不建解析」的口径都真在本文件里出现过
        assert!(B1_SELF.contains(concat!("settings/", "mutate")));
        assert!(
            B1_SELF.contains("不建解析器"),
            "「不建解析」的理由没落进注释 ⇒ 上面的反向判据是空的"
        );
        assert!(
            B1_SELF.contains("SettingsNamespaceView"),
            "回执那一型的符号名没写进注释 ⇒ 下一批会以为漏了东西"
        );
    }

    /// ④ 主干源码锁（运行时 `std::fs::read_to_string`，**不** `include_str!`）：那 7 发调用点
    /// 仍在、请求体逐字仍是 `ops` 数组 + `path` 数组、且 mutate 这一族从不带 `expectedRevision`。
    /// 三颗文件任一颗读不到 ⇒ 打印原因后 skip（别的机器上没有主干副本不算红）。
    #[test]
    fn the_seven_mainline_settings_mutate_callsites_are_still_there() {
        let needle = concat!("CallOkAsync(", "\"settings/mutate\"");
        let mut total = 0;
        let mut sources = Vec::with_capacity(B1_TRUNK.len());
        for (file, want) in B1_TRUNK {
            let Some(src) = b1_mainline(file) else { return };
            b1_assert_hits(&src, needle, want, file);
            total += want;
            sources.push((file, src));
        }
        assert_eq!(total, 7, "主干这一族的活调用点现测是 7 发");

        // 请求体形状：path 是数组、op 是 unset（on1 §4 的 J1）
        let model = &sources[0].1;
        b1_assert_hits(
            model,
            r#"ops = new[] { new { op = "unset", path = new[] { "models" } } },"#,
            1,
            "J1 unset 形状",
        );
        b1_assert_hits(
            model,
            r#"foreach (var ns in new[] { "llm-deepseek", "llm-pi-ai" })"#,
            1,
            "一发一个 ns（两支各一趟）",
        );
        b1_assert_hits(
            &sources[1].1,
            r#"ops = new[] { new { op = "unset", path = new[] { field } } },"#,
            1,
            "字段级恢复默认",
        );
        let xaml = &sources[2].1;
        b1_assert_hits(xaml, "new { ns = editor.SettingsNs, ops }", 1, "编辑卡那一发");
        b1_assert_hits(xaml, "new { ns = group.Key, ops }", 1, "整页保存那一发");
        b1_assert_hits(
            xaml,
            r#"ops = new object[] { new { op = "unset", path = row.SettingsPath } },"#,
            1,
            "删卡那一发（多段 path）",
        );
        b1_assert_hits(
            xaml,
            r#"ops = new[] { new { op = "set", path = new[] { "defaultPreset" }, value } },"#,
            1,
            "默认权限档那一发",
        );
        // 反向半边：这一族从不递 expectedRevision（等值 0，不是「大概没有」）
        b1_assert_hits(model, "expectedRevision", 0, "ModelExtras 不带 revision");
        b1_assert_hits(&sources[1].1, "expectedRevision", 0, "SettingsExtras 不带 revision");
        // xaml.cs 的那 2 处命中全在 `settings/replace`（`ResetSectionAsync`）⇒ mutate 调用点 4、
        // revision 2，两数各自钉死，谁漂了都红。
        b1_assert_hits(xaml, "expectedRevision", 2, "xaml.cs 的 revision 只属 replace 族");
        // 锁不许只靠 needle 活着：三颗文件的**规模下界** + **EOF 护栏**。
        // 为什么不再用等值行数闸：`MainWindow.xaml.cs` 是**被实时编辑的主干文件**（本轮实测一夜四变），
        // 把它钉成 `== 行数` 意味着每改一次就红一次，与「七发调用点还在不在」毫无关系 ⇒ **稳定假红源**。
        // 本锁的真判据是上面那 14 枚串锚（本轮实测 100% 全绿）；行数只保留一个用途：防「读短了 = 半颗文件」
        // 让命中数假 0。⇒ 下界 = 开工现测值往下取整，另加每颗一枚「以 `}` 收尾」的截断护栏（三颗 C# 文件皆然）。
        // 这条改口推翻 B-1 当初「不用 `>=` 因为下界等于没锁」的自我论证：`>=` 单用确实等于没锁，
        // **`>=` + EOF 护栏同用**才把「读短」这条路堵死，同时不再为主干加长买单。
        for (file, src) in &sources {
            eprintln!("b1 主干现测 {file} = {} 行", src.lines().count());
        }
        for (idx, floor) in [(0usize, 240usize), (1, 600), (2, 18000)] {
            let (file, src) = &sources[idx];
            assert!(
                src.lines().count() >= floor,
                "{file} 只读到 {} 行（下界 {floor}）⇒ 疑似半颗文件",
                src.lines().count()
            );
            assert!(
                src.trim_end().ends_with('}'),
                "{file} 不是以 `}}` 收尾 ⇒ 读短了 = 半颗文件，上面的 needle 命中数会假 0"
            );
        }
    }
}
