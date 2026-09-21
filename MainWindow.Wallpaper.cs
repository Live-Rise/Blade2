using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Blade2.Dsh;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Blade2;

/// <summary>
/// Wallpaper Engine 壁纸源（个性化分区「背景皮肤」卡内的第三项）。
/// 插件 @baiiii/dsh-wallpaper-local 由 <see cref="Blade2.Dsh.DshPluginBootstrap"/>
/// 默认安装，服务端路由把本机 WE 创意工坊目录（steamapps/workshop/content/431960）
/// 的壁纸清单与文件吐出来；插件自带的 Web 前端只注入 dsh Web 界面，壳用不到，
/// 壳只把这些路由当又一个皮肤素材源（与「导入图片/导入视频」并列的第三种来源）。
///
/// 与单槽位皮肤模型的关系：套用某个 WE 壁纸 = 把它的文件下载进皮肤槽位文件
/// （视频进 shell-skin.video、图片进 shell-skin.img），再走既有的播放/显示管线。
/// 槽位语义因此完全不变——持久化、透明度滑杆、清除皮肤、与导入互斥都不用改。
/// 槽位旁边只多一个 shell-skin.we 记「当前背景来自哪个 WE 壁纸」，用于网格里
/// 标出「使用中」（纯展示，删了也不影响背景本身）。
///
/// 呈现形态：复用插件页的二级菜单（Windows 设置「应用」页钻取）——背景皮肤卡里放
/// 一张导航卡，点卡进二级页；二级页是缩略图网格（GridView 横向滚动、虚拟化加载），
/// 点哪个缩略图就套哪个。所有 tile 统一一张模板：图片项缩略图走 thumb 路由（插件
/// 回退吐原图，壳按 tile 宽解码），视频项直读本机工坊目录里的 preview.* 预览图
/// （插件 thumb 路由拿不到视频预览：thumbSlug 指向空的 THUMBS 目录，实测 404，详见
/// <see cref="WeWorkspace"/>）；没有预览图的项露出底层字形当占位。网格里不放手播
/// 元素——MediaPlayerElement 的 content island 在 GridView 模板里会抢指针输入
/// （实测：误路由到别的 tile、有播放时吞掉整个网格的指针事件、PointerExited 不触发），
/// 视频预览一律交给套用后的整屏背景（静音循环，失焦可配暂停）。
/// 清单只在钻进二级页时才拉，拉过一次壳侧缓存，再进页秒开；插件安装由引导器的
/// 目录存在性检查兜底（见 DshPluginBootstrap.RequiredPackages），启动路径零成本。
/// </summary>
public sealed partial class MainWindow
{
    private DshWallpaperClient? _weClient;
    private GridView? _weGrid;
    private TextBlock? _weWallpaperStatus;
    /// <summary>当前背景来自哪个 WE 壁纸（壳启动时从 sidecar 恢复；本地导入会清掉）。</summary>
    private string? _weAppliedId;
    /// <summary>清单缓存（拉过一次后二级页再进直接填，不再发请求）。</summary>
    private WallpaperList? _weList;
    private bool _weLoading;
    /// <summary>tile 模板懒加载：XamlReader.Load 只能在 UI 线程跑，static 字段初始化的线程不定。</summary>
    private DataTemplate? _weTileTemplate;

    /// <summary>当前背景的 WE 来源标记（sidecar，纯展示用）。</summary>
    private string SkinWeFile => Path.Combine(DataHome, "shell-skin.we");

    /// <summary>在背景皮肤卡里追加 WE 导航卡（插件页同款钻取卡：图标 + 标题/说明 + chevron）。</summary>
    internal void AddWallpaperEngineRow(StackPanel card)
    {
        card.Children.Add(MakeSettingsNavCard(
            "SkinWeNavCard", "\uEB9F",
            L("Wallpaper Engine"),
            L("用本机 Wallpaper Engine 订阅的壁纸做背景（视频静音循环），与上面导入的图片/视频二选一"),
            () => OpenSettingsSubPage(L("Wallpaper Engine"), AddWeWallpaperSubPage)));
    }

