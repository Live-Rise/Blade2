using System;
using System.IO;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Blade2;

/// <summary>
/// 设置「个性化」分区（壳内建分区：不进内核 settings/mutate、不参与「恢复本页默认」）：
///  · 自定义指令：读写 <c>$DSH_HOME/AGENTS.md</c>。内核 agent-instructions 插件
///    （standard/cordis/ptc 预设都注册，maxBytes 65536）把这个文件当 user-global
///    指令注入每个会话的上下文，所以这里写的就是「对所有对话始终生效」的那段话，
///    不需要内核改动。DSH_HOME = DataHome（见 DshKernelHost.StartAsync），与 shell.json 同目录。
///  · 窗口材质：窗口级 SystemBackdrop 选择（Mica / Mica Alt / 亚克力 / 无），
///    切换即时生效并持久化到壳本地 shell.json（与托盘开关同文件）。
///  · 背景皮肤：导入/清除内容区背景图（从「通用设置」搬来），透明度滑杆实时调整并持久化。
/// 指令用防抖自动落盘（停手约 1 秒）而不是显式保存按钮：写指令是低风险编辑，
/// 误触写进的也只是用户自己那段话，撤回来再删掉即可；切分区/换语言前还会强制落盘。
/// </summary>
public partial class MainWindow
{
    private const string PersonalizationSectionId = "personalization";

    /// <summary>内核 agent-instructions 的 user-global 指令文件（$DSH_HOME/AGENTS.md）。
    /// 只碰这一个文件，不动各工作区自己的 AGENTS.md。</summary>
    private string InstructionsFile => Path.Combine(DataHome, "AGENTS.md");

    /// <summary>编辑器长度上限。内核 64KB 渲染预算由全部指令文件（user-global + 各工作区）分摊，
    /// 壳只占其中一份：4000 字足够写约束，也挤不爆预算。</summary>
    private const int InstructionsMaxLength = 4000;

    private TextBox? _instructionsBox;
    private TextBlock? _instructionsStatus;

    /// <summary>指令状态行语气：Success 只是保存成功那一瞬，常态提示走 Neutral（次要文字色），
    /// 免得「共 0 字 · 停止输入后自动保存」这种说明文字常年顶着成功绿。</summary>
    private enum InstructionsStatusTone
    {
        Neutral,
        Success,
        Failure,
    }
    // 防抖落盘计时器：停手约 1 秒才写文件，避免逐字符写盘（同记忆卡策略）。
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _instructionsSaveTimer;
    // 内存草稿：切分区/换语言只换视图，控件重建后回填，不丢未提交内容（同 _settingsEdits 的约定）。
    // null = 本次会话还没读过指令文件。
    private string? _instructionsDraft;
    // 上次落盘的内容：脏检查以它为准。
    private string _instructionsLoaded = "";
    private bool _instructionsDirty;
    // 程序化写 Text 也会触发 TextChanged，用它把「回填」和「用户输入」区分开。
    private bool _instructionsSuppressChange;

    private void RenderPersonalizationSection()
    {
        SettingsHost.Children.Add(MakeSectionDesc(L("指令决定它怎么回应你，材质与皮肤决定它看起来是什么样。")));
        RenderInstructionsCard();
        RenderMaterialCard();
        RenderBubbleCard();
        RenderSkinCard();
    }

    // ---------------- 窗口材质（窗口级 SystemBackdrop，壳本地 shell.json） ----------------

    /// <summary>材质下拉选项：id = shell.json material 值（同时作 ComboBoxItem.Tag），
    /// 标签为中文原文，英文翻译走 ShellEnglish（本地化扫描覆盖 ComboBoxItem.Content）。</summary>
    private static readonly (string Id, string Label)[] MaterialChoices =
    {
        (MaterialMica, "Mica"),
        (MaterialMicaAlt, "Mica Alt"),
        (MaterialAcrylic, "亚克力"),
        (MaterialNone, "无（纯色）"),
    };

