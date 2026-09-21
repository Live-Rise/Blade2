// ---------------- 图片放大浮层 ----------------
// 发送前与发送后的图片共用同一个放大层：
//   · 发送前：输入区待发送附件 chip 的 38dip 缩略图（AddPendingAttachment 里挂 Click，
//     点击后按全尺寸 DataBase64 现解码——chip 只解了 96dip 缩略，放大不能拿它充数）；
//   · 发送后：user-image 气泡里的位图（UserImageTpl 的透明 Button 挂 OnChatImageOpenClick，
//     直接复用气泡已解码的全尺寸 ImageSource，不二次解码）。
// 关闭三路：Esc（根层隧道键 OnRootPreviewKeyDown 优先让路给本层）、点背板空白处、右上关闭钮。
// 位图可能被气泡与浮层共享（发送后路径），关闭时只清浮层引用，不动 source 本身。

using System;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;

namespace Blade2;

public sealed partial class MainWindow
{
    /// <summary>打开放大浮层：全图 + 文件名贴底，关闭钮取焦点（Enter/Space 也能退）。
    /// 打开前记下当前焦点元素，关闭时还回去——键盘用户从缩略图/气泡图 Tab 进来，
    /// Esc 退出后焦点若丢了就得从头 Tab。</summary>
    private void ShowImageLightbox(ImageSource source, string title)
    {
        _lightboxReturnFocus = FocusManager.GetFocusedElement() as FrameworkElement;
        LightboxImage.Source = source;
        LightboxCaption.Text = title;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(LightboxImage, title);
        ImageLightboxLayer.Visibility = Visibility.Visible;
        // 此刻浮层刚 Visible、尚未过布局，ActualWidth 还是 0；真实尺寸由紧随其后的
        // SizeChanged 补算（OnImageLightboxSizeChanged）。这里只在已有尺寸时（二次打开）先算一版。
        ApplyLightboxSizeLimits();
        LightboxCloseButton.Focus(FocusState.Programmatic);
    }

    /// <summary>关闭浮层后要还回焦点的元素（打开时记住的）；已不在可视化树里（附件已移除、
    /// 气泡被虚拟化回收）就不还，Focus 对卸载元素本就是空操作。</summary>
    private FrameworkElement? _lightboxReturnFocus;

    private void HideImageLightbox()
    {
        if (ImageLightboxLayer.Visibility != Visibility.Visible)
        {
            return;
        }
        ImageLightboxLayer.Visibility = Visibility.Collapsed;
        // 只断浮层这一侧的引用：source 可能还挂在聊天气泡上（共享 BitmapImage）
        LightboxImage.Source = null;
        if (_lightboxReturnFocus is { IsLoaded: true } back)
        {
            back.Focus(FocusState.Programmatic);
        }
        _lightboxReturnFocus = null;
    }

    /// <summary>图片显示上限 = 浮层实际尺寸扣掉背板边距与底部文件名行。浮层铺满窗口
    /// （Grid.RowSpan=2），其 ActualWidth/ActualHeight 即窗口客户区尺寸；窗口缩放时由
    /// SizeChanged 重算，大图不会溢出到背板外。</summary>
    private void ApplyLightboxSizeLimits()
    {
        var w = ImageLightboxLayer.ActualWidth;
        var h = ImageLightboxLayer.ActualHeight;
        if (w <= 0 || h <= 0)
        {
            return;
        }
        LightboxImage.MaxWidth = Math.Max(160.0, w - 64);
        LightboxImage.MaxHeight = Math.Max(120.0, h - 144);
    }

    private void OnImageLightboxSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (ImageLightboxLayer.Visibility == Visibility.Visible)
        {
            ApplyLightboxSizeLimits();
        }
    }

    /// <summary>背板点击：点在图上不关（防误触），点背板空白处才退。
    /// 内层 Grid 无背景不吃命中，空白处的 OriginalSource 即浮层自身，均落到关闭分支。</summary>
    private void OnImageLightboxBackdropTapped(object sender, TappedRoutedEventArgs e)
    {
        if (IsInSubtree(LightboxImage, e.OriginalSource as DependencyObject))
        {
            return;
        }
        HideImageLightbox();
    }

    private void OnImageLightboxCloseClick(object sender, RoutedEventArgs e) => HideImageLightbox();

    /// <summary>user-image 气泡图片点击放大。DataContext 即当前气泡（ListView 复用容器时
    /// 随容器换成对应项）；Image 为空（历史字节取失败时的占位气泡）不弹。</summary>
    private void OnChatImageOpenClick(object sender, RoutedEventArgs e)
    {
        if (sender is Button { DataContext: ChatBubble bubble } && bubble.Image is { } source)
        {
            ShowImageLightbox(source, bubble.Text);
        }
    }
}