    /// <summary>WE 二级页内容（每次进页现建，同插件页各钻取页）：缩略图网格铺满。</summary>
    private void AddWeWallpaperSubPage(StackPanel host)
    {
        var card = NewCardIn(
            host,
            L("Wallpaper Engine"),
            L("点缩略图即套用；视频项显示工坊预览图，与上面导入的图片/视频二选一。"));

        _weGrid = new GridView
        {
            SelectionMode = ListViewSelectionMode.Single,
            IsItemClickEnabled = true,
            Padding = new Thickness(0, Sp8, 0, 0),
        };
        Aut(_weGrid, "SkinWeGrid", L("Wallpaper Engine 壁纸缩略图"));
        _weGrid.ItemTemplate = WeTileTemplate;
        // 选中框贴住缩略图：GridViewItem 默认 presenter 的 padding 会在内容与选中
        // 描边之间留一圈空白（缩略图越小越明显），归零后描边沿缩略图边缘走。
        _weGrid.ItemContainerStyle = new Style
        {
            TargetType = typeof(GridViewItem),
            Setters =
            {
                new Setter(Control.PaddingProperty, new Thickness(0)),
                new Setter(Control.MarginProperty, new Thickness(0)),
                new Setter(Control.HorizontalContentAlignmentProperty, HorizontalAlignment.Left),
                new Setter(Control.VerticalContentAlignmentProperty, VerticalAlignment.Top),
            },
        };
        _weGrid.ItemClick += (_, e) =>
        {
            if (e.ClickedItem is WeTile tile)
            {
                _ = ApplyWallpaperAsync(tile.Item);
            }
        };
        card.Children.Add(_weGrid);

        _weWallpaperStatus = new TextBlock
        {
            Style = AppStyle("CaptionTextStyle"),
            Foreground = ThemeBrush("TextSecondaryBrush"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, Sp8, 0, 0),
        };
        card.Children.Add(_weWallpaperStatus);

        _weAppliedId ??= ReadSkinWeId();
        if (_weList is { } cached)
        {
            FillWeGrid(cached); // 缓存命中：进页即完整网格
        }
        else
        {
            SetWeStatus(L("正在读取…"));
            _ = LoadWallpaperEngineLibraryAsync(); // 二级页「直接显示」：进页就拉，不等点击
        }
    }

    private void SetWeStatus(string text)
    {
        if (_weWallpaperStatus is not null)
        {
            _weWallpaperStatus.Text = text;
        }
    }

    /// <summary>
    /// 拉清单并填充网格。只在钻进二级页且无缓存时调用，不进启动路径。
    /// 失败/空列表都在状态行给原因，且不写缓存——下次进页自动重试。
    /// </summary>
    private async Task LoadWallpaperEngineLibraryAsync()
    {
        if (_weGrid is null || _weLoading)
        {
            return;
        }
        _weLoading = true;
        SetWeStatus(L("正在读取…"));
        _weClient ??= new DshWallpaperClient(() => _rpc);

        var list = await _weClient.GetListAsync();
        if (list is null)
        {
            SetWeStatus(_weClient.Status switch
            {
                WallpaperRouteStatus.Missing =>
                    L("未检测到壁纸插件。重启 Blade² 会自动重试安装；离线时这条会一直出现，连上网再试。"),
                _ => _weClient.LastError is { Length: > 0 } err
                    ? LF("读取失败：{0}", err)
                    : L("读取失败：壁纸服务没响应。本机需要装有 Wallpaper Engine 并订阅壁纸。"),
            });
            _weLoading = false;
            return;
        }
        if (list.Items.Count == 0)
        {
            SetWeStatus(L("本机没有可用的 Wallpaper Engine 壁纸：需要安装 Wallpaper Engine 并订阅壁纸。"));
            _weLoading = false;
            return;
        }

        _weList = list;
        FillWeGrid(list);
        _weLoading = false;
    }

