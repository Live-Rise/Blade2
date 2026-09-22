<div align="center">

<img src="Assets/Square150x150Logo.png" width="72" alt="Blade² — WinUI 3 desktop client for dsh">

# Blade²

</div>

**WinUI dsh desktop** — a native WinUI 3 desktop client for the dsh (DeepSeek Harness) agent kernel. No Electron, no WebView — just XAML talking straight to the kernel RPC. One self-contained MSIX, ready out of the box.

```
  ● open Blade²
  │
  ● bundled kernel on 127.0.0.1 (token handshake)
  │
  ● native WinUI · chat · approvals · settings · pets
  ✓ ready
```

**A Windows app for dsh. Not a browser window.**

[简体中文](README.zh.md) · v0.8.1 · Windows 10 19041+ · x64 · .NET 8 · WinUI 3

> **Status** · Rust rewrite of the Windows shell in progress · macOS version in development

<img src="docs/app-topbar.png" width="720" alt="Blade² — WinUI 3 native top bar">

---

## vs the official desktop app

- **Native interface** — no WebView; drawn in WinUI 3, a truly native Windows app
- **One package** — kernel and runtime bundled in the MSIX, nothing else to install
- **Isolated data** — its own data directory, never touches the CLI's `~/.dsh`
- **System integration** — system tray, live system light/dark tracking, Mica / acrylic material skins, system notifications
- **Desktop pet** — Codex-compatible pet packs; auto-imports your existing Codex pets, one-click install for new ones
- **Wallpaper skins** — custom images and videos as background skins, automatically compatible with Wallpaper Engine live wallpapers
- **Refined interactions** — turn rail, image lightbox, undo this turn, snippet search
- **Usage stats** — activity heatmap, current/longest streaks, daily token trend, per-model breakdown, with a selectable time range
- **Ready out of the box** — auto-approval, browser-use, computer-use, and memory MCP built in

<img src="docs/ScreenShot.png" width="720" alt="Blade² — dsh chat in the WinUI 3 main window">

## How it works

- The shell wraps the **kernel**, not the Web UI — upgrading dsh means swapping the kernel tree (`check-dsh-contract.ps1` verifies the contract).
- `Kernel/` (trimmed dsh + `node.exe`) ships inside the same MSIX. No system Node, no global npm, no Electron.
- All state lives in `%LOCALAPPDATA%\Blade2`, fully separate from the CLI's `~/.dsh`.
- The kernel binds `127.0.0.1` on a random port behind a process-level launch token — no credentials file ships in the package.

Key sources: [`Dsh/DshKernelHost.cs`](Dsh/DshKernelHost.cs) · [`Dsh/DshRpcClient.cs`](Dsh/DshRpcClient.cs) · [`Dsh/DshPetClient.cs`](Dsh/DshPetClient.cs) · [`MainWindow.xaml.cs`](MainWindow.xaml.cs)

## Install

Grab `Blade2_<version>_x64.msix` from [Releases](../../releases) (~140 MB, self-contained).

1. Trust the self-signed publisher cert once: `certutil -addstore Root Blade2.cer` in an admin prompt (the `.cer` is attached to the release). Without it Windows refuses the install with `0x800B010A`.
2. Double-click the MSIX, or run `Add-AppxPackage <msix>`. Windows 10 19041+ x64.

Already using the dsh CLI? Copy `~/.dsh/.credentials.yaml` into `%LOCALAPPDATA%\Blade2\` — nothing is migrated automatically.

## Build from source

Prerequisites: Windows x64, [.NET 8 SDK](https://dotnet.microsoft.com/), Visual Studio 2022 (only for its MSBuild, only when packaging).

```sh
dotnet run                # dev mode; falls back to system dsh.cmd if no bundled kernel
pwsh -File run-dev.ps1    # kills stale processes, launches the Debug build
```

Release MSIX packaging requires VS's MSBuild — `dotnet build` never produces an MSIX. Bump the version in **both** `Blade2.csproj` `<Version>` and `Package.appxmanifest` Identity.

```powershell
msbuild Blade2.csproj /p:Configuration=Release /p:Platform=x64 `
  /p:AppxPackageDir=C:/AppxPkgs/ /p:GenerateAppxPackageOnBuild=true /p:AppxBundle=Never
signtool sign /fd SHA256 /sha1 <thumbprint> <msix>
Add-AppxPackage <msix>    # kill the shell and its bundled node.exe first (0x80073D02)
```

Helper scripts: `build-0800.ps1` (one-shot release build) · `install-0800.ps1` (kill processes, verify signature, install) · `check-dsh-contract.ps1 -CliOnly`.

> Package identity is `Blade2` (`²` is illegal in a package identity; only the display name carries it). Publisher stays `CN=DshWinUI`. From Git Bash, `export MSYS_NO_PATHCONV=1` first.

## Data & debugging

- Data home: `%LOCALAPPDATA%\Blade2` (sessions, settings, `shell.json`, skin, `pets\`)
- Journal: `...\sessions\<ws>\<session>\session.v3.jsonl.zstd`
- Unhandled exceptions: `%TEMP%\blade2_unhandled.txt`
- UI automation: **UIA only**

No auto-update — install the newer MSIX over the old one.

## Platform status

- **Windows** — shipping .NET / WinUI 3 shell; Rust rewrite in progress.
- **macOS** — in development. See [`mac/README.md`](mac/README.md).

## Contributing

Bug fixes and doc improvements welcome. Design notes and kernel contract: [`docs/DESIGN.zh.md`](docs/DESIGN.zh.md).

## License

- [dsh / DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) — bundled in trimmed form under [MIT](Kernel/dsh/LICENSE) (© 2026 DeepSeek).
- Shell source: [MIT](LICENSE) (© 2026 SAKUSORA).
