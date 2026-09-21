using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

/// <summary>
/// 设置分区「技能」：Blade² 已安装技能的全部家当——按根目录列出、添加、删除、关闭。
///
/// 为什么是自己扫目录而不是只调 skills/list：
///   内核唯一的技能 RPC 是 skills/list（@deepseek-ai/dsh-api-session-controller 的
///   SessionSkillCatalog.list），它只回 isUserInvocable 的**元数据**，既不带文件路径，
///   也看不见被关闭的技能——只靠它，「关闭后重新打开」就永远做不到了。
///   内核 @deepseek-ai/dsh-skill-filesystem 的权威语义是：技能 = 根目录下的目录包
///   （&lt;name&gt;/SKILL.md）或扁平 .md，frontmatter 决定可见性，watcher 热加载。
///   所以这里按同一组根目录自己枚举文件，与内核同一份真相。
///
/// 根目录与优先级（dsh-skill-filesystem 的 rank，数字小者胜出，同名其余被忽略）：
///   100 project-dsh   &lt;projectRoot&gt;/.dsh/skills   项目根 = 从会话 cwd 向上最近的含 .git 目录
///   200 project-agents&lt;projectRoot&gt;/.agents/skills
///   300 preset        &lt;dshHome&gt;/.agent-presets/&lt;预设&gt;/skills 与随包预设（dsh-agent-presets/presets/&lt;预设&gt;/skills）
///   400 user-dsh      &lt;dshHome&gt;/skills（= Blade² 数据家，内核的 DSH_HOME）
///   500 user-agents   ~/.agents/skills
///
/// 「关闭」的内核语义：frontmatter 没有 disabled 位，可见性由两个布尔控制——
///   user-invocable: false        → / 菜单与 skills/list 都不再出现
///   disable-model-invocation: true → 模型也不再能调用
/// 两个一起置位才是彻底关闭；恢复即移除这两行（缺省都是 true）。
/// 内核 watcher（patchReload=live）会即时拾取改动，无需重启。
///
/// 只读根（随包预设）不给删/关：它们属于安装包，升级即被覆盖，禁用入口没有意义。
/// </summary>
public sealed partial class MainWindow
{
    /// <summary>技能名规则：与内核 dsh-skill 的 SKILL_NAME 正则逐字一致（小写 kebab-case）。</summary>
    private static readonly Regex SkillNamePattern = new("^[a-z0-9]+(?:-[a-z0-9]+)*$", RegexOptions.Compiled, TimeSpan.FromSeconds(1));

    /// <summary>frontmatter 里的两个可见性开关（键名与内核 parseInvocationPolicy 一致）。</summary>
    private const string SkillUserInvocableKey = "user-invocable";
    private const string SkillDisableModelInvocationKey = "disable-model-invocation";

    /// <summary>一个已安装的技能（来自某个根目录里的一个文件）。</summary>
    private sealed class SkillEntry
    {
        public required string Name = "";
        public string Description = "";
        public string WhenToUse = "";
        public required string FilePath = "";
        /// <summary>目录包 = 文件在 &lt;root&gt;/&lt;name&gt;/SKILL.md；删除时整目录删。</summary>
        public required bool IsBundle = true;
        public bool UserInvocable = true;
        public bool ModelInvocable = true;
        public required SkillRoot Root = null!;
        /// <summary>同名技能在更高优先级根里已存在（内核只会用那个，本条目被忽略）。</summary>
        public bool Shadowed;
        public bool Writable => Root.Writable;
    }

    /// <summary>一个技能根目录（内核 dsh-skill-filesystem 的一个 root）。</summary>
    private sealed class SkillRoot
    {
        public required string Label = "";
        public required string Path = "";
        public required int Rank = 0;
        /// <summary>用户可增删改的根（项目根与用户根）；随包预设只读。</summary>
        public required bool Writable = false;
        /// <summary>会话缺失时项目根不可用（没有 cwd 就定不了项目根）。</summary>
        public required bool Available = true;
        public required string UnavailableReason = "";
        public List<SkillEntry> Entries { get; } = new();
    }

