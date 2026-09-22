using System.Diagnostics;
using System.Reflection;
using System.Security.Cryptography.X509Certificates;
using System.Security.Principal;
using System.Text;

// Blade2-Setup.exe — 下一步式安装向导（面向普通用户，免费路线）
// 双击运行 → UAC 授权 → 欢迎页 → 安装说明页 → 确认页 → 安装进度 → 完成页（可选启动）
// MSIX 来源: 优先取 exe 同目录的 Blade2_*_x64.msix, 没有则用编译期内嵌的那份。

const string TargetVersion = "0.8.2.0";
const string PublisherThumbprint = "E2B4870249B661186E86F816E2A403261E74F6A0";
const string PackageName = "Blade2";
const string DisplayName = "Blade²";

// CN=DshWinUI 发布者证书公钥(DER→Base64), 2027-09-11 到期, 不含私钥
const string PublisherCertBase64 =
    "MIIDBDCCAeygAwIBAgIQMBHiR00op5BMr5g+MlgyBjANBgkqhkiG9w0BAQsFADATMREwDwYDVQQDDAhEc2hXaW5VSTAeFw0yNjA5MTEwNDMyMTJa" +
    "Fw0yNzA5MTEwNDUyMTJaMBMxETAPBgNVBAMMCERzaFdpblVJMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA46N4QHNvNSAU1TkQ1Z" +
    "IVbtpQt9W3Z7plsnfF0vi5E9vBSV51VZXPKMDKMFKMaUioxp5TQpsJk4qMDdlUyYhPFwzmmrDB6/iOvxt7V1wDM6YDrXTH0/ZkXl8gacmP8fbX2" +
    "FnxGcSdi0m8F2T/p2+E8oYwZt6r+x2TO0zanhux/upvtNuo7Xen6Ard1qX3OWmCNc+56G7Wv0Q7qNrRrzwcIvBr7K3eqyC24+/Qufy/82WRHz" +
    "S1w2k4Unz05pTxO/zjhlNKV9Kv713EyNvJm9QYTRUJPJFNRws3XA8kQLYuWD8trEIZVX6w8cVX8xlXCkjijjb0sKH0EWLOt/MHoBvQzQIDAQ" +
    "ABo1QwUjAOBgNVHQ8BAf8EBAMCB4AwEwYDVR0lBAwwCgYIKwYBBQUHAwMwDAYDVR0TAQH/BAIwADAdBgNVHQ4EFgQU3SZzuYv7Pv3hPXfRSu" +
    "b840CaRVgwDQYJKoZIhvcNAQELBQADggEBAAQp9eToo1p4gcrVI0OabrY5xD45/ZT+PERySLlR0i7fdzx1GvuKS1j7noQMwbuQnohJPz5gwb" +
    "jRi7RWnYTz0+FEbuGNNeq9XfD9IgTQfSRRNBtKD7Xi/IL3+NJfwvWmIPnMx9nqeVY2dSLmEbdZRy3pPGlmwiRaGz4TiEDJ9NsqF+wpwMW1Uc" +
    "Q4qy1taDibqiFvIEG0WblZi91oHdpFed1NCFEx0lpEUnYKM6jhaWk2NtUixTcn9Zo0izhVGqtHNprwNAvba3/NTNjVCT0Jk45sP7ezqUNdl" +
    "DvVlqe1qRAqEykUzB1MYA7pJW//fpIewEiXdF40xASvYlIYsoCT5i4=";

try { Console.OutputEncoding = Encoding.UTF8; } catch { /* 重定向输出时忽略 */ }

// --no-pause：无人值守（每页按默认选项走，装完不等人按键）
bool autoAnswer = args.Any(a => a.Equals("--no-pause", StringComparison.OrdinalIgnoreCase));

// ---------- 向导外壳 ----------

void Rule(char c = '-') => Console.WriteLine(new string(c, 66));

void Head(string step, string title)
{
    Console.WriteLine();
    Rule('=');
    Console.WriteLine($"  {step}  {title}");
    Rule('=');
}

void Ok(string s)   => Console.WriteLine($"  [完成] {s}");
void Info(string s) => Console.WriteLine($"  {s}");
void Note(string s) => Console.WriteLine($"  [注意] {s}");

// 页脚输入：回车/空行 = 默认项；也接受关键词（下一步/上一步/取消）。Console.ReadLine 在输入被
// 重定向到空流时返回 null（无人值守），同样按默认项走——按键读不了，见不得 InvalidOperationException。
Choice Ask(Choice fallback, string hint)
{
    if (autoAnswer) return fallback;
    Console.WriteLine();
    Console.Write($"  {hint}");
    var line = Console.ReadLine();
    if (line is null) return fallback;
    var t = line.Trim();
    return t switch
    {
        "" or "n" or "N" or "next" or "下一步" or "继续" => Choice.Next,
        "b" or "B" or "back" or "上一步" or "返回" => Choice.Back,
        _ => Choice.Cancel,
    };
}

void WaitEnter()
{
    if (autoAnswer) return;
    try { Console.WriteLine(); Console.Write("  按回车退出..."); Console.ReadLine(); } catch { }
}

