using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Foundation;

namespace Blade2.Dsh;

/// <summary>
/// 壳内自绘的宠物精灵：把插件图集（8 列 × 9/11 行、192×208 单元格）
/// 按 <see cref="PetDefinition.Tracks"/> 的时长逐帧播放。
///
/// 渲染走「视口裁剪」：整幅图集只解一次（一张 <see cref="BitmapImage"/>），
/// 放进一个比单元格大的 Image，再用负 Margin 把当前帧移进尺寸恰为一格的
/// 视口 Grid，视口的 Clip 把其余部分裁掉。每帧只改一次 Margin，不复制像素、
/// 不新建位图——WinUI 3 没有 UWP 的 CroppedBitmap，这条路径替代它。
///
/// 降级阶梯（构造时注入的图集字节解不动时逐级下沉）：
/// ① 图集解码成功（WebP 需系统 WebP 映像扩展，缺失时这一步失败）；
/// ② 预览 GIF（Image 原生支持动图，白得一段动画）；
/// ③ 都不行 → 隐藏精灵，由设置页诊断卡片说明原因。
/// </summary>
public sealed class DshPetSprite : IDisposable
{
    /// <summary>9 态动画的行序（registry.ts 的 PET_ROW_ORDER，v2 图集的第 10/11 行是观察向，不进轨道）。</summary>
    private static readonly string[] RowOrder =
    {
        "idle", "running-right", "running-left", "waving", "jumping",
        "failed", "waiting", "running", "review",
    };

    private enum Mode { None, Atlas, Gif }

    private readonly Grid _viewport;
    private readonly Image _target;
    private readonly DispatcherTimer _timer = new();
    private BitmapImage? _atlas;
    private Mode _mode = Mode.None;

    private PetDefinition? _pet;
    private string _track = "idle";
    private int _frameIndex;
    /// <summary>显示尺寸（单元格高度 px）。插件没报过值时用默认值。</summary>
    private int _displayCellHeight = 128;
    private bool _disposed;

    public DshPetSprite(Grid viewport, Image target)
    {
        _viewport = viewport ?? throw new ArgumentNullException(nameof(viewport));
        _target = target ?? throw new ArgumentNullException(nameof(target));
        _timer.Tick += OnTick;
    }

    /// <summary>当前轨道名（= 插件状态视图的 animation）。</summary>
    public string Track => _track;

    /// <summary>图集帧已换页：真实显示面（逐像素透明的分层窗口）要跟着补一帧像素。</summary>
    public event Action? FrameApplied;

    /// <summary>已解码图集（可渲染）。</summary>
    public bool Ready => _atlas is not null && _pet is not null;

    /// <summary>图集解不了时的兜底：预览 GIF 直接交给 Image（自带动图播放）。</summary>
    public async Task<bool> TryShowGifAsync(byte[] gifBytes)
    {
        if (gifBytes is not { Length: > 0 } || _disposed) return false;
        BitmapImage bitmap;
        try
        {
            bitmap = await DecodeAsync(gifBytes);
        }
        catch (Exception)
        {
            return false;
        }
        if (bitmap.PixelWidth <= 0 || bitmap.PixelHeight <= 0) return false;
        Stop();
        _mode = Mode.Gif;
        _atlas = null;
        _pet = null;
        _target.Stretch = Stretch.Uniform;
        _target.Margin = new Thickness(0);
        _target.Source = bitmap;
        ApplyViewportSize(bitmap.PixelWidth, bitmap.PixelHeight);
        return true;
    }