    /// <summary>壳内建分区「技能」入口：枚举全部根 → 渲染（增删改后整体重渲染，逻辑保持单一）。</summary>
    private async Task RenderSkillsSectionAsync()
    {
        if (_rpc is null)
        {
            return;
        }
        SettingsHost.Children.Add(MakeSectionDesc(
            L("Blade² 已安装的全部技能。技能是带说明的指令包：在输入框键入 /名称 即可调用，模型也可能在合适的时机自行调用。")));

        List<SkillRoot> roots;
        try
        {
            roots = ScanSkillRoots();
        }
        catch (Exception ex)
        {
            SettingsHost.Children.Add(MakeSectionDesc(LF("扫描技能目录失败：{0}", ex.Message)));
            return;
        }

        // ---- 概览卡：计数 + 优先级说明 + 添加入口 ----
        var all = roots.SelectMany(r => r.Entries).ToList();
        var overview = NewCard(L("已安装技能"), L("按内核 @deepseek-ai/dsh-skill-filesystem 的根目录与优先级扫描，与内核看到的是同一份真相。"));
        overview.Children.Add(new TextBlock
        {
            Text = LF("共 {0} 个技能：{1}", all.Count,
                string.Join("，", roots
                    .Where(r => r.Entries.Count > 0)
                    .Select(r => LF("{0} {1} 个", r.Label, r.Entries.Count)))),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, Sp8, 0, Sp4),
        });
        overview.Children.Add(new TextBlock
        {
            Text = L("同名技能只生效优先级最高的一个：项目 > 预设 > 用户；被遮蔽的条目在列表里标注。"),
            Style = AppStyle("CardDescriptionTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });

        var addButton = new Button
        {
            Content = L("添加技能"),
            Margin = new Thickness(0, Sp8, 0, Sp4),
        };
        Aut(addButton, "SkillsAddButton", L("添加技能"));
        addButton.Click += async (_, _) => await AddSkillAsync(roots);
        overview.Children.Add(addButton);

        var status = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextSecondaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, Sp4, 0, Sp4),
        };
        overview.Children.Add(status);

        if (all.Count == 0)
        {
            overview.Children.Add(new TextBlock
            {
                Text = L("还没有安装任何技能。用上面的「添加技能」写第一个，或在项目的 .dsh/skills 目录里放一个。"),
                Style = AppStyle("CardDescriptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        foreach (var hint in roots.Where(r => !r.Available).Select(r => r.UnavailableReason).Where(h => h.Length > 0))
        {
            overview.Children.Add(new TextBlock
            {
                Text = hint,
                Style = AppStyle("CardDescriptionTextStyle"),
                TextWrapping = TextWrapping.Wrap,
            });
        }

        // ---- 每个根一张卡：根路径 + 该根下的技能行 ----
        foreach (var root in roots)
        {
            var card = NewCard(root.Label, root.Path);
            if (root.Entries.Count == 0)
            {
                card.Children.Add(new TextBlock
                {
                    Text = root.Available ? L("（这个目录里还没有技能）") : root.UnavailableReason,
                    Style = AppStyle("CardDescriptionTextStyle"),
                    TextWrapping = TextWrapping.Wrap,
                    Margin = new Thickness(0, Sp8, 0, Sp8),
                });
                continue;
            }
            foreach (var skill in root.Entries)
            {
                card.Children.Add(MakeSkillRow(skill, status, roots));
            }
        }
    }

    /// <summary>一行技能：名称/说明/适用场景在左，删除按钮与开关在右（开关贴最右缘）。</summary>
    private Grid MakeSkillRow(SkillEntry skill, TextBlock status, List<SkillRoot> roots)
    {
        var row = new Grid
        {
            Padding = new Thickness(0, Sp8, 0, Sp8),
            ColumnSpacing = Sp12,
        };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var left = new StackPanel { Spacing = Sp2, VerticalAlignment = VerticalAlignment.Center };
        left.Children.Add(new TextBlock
        {
            Text = "/" + skill.Name + (skill.Shadowed ? L("（已被遮蔽，不生效）") : ""),
            Style = AppStyle("BodyTextStyle"),
            TextWrapping = TextWrapping.Wrap,
        });
        if (skill.Description.Length > 0)
        {
            left.Children.Add(new TextBlock
            {
                Text = skill.Description,
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextSecondaryBrush"),
                TextWrapping = TextWrapping.Wrap,
                IsTextSelectionEnabled = true,
            });
        }
        if (skill.WhenToUse.Length > 0)
        {
            left.Children.Add(new TextBlock
            {
                Text = LF("适用场景：{0}", skill.WhenToUse),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                TextWrapping = TextWrapping.Wrap,
                IsTextSelectionEnabled = true,
            });
        }
        if (!skill.ModelInvocable)
        {
            left.Children.Add(new TextBlock
            {
                Text = L("模型不可自行调用（仅你能调用）"),
                Style = AppStyle("CaptionTextStyle"),
                Foreground = ThemeBrush("TextTertiaryBrush"),
                TextWrapping = TextWrapping.Wrap,
            });
        }
        Grid.SetColumn(left, 0);
        row.Children.Add(left);

        var toggle = new ToggleSwitch
        {
            OnContent = L("开"),
            OffContent = L("关"),
            IsOn = skill.UserInvocable,
            VerticalAlignment = VerticalAlignment.Center,
            Style = AppStyle("SettingsToggleSwitchStyle"),
        };
        Aut(toggle, $"SkillToggle_{skill.Name}", LF("启用技能 {0}", skill.Name));
        toggle.Toggled += async (_, _) =>
        {
            await SetSkillEnabledAsync(skill, toggle.IsOn, status, roots);
        };
        Grid.SetColumn(toggle, 2);
        row.Children.Add(toggle);

        var remove = new Button
        {
            Content = new FontIcon { Glyph = "\uE74D", FontSize = GlyphBody },
            VerticalAlignment = VerticalAlignment.Center,
        };
        Aut(remove, $"SkillRemove_{skill.Name}", LF("删除技能 {0}", skill.Name));
        ToolTipService.SetToolTip(remove, LF("删除 {0}", skill.Name));
        remove.Click += async (_, _) => await RemoveSkillAsync(skill, status, roots);
        Grid.SetColumn(remove, 1);
        row.Children.Add(remove);

        return row;
    }

    // ---------------- 根目录枚举（镜像内核 dsh-skill-filesystem） ----------------

    /// <summary>枚举本部署的全部技能根并解析其中的技能。文件级异常只跳过该条，不让整页失败。</summary>
    private List<SkillRoot> ScanSkillRoots()
    {
        var roots = new List<SkillRoot>();
        var dshHome = DataHome; // 内核启动时拿到的就是它（DshKernelHost.StartAsync(dshHome)）
        var cwd = ActiveSessionCwd;

        // 100 / 200：项目根（最近含 .git 的祖先，找不到就用 cwd —— 与内核 findProjectRoot 同规则）
        if (cwd is { Length: > 0 } projectCwd)
        {
            var projectRoot = FindProjectRoot(projectCwd);
            roots.Add(MakeRoot(L("项目"), Path.Combine(projectRoot, ".dsh", "skills"), 100));
            roots.Add(MakeRoot(L("项目（兼容）"), Path.Combine(projectRoot, ".agents", "skills"), 200));
        }
        else
        {
            var missing = MakeRoot(L("项目"), "", 100, writable: false);
            missing.Available = false;
            missing.UnavailableReason = L("项目技能需要先选中一个会话：它们按会话所在的项目目录解析。");
            roots.Add(missing);
            var missingCompat = MakeRoot(L("项目（兼容）"), "", 200, writable: false);
            missingCompat.Available = false;
            roots.Add(missingCompat);
        }

        // 300：预设自带技能（随包分发，只读）
        var presetRoot = MakeRoot(L("预设"), "", 300, writable: false);
        var presetDirs = new List<(string Label, string Path)>();
        var localPresetRoot = Path.Combine(dshHome, ".agent-presets");
        try
        {
            if (Directory.Exists(localPresetRoot))
            {
                presetDirs.AddRange(Directory.GetDirectories(localPresetRoot)
                    .Select(d => (Label: Path.GetFileName(d), Path: Path.Combine(d, "skills"))));
            }
        }
        catch (Exception) { /* 读不到本地预设目录就只列随包预设 */ }
        var shippedPresetRoot = Path.Combine(dshHome, "profiles", "node_modules", "@deepseek-ai", "dsh-agent-presets", "presets");
        try
        {
            if (Directory.Exists(shippedPresetRoot))
            {
                presetDirs.AddRange(Directory.GetDirectories(shippedPresetRoot)
                    .Select(d => (Label: Path.GetFileName(d), Path: Path.Combine(d, "skills"))));
            }
        }
        catch (Exception) { /* 内核不含 dsh-agent-presets（npm 版内核）时跳过 */ }
        presetRoot.Path = presetDirs.Count > 0
            ? string.Join("\n", presetDirs.Select(d => d.Path))
            : L("（本部署没有预设自带技能）");
        presetRoot.Entries.AddRange(presetDirs.SelectMany(d => ReadSkillDir(d.Path)).Select(s =>
        {
            s.Root = presetRoot;
            return s;
        }));
        roots.Add(presetRoot);

        // 400：用户根（新增技能的默认去处）
        roots.Add(MakeRoot(L("用户"), Path.Combine(dshHome, "skills"), 400));
        // 500：用户兼容根（~/.agents/skills，与内核默认 agentsHome 一致）
        var agentsHome = Environment.GetEnvironmentVariable("DSH_AGENTS_HOME");
        if (string.IsNullOrWhiteSpace(agentsHome))
        {
            agentsHome = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".agents");
        }
        roots.Add(MakeRoot(L("用户（兼容）"), Path.Combine(agentsHome, "skills"), 500));

        MarkShadowed(roots);
        return roots;
    }

    private static SkillRoot MakeRoot(string label, string path, int rank, bool writable = true)
        => new() { Label = label, Path = path, Rank = rank, Writable = writable, Available = true, UnavailableReason = "" };

    /// <summary>当前会话的工作目录（项目根与项目技能都按它解析）。</summary>
    private string? ActiveSessionCwd
    {
        get
        {
            var sid = _activeSessionId;
            if (string.IsNullOrWhiteSpace(sid))
            {
                return null;
            }
            var cwd = _sessions.FirstOrDefault(s => s.SessionId == sid)?.Cwd;
            return string.IsNullOrWhiteSpace(cwd) ? null : cwd;
        }
    }

    /// <summary>从 cwd 向上找最近的含 .git 的目录；找不到就退回 cwd（内核 findProjectRoot 同规则）。</summary>
    private static string FindProjectRoot(string cwd)
    {
        var current = cwd;
        while (true)
        {
            try
            {
                if (Directory.Exists(Path.Combine(current, ".git")))
                {
                    return current;
                }
            }
            catch (Exception) { /* 权限/路径异常：继续向上，最终退回 cwd */ }
            var parent = Path.GetDirectoryName(current.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar));
            if (string.IsNullOrEmpty(parent) || parent == current)
            {
                return cwd;
            }
            current = parent;
        }
    }

    /// <summary>同名遮蔽标记：低 rank 根里的同名条目内核不会加载。</summary>
    private static void MarkShadowed(List<SkillRoot> roots)
    {
        var seen = new HashSet<string>(StringComparer.Ordinal);
        foreach (var entry in roots.OrderBy(r => r.Rank).SelectMany(r => r.Entries))
        {
            entry.Shadowed = !seen.Add(entry.Name);
        }
    }

    /// <summary>读一个根目录下的全部技能：目录包 &lt;name&gt;/SKILL.md 与扁平 .md 都认。</summary>
    private static List<SkillEntry> ReadSkillDir(string root)
    {
        var entries = new List<SkillEntry>();
        if (root.Length == 0 || !Directory.Exists(root))
        {
            return entries;
        }
        foreach (var dir in SafeGetDirectories(root))
        {
            if (Path.GetFileName(dir) is ".system") continue; // 用户根跳过 .system（内核同规则）
            var file = Path.Combine(dir, "SKILL.md");
            if (File.Exists(file))
            {
                if (TryReadSkill(file, bundle: true) is { } skill)
                {
                    entries.Add(skill);
                }
            }
        }
        foreach (var file in SafeGetFiles(root, "*.md"))
        {
            if (TryReadSkill(file, bundle: false) is { } skill)
            {
                entries.Add(skill);
            }
        }
        return entries.OrderBy(e => e.Name, StringComparer.Ordinal).ToList();
    }

    private static IEnumerable<string> SafeGetDirectories(string root)
    {
        try { return Directory.GetDirectories(root); }
        catch (Exception) { return Array.Empty<string>(); }
    }

    private static IEnumerable<string> SafeGetFiles(string root, string pattern)
    {
        try { return Directory.GetFiles(root, pattern); }
        catch (Exception) { return Array.Empty<string>(); }
    }

    /// <summary>解析一个技能文件。frontmatter 缺失/必填字段缺失/名字非法 → 内核同样会忽略它，这里也跳过。</summary>
    private static SkillEntry? TryReadSkill(string file, bool bundle)
    {
        try
        {
            var text = File.ReadAllText(file);
            if (!SkillFrontmatter.Split(text, out var head, out _))
            {
                return null;
            }
            var name = Unquote(head.TryGetValue("name", out var n) ? n : "");
            var description = Unquote(head.TryGetValue("description", out var d) ? d : "");
            if (name.Length == 0 || description.Length == 0 || !SkillNamePattern.IsMatch(name))
            {
                return null;
            }
            return new SkillEntry
            {
                Name = name,
                Description = description,
                WhenToUse = Unquote(head.TryGetValue("whenToUse", out var w) ? w : ""),
                FilePath = file,
                IsBundle = bundle,
                UserInvocable = Flag(head, SkillUserInvocableKey, true),
                ModelInvocable = !Flag(head, SkillDisableModelInvocationKey, false),
                Root = null!,
            };
        }
        catch (Exception)
        {
            return null; // 读不了/不是 UTF-8：跳过该条，不影响其它技能
        }
    }

    /// <summary>frontmatter 布尔位：缺失即缺省值（与内核 frontmatterBoolean 的 default 语义一致）。</summary>
    private static bool Flag(Dictionary<string, string> head, string key, bool fallback)
        => head.TryGetValue(key, out var raw) && bool.TryParse(raw.Trim(), out var value) ? value : fallback;

    private static string Unquote(string value)
    {
        var v = value.Trim();
        if (v.Length >= 2 && ((v[0] == '"' && v[^1] == '"') || (v[0] == '\'' && v[^1] == '\'')))
        {
            return v[1..^1];
        }
        return v;
    }

    /// <summary>frontmatter 切分与键值读取：只认「--- / 键: 值 / ---」这一最小 YAML 子集（技能只用得这些）。</summary>
    private static class SkillFrontmatter
    {
        public static bool Split(string text, out Dictionary<string, string> head, out string body)
        {
            head = new Dictionary<string, string>(StringComparer.Ordinal);
            body = "";
            var lines = text.Split('\n');
            if (lines.Length == 0 || lines[0].TrimEnd('\r') != "---")
            {
                return false;
            }
            var closing = -1;
            for (var i = 1; i < lines.Length; i++)
            {
                if (lines[i].TrimEnd('\r') == "---")
                {
                    closing = i;
                    break;
                }
            }
            if (closing < 0)
            {
                return false;
            }
            for (var i = 1; i < closing; i++)
            {
                var line = lines[i].TrimEnd('\r');
                if (line.Length == 0 || line.TrimStart().StartsWith('#'))
                {
                    continue;
                }
                var colon = line.IndexOf(':');
                if (colon <= 0)
                {
                    continue;
                }
                head[line[..colon].Trim()] = line[(colon + 1)..].Trim();
            }
            body = string.Join("\n", lines[(closing + 1)..]).TrimStart('\r', '\n');
            return true;
        }
    }

    // ---------------- 增 / 删 / 开关 ----------------

    /// <summary>添加技能：写一个目录包到用户根或当前项目根。名称按内核 kebab-case 规则校验，
    /// 且不得与任何根里的既有技能同名（否则内核只会加载优先级高的那个，白写）。</summary>
    private async Task AddSkillAsync(List<SkillRoot> roots)
    {
        var userRoot = roots.First(r => r.Rank == 400);
        var projectRoot = roots.FirstOrDefault(r => r.Rank == 100);
        var targets = new List<SkillRoot> { userRoot };
        if (projectRoot is { Available: true })
        {
            targets.Add(projectRoot);
        }

        var nameBox = new TextBox { Header = L("名称"), PlaceholderText = "kebab-case，例如 pdf-report" };
        Aut(nameBox, "SkillNameBox", L("技能名称"));
        var descBox = new TextBox { Header = L("说明（必填）"), PlaceholderText = L("一句话说明这个技能做什么；调用时会显示它") };
        Aut(descBox, "SkillDescBox", L("技能说明"));
        var whenBox = new TextBox { Header = L("适用场景（可选）"), PlaceholderText = L("什么时候该用它，例如「生成周报时」") };
        Aut(whenBox, "SkillWhenBox", L("适用场景"));
        var bodyBox = new TextBox
        {
            Header = L("指令正文（可选）"),
            PlaceholderText = L("模型加载这个技能后读到的具体指令"),
            AcceptsReturn = true,
            TextWrapping = TextWrapping.Wrap,
            MinHeight = 120,
        };
        Aut(bodyBox, "SkillBodyBox", L("指令正文"));

        var targetBox = new ComboBox { Header = L("安装到"), HorizontalAlignment = HorizontalAlignment.Stretch };
        Aut(targetBox, "SkillTargetBox", L("安装到哪个根目录"));
        foreach (var (index, root) in targets.Select((r, i) => (i, r)))
        {
            targetBox.Items.Add(new ComboBoxItem { Content = LF("{0}（{1}）", root.Label, root.Path), Tag = index });
        }
        targetBox.SelectedIndex = 0;

        var error = new TextBlock { Foreground = ThemeBrush("InfoBrush"), TextWrapping = TextWrapping.Wrap };
        var form = new StackPanel { Spacing = Sp8, Width = 460 };
        form.Children.Add(nameBox);
        form.Children.Add(descBox);
        form.Children.Add(whenBox);
        form.Children.Add(bodyBox);
        form.Children.Add(targetBox);
        form.Children.Add(error);

        var dialog = new ContentDialog
        {
            Title = L("添加技能"),
            Content = new ScrollViewer { Content = form, MaxHeight = 560 },
            PrimaryButtonText = L("创建"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }

        var name = nameBox.Text.Trim();
        var description = descBox.Text.Trim();
        var failure = name.Length == 0 ? L("名称不能为空。")
            : !SkillNamePattern.IsMatch(name) ? L("名称只能用小写字母、数字和短横线（kebab-case）。")
            : description.Length == 0 ? L("说明不能为空：调用菜单里只显示名称和说明。")
            : roots.SelectMany(r => r.Entries).Any(e => e.Name == name)
                ? L("已有同名技能：内核只会加载优先级最高的那个，请先处理已有的。")
                : null;
        if (failure is not null)
        {
            error.Text = failure;
            await ShowErrorAsync(failure);
            return;
        }

        try
        {
            var root = targets[targetBox.SelectedIndex < 0 ? 0 : targetBox.SelectedIndex];
            Directory.CreateDirectory(root.Path);
            var file = Path.Combine(root.Path, name, "SKILL.md");
            Directory.CreateDirectory(Path.GetDirectoryName(file)!);
            var head = new StringBuilder();
            head.Append("---\n");
            head.Append("name: ").Append(YamlScalar(name)).Append('\n');
            head.Append("description: ").Append(YamlScalar(description)).Append('\n');
            if (whenBox.Text.Trim().Length > 0)
            {
                head.Append("whenToUse: ").Append(YamlScalar(whenBox.Text.Trim())).Append('\n');
            }
            head.Append("---\n\n");
            if (bodyBox.Text.Trim().Length > 0)
            {
                head.Append(bodyBox.Text.TrimEnd()).Append("\n");
            }
            File.WriteAllText(file, head.ToString(), new UTF8Encoding(false));
            await RenderSectionAsync(SkillsSectionId);
        }
        catch (Exception ex)
        {
            await ShowErrorAsync(LF("创建技能失败：{0}", ex.Message));
        }
    }

    /// <summary>删除技能：确认对话框里给出确切路径，删除文件或整个目录包。
    /// 只允许删可写根（项目/用户）里的条目；随包预设只读，不进这里。</summary>
    private async Task RemoveSkillAsync(SkillEntry skill, TextBlock status, List<SkillRoot> roots)
    {
        if (!skill.Writable)
        {
            await ShowErrorAsync(L("这个技能随预设分发，不能在这里删除。"));
            return;
        }
        var dialog = new ContentDialog
        {
            Title = LF("删除「{0}」？", skill.Name),
            Content = new StackPanel
            {
                Spacing = Sp8,
                Children =
                {
                    new TextBlock
                    {
                        Text = L("将删除这个技能文件，内核随即不再加载它。此操作不可撤销。"),
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new TextBlock
                    {
                        Text = skill.FilePath,
                        Style = AppStyle("CodeTextStyle"),
                        TextWrapping = TextWrapping.Wrap,
                        IsTextSelectionEnabled = true,
                    },
                },
            },
            PrimaryButtonText = L("删除"),
            CloseButtonText = L("取消"),
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Content.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return;
        }
        try
        {
            if (skill.IsBundle)
            {
                var dir = Path.GetDirectoryName(skill.FilePath);
                if (!string.IsNullOrEmpty(dir) && IsInsideRoot(dir, skill.Root.Path))
                {
                    Directory.Delete(dir, recursive: true);
                }
                else
                {
                    File.Delete(skill.FilePath);
                }
            }
            else
            {
                File.Delete(skill.FilePath);
            }
            await RenderSectionAsync(SkillsSectionId);
        }
        catch (Exception ex)
        {
            await ShowErrorAsync(LF("删除技能失败：{0}", ex.Message));
        }
    }

    /// <summary>开关技能：改写 frontmatter 的 user-invocable / disable-model-invocation 两个布尔，
    /// 其余行与正文原样保留（内核 watcher 热加载，无需重启）。</summary>
    private async Task SetSkillEnabledAsync(SkillEntry skill, bool enabled, TextBlock status, List<SkillRoot> roots)
    {
        if (!skill.Writable)
        {
            await ShowErrorAsync(L("这个技能随预设分发，不能在这里关闭。"));
            await RenderSectionAsync(SkillsSectionId);
            return;
        }
        try
        {
            var text = await File.ReadAllTextAsync(skill.FilePath);
            if (!SkillFrontmatter.Split(text, out var head, out var body))
            {
                throw new InvalidOperationException(L("这个文件没有 frontmatter，无法改写它的开关。"));
            }
            var newline = text.Contains("\r\n") ? "\r\n" : "\n";
            var lines = text.Split('\n');
            var rebuilt = new List<string> { "---" };
            foreach (var line in lines.Skip(1))
            {
                var trimmed = line.TrimEnd('\r');
                if (trimmed == "---")
                {
                    break;
                }
                var colon = trimmed.IndexOf(':');
                var key = colon > 0 ? trimmed[..colon].Trim() : "";
                if (key is SkillUserInvocableKey or SkillDisableModelInvocationKey)
                {
                    continue; // 两个开关统一在末尾按目标态重写
                }
                rebuilt.Add(trimmed);
            }
            rebuilt.Add($"{SkillUserInvocableKey}: {(enabled ? "false" : "true")}");
            rebuilt.Add($"{SkillDisableModelInvocationKey}: {(enabled ? "false" : "true")}");
            rebuilt.Add("---");
            var output = string.Join(newline, rebuilt) + newline + body.TrimStart('\r', '\n');
            await File.WriteAllTextAsync(skill.FilePath, output, new UTF8Encoding(false));
            status.Text = enabled
                ? LF("「{0}」已开启。", skill.Name)
                : LF("「{0}」已关闭：/ 菜单与模型都不会再看到它。", skill.Name);
            await RenderSectionAsync(SkillsSectionId);
        }
        catch (Exception ex)
        {
            await ShowErrorAsync(LF("切换技能开关失败：{0}", ex.Message));
            await RenderSectionAsync(SkillsSectionId);
        }
    }

    /// <summary>待删目录必须仍在根目录内：防符号链接/异常路径把删除范围扩到根外。</summary>
    private static bool IsInsideRoot(string path, string root)
    {
        if (root.Length == 0)
        {
            return false;
        }
        var full = Path.GetFullPath(path);
        var rootFull = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        return full.StartsWith(rootFull + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase)
            || full.Equals(rootFull, StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>YAML 标量：含特殊字符时用单引号包裹（内部单引号翻倍），保证 frontmatter 仍是合法 YAML。</summary>
    private static string YamlScalar(string value)
    {
        var needsQuote = value.Length == 0
            || value.StartsWith(' ') || value.EndsWith(' ')
            || value.StartsWith('"') || value.StartsWith('\'')
            || value.StartsWith('#') || value.StartsWith('&') || value.StartsWith('*')
            || value.StartsWith('!') || value.StartsWith('|') || value.StartsWith('>')
            || value.StartsWith('%') || value.StartsWith('@') || value.StartsWith('`')
            || value.Contains(": ") || value.EndsWith(':') || value.Contains(" #")
            || value.Contains('\n') || value.Contains('\t');
        return needsQuote ? "'" + value.Replace("'", "''") + "'" : value;
    }
}