    /// <summary>把清单包成缩略图 tile 填进网格，并回显当前背景对应的项。</summary>
    private void FillWeGrid(WallpaperList list)
    {
        if (_weGrid is null)
        {
            return;
        }
        _weGrid.ItemsSource = BuildWeTiles(list);
        var applied = _weGrid.Items.OfType<WeTile>()
            .FirstOrDefault(t => string.Equals(t.Item.Id, _weAppliedId, StringComparison.Ordinal));
        if (applied is not null)
        {
            _weGrid.SelectedItem = applied;
        }
        var status = LF("共 {0} 个壁纸，点缩略图套用", list.Items.Count);
        if (list.Excluded > 0)
        {
            status += LF("（已隐藏 {0} 个低分辨率预览图）", list.Excluded);
        }
        SetWeStatus(status);
    }

    private List<WeTile> BuildWeTiles(WallpaperList list)
    {
        var baseUri = _rpc?.BaseUri;
        return list.Items
            .Select(i =>
            {
                var applied = string.Equals(i.Id, _weAppliedId, StringComparison.Ordinal);
                return new WeTile(
                    i,
                    baseUri,
                    applied ? i.Name + " · " + L("使用中") : i.Name);
            })
            .ToList();
    }

    /// <summary>
    /// 套用 WE 壁纸：下载进皮肤槽位文件再走既有管线（视频=静音循环播放，图片=背景笔）。
    /// 先停正在播的视频再下载——播放器占着槽位文件时覆盖它会失败。
    /// </summary>
    private async Task ApplyWallpaperAsync(WallpaperItem item)
    {
        if (_weClient is null)
        {
            return;
        }
        var url = item.IsVideo
            ? (string.IsNullOrEmpty(item.VideoUrl) ? item.FullUrl : item.VideoUrl)
            : item.FullUrl;
        if (string.IsNullOrEmpty(url))
        {
            SetWeStatus(L("这个壁纸没有可用的文件。"));
            return;
        }
        var dest = item.IsVideo ? SkinVideoFile : SkinFile;
        if (string.Equals(item.Id, _weAppliedId, StringComparison.Ordinal) && File.Exists(dest))
        {
            // 点的就是当前背景：槽位文件还在，直接回显，不重下（视频项动辄上百 MB）
            SetWeStatus(LF("已应用「{0}」", item.Name));
            return;
        }

        SetWeStatus(LF("正在下载「{0}」…", item.Name));
        StopSkinVideo(); // 播放器持文件：先停再覆盖，否则移动临时文件会失败
        var ok = await _weClient.DownloadToFileAsync(url, dest);
        if (!ok)
        {
            SetWeStatus(L("下载失败：壁纸服务没返回这个文件。"));
            return;
        }

        if (item.IsVideo)
        {
            DeleteSkinQuietly(SkinFile); // 单槽位：图片让位
            ApplySkinVideoFromPath(dest);
        }
        else
        {
            DeleteSkinQuietly(SkinVideoFile);
            ApplySkinFromPath(dest);
        }
        WriteSkinWeId(item.Id);
        _weAppliedId = item.Id;
        if (_weList is { } list)
        {
            // 重建网格：tile 名称带上「使用中」、选中框移到刚套用的项
            _weGrid!.ItemsSource = BuildWeTiles(list);
            _weGrid.SelectedItem = _weGrid.Items.OfType<WeTile>()
                .FirstOrDefault(t => string.Equals(t.Item.Id, item.Id, StringComparison.Ordinal));
        }
        SetWeStatus(LF("已应用「{0}」", item.Name));
    }

    // ---- 缩略图 tile（DataTemplate 由 XamlReader 构建，图片/视频共用一张） ----