    /// <summary>
    /// 装载一只宠物：解码图集并记住轨道定义。返回 false = 解不动
    /// （调用方应尝试 GIF 降级）。必须在 UI 线程调用。
    /// </summary>
    public async Task<bool> LoadAsync(PetDefinition pet, byte[] atlasBytes)
    {
        if (_disposed || atlasBytes is not { Length: > 0 }) return false;
        Stop();
        _mode = Mode.None;
        // 摘掉 Image 上的旧位图即可，绝不能用同步 SetSource(default) 收空流：换宠物时
        // _atlas 必然非空（首载才是 null），而同步 SetSource 会让这个方法再也回不来，
        // 轮询随之永远卡在这一步。
        _target.Source = null;
        _atlas = null;
        _pet = null;

        BitmapImage bitmap;
        try
        {
            bitmap = await DecodeAsync(atlasBytes);
        }
        catch (Exception)
        {
            // WebP 映像扩展缺失（0x88965054 / WINCODEC_ERR_COMPONENTNOTFOUND）也落在这里
            return false;
        }
        if (bitmap.PixelWidth <= 0 || bitmap.PixelHeight <= 0) return false;

        _atlas = bitmap;
        _pet = pet;
        _mode = Mode.Atlas;
        _target.Source = bitmap;
        // 必须是 stretch 到显式宽高，不能是 None：None 按位图原始像素尺寸画，
        // 而显式宽高是「图集 × scale」。 Elements 的实际绘制尺寸就成了原始 1536x1872
        // 而不是 1181x1440，一格在图面上的间距是 192 而 Margin 每帧只挪 147.7，
        // 视窗于是每帧只走过 77% 格宽——看着就是帧序列持续向右平移。
        _target.Stretch = Stretch.Fill;
        _frameIndex = 0;
        _track = "idle";
        ApplyViewportSize(bitmap.PixelWidth, bitmap.PixelHeight);
        ApplyFrame(0);
        return true;
    }

    /// <summary>按插件状态切轨道；同一轨道重复设置时空转（轮询每秒来一次）。</summary>
    public void SetTrack(string animation)
    {
        if (_disposed || _pet is null || _atlas is null || _mode != Mode.Atlas) return;
        var next = _pet.Tracks.ContainsKey(animation) ? animation : "idle";
        if (next == _track && _timer.IsEnabled) return;
        _track = next;
        _frameIndex = 0;
        ApplyFrame(0);
        ScheduleNext();
    }

    /// <summary>精灵显示尺寸（单元格等比缩放：size 是单元格高度 px）。</summary>
    public void ApplySize(int cellHeight)
    {
        if (cellHeight <= 0) return;
        _displayCellHeight = cellHeight;
        switch (_mode)
        {
            case Mode.Atlas:
                if (_atlas is not null) ApplyViewportSize(_atlas.PixelWidth, _atlas.PixelHeight);
                ApplyFrame(_frameIndex);
                break;
            case Mode.Gif:
                if (_target.Source is BitmapImage { PixelWidth: > 0, PixelHeight: > 0 } gif)
                    ApplyViewportSize(gif.PixelWidth, gif.PixelHeight);
                break;
        }
    }

    /// <summary>停掉帧定时器（切宠物/隐藏/GIF 降级时）。</summary>
    public void Stop()
    {
        _timer.Stop();
        _frameIndex = 0;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _timer.Stop();
        _timer.Tick -= OnTick;
        _atlas = null;
        _pet = null;
    }

    // ---------------- 内部 ----------------

    private static async Task<BitmapImage> DecodeAsync(byte[] bytes)
    {
        var bitmap = new BitmapImage();
        using var stream = new Windows.Storage.Streams.InMemoryRandomAccessStream();
        using (var writer = new Windows.Storage.Streams.DataWriter(stream))
        {
            writer.WriteBytes(bytes);
            await writer.StoreAsync();
            await writer.FlushAsync();
            writer.DetachStream();
        }
        stream.Seek(0);
        await bitmap.SetSourceAsync(stream);
        return bitmap;
    }

    /// <summary>当前缩放系数（显示单元格高度 / 宠物单元格高度）。</summary>
    private double Scale => _pet is { CellHeight: > 0 } pet
        ? _displayCellHeight / (double)pet.CellHeight
        : 1;

