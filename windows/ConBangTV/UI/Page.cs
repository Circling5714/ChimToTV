using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Một trang nội dung (bên phải menu). Trang tự tải dữ liệu (Load), hiện chữ "Đang tải…" / lỗi kèm nút thử lại.
public abstract class Page : UserControl
{
    /// Mục menu sáng lên khi trang đang mở.
    public string MenuKey { get; init; } = "";

    /// Mục đang chọn lúc rời trang (quay lại thì chọn lại).
    public FrameworkElement? LastFocus;

    protected readonly ScrollViewer Scroller;
    protected readonly StackPanel Body;
    private int _loadToken;

    protected static MainWindow Main => (MainWindow)Application.Current.MainWindow;

    protected Page()
    {
        Body = new StackPanel { Margin = new Thickness(36, 26, 28, 30) };
        Scroller = Ui.Scroll(Body);
        Content = Scroller;
        Focusable = false;
    }

    /// Gọi khi trang được mở.
    public async Task Start()
    {
        var t = ++_loadToken;
        try
        {
            await Load();
        }
        catch (Exception e)
        {
            if (t != _loadToken) return;
            Log.D("trang", $"{GetType().Name}: {e}");
            ShowError(Util.Friendly(e));
        }
    }

    protected abstract Task Load();

    /// Quay lại trang này (từ trang sau / trình phát).
    public virtual void Returned()
    {
    }

    /// Mục chọn đầu tiên khi bấm phím mũi tên lần đầu.
    public virtual FrameworkElement? FirstTarget() => KeyNav.First(Body);

    public void FocusFirst()
    {
        if (FirstTarget() is { } f) KeyNav.FocusAndShow(f);
    }

    /// Không còn mục nào theo hướng ▲ / ▼: cuộn hết lên / xuống (vd còn chữ mô tả ở cuối trang).
    public virtual void ScrollEdge(KeyNav.Dir dir)
    {
        if (dir == KeyNav.Dir.Up) Scroller.ScrollToTop();
        else Scroller.ScrollToBottom();
    }

    protected void Loading(string text = "Đang tải…")
    {
        Body.Children.Clear();
        var p = new StackPanel { HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 140, 0, 0) };
        p.Children.Add(new ProgressBar { IsIndeterminate = true, Width = 260 });
        p.Children.Add(Ui.Dim(text, 15, new Thickness(0, 14, 0, 0)));
        ((TextBlock)p.Children[1]).HorizontalAlignment = HorizontalAlignment.Center;
        Body.Children.Add(p);
    }

    protected virtual void ShowError(string message)
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Text("Không tải được", 24, weight: FontWeights.SemiBold, margin: new Thickness(0, 60, 0, 10)));
        Body.Children.Add(Ui.Dim(message, 16, new Thickness(0, 0, 0, 20)));
        Body.Children.Add(Ui.Row(
            Ui.Pill("↻  Thử lại", () => _ = Start()),
            Ui.Pill("🌐  Đổi trang phim", () => Main.ShowRoot(new SitesPage()))));
        Dispatcher.BeginInvoke(() => FocusFirst(), System.Windows.Threading.DispatcherPriority.Loaded);
    }

    /// Mở chi tiết phim.
    protected static void OpenFilm(Film f) => Main.Push(new DetailPage(f.Url, f));

    /// Thẻ phim mở trang chi tiết; xoá được bằng Delete / chuột phải nếu có onRemove.
    protected static Button FilmCard(Film f, Action? open = null, string? sub = null, string? fresh = null, Func<Task>? onRemove = null)
    {
        var c = Ui.Card(f, open ?? (() => OpenFilm(f)), sub, fresh);
        if (onRemove != null)
        {
            c.PreviewKeyDown += async (_, e) =>
            {
                if (e.Key != Key.Delete) return;
                e.Handled = true;
                await onRemove();
            };
            c.MouseRightButtonUp += async (_, e) =>
            {
                e.Handled = true;
                await onRemove();
            };
        }
        return c;
    }
}