    /// <summary>窗口材质卡：ComboBox 下拉，选择即时换材质并写回壳本地配置（无保存按钮，
    /// 与托盘开关同一形态）。打开分区时选中态 = 当前生效材质。</summary>
    private void RenderMaterialCard()
    {
        var card = NewCard(
            L("窗口材质"),
            L("侧栏与顶栏透出的系统材质：Mica 柔和、亚克力更透；系统不支持时自动回退。"));

        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200) };
        Aut(box, "Setting_shell_material", L("窗口材质"));
        foreach (var (id, label) in MaterialChoices)
        {
            var item = new ComboBoxItem { Content = label, Tag = id };
            box.Items.Add(item);
            if (id == _shellMaterial)
            {
                box.SelectedItem = item;
            }
        }
        // 先赋初值后挂事件，Loaded 标记再挡掉装载期的一次性回调：初始选中不算用户操作
        var ready = false;
        box.SelectionChanged += (_, _) =>
        {
            try
            {
                if (ready && (box.SelectedItem as ComboBoxItem)?.Tag is string id)
                {
                    SetShellMaterial(id);
                }
            }
            catch (Exception) { } // 事件入口兜底
        };
        box.Loaded += (_, _) => ready = true;
        card.Children.Add(MakeRow(L("材质"), L("切换后立即生效，重启后保持"), box));
    }

    // ---------------- 消息气泡（气泡材质 + 不透明度，壳本地 shell.json） ----------------

    /// <summary>气泡材质下拉选项：id = shell.json bubbleMaterial 字段值（同时作 ComboBoxItem.Tag），
    /// 标签为中文原文（与窗口材质卡同一形态：壳自有选项）。</summary>
    private static readonly (string Id, string Label)[] BubbleMaterialChoices =
    {
        (BubbleMaterialTranslucent, "半透明"),
        (BubbleMaterialAcrylic, "亚克力"),
        (BubbleMaterialFollow, "跟随窗口材质"),
    };

    /// <summary>消息气泡卡：气泡材质下拉 + 不透明度滑块，都即时生效并写回壳本地配置
    ///（与窗口材质卡同一形态，无保存按钮）。半透明档把当前主题面色按不透明度透出来，
    /// 背后是视频/壁纸时画面直接透出；亚克力档用 WinUI 3 自带的 AcrylicBrush；
    /// 跟随档跟窗口材质走：mica/mica-alt 平面、acrylic 玻璃、none 纯色卡。</summary>
    private void RenderBubbleCard()
    {
        var card = NewCard(
            L("消息气泡"),
            L("气泡背景：半透明直接透出背后画面，亚克力是系统材质；两项都即时生效。"));

        var box = new ComboBox { MinWidth = TokenDouble("FieldMinWidth", 200) };
        Aut(box, "Setting_shell_bubbleMaterial", L("气泡材质"));
        foreach (var (id, label) in BubbleMaterialChoices)
        {
            var item = new ComboBoxItem { Content = label, Tag = id };
            box.Items.Add(item);
            if (id == _bubbleMaterial)
            {
                box.SelectedItem = item;
            }
        }
        // 先赋初值后挂事件，Loaded 标记再挡掉装载期的一次性回调：初始选中不算用户操作
        var ready = false;
        box.SelectionChanged += (_, _) =>
        {
            try
            {
                if (ready && (box.SelectedItem as ComboBoxItem)?.Tag is string id)
                {
                    SetBubbleMaterial(id);
                }
            }
            catch (Exception) { } // 事件入口兜底
        };
        box.Loaded += (_, _) => ready = true;
        card.Children.Add(MakeRow(L("气泡材质"), L("半透明最透，亚克力是系统材质，跟随跟窗口材质走"), box));

        var slider = Aut(new Slider
        {
            Minimum = BubbleOpacityMin * 100,
            Maximum = BubbleOpacityMax * 100,
            StepFrequency = 5,
            Value = Math.Round(_bubbleOpacity * 100),
            Width = 220,
        }, "Setting_shell_bubbleOpacity", L("气泡不透明度"));
        var value = new TextBlock
        {
            Style = AppStyle("BodyTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
            MinWidth = 48,
            Text = LF("{0}%", (int)slider.Value),
        };
        var opacityRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = TokenDouble("Space12", 12) };
        opacityRow.Children.Add(slider);
        opacityRow.Children.Add(value);
        card.Children.Add(MakeRow(L("气泡不透明度"), L("数值越大气泡自身越实，背后画面透出越少"), opacityRow));
        slider.ValueChanged += (_, _) =>
        {
            value.Text = LF("{0}%", (int)slider.Value);
            SetBubbleOpacity(slider.Value / 100);
        };
    }

    // ---------------- 自定义指令（$DSH_HOME/AGENTS.md） ----------------

    /// <summary>自定义指令卡：整卡宽的多行编辑器 + 下方状态行（无保存按钮：停手自动保存）。
    /// 版式对齐参考形态（大编辑区在下、状态行在下），文案全部自撰。</summary>
    private void RenderInstructionsCard()
    {
        var card = NewCard(
            L("自定义指令"),
            L("写给所有对话的长期说明：怎么称呼你、用什么语气、有哪些固定约束。对所有会话始终生效。"));

        _instructionsBox = new TextBox
        {
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 160,
            MaxHeight = 320,
            MaxLength = InstructionsMaxLength,
            Margin = new Thickness(0, 4, 0, 0),
        };
        Aut(_instructionsBox, "PersonalizationInstructionsEditor", L("指令内容"));
        _instructionsBox.TextChanged += (_, _) => OnInstructionsTextChanged();
        // 失焦即落盘：鼠标点到别处（同页其他控件/回聊天页）不再等 1s 防抖。
        _instructionsBox.LostFocus += (_, _) => FlushInstructionsPendingSave();
        card.Children.Add(_instructionsBox);

        _instructionsStatus = new TextBlock
        {
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 8, 0, 0),
        };
        card.Children.Add(_instructionsStatus);

        StartInstructionsSaveTimer();
        LoadInstructions();
        UpdateInstructionsState();
    }

    /// <summary>读指令文件进内存草稿（只读一次磁盘；之后草稿归窗口管）。
    /// 文件不存在/读失败 = 空指令，不当错误——大多数用户从没创建过这个文件。</summary>
    private void LoadInstructions()
    {
        if (_instructionsDraft is null)
        {
            var text = "";
            try
            {
                if (File.Exists(InstructionsFile))
                {
                    text = File.ReadAllText(InstructionsFile).Trim();
                }
            }
            catch (Exception) { } // 读不到按空处理：编辑器仍可写入并覆盖
            _instructionsDraft = text;
            _instructionsLoaded = text;
        }
        _instructionsDirty = _instructionsDraft != _instructionsLoaded;
        if (_instructionsBox is not null)
        {
            _instructionsSuppressChange = true;
            try
            {
                _instructionsBox.Text = _instructionsDraft;
            }
            finally
            {
                _instructionsSuppressChange = false;
            }
        }
    }

    private void OnInstructionsTextChanged()
    {
        if (_instructionsSuppressChange || _instructionsBox is null)
        {
            return;
        }
        _instructionsDraft = _instructionsBox.Text ?? "";
        _instructionsDirty = _instructionsDraft != _instructionsLoaded;
        UpdateInstructionsState();
        // 防抖：停手约 1 秒才落盘，避免逐字符写文件；改回原内容则撤销待触发的保存。
        if (_instructionsSaveTimer is { } timer)
        {
            timer.Stop();
            if (_instructionsDirty)
            {
                timer.Start();
            }
        }
    }

    /// <summary>落盘指令。清空 = 删文件：留一个空 AGENTS.md 只会往上下文里塞一段空指令。
    /// 内核按轮做版本比对，写入后下一条消息即带上新内容，无需重启。</summary>
    private void SaveInstructions()
    {
        if (!_instructionsDirty)
        {
            return; // 内容与上次落盘一致（计时器可能在用户改回原样后仍到点）
        }
        var text = (_instructionsDraft ?? "").Trim();
        try
        {
            if (text.Length == 0)
            {
                if (File.Exists(InstructionsFile))
                {
                    File.Delete(InstructionsFile);
                }
            }
            else
            {
                Directory.CreateDirectory(Path.GetDirectoryName(InstructionsFile)!);
                File.WriteAllText(InstructionsFile, text + Environment.NewLine);
            }
            _instructionsDraft = text;
            _instructionsLoaded = text;
            _instructionsDirty = false;
            SetInstructionsStatus(LF("已自动保存 · {0}", DateTime.Now.ToString("HH:mm:ss")), InstructionsStatusTone.Success);
        }
        catch (Exception ex)
        {
            SetInstructionsStatus(LF("保存失败：{0}", ex.Message), InstructionsStatusTone.Failure);
        }
        UpdateInstructionsState();
    }

    private void UpdateInstructionsState()
    {
        if (_instructionsStatus is null)
        {
            return;
        }
        if (!_instructionsDirty)
        {
            // 刚保存/刚载入：保留上一次的结果文案（已自动保存 / 失败原因），不刷掉。
            if (string.IsNullOrEmpty(_instructionsStatus.Text))
            {
                SetInstructionsStatus(
                    _instructionsLoaded.Length == 0
                        ? L("共 0 字 · 停止输入后自动保存")
                        : LF("字数 {0} / {1}", _instructionsLoaded.Length, InstructionsMaxLength),
                    InstructionsStatusTone.Neutral);
            }
            return;
        }
        SetInstructionsStatus(L("有未保存的修改…"), InstructionsStatusTone.Neutral);
    }

    /// <summary>离开分区前落盘未保存修改（防抖计时器可能还没到点）。</summary>
    private void FlushInstructionsPendingSave()
    {
        try
        {
            if (_instructionsDirty)
            {
                SaveInstructions();
            }
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine($"[instructions/save] {ex}");
        }
    }

    /// <summary>停掉防抖计时器并解掉控件引用（分区卸载时调用，避免后台 tick 写已丢弃的控件树）。</summary>
    private void UnloadInstructionsCard()
    {
        _instructionsSaveTimer?.Stop();
        // 必须置空：否则 StartInstructionsSaveTimer 的 not null 守卫会让已停的旧计时器
        // 跨分区复用，防抖静默失效 = 自动保存失败。
        _instructionsSaveTimer = null;
        _instructionsBox = null;
        _instructionsStatus = null;
    }

    /// <summary>启动指令保存防抖（1s）。渲染卡时调用一次，之后 TextChanged 只重启计时。</summary>
    private void StartInstructionsSaveTimer()
    {
        if (_instructionsSaveTimer is not null)
        {
            return;
        }
        // CreateTimer 是实例方法：走 Window 继承来的 DispatcherQueue 属性（同记忆卡）。
        var timer = DispatcherQueue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(1000);
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            SaveInstructions();
        };
        _instructionsSaveTimer = timer;
    }

    private void SetInstructionsStatus(string text, InstructionsStatusTone tone)
    {
        if (_instructionsStatus is null)
        {
            return;
        }
        _instructionsStatus.Text = text;
        _instructionsStatus.Foreground = tone switch
        {
            InstructionsStatusTone.Success => ThemeBrush("SuccessBrush"),
            _ => ThemeBrush("TextSecondaryBrush"),
        };
    }

    // ---------------- 背景皮肤（内容区背景图，自「通用设置」迁入） ----------------

    private void RenderSkinCard()
    {
        var skinCard = NewCard(
            L("背景皮肤"),
            L("导入图片或视频作为写代码时的内容区背景，用滑杆调到既能看出图、又不吃正文的浓度"));

        var skinRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = TokenDouble("Space8", 8) };
        var importBtn = Aut(new Button
        {
            Content = L("导入图片"),
            Style = AppStyle("CompactButtonStyle"),
        }, "SkinImportButton", L("导入背景皮肤"));
        importBtn.Click += (_, _) => _ = PickAndApplySkinAsync();
        var importVideoBtn = Aut(new Button
        {
            Content = L("导入视频"),
            Style = AppStyle("CompactButtonStyle"),
        }, "SkinVideoImportButton", L("导入视频背景皮肤"));
        importVideoBtn.Click += (_, _) => _ = PickAndApplySkinVideoAsync();
        var clearBtn = Aut(new Button
        {
            Content = L("清除皮肤"),
            Style = AppStyle("CompactButtonStyle"),
        }, "SkinClearButton", L("清除背景皮肤"));
        clearBtn.Click += (_, _) => ClearSkin();
        skinRow.Children.Add(importBtn);
        skinRow.Children.Add(importVideoBtn);
        skinRow.Children.Add(clearBtn);
        skinCard.Children.Add(MakeRow(
            L("背景图"),
            L("png / jpg / webp / bmp · mp4 / webm / mov（视频静音循环，与图片二选一）"),
            skinRow));

        // Wallpaper Engine 壁纸源（第三种来源：默认安装的插件供文件，壳只消费路由）
        AddWallpaperEngineRow(skinCard);

        var opacitySlider = Aut(new Slider
        {
            Minimum = 10,
            Maximum = 80,
            StepFrequency = 5,
            Value = SkinOpacityPercent,
            Width = 200,
        }, "SkinOpacitySlider", L("背景透明度"));
        _skinOpacityValue = new TextBlock
        {
            Style = AppStyle("BodyTextStyle"),
            VerticalAlignment = VerticalAlignment.Center,
            MinWidth = 44,
            Text = LF("{0}%", SkinOpacityPercent),
        };
        var opacityRow = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = TokenDouble("Space12", 12),
        };
        opacityRow.Children.Add(opacitySlider);
        opacityRow.Children.Add(_skinOpacityValue);
        opacitySlider.ValueChanged += (_, _) => OnSkinOpacityChanged(opacitySlider.Value);
        skinCard.Children.Add(MakeRow(
            L("背景透明度"),
            L("拖动即时生效；没有导入图片时先记下，导入后按这个浓度显示"),
            opacityRow));

        // 失焦暂停：视频背景只在窗口聚焦时播放，切走/最小化自动停，回焦继续
        var pauseToggle = Aut(new ToggleSwitch
        {
            IsOn = _skinVideoPauseOnBlur,
            OffContent = L("关"),
            OnContent = L("开"),
            VerticalAlignment = VerticalAlignment.Center,
            Style = AppStyle("SettingsToggleSwitchStyle"),
        }, "SkinVideoPauseToggle", L("窗口失焦时暂停视频背景"));
        pauseToggle.Toggled += (_, _) => OnSkinVideoPauseToggled(pauseToggle.IsOn);
        skinCard.Children.Add(MakeRow(
            L("失焦暂停视频"),
            L("视频背景只在窗口聚焦时播放；切到别的程序时自动暂停，回焦继续"),
            pauseToggle));
    }

    /// <summary>透明度滑杆拖动：实时改当前 ImageBrush，落盘走 300ms 防抖（一次拖动不写几十次文件）。</summary>
    private void OnSkinOpacityChanged(double percent)
    {
        _skinOpacity = Math.Clamp(percent, 10, 80) / 100.0;
        if (_skinOpacityValue is not null)
        {
            _skinOpacityValue.Text = LF("{0}%", SkinOpacityPercent);
        }
        if (_skinBrush is not null)
        {
            _skinBrush.Opacity = _skinOpacity;
        }
        if (_skinVideoElement is not null)
        {
            _skinVideoElement.Opacity = _skinOpacity;
        }
        _skinOpacitySaveTimer?.Stop();
        _skinOpacitySaveTimer?.Start();
    }
}