// ---------- 0. 管理员自提权 ----------
// app.manifest 已声明 requireAdministrator，正常双击即为管理员；这里的兜底只防
// 重新编译时丢了 manifest 的场景（不靠它弹 UAC）。
using var me = WindowsIdentity.GetCurrent();
if (!new WindowsPrincipal(me).IsInRole(WindowsBuiltInRole.Administrator))
{
    Console.WriteLine("  需要管理员权限(导入证书 + 安装应用), 即将弹出 UAC, 请点击\"是\"...");
    Process.Start(new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = true, Verb = "runas" });
    return 0;
}

// ---------- 页面状态 ----------
var page = 0;
var installedBefore = RunPs($"Get-AppxPackage -Name {PackageName} | Select-Object -ExpandProperty Version").Output.Trim();
var msix = ResolveMsix();

// 欢迎页与说明页只渲染，确认页开始要看当前版本；安装页真正干活。
var pages = new Func<Choice>[]
{
    PageWelcome,
    PageNotes,
    PageConfirm,
};

while (page < pages.Length)
{
    var choice = pages[page]();
    switch (choice)
    {
        case Choice.Next:
            page++;
            break;
        case Choice.Back:
            page = Math.Max(0, page - 1);
            break;
        default:
            Console.WriteLine();
            Info("已取消安装，未做任何改动。");
            WaitEnter();
            return 0;
    }
}

var failure = PageInstall(); // 页内自带「正在安装」标题
if (failure is not null)
{
    Note(failure);
    WaitEnter();
    return 1;
}

Head("安装完成", DisplayName);
var installed = RunPs($"(Get-AppxPackage -Name {PackageName} | Select-Object -First 1).Version").Output.Trim();
if (installed == TargetVersion) Ok($"版本校验通过: {TargetVersion}");
else Note($"已安装版本 {installed}, 预期 {TargetVersion}");

Info("可从开始菜单搜索 \"Blade2\" 启动。");
if (!autoAnswer)
{
    Console.WriteLine();
    Console.Write("  现在启动 Blade2？[Y/n] ");
    var answer = Console.ReadLine();
    if (answer is null || answer.Trim().ToLowerInvariant() is "" or "y" or "yes" or "是")
    {
        TryLaunch();
    }
}
WaitEnter();
return 0;

// ---------- 页面 ----------

Choice PageWelcome()
{
    Head("第 1 步 / 共 3 步", $"欢迎使用 {DisplayName} 安装向导");
    Info($"本向导将引导你完成 {DisplayName} {TargetVersion} 的安装。");
    Console.WriteLine();
    Info($"  产品:      {DisplayName}（dsh 内核 WinUI 桌面客户端）");
    Info($"  版本:      {TargetVersion}");
    Info($"  发布者:    CN=DshWinUI");
    Info($"  安装方式:  当前用户（应用包）");
    Console.WriteLine();
    Note("安装前建议保存并关闭正在运行的 Blade2。");
    return Ask(Choice.Next, "按回车开始: ");
}

Choice PageNotes()
{
    Head("第 2 步 / 共 3 步", "安装说明");
    Info("本安装器将执行以下操作：");
    Console.WriteLine();
    Info("  1. 导入 Blade2 发布者证书（否则系统会以 0x800B010A 拒绝自签名安装包）");
    Info($"  2. 结束正在运行的 {DisplayName} 进程");
    Info($"  3. 安装/升级 {DisplayName} {TargetVersion}");
    Info("  4. 校验安装结果");
    Console.WriteLine();
    Info("安装过程需要管理员权限（导入证书、写系统应用包存储）。");
    return Ask(Choice.Next, "[回车] 下一步   [B] 上一步   [X] 取消: ");
}

Choice PageConfirm()
{
    Head("第 3 步 / 共 3 步", "确认安装");
    if (msix.Path is null)
    {
        // exe 同目录没有 MSIX、内嵌资源也没有（编译时 payload 为空）：无从装起
        Note("找不到安装包。请从发布页重新下载 Blade2-Setup.exe。");
        return Choice.Cancel;
    }
    Info($"目标版本: {TargetVersion}");
    Info(installedBefore == TargetVersion
        ? $"当前版本: {installedBefore}（已是最新版本，将跳过安装）"
        : installedBefore.Length > 0
            ? $"当前版本: {installedBefore}（将升级覆盖）"
            : "当前版本: 未安装");
    Info($"安装包:   {Path.GetFileName(msix.Path)}（{new FileInfo(msix.Path!).Length / 1024 / 1024} MB）");
    Console.WriteLine();
    Info("点击「开始安装」继续。");
    return Ask(Choice.Next, "[回车] 开始安装   [B] 上一步   [X] 取消: ");
}

// ---------- 安装页：落每一步的结果，失败即停 ----------

