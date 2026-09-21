using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;

namespace Blade2;

/// <summary>
/// 壳内建分区「宠物」+ 桌面宠物运行时。
///
/// 分三块：设置分区（RenderPetSection：显隐/尺寸、宠物库、安装、诊断）、
/// 独立窗口里的桌面宠物（状态轮询 + 图集帧动画 + 点击互动）、安装交互
/// （拖放 zip / 浏览 / 粘贴命令行）。
///
/// 宠物能力全部来自内核插件 @linxin666/dsh-pet（Codex Pet 兼容）：壳不自己
/// 实现任何宠物逻辑，只做两件事——把 Codex 宠物包装进 $DSH_HOME/pets/，
/// 以及把插件的图集帧（renderer=sprite2d/frames2d）自绘出来（原生渲染，
/// 无 WebView）。live2d 宠物壳不渲染，交给插件 Web UI。
/// 插件路由由 <see cref="DshPetClient"/> 走已鉴权的内核 HttpClient 访问；
/// 插件不在时 Status=Missing，分区给出安装/重启提示，其余功能静默不渲染。
///
/// 宠物画在 <see cref="PetWindow"/>——与主窗口平级的独立置顶窗口，不在主窗口里。
/// 插件把它定义为 host-global 的悬浮面（/api/pet/* 没有 session 维度，插件
/// issue #48），主窗口一被盖住/最小化就跟着消失的那不是桌面宠物。
/// </summary>
public sealed partial class MainWindow
{
    private DshPetClient? _petClient;
    private DshPetStore? _petStore;
    /// <summary>宠物窗口本体（null = 握手还没成功、运行时未启动）。</summary>
    private PetWindow? _petWindow;
    private bool _petRuntimeStarted;
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _petPollTimer;
    /// <summary>已装载图集的宠物 id（换宠物才重新下图集，轮询不重复下载）。</summary>
    private string? _petLoadedId;
    /// <summary>最近一次互动后的强制 waving 截止时刻（点击反馈）。</summary>
    private DateTime _petWaveUntil;
    /// <summary>安装卡的状态行（失败原因/成功提示，同一处刷新）。</summary>
    private TextBlock? _petInstallStatus;
    /// <summary>宠物库卡的行容器（安装成功后整库重载）。</summary>
    private StackPanel? _petLibraryRows;
    /// <summary>宠物库卡的「来源」判定缓存：本地 pets 目录 + Codex pets 目录的 id 集合。</summary>
    private HashSet<string>? _petLocalIds;
    private HashSet<string>? _petCodexIds;
    /// <summary>尺寸滑杆防抖：一次拖动只写一次 set-config。</summary>
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _petSizeDebounce;
    private double _petPendingSize;
    /// <summary>轮询重入哨兵：定时器的 Tick 不等 await 返回就会再响，不拦会叠成一串。</summary>
    private bool _petTickRunning;

    // ---------------- 运行时 ----------------

    /// <summary>
    /// 启动宠物运行时（BootAsync 握手成功后调用一次）。插件没装也不停：
    /// 轮询 404 → Status=Missing，等用户装完重启内核后再探。
    /// 设置分区可能在启动前就被打开（它会先建 client），因此幂等哨兵与
    /// client 本身分开：进过这里就一定把精灵与轮询配齐。
    /// </summary>
    private void StartPetRuntime()
    {
        if (_petRuntimeStarted) return;
        _petRuntimeStarted = true;
        _petClient ??= new DshPetClient(() => _rpc);
        _petStore ??= new DshPetStore(Path.Combine(DataHome, "pets"));
        // 独立窗口（不是主窗口里的一层）：拖拽位置/点击互动都归它自己管，
        // 这里只喂状态。窗口惰性建外观，宠物不可见时不占屏幕。
        var win = new PetWindow(Path.Combine(DataHome, "pet-window.json"));
        win.SetTheme(_shellDark);
        win.InteractRequested += OnPetInteractAsync;
        _petWindow = win;
        _petPollTimer = DispatcherQueue.CreateTimer();
        _petPollTimer.Interval = TimeSpan.FromSeconds(1.5);
        _petPollTimer.Tick += (_, _) => _ = OnPetPollTickAsync();
        _petPollTimer.Start();
    }