    /// <summary>
    /// 网格 tile 视图模型：图片项缩略图走 thumb 路由，视频项直读本机工坊 preview.*
    /// （插件 thumb 路由供不出视频预览，见 <see cref="WeWorkspace"/>）；都没有时
    /// 缩略图为 null，模板底层字形兜底。
    /// </summary>
    private sealed class WeTile
    {
        public WeTile(WallpaperItem item, Uri? baseUri, string label)
        {
            Item = item;
            Label = label;
            if (item.IsVideo)
            {
                var local = WeWorkspace.PreviewFileFor(item.Id);
                if (local is not null)
                {
                    ThumbSource = new BitmapImage(new Uri(local)) { DecodePixelWidth = 240 };
                }
            }
            else if (!string.IsNullOrEmpty(item.ThumbUrl) && baseUri is not null)
            {
                // thumb 路由吐的是原图（本机实测 1-4.5MB 的 png），按 tile 宽解码，别整图进内存
                ThumbSource = new BitmapImage(new Uri(baseUri, item.ThumbUrl.TrimStart('/')))
                {
                    DecodePixelWidth = 240,
                };
            }
        }

        public WallpaperItem Item { get; }
        public bool IsVideo => Item.IsVideo;
        public ImageSource? ThumbSource { get; }
        public string Label { get; }

        /// <summary>GridViewItem 的 UIA 名字取自数据项 ToString，别让读屏软件念类名。</summary>
        public override string ToString() => Label;
    }

    /// <summary>
    /// tile 模板懒加载：XamlReader.Load 只能在 UI 线程跑，static 字段初始化的线程不定，
    /// 改为首次进二级页（必在 UI 线程）时构建并缓存。
    /// </summary>
    private DataTemplate WeTileTemplate => _weTileTemplate ??= BuildWeTileTemplate();

    /// <summary>
    /// 构建 tile 模板：一张模板通吃图片/视频项——Image 绑缩略图源，底层字形兜底
    /// （无预览图的项露出来）。标题两行截断：工坊标题普遍长（「鸣潮 XX 独享版
    /// 4K120hz」这种），单行砍半不如换行显示完整；网格里不放 MediaPlayerElement，
    /// 它的 content island 会抢指针输入，视频预览交给套用后的整屏背景。
    /// </summary>
    private static DataTemplate BuildWeTileTemplate()
    {
        return (DataTemplate)Microsoft.UI.Xaml.Markup.XamlReader.Load("""
            <DataTemplate xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation">
              <StackPanel Width="240" Margin="0,0,12,12">
                <Grid Height="150">
                  <FontIcon Glyph="&#xE768;" FontSize="30" Opacity="0.55"
                            HorizontalAlignment="Center" VerticalAlignment="Center"
                            Foreground="{ThemeResource TextFillColorSecondaryBrush}"/>
                  <Image Stretch="UniformToFill" Source="{Binding ThumbSource}"/>
                </Grid>
                <TextBlock Text="{Binding Label}" FontSize="12" TextWrapping="Wrap" MaxLines="2"
                           TextTrimming="CharacterEllipsis" Margin="0,6,0,0"
                           Foreground="{ThemeResource TextFillColorSecondaryBrush}"/>
              </StackPanel>
            </DataTemplate>
            """);
    }

    private void DeleteSkinQuietly(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch (Exception) { } // 删不掉不影响新背景生效（启动时视频优先、图片次之）
    }

    private string? ReadSkinWeId()
    {
        try
        {
            var text = File.ReadAllText(SkinWeFile).Trim();
            return text.Length > 0 ? text : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private void WriteSkinWeId(string id)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SkinWeFile)!);
            File.WriteAllText(SkinWeFile, id);
        }
        catch (Exception) { } // 标记写不了只是网格里不标「使用中」，背景本身已生效
    }

    /// <summary>当前背景不再是 WE 来源时清标记（清除皮肤、本地导入图片/视频）。</summary>
    private void ClearSkinWeId()
    {
        DeleteSkinQuietly(SkinWeFile);
        _weAppliedId = null;
    }
}
