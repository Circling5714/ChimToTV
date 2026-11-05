using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Dựng nhanh các phần giao diện dùng chung (chữ, nút, thẻ phim, lưới phim).
public static class Ui
{
    public static Brush Res(string key) => (Brush)Application.Current.Resources[key];

    public static TextBlock Text(string text, double size = 15, Brush? color = null, FontWeight? weight = null, bool wrap = true,
        Thickness? margin = null)
    {
        return new TextBlock
        {
            Text = text,
            FontSize = size,
            Foreground = color ?? Res("TextBrush"),
            FontWeight = weight ?? FontWeights.Normal,
            TextWrapping = wrap ? TextWrapping.Wrap : TextWrapping.NoWrap,
            TextTrimming = wrap ? TextTrimming.None : TextTrimming.CharacterEllipsis,
            Margin = margin ?? new Thickness(0),
        };
    }

    public static TextBlock Dim(string text, double size = 14, Thickness? margin = null) => Text(text, size, Res("TextDim"), margin: margin);

    public static TextBlock Header(string text, Thickness? margin = null) =>
        Text(text, 21, weight: FontWeights.SemiBold, wrap: false, margin: margin ?? new Thickness(0, 10, 0, 14));

    public static Button Pill(string text, Action onClick, string? tag = null, double? minWidth = null)
    {
        var b = new Button { Content = text, Tag = tag };
        if (minWidth != null) b.MinWidth = minWidth.Value;
        b.Click += (_, _) => onClick();
        return b;
    }

    public static WrapPanel Row(params UIElement[] items)
    {
        var p = new WrapPanel { Orientation = Orientation.Horizontal };
        foreach (var i in items) p.Children.Add(i);
        return p;
    }

    /// Thẻ phim. sub: dòng phụ dưới tên (vd tập đang xem); fresh: số tập mới (góc phải); progress: phần đã xem (0..1).
    public static Button Card(Film f, Action onClick, string? sub = null, string? fresh = null, double progress = -1)
    {
        var b = new Button { Style = (Style)Application.Current.Resources["Card"], DataContext = f, ToolTip = f.Title };
        b.Click += (_, _) => onClick();
        b.Loaded += (_, _) =>
        {
            b.ApplyTemplate();
            if (b.Template.FindName("Title", b) is TextBlock t) t.Text = f.Title;
            if (b.Template.FindName("Badge", b) is TextBlock badge)
            {
                badge.Text = f.Badge ?? "";
                if (b.Template.FindName("BadgeBox", b) is Border bb) bb.Visibility = string.IsNullOrEmpty(f.Badge) ? Visibility.Collapsed : Visibility.Visible;
            }
            if (sub != null && b.Template.FindName("Sub", b) is TextBlock s)
            {
                s.Text = sub;
                s.Visibility = Visibility.Visible;
            }
            if (fresh != null && b.Template.FindName("NewText", b) is TextBlock nt && b.Template.FindName("NewBox", b) is Border nb)
            {
                nt.Text = fresh;
                nb.Visibility = Visibility.Visible;
            }
            if (progress >= 0 && b.Template.FindName("ProgressBack", b) is Border pb && b.Template.FindName("ProgressBar", b) is Border bar)
            {
                pb.Visibility = Visibility.Visible;
                bar.Width = 162 * Math.Clamp(progress, 0.03, 1);
            }
            if (b.Template.FindName("Poster", b) is Image img && ImageLoader.GetUrl(img) != f.Poster) ImageLoader.SetUrl(img, f.Poster);
        };
        return b;
    }

    /// Một nhóm phim: tiêu đề + lưới thẻ (tự xuống hàng).
    public static StackPanel Section(string title, IEnumerable<UIElement> cards, string? note = null)
    {
        var p = new StackPanel { Margin = new Thickness(0, 0, 0, 12) };
        var head = new DockPanel { Margin = new Thickness(0, 6, 0, 14) };
        head.Children.Add(Text(title, 21, weight: FontWeights.SemiBold, wrap: false));
        if (note != null)
        {
            var n = Dim("  " + note, 14);
            n.VerticalAlignment = VerticalAlignment.Bottom;
            n.Margin = new Thickness(8, 0, 0, 3);
            head.Children.Add(n);
        }
        p.Children.Add(head);
        var grid = new WrapPanel();
        foreach (var c in cards) grid.Children.Add(c);
        p.Children.Add(grid);
        return p;
    }

    public static ScrollViewer Scroll(UIElement content) => new()
    {
        Content = content,
        VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
        Focusable = false,
        PanningMode = PanningMode.VerticalOnly,
    };

    public static void OpenBrowser(string url)
    {
        try
        {
            Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
        }
        catch (Exception e)
        {
            Log.D("ui", "Mở trình duyệt lỗi: " + e.Message);
        }
    }

    /// "12:30 / 23:00" → phần đã xem.
    public static double Progress(int pos, double duration) => duration > 0 ? pos / duration : -1;
}
