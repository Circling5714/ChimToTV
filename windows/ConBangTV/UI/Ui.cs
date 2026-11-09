using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Input;
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

    /// <summary>
    /// Hàng vuốt ngang của trang chủ tổng hợp: tiêu đề (+ «Xem thêm ›» bên phải cho chuột), dải cuộn ngang các thẻ (phím ◄ ►
    /// cuộn theo ô đang chọn; chuột: nút ‹ › hiện khi rê chuột, Shift + lăn chuột). title: tiêu đề (nhấn phải để sửa nhanh hàng).
    /// </summary>
    public static StackPanel Strip(TextBlock title, IEnumerable<UIElement> cards, Action? more = null)
    {
        var p = new StackPanel { Margin = new Thickness(0, 0, 0, 2) };
        var head = new DockPanel { Margin = new Thickness(0, 6, 0, 10), LastChildFill = true };
        if (more != null)
        {
            var link = new Button
            {
                Content = "Xem thêm ›", Focusable = false, Background = Brushes.Transparent, Foreground = Res("Brand"),
                Padding = new Thickness(8, 2, 4, 2), Margin = new Thickness(12, 0, 0, 0), FontSize = 15, FontWeight = FontWeights.SemiBold,
                VerticalAlignment = VerticalAlignment.Bottom,
            };
            link.Click += (_, _) => more();
            DockPanel.SetDock(link, Dock.Right);
            head.Children.Add(link); // chỉ để bấm chuột — phím đã có ô «Xem thêm» cuối dải
        }
        head.Children.Add(title);
        p.Children.Add(head);

        var row = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(10, 10, 10, 0) };
        foreach (var c in cards) row.Children.Add(c);
        // Lề âm: thẻ phóng to khi chọn không bị cắt ở mép dải mà vẫn thẳng hàng với tiêu đề
        var sv = new StripScroller
        {
            Content = row, HorizontalScrollBarVisibility = ScrollBarVisibility.Hidden, VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
            Focusable = false, PanningMode = PanningMode.HorizontalOnly, Margin = new Thickness(-10, -10, -10, 0),
        };
        Button Arrow(string text, HorizontalAlignment side, int dir)
        {
            var b = new Button
            {
                Content = text, Focusable = false, Width = 46, Height = 72, FontSize = 30, Padding = new Thickness(0, -6, 0, 0),
                HorizontalAlignment = side, VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 100, 0, 0),
                Background = new SolidColorBrush(Color.FromArgb(0xE0, 0x17, 0x1A, 0x21)), Visibility = Visibility.Collapsed,
            };
            b.Click += (_, _) => sv.ScrollToHorizontalOffset(sv.HorizontalOffset + dir * Math.Max(200, sv.ViewportWidth * 0.8));
            return b;
        }
        var left = Arrow("‹", HorizontalAlignment.Left, -1);
        var right = Arrow("›", HorizontalAlignment.Right, 1);
        var box = new Grid();
        box.Children.Add(sv);
        box.Children.Add(left);
        box.Children.Add(right);
        void Arrows()
        {
            var hover = box.IsMouseOver;
            left.Visibility = hover && sv.HorizontalOffset > 1 ? Visibility.Visible : Visibility.Collapsed;
            right.Visibility = hover && sv.HorizontalOffset < sv.ScrollableWidth - 1 ? Visibility.Visible : Visibility.Collapsed;
        }
        box.MouseEnter += (_, _) => Arrows();
        box.MouseLeave += (_, _) => Arrows();
        sv.ScrollChanged += (_, _) => Arrows();
        p.Children.Add(box);
        return p;
    }

    /// Ô «Xem thêm» hẹp cuối dải vuốt ngang (cao bằng poster); count: số phim (null = không ghi).
    public static Button MoreTile(Action onClick, string? count)
    {
        var text = new TextBlock { TextAlignment = TextAlignment.Center, FontSize = 15, FontWeight = FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap };
        text.Inlines.Add(new Run("›") { FontSize = 34 });
        text.Inlines.Add(new LineBreak());
        text.Inlines.Add(new Run("Xem thêm"));
        if (count != null)
        {
            text.Inlines.Add(new LineBreak());
            text.Inlines.Add(new Run(count) { FontSize = 13, FontWeight = FontWeights.Normal });
        }
        var b = new Button
        {
            Content = text, Width = 112, Height = 248, Padding = new Thickness(6), Margin = new Thickness(0, 0, 18, 22),
            VerticalAlignment = VerticalAlignment.Top,
        };
        b.Click += (_, _) => onClick();
        return b;
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

/// Dải vuốt ngang (Ui.Strip): lăn chuột dọc để trang cuộn (ScrollViewer mặc định nuốt mất), Shift + lăn chuột cuộn dải. KeyNav
/// nhận ra dải để ▲ ▼ chỉ chọn ô đang thấy.
public sealed class StripScroller : ScrollViewer
{
    protected override void OnMouseWheel(MouseWheelEventArgs e)
    {
        if ((Keyboard.Modifiers & ModifierKeys.Shift) == 0) return;
        ScrollToHorizontalOffset(HorizontalOffset - e.Delta);
        e.Handled = true;
    }
}
