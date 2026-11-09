using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;

namespace ConBangTV.UI;

/// Đi bằng phím mũi tên / remote theo vị trí trên màn hình (như bản TV): từ mục đang chọn, tìm mục gần nhất theo hướng
/// bấm (cách tính của FocusFinder trên Android: khoảng cách chính × 13 + lệch ngang / dọc).
public static class KeyNav
{
    public enum Dir { Left, Right, Up, Down }

    public static Dir? DirOf(Key k) => k switch
    {
        Key.Left => Dir.Left,
        Key.Right => Dir.Right,
        Key.Up => Dir.Up,
        Key.Down => Dir.Down,
        _ => null,
    };

    /// Các mục chọn được trong root (nút, ô chữ), đang hiện và bật.
    public static List<FrameworkElement> Targets(DependencyObject root)
    {
        var list = new List<FrameworkElement>();
        void Walk(DependencyObject d)
        {
            if (d is UIElement { IsVisible: false }) return;
            if (d is FrameworkElement fe && IsTarget(fe))
            {
                list.Add(fe);
                return; // không đi vào trong nút
            }
            var n = VisualTreeHelper.GetChildrenCount(d);
            for (var i = 0; i < n; i++) Walk(VisualTreeHelper.GetChild(d, i));
        }
        Walk(root);
        return list;
    }

    private static bool IsTarget(FrameworkElement e) =>
        e is ButtonBase or TextBox or PasswordBox && e.Focusable && e.IsEnabled && e.IsVisible && e.ActualWidth > 0;

    public static Rect Bounds(FrameworkElement e, Visual root)
    {
        try
        {
            return e.TransformToAncestor(root).TransformBounds(new Rect(0, 0, e.ActualWidth, e.ActualHeight));
        }
        catch (InvalidOperationException)
        {
            return Rect.Empty;
        }
    }

    /// Dải vuốt ngang chứa e (null: không nằm trong dải).
    private static StripScroller? StripOf(DependencyObject e)
    {
        for (var d = VisualTreeHelper.GetParent(e); d != null; d = VisualTreeHelper.GetParent(d))
            if (d is StripScroller s) return s;
        return null;
    }

    /// Ô đang thấy (cả một phần) trong dải.
    private static bool InView(FrameworkElement t, StripScroller s)
    {
        var r = Bounds(t, s);
        return !r.IsEmpty && r.Right > 12 && r.Left < s.ViewportWidth - 12;
    }

    /// Mục kế tiếp theo hướng (null = không còn mục nào theo hướng đó). Ô của dải vuốt ngang đang khuất chỉ chọn được bằng
    /// ◄ ► trong chính dải đó (▲ ▼ chọn ô đang thấy của dải kia).
    public static FrameworkElement? Next(Visual root, FrameworkElement current, Dir dir)
    {
        var cur = Bounds(current, root);
        if (cur.IsEmpty) return null;
        var curStrip = StripOf(current);
        FrameworkElement? best = null, bestRow = null;
        double bestScore = double.MaxValue, bestRowScore = double.MaxValue;
        foreach (var t in Targets(root))
        {
            if (ReferenceEquals(t, current)) continue;
            var ts = StripOf(t);
            // ◄ ► trong dải: chỉ đi trong chính dải (cuối dải thì đứng yên; ◄ ở đầu dải ra được menu bên trái — ngoài dải)
            if (curStrip != null && dir is Dir.Left or Dir.Right && ts != null && !ReferenceEquals(ts, curStrip)) continue;
            if (ts != null && !(dir is Dir.Left or Dir.Right && ReferenceEquals(ts, curStrip)) && !InView(t, ts)) continue;
            var r = Bounds(t, root);
            if (r.IsEmpty) continue;
            double major, minor;
            double cx = (cur.Left + cur.Right) / 2, cy = (cur.Top + cur.Bottom) / 2;
            double rx = (r.Left + r.Right) / 2, ry = (r.Top + r.Bottom) / 2;
            switch (dir)
            {
                case Dir.Down:
                    if (ry <= cy + 2 || r.Bottom <= cur.Bottom) continue;
                    major = Math.Max(0, r.Top - cur.Bottom);
                    minor = Gap(cur.Left, cur.Right, r.Left, r.Right);
                    break;
                case Dir.Up:
                    if (ry >= cy - 2 || r.Top >= cur.Top) continue;
                    major = Math.Max(0, cur.Top - r.Bottom);
                    minor = Gap(cur.Left, cur.Right, r.Left, r.Right);
                    break;
                case Dir.Right:
                    if (rx <= cx + 2 || r.Right <= cur.Right) continue;
                    major = Math.Max(0, r.Left - cur.Right);
                    minor = Gap(cur.Top, cur.Bottom, r.Top, r.Bottom);
                    break;
                default:
                    if (rx >= cx - 2 || r.Left >= cur.Left) continue;
                    major = Math.Max(0, cur.Left - r.Right);
                    minor = Gap(cur.Top, cur.Bottom, r.Top, r.Bottom);
                    break;
            }
            var score = 13 * major * major + minor * minor;
            if (score < bestScore)
            {
                bestScore = score;
                best = t;
            }
            // ◄ / ►: mục cùng hàng (chồng nhau theo chiều dọc) được ưu tiên — hàng nút xuống dòng thì không nhảy sang dòng kia
            if (dir is Dir.Left or Dir.Right && r.Bottom > cur.Top + 2 && r.Top < cur.Bottom - 2 && score < bestRowScore)
            {
                bestRowScore = score;
                bestRow = t;
            }
        }
        return bestRow ?? best;
    }

    /// Khoảng lệch giữa hai đoạn (0 nếu chồng lên nhau).
    private static double Gap(double a1, double a2, double b1, double b2)
    {
        if (b2 < a1) return a1 - b2;
        if (b1 > a2) return b1 - a2;
        // Chồng nhau: ưu tiên mục thẳng hàng tâm
        return Math.Abs((a1 + a2) / 2 - (b1 + b2) / 2) * 0.1;
    }

    /// Chọn mục và cuộn cho thấy (chừa lề để thấy cả tiêu đề nhóm phía trên).
    public static void FocusAndShow(FrameworkElement e)
    {
        e.Focus();
        Keyboard.Focus(e);
        e.BringIntoView(new Rect(-30, -70, e.ActualWidth + 60, e.ActualHeight + 110));
    }

    /// Mục đầu tiên trong root (theo thứ tự trên → dưới, trái → phải).
    public static FrameworkElement? First(Visual root)
    {
        return Targets(root).Select(t => (t, r: Bounds(t, root))).Where(x => !x.r.IsEmpty)
            .OrderBy(x => Math.Round(x.r.Top / 8)).ThenBy(x => x.r.Left).Select(x => x.t).FirstOrDefault();
    }

    /// Phần tử là con (cháu) của root không.
    public static bool IsInside(DependencyObject? e, DependencyObject root)
    {
        for (var d = e; d != null; d = VisualTreeHelper.GetParent(d) ?? LogicalTreeHelper.GetParent(d))
            if (ReferenceEquals(d, root)) return true;
        return false;
    }
}