    /// <summary>收掉宠物运行时（应用退出时由 <see cref="Shutdown"/> 调用）。</summary>
    private void StopPetRuntime()
    {
        _petPollTimer?.Stop();
        var win = _petWindow;
        _petWindow = null;
        if (win is null) return;
        win.InteractRequested -= OnPetInteractAsync;
        win.Sprite.Stop();
        try
        {
            // 显式关掉：壳进程要等所有窗口都关了才退，主窗口关了还留着一个
            // 置顶宠物窗 = 进程赖在后台不走。Closed 里会存位置、摘子类化。
            win.Close();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] close failed: {ex}");
        }
    }

    /// <summary>一次轮询：拉状态 → 显隐 → 装载/切轨道 → 尺寸。</summary>
    private async Task OnPetPollTickAsync()
    {
        var client = _petClient;
        var win = _petWindow;
        var sprite = win?.Sprite;
        if (client is null || win is null || sprite is null) return;
        // DispatcherQueueTimer 的 Tick 不等待 await：上一次还没回来就又响一次，会把
        // 图集下载与装载叠成一串（装载卡住时尤其明显）。这里挡掉重入。
        if (_petTickRunning) return;
        _petTickRunning = true;
        try
        {
            var state = await client.GetStateAsync();
            Debug.WriteLine($"[pet] poll: state={(state is null ? "null" : $"pet={state.PetId} anim={state.Animation} vis={state.Visible} size={state.Size}")}  loadedId={_petLoadedId ?? "-"}  winVis={(_petWindow?.AppWindow.IsVisible.ToString() ?? "?")}");
            if (state is null)
            {
                // 内核重启中/插件未装：收回精灵，不打扰用户
                win.HidePet();
                sprite.Stop();
                _petLoadedId = null;
                return;
            }
            if (!state.Visible)
            {
                win.HidePet();
                sprite.Stop();
                return;
            }
            if (!await EnsurePetLoadedAsync(state.PetId)) { Debug.WriteLine("[pet] poll: EnsurePetLoadedAsync=false -> 不显示"); return; }
            win.ShowPet();
            // 互动后的强制 waving 优先于插件状态（jumping/failed 同理是一次性反馈）
            var animation = DateTime.UtcNow < _petWaveUntil && sprite.Track != "waving" ? "waving" : state.Animation;
            sprite.SetTrack(animation);
            sprite.ApplySize(state.Size);
        }
        catch (Exception ex)
        {
            // 轮询绝不能把异常抛进 DispatcherQueueTimer 的 Tick（会变成未观察异常）
            Debug.WriteLine($"[pet] poll failed: {ex}");
        }
        finally
        {
            _petTickRunning = false;
        }
    }

    /// <summary>
    /// 立刻跑一次轮询。插件侧 set-pet/set-config/set-visible 都是即时生效的，等下一个
    /// 1.5s 周期才反应会显得"切不动"；状态一改完就主动推一次。
    /// </summary>
    private void KickPetPoll() => _ = OnPetPollTickAsync();

    /// <summary>确保 id 对应的图集已装载；返回 false = 这只宠物壳画不了。</summary>
    private async Task<bool> EnsurePetLoadedAsync(string petId)
    {
        var client = _petClient;
        var sprite = _petWindow?.Sprite;
        if (client is null || sprite is null || petId == "") return false;
        if (_petLoadedId == petId && sprite.Ready) return true;
        _petLoadedId = null;

        var pets = await client.GetPetsAsync();
        var pet = pets?.FirstOrDefault(p => p.Id == petId);
        Debug.WriteLine($"[pet] EnsurePet: want={petId} pets={(pets is null ? "null" : pets.Count.ToString())} found={(pet is null ? "no" : pet.Id)} renderable={(pet?.ShellRenderable.ToString() ?? "?")} atlasUrl={(pet?.AtlasUrl ?? "-")}");
        if (pet is null) return false;
        if (!pet.ShellRenderable)
        {
            // live2d 宠物（cubism 运行时）壳不渲染：交给插件的 Web UI
            _petWindow?.HidePet();
            return false;
        }

        var rpc = _rpc;
        if (rpc is null || pet.AtlasUrl == "") return false;
        var atlasUrl = new Uri(rpc.BaseUri, pet.AtlasUrl).ToString();
        var bytes = await client.GetAssetBytesAsync(atlasUrl);
        Debug.WriteLine($"[pet] EnsurePet: atlas bytes={(bytes is null ? "null" : bytes.Length.ToString())}");
        if (bytes is null) return false;
        if (await sprite.LoadAsync(pet, bytes))
        {
            Debug.WriteLine("[pet] EnsurePet: LoadAsync(图集) ok");
            _petLoadedId = petId;
            return true;
        }
        // 图集解不动（多半是系统缺 WebP 映像扩展）：退到预览 GIF
        var gif = await client.GetAssetBytesAsync($"/pet/{Uri.EscapeDataString(petId)}/previews/idle.gif");
        Debug.WriteLine($"[pet] EnsurePet: 图集解不动，GIF bytes={(gif is null ? "null" : gif.Length.ToString())}");
        if (gif is not null && await sprite.TryShowGifAsync(gif))
        {
            Debug.WriteLine("[pet] EnsurePet: GIF 降级 ok");
            _petLoadedId = petId;
            return true;
        }
        _petWindow?.HidePet();
        return false;
    }

    /// <summary>用户点了宠物（按下+抬起且位移小于阈值，见 PetSurfaceLayer 的 DragThresholdPx）：喂一次互动，并强制播一段 waving。</summary>
    private async Task OnPetInteractAsync()
    {
        var client = _petClient;
        var win = _petWindow;
        if (client is null || win is null) return;
        try
        {
            // 互动照发（喂亲和度、触发插件侧计数）；回执文案没人消费，直接丢
            _ = await client.InteractAsync("pet");
            _petWaveUntil = DateTime.UtcNow.AddSeconds(2.5);
            win.Sprite.SetTrack("waving");
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] interact failed: {ex}");
        }
    }

    // ---------------- 设置分区 ----------------

    /// <summary>
    /// 壳内建分区「宠物」：插件开关态/显示参数、宠物库（选用/删除）、
    /// 安装（拖放 zip / 浏览 / 粘贴命令行）、诊断。
    /// 卡内数据全部异步拉：分区先出骨架，网络回来再填。
    /// </summary>
    private async void RenderPetSection()
    {
        SettingsHost.Children.Add(MakeSectionDesc(
            L("桌面宠物由内核插件 @linxin666/dsh-pet 提供（Codex Pet 兼容）：聊天窗口里常驻一只宠物，模型干活时它跟着动，点它可以逗一逗。宠物素材装在 $DSH_HOME/pets，重启内核后收录。")));

        var client = _petClient ??= new DshPetClient(() => _rpc);
        _petStore ??= new DshPetStore(Path.Combine(DataHome, "pets"));

        RenderPetDisplayCard(client);
        _petLibraryRows = RenderPetLibraryCard();
        RenderPetInstallCard();
        RenderPetDiagnosticsCard();

        // 探测插件路由：不在就把显示卡/库卡换成安装提示（安装卡本身仍可用——
        // 装宠物只是写目录，等插件就位后重启内核即收录）。
        var state = await client.GetStateAsync();
        if (client.Status == PetRouteStatus.Missing)
        {
            PetRouteMissingNotice();
            return;
        }
        if (state is not null) FillPetDisplayCard(state);
        await ReloadPetLibraryAsync();
    }

    /// <summary>插件路由 404 时的提示条（替换显示卡内容）。</summary>
    private void PetRouteMissingNotice()
    {
        if (_petDisplayBody is null) return;
        _petDisplayBody.Children.Clear();
        _petDisplayBody.Children.Add(new TextBlock
        {
            Text = L("宠物插件还没就位：安装包缺失或内核还没加载它。装过宠物后重启一次 Blade² 即可；引导器下次启动会自动重试安装。"),
            Style = AppStyle("CardDescriptionTextStyle"),
            Foreground = ThemeBrush("WarningBrush"),
            TextWrapping = TextWrapping.Wrap,
        });
    }

    private StackPanel? _petDisplayBody;
    private ToggleSwitch? _petVisibleToggle;
    private Slider? _petSizeSlider;
    private TextBlock? _petSizeValue;

    /// <summary>显示卡：显隐开关 + 尺寸滑杆（都写插件 set-config）。</summary>
    private void RenderPetDisplayCard(DshPetClient client)
    {
        var rows = NewCard(
            L("显示"),
            L("宠物显示在聊天窗口右下角，跟着模型的工作状态切换动画；点它可以互动。"));

        _petVisibleToggle = Aut(new ToggleSwitch
        {
            OffContent = L("关"),
            OnContent = L("开"),
            VerticalAlignment = VerticalAlignment.Center,
            Style = AppStyle("SettingsToggleSwitchStyle"),
        }, "PetVisibleToggle", L("显示宠物"));
        rows.Children.Add(MakeRow(L("显示宠物"), L("关掉后窗口里不再显示，插件其余功能照常"), _petVisibleToggle));
        _petVisibleToggle.Toggled += (_, _) =>
        {
            _ = WritePetConfigAsync(client, visible: _petVisibleToggle.IsOn);
            if (_petVisibleToggle.IsOn)
            {
                _petPollTimer?.Start();
                KickPetPoll();
            }
        };

        _petSizeSlider = Aut(new Slider
        {
            Minimum = 48,
            Maximum = 320,
            StepFrequency = 8,
            Value = 128,
            Width = 220,
        }, "PetSizeSlider", L("宠物大小"));
        _petSizeValue = new TextBlock
        {
            Style = AppStyle("BodyTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
            MinWidth = 48,
            Text = "",
        };
        var sizeRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = TokenDouble("Space12", 12) };
        sizeRow.Children.Add(_petSizeSlider);
        sizeRow.Children.Add(_petSizeValue);
        rows.Children.Add(MakeRow(L("宠物大小"), L("单元格高度的像素值，拖动即时生效"), sizeRow));
        _petSizeSlider.ValueChanged += (_, _) =>
        {
            _petSizeValue!.Text = LF("{0} px", (int)_petSizeSlider.Value);
            _petPendingSize = _petSizeSlider.Value;
            if (_petSizeDebounce is null)
            {
                var timer = DispatcherQueue.CreateTimer();
                timer.Interval = TimeSpan.FromMilliseconds(250);
                timer.Tick += (_, _) =>
                {
                    timer.Stop();
                    _ = WritePetConfigAsync(client, size: _petPendingSize);
                };
                _petSizeDebounce = timer;
            }
            _petSizeDebounce.Stop();
            _petSizeDebounce.Start();
        };

        _petDisplayBody = rows;
    }

    private void FillPetDisplayCard(PetStateView state)
    {
        if (_petVisibleToggle is not null) _petVisibleToggle.IsOn = state.Visible;
        if (_petSizeSlider is not null && state.Size > 0)
        {
            _petSizeSlider.Value = Math.Clamp(state.Size, _petSizeSlider.Minimum, _petSizeSlider.Maximum);
            _petSizeValue!.Text = LF("{0} px", (int)_petSizeSlider.Value);
        }
    }

    private async Task WritePetConfigAsync(DshPetClient client, double? size = null, bool? visible = null)
    {
        try
        {
            await client.SetConfigAsync(size: size, visible: visible);
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[pet] set-config failed: {ex}");
        }
    }

    /// <summary>宠物库卡：注册表清单一行一只（预览图 + 名称 + 来源 + 使用/删除）。</summary>
    private StackPanel RenderPetLibraryCard()
    {
        var rows = NewCard(
            L("宠物库"),
            L("已收录的宠物（插件内置 + ~/.codex/pets + 本机安装的）。切换立即生效。"));
        rows.Children.Add(new TextBlock
        {
            Text = L("正在读取宠物清单…"),
            Style = AppStyle("CardDescriptionTextStyle"),
        });
        return rows;
    }

    /// <summary>重载宠物库行。安装成功后调用（新装的要重启内核才进注册表）。</summary>
    private async Task ReloadPetLibraryAsync()
    {
        if (_petLibraryRows is null || _petClient is null) return;
        var rows = _petLibraryRows;
        rows.Children.Clear();
        _petLocalIds = _petStore?.ScanLocal().Select(p => p.Id).ToHashSet(StringComparer.Ordinal)
            ?? new HashSet<string>(StringComparer.Ordinal);
        _petCodexIds = DshPetStore.ScanCodexIds().ToHashSet(StringComparer.OrdinalIgnoreCase);

        var pets = await _petClient.GetPetsAsync();
        if (pets is null || pets.Count == 0)
        {
            rows.Children.Add(new TextBlock
            {
                Text = L("一只宠物都没有。装一个：把 Codex 宠物 zip 拖到下面的安装区，或粘贴 petdex install <宠物标识>。"),
                Style = AppStyle("CardDescriptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
            return;
        }
        var state = await _petClient.GetStateAsync();
        foreach (var pet in pets.OrderBy(p => p.DisplayName, StringComparer.Ordinal))
        {
            rows.Children.Add(await MakePetRowAsync(pet, state?.PetId == pet.Id));
        }
    }

    /// <summary>宠物库的一行：64px 预览（GIF/PNG 预览图优先，拿不到用字形占位）+ 名称/来源 + 操作。</summary>
    private async Task<FrameworkElement> MakePetRowAsync(PetDefinition pet, bool active)
    {
        var row = new Grid { ColumnSpacing = TokenDouble("Space12", 12), Padding = new Thickness(0, Sp6, 0, Sp6) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var preview = await LoadPetPreviewAsync(pet);
        preview.Width = 56;
        preview.Height = 56;
        preview.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(preview, 0);
        row.Children.Add(preview);

        var text = new StackPanel { Spacing = Sp2, VerticalAlignment = VerticalAlignment.Center };
        var title = new TextBlock { Text = pet.DisplayName, Style = AppStyle("BodyTextStyle") };
        text.Children.Add(title);
        var meta = new List<string>();
        if (active) meta.Add(L("使用中"));
        meta.Add(PetSourceLabel(pet.Id));
        // 不能自绘的两类要分开说：live2d 是渲染器不同，另一种是轨道数据不全
        if (pet.Renderer == "live2d") meta.Add(L("Live2D（壳内不渲染）"));
        else if (!pet.ShellRenderable) meta.Add(L("图集帧数据不全，壳内不渲染"));
        text.Children.Add(new TextBlock
        {
            Text = string.Join(" · ", meta),
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextSecondaryBrush"),
        });
        if (!string.IsNullOrEmpty(pet.Description))
        {
            text.Children.Add(new TextBlock
            {
                Text = pet.Description,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                TextWrapping = TextWrapping.Wrap,
                MaxLines = 2,
            });
        }
        Grid.SetColumn(text, 1);
        row.Children.Add(text);

        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = Sp4, VerticalAlignment = VerticalAlignment.Center };
        var use = Aut(new Button
        {
            Content = active ? L("使用中") : L("使用"),
            Style = AppStyle(active ? "CompactButtonStyle" : "AccentButtonStyle"),
            IsEnabled = !active,
        }, $"PetUse_{pet.Id}", LF("使用宠物 {0}", pet.DisplayName));
        use.Click += (_, _) => _ = SelectPetAsync(pet);
        actions.Children.Add(use);
        if (_petLocalIds?.Contains(pet.Id) == true)
        {
            var remove = Aut(new Button
            {
                Content = L("删除"),
                Style = AppStyle("CompactButtonStyle"),
                Foreground = ThemeBrush("ErrorBrush"),
            }, $"PetRemove_{pet.Id}", LF("删除宠物 {0}", pet.DisplayName));
            remove.Click += (_, _) => _ = DeletePetAsync(pet);
            actions.Children.Add(remove);
        }
        Grid.SetColumn(actions, 2);
        row.Children.Add(actions);
        return row;
    }

    /// <summary>来源标记：本机安装 / Codex 目录 / 插件内置。</summary>
    private string PetSourceLabel(string petId)
        => _petLocalIds?.Contains(petId) == true ? L("本机安装")
            : _petCodexIds?.Contains(petId) == true ? L("Codex 宠物")
            : L("插件内置");

    /// <summary>宠物预览：previews/idle.gif（或 png/webp），都没有就字形占位。</summary>
    private async Task<FrameworkElement> LoadPetPreviewAsync(PetDefinition pet)
    {
        if (_petClient is not null)
        {
            foreach (var name in new[] { "idle.gif", "idle.png", "idle.webp" })
            {
                var bytes = await _petClient.GetAssetBytesAsync($"/pet/{Uri.EscapeDataString(pet.Id)}/previews/{name}");
                if (bytes is null or { Length: 0 }) continue;
                try
                {
                    var image = new Image { Stretch = Stretch.Uniform };
                    image.Source = await MakeBitmapAsync(bytes, decodePixelWidth: 112);
                    return image;
                }
                catch (Exception)
                {
                    // 这张预览解不动就试下一张（WebP 扩展缺失时 .webp 会走到这里）
                }
            }
        }
        return new FontIcon
        {
            Glyph = pet.ShellRenderable ? "\uE76E" : "\uE9CE",
            FontSize = 28,
            Foreground = ThemeBrush("TextTertiaryBrush"),
            VerticalAlignment = VerticalAlignment.Center,
        };
    }

    private async Task SelectPetAsync(PetDefinition pet)
    {
        if (_petClient is null) return;
        Debug.WriteLine($"[pet] SelectPet: 点击「使用」 id={pet.Id}");
        var ok = await _petClient.SetPetAsync(pet.Id);
        Debug.WriteLine($"[pet] SelectPet: SetPetAsync({pet.Id}) = {ok}");
        _petLoadedId = null; // 强制重下图集
        if (ok)
        {
            await ReloadPetLibraryAsync();
            KickPetPoll();   // 插件已即时应用，不等下一个 1.5s 周期
        }
    }

    private async Task DeletePetAsync(PetDefinition pet)
    {
        if (_petStore is null) return;
        // 删除前问一句：删掉的是用户装进来的宠物，删了要重启内核才从注册表消失
        var dialog = new ContentDialog
        {
            Title = LF("删除宠物「{0}」？", pet.DisplayName),
            Content = L("只删除本机安装目录里的文件。插件内置与 Codex 目录里的宠物不受影响；删除后重启 Blade² 生效。"),
            PrimaryButtonText = L("删除"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Content.XamlRoot,
        };
        var result = await dialog.ShowAsync();
        if (result != ContentDialogResult.Primary) return;
        if (_petStore.Delete(pet.Id)) await ReloadPetLibraryAsync();
    }

    // ---------------- 安装 ----------------

    /// <summary>
    /// 安装卡：拖放区（Codex 宠物 zip）+ 浏览按钮 + 命令行粘贴。
    /// 命令行只解析不执行（见 <see cref="DshPetStore.InstallFromCommandAsync"/>）。
    /// </summary>
    private void RenderPetInstallCard()
    {
        var rows = NewCard(
            L("安装宠物"),
            L("兼容 Codex 宠物包（pet.json + 图集）。拖放 zip 到下方区域，或粘贴安装命令。"));

        var drop = new Border
        {
            CornerRadius = new CornerRadius(TokenDouble("RadiusMedium", 8)),
            BorderThickness = Stroke1,
            BorderBrush = ThemeBrush("StrokeBrush"),
            Background = ThemeBrush("CardSecondaryBrush"),
            Padding = new Thickness(Sp16, Sp16, Sp16, Sp16),
            AllowDrop = true,
            HorizontalAlignment = HorizontalAlignment.Stretch,
        };
        drop.DragOver += OnPetDropDragOver;
        drop.DragLeave += OnPetDropDragLeave;
        drop.Drop += OnPetDrop;
        var dropText = new StackPanel { Spacing = Sp4, HorizontalAlignment = HorizontalAlignment.Center };
        dropText.Children.Add(new TextBlock
        {
            Text = L("拖放以安装"),
            Style = AppStyle("BodyStrongTextStyle"),
            HorizontalAlignment = HorizontalAlignment.Center,
        });
        dropText.Children.Add(new TextBlock
        {
            Text = L("把 Codex 宠物 zip 从文件资源管理器拖到这里即可安装"),
            Style = AppStyle("CardDescriptionTextStyle"),
            HorizontalAlignment = HorizontalAlignment.Center,
        });
        var browse = Aut(new Button
        {
            Content = L("浏览并安装"),
            Style = AppStyle("AccentButtonStyle"),
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, Sp12, 0, 0),
        }, "PetBrowseButton", L("浏览并安装宠物压缩包"));
        browse.Click += (_, _) => _ = PickAndInstallPetZipAsync();
        dropText.Children.Add(browse);
        drop.Child = dropText;
        rows.Children.Add(drop);

        var commandBox = Aut(new TextBox
        {
            PlaceholderText = L("petdex install <宠物标识>"),
            HorizontalAlignment = HorizontalAlignment.Stretch,
        }, "PetCommandBox", L("宠物安装命令行"));
        var install = Aut(new Button
        {
            Content = L("安装"),
            Style = AppStyle("AccentButtonStyle"),
        }, "PetInstallButton", L("按命令行安装宠物"));
        install.Click += (_, _) => _ = InstallPetFromCommandAsync(commandBox.Text);
        var commandRow = new Grid { ColumnSpacing = TokenDouble("Space8", 8), Margin = new Thickness(0, Sp12, 0, 0) };
        commandRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        commandRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(commandBox, 0);
        Grid.SetColumn(install, 1);
        commandRow.Children.Add(commandBox);
        commandRow.Children.Add(install);
        rows.Children.Add(commandRow);

        _petInstallStatus = new TextBlock
        {
            Text = "",
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, Sp8, 0, 0),
        };
        rows.Children.Add(_petInstallStatus);
    }

    private void OnPetDropDragOver(object sender, DragEventArgs e)
    {
        if (e.DataView.Contains(StandardDataFormats.StorageItems))
        {
            e.AcceptedOperation = DataPackageOperation.Copy;
            e.Handled = true;
            if (sender is Border border) border.BorderBrush = ThemeBrush("AccentBrush");
        }
    }

    private void OnPetDropDragLeave(object sender, DragEventArgs e)
    {
        if (sender is Border border) border.BorderBrush = ThemeBrush("StrokeBrush");
    }

    /// <summary>拖放安装：只接 .zip（宠物包）。多文件逐个装，全部结果汇总到状态行。</summary>
    private async void OnPetDrop(object sender, DragEventArgs e)
    {
        if (sender is Border border) border.BorderBrush = ThemeBrush("StrokeBrush");
        if (!e.DataView.Contains(StandardDataFormats.StorageItems)) return;
        var deferral = e.GetDeferral();
        try
        {
            var items = await e.DataView.GetStorageItemsAsync();
            var zips = items.OfType<StorageFile>()
                .Where(f => f.Path.EndsWith(".zip", StringComparison.OrdinalIgnoreCase))
                .ToArray();
            if (zips.Length == 0)
            {
                PetInstallSay(L("只支持 Codex 宠物 zip（里面有 pet.json 和图集）。"), warn: true);
                return;
            }
            foreach (var zip in zips)
            {
                await InstallPetZipAsync(zip.Path);
            }
        }
        catch (Exception ex)
        {
            PetInstallSay(LF("安装失败：{0}", ex.Message), warn: true);
        }
        finally
        {
            deferral.Complete();
        }
        e.Handled = true;
    }

    private async Task PickAndInstallPetZipAsync()
    {
        try
        {
            var picker = new Windows.Storage.Pickers.FileOpenPicker();
            var hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
            WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
            picker.FileTypeFilter.Add(".zip");
            picker.SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.Downloads;
            var file = await picker.PickSingleFileAsync();
            if (file is null) return;
            await InstallPetZipAsync(file.Path);
        }
        catch (Exception ex)
        {
            PetInstallSay(LF("选择文件失败：{0}", ex.Message), warn: true);
        }
    }

    /// <summary>装一个本地 zip，并把结果与「重启内核才收录」的提示一起显示。</summary>
    private async Task InstallPetZipAsync(string zipPath)
    {
        if (_petStore is null) return;
        PetInstallSay(LF("正在安装 {0}…", Path.GetFileName(zipPath)));
        var result = await _petStore.InstallZipFileAsync(zipPath);
        if (result.Ok && result.Pet is not null)
        {
            PetInstallSay(LF("已安装「{0}」到 {1}。重启 Blade² 后出现在宠物库里。",
                result.Pet.DisplayName, result.Pet.Directory));
            await ReloadPetLibraryAsync();
            return;
        }
        PetInstallSay(result.Message, warn: true);
    }

    /// <summary>按粘贴的命令行安装（只解析，不执行）。</summary>
    private async Task InstallPetFromCommandAsync(string command)
    {
        if (_petStore is null) return;
        if (string.IsNullOrWhiteSpace(command))
        {
            PetInstallSay(L("先粘贴一条安装命令，例如 petdex install whale-girl。"), warn: true);
            return;
        }
        PetInstallSay(L("正在解析并安装…"));
        var result = await _petStore.InstallFromCommandAsync(command);
        if (result.Ok && result.Pet is not null)
        {
            PetInstallSay(LF("已安装「{0}」到 {1}。重启 Blade² 后出现在宠物库里。",
                result.Pet.DisplayName, result.Pet.Directory));
            await ReloadPetLibraryAsync();
            return;
        }
        PetInstallSay(result.Message, warn: true);
    }

    private void PetInstallSay(string message, bool warn = false)
    {
        if (_petInstallStatus is null) return;
        _petInstallStatus.Text = message;
        _petInstallStatus.Foreground = warn ? ThemeBrush("ErrorBrush") : ThemeBrush("TextSecondaryBrush");
    }

    // ---------------- 诊断 ----------------

    /// <summary>诊断卡：插件挂载状态 + 注册表诊断（清单加载的 error/warning）+ 渲染降级说明。</summary>
    private async void RenderPetDiagnosticsCard()
    {
        var rows = NewCard(
            L("诊断"),
            L("宠物不显示时先看这里：插件在不在、注册表报了什么、壳能不能画。"));

        var plugin = DshPluginBootstrap.GetStatus(DataHome)
            .FirstOrDefault(s => s.Id == "@linxin666/dsh-pet");
        rows.Children.Add(new TextBlock
        {
            Text = plugin is null || !plugin.Installed
                ? L("宠物插件（@linxin666/dsh-pet）未安装。")
                : plugin.Mounted
                    ? L("宠物插件已安装并选入内核 bundle。")
                    : L("宠物插件已安装但没选进 package.json 的 dsh.profile.bundles，内核不会加载它。"),
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });

        var body = new StackPanel { Spacing = Sp4 };
        rows.Children.Add(body);
        body.Children.Add(new TextBlock
        {
            Text = L("正在读取注册表诊断…"),
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
        });

        var client = _petClient;
        if (client is null) return;
        var list = await client.GetDiagnosticsAsync();
        body.Children.Clear();
        if (client.Status == PetRouteStatus.Missing)
        {
            body.Children.Add(new TextBlock
            {
                Text = L("插件路由没有响应（/api/pet/* 404）：宠物功能整体不可用，重启 Blade² 让内核重新加载插件。"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("WarningBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
            return;
        }
        if (list.Count == 0)
        {
            body.Children.Add(new TextBlock
            {
                Text = L("注册表没有报错。"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
            });
        }
        foreach (var item in list)
        {
            body.Children.Add(new TextBlock
            {
                Text = $"[{item.Level}] {item.Source}: {item.Message}",
                Style = AppStyle("CaptionTextStyle"),
                Foreground = item.Level == "error" ? ThemeBrush("ErrorBrush") : ThemeBrush("WarningBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        body.Children.Add(new TextBlock
        {
            Text = L("若宠物位置显示不出来：Windows 需要 WebP 映像扩展才能解 .webp 图集，缺失时壳会退到预览 GIF，再不行就连精灵一起隐藏。"),
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextTertiaryBrush"),
            TextWrapping = TextWrapping.Wrap,
        });
    }
}