    /// <summary>
    /// 视口尺寸 + 裁剪：视口恒为一格（cell × scale），Image 是整幅图集
    /// （atlas × scale），靠负 Margin 把当前帧挪进视口，Clip 裁掉溢出部分。
    /// </summary>
    private void ApplyViewportSize(int atlasPixelWidth, int atlasPixelHeight)
    {
        var scale = Scale;
        Debug.WriteLine($"[pet] ApplyViewportSize: atlas={atlasPixelWidth}x{atlasPixelHeight} cellH={_displayCellHeight} scale={scale:F4} mode={_mode} petCell={(_pet is null ? 0 : _pet.CellWidth)}x{(_pet is null ? 0 : _pet.CellHeight)}");
        if (_mode == Mode.Atlas && _pet is not null)
        {
            _viewport.Width = Math.Max(8, _pet.CellWidth * scale);
            _viewport.Height = Math.Max(8, _pet.CellHeight * scale);
            _target.Width = Math.Max(8, atlasPixelWidth * scale);
            _target.Height = Math.Max(8, atlasPixelHeight * scale);
            _viewport.Clip = new RectangleGeometry
            {
                Rect = new Rect(0, 0, _viewport.Width, _viewport.Height),
            };
            Debug.WriteLine($"[pet] ApplyViewportSize: viewport={_viewport.Width:F1}x{_viewport.Height:F1} image={_target.Width:F1}x{_target.Height:F1}");
            return;
        }
        // GIF：整图就是动画，视口按显示高度等比收
        var gifScale = atlasPixelHeight > 0 ? _displayCellHeight / (double)atlasPixelHeight : 1;
        _viewport.Width = Math.Max(8, atlasPixelWidth * gifScale);
        _viewport.Height = Math.Max(8, atlasPixelHeight * gifScale);
        _viewport.Clip = new RectangleGeometry
        {
            Rect = new Rect(0, 0, _viewport.Width, _viewport.Height),
        };
    }

    /// <summary>把第 index 帧所在的单元格移进视口（改 Margin，不动位图）。</summary>
    private void ApplyFrame(int index)
    {
        if (_mode != Mode.Atlas || _pet is null || _atlas is null) return;
        if (!_pet.Tracks.TryGetValue(_track, out var track)) return;
        var frames = track.Frames;
        if (frames.Count == 0) return;
        var col = frames[Math.Clamp(index, 0, frames.Count - 1)];
        var row = Array.IndexOf(RowOrder, _track);
        // 越界的帧（图集行列不够）停在第 0 帧，至少有个静止形象
        var maxRows = _atlas.PixelHeight / Math.Max(1, _pet.CellHeight);
        var maxCols = _atlas.PixelWidth / Math.Max(1, _pet.CellWidth);
        if (row < 0 || row >= maxRows || col < 0 || col >= maxCols) row = col = 0;
        var scale = Scale;
        _target.Margin = new Thickness(
            -col * _pet.CellWidth * scale, -row * _pet.CellHeight * scale, 0, 0);
        FrameApplied?.Invoke();
    }

    private int TrackDurationMs(int index)
    {
        if (_pet is null || !_pet.Tracks.TryGetValue(_track, out var track)) return 500;
        if (track.DurationsMs.Count == 0) return 500;
        return track.DurationsMs[Math.Min(index, track.DurationsMs.Count - 1)];
    }

    private void ScheduleNext()
    {
        if (_disposed || _pet is null) return;
        var ms = TrackDurationMs(_frameIndex);
        _timer.Interval = TimeSpan.FromMilliseconds(Math.Max(16, ms));
        _timer.Start();
    }

    private void OnTick(object? sender, object e)
    {
        if (_disposed || _pet is null || !_pet.Tracks.TryGetValue(_track, out var track))
        {
            _timer.Stop();
            return;
        }
        if (track.Frames.Count == 0)
        {
            _timer.Stop();
            return;
        }
        var next = _frameIndex + 1;
        if (next >= track.Frames.Count)
        {
            if (track.Loop)
            {
                next = 0;
            }
            else
            {
                // 非循环轨道（jumping/failed）停尾帧后回退——插件默认回 idle
                var fallback = track.Fallback is { } fb && _pet.Tracks.ContainsKey(fb) ? fb : "idle";
                if (fallback != _track)
                {
                    SetTrack(fallback);
                    return;
                }
                next = track.Frames.Count - 1;
            }
        }
        _frameIndex = next;
        ApplyFrame(next);
        ScheduleNext();
    }
}