string? PageInstall()
{
    Head("正在安装", $"{DisplayName} {TargetVersion}");

    Info("[1/4] 导入发布者证书…");
    var certBytes = Convert.FromBase64String(PublisherCertBase64);
    var cert = new X509Certificate2(certBytes);
    foreach (var (loc, store) in new[] { (StoreLocation.LocalMachine, StoreName.Root), (StoreLocation.LocalMachine, StoreName.TrustedPeople) })
    {
        try
        {
            using var s = new X509Store(store, loc);
            s.Open(OpenFlags.ReadWrite);
            if (s.Certificates.Find(X509FindType.FindByThumbprint, PublisherThumbprint, false).Count > 0)
            {
                Ok($"证书已在 {store} 信任存储");
                continue;
            }
            s.Add(cert);
            Ok($"证书已导入 {store}");
        }
        catch (Exception ex)
        {
            return $"导入证书失败: {ex.Message}";
        }
    }

    Info("[2/4] 结束正在运行的 Blade2…");
    var running = Process.GetProcessesByName(PackageName);
    if (running.Length == 0) Ok("没有正在运行的 Blade2");
    else
    {
        foreach (var p in running)
        {
            try { p.Kill(entireProcessTree: true); Ok($"已结束 PID {p.Id}"); } catch { }
        }
        System.Threading.Thread.Sleep(2000); // 等文件句柄释放：不然 Add-AppxPackage 会撞文件占用
    }

    Info($"[3/4] 安装 {PackageName} {TargetVersion}…");
    var (code, output) = RunPs($"Get-AppxPackage -Name {PackageName} | Select-Object -ExpandProperty Version");
    var current = output.Trim();
    if (current == TargetVersion)
    {
        Ok($"已是最新版本 {TargetVersion}，跳过安装。");
        return null;
    }
    if (current.Length > 0)
    {
        Info($"检测到旧版本 {current}，正在卸载…");
        var (_, fullName) = RunPs($"(Get-AppxPackage -Name {PackageName}).PackageFullName");
        // 同版本号不同内容也会被 0x80073CFB 挡住：先删后装是唯一稳的路（与 install-*.ps1 同理）。
        RunPs($"Remove-AppxPackage -Package '{fullName.Trim()}'");
    }
    (code, output) = RunPs($"try {{ Add-AppxPackage -Path '{msix.Path}' -ErrorAction Stop }} catch {{ Write-Output $_.Exception.Message; exit 2 }}");
    if (code != 0)
    {
        return $"安装出错: {output.Trim()}\n若错误含 0x800B010A，说明证书仍未被信任，请把本窗口截图反馈给发布者。";
    }
    Ok("Add-AppxPackage 成功");

    Info("[4/4] 校验…");
    (code, output) = RunPs($"(Get-AppxPackage -Name {PackageName} | Select-Object -First 1).Version");
    if (output.Trim() == TargetVersion) Ok($"版本校验通过: {TargetVersion}");
    else Note($"已安装版本 {output.Trim()}，预期 {TargetVersion}");
    return null;
}

// ---------- 工具 ----------

static (string? Path, bool Embedded) ResolveMsix()
{
    var beside = Directory.GetFiles(AppContext.BaseDirectory, $"{PackageName}_*_x64.msix");
    if (beside.Length > 0) return (beside[0], false);
    var asm = Assembly.GetExecutingAssembly();
    var name = asm.GetManifestResourceNames().FirstOrDefault(n => n.EndsWith(".msix", StringComparison.OrdinalIgnoreCase));
    if (name is not null)
    {
        using var stream = asm.GetManifestResourceStream(name)!;
        var path = Path.Combine(Path.GetTempPath(), PackageName, TargetVersion, $"{PackageName}_{TargetVersion}_x64.msix");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        using var file = File.Create(path);
        stream.CopyTo(file);
        return (path, true);
    }
    return (null, false);
}

static void TryLaunch()
{
    try
    {
        // MSIX 应用不能直接跑 exe 路径：拼 AUMID 走 shell:appsFolder 启动（开始菜单同一条路）
        var (pfnCode, pfnOut) = RunPs($"(Get-AppxPackage -Name {PackageName}).PackageFamilyName");
        var (appIdCode, appIdOut) = RunPs($"(Get-AppxPackageManifest (Get-AppxPackage -Name {PackageName}).PackageFullName).Package.Applications.Application.Id");
        var family = pfnOut.Trim();
        var id = appIdOut.Trim();
        if (pfnCode != 0 || appIdCode != 0 || family.Length == 0 || id.Length == 0) return;
        Process.Start(new ProcessStartInfo("explorer.exe", $"shell:appsFolder\\{family}!{id}") { UseShellExecute = true });
    }
    catch (Exception) { } // 启动失败不阻塞安装器收尾
}

static (int Code, string Output) RunPs(string command)
{
    var psi = new ProcessStartInfo("powershell.exe")
    {
        Arguments = $"-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{command}\"",
        RedirectStandardOutput = true,
        RedirectStandardError = true,
        UseShellExecute = false,
        CreateNoWindow = true
    };
    using var proc = Process.Start(psi)!;
    var stdout = proc.StandardOutput.ReadToEnd();
    var stderr = proc.StandardError.ReadToEnd();
    proc.WaitForExit();
    return (proc.ExitCode, stdout + stderr);
}

enum Choice { Next, Back, Cancel }
