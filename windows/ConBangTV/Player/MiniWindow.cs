using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using ConBangTV.UI;

namespace ConBangTV.Player;

/// Cửa sổ phim nhỏ (bản TV: MiniPlayer — "thu nhỏ phim"): trình phát chuyển sang cửa sổ nhỏ luôn nổi trên cùng ở góc màn
/// hình, phim chạy tiếp trong lúc dùng app hoặc làm việc khác. Thanh dưới: phát / dừng, phóng to, tắt; kéo thanh để dời,
/// kéo góc để đổi cỡ. Không có Owner: thu nhỏ cửa sổ app thì cửa sổ phim vẫn hiện.
public sealed class MiniWindow : Window
{
    public readonly ContentControl Host = new() { Focusable = false };
    private readonly TextBlock _title = new() { Foreground = Brushes.White, FontSize = 13, VerticalAlignment = VerticalAlignment.Center,
        TextTrimming = TextTrimming.CharacterEllipsis, Margin = new Thickness(10, 0, 6, 0) };
    private readonly Button _play;

    public MiniWindow(Action toggle, Action expand, Action close)
    {
        Title = "Côn Bằng TV — phim thu nhỏ";
        WindowStyle = WindowStyle.None;
        ResizeMode = ResizeMode.CanResizeWithGrip;
        Topmost = true;
        ShowInTaskbar = false;
        Background = Brushes.Black;
        FontFamily = new FontFamily("Segoe UI");
        MinWidth = 280;
        MinHeight = 190;
        Width = 480;
        Height = 270 + 40;
        ShowActivated = false;

        var bar = new DockPanel { Height = 40, Background = (Brush)Application.Current.Resources["Surface"], LastChildFill = true };
        Button Small(string text, Action a, string tip)
        {
            var b = Ui.Pill(text, a);
            b.Margin = new Thickness(0, 4, 6, 4);
            b.Padding = new Thickness(10, 3, 10, 3);
            b.FontSize = 14;
            b.Focusable = false;
            b.ToolTip = tip;
            return b;
        }
        _play = Small("⏸", toggle, "Phát / dừng (Space)");
        var buttons = new StackPanel { Orientation = Orientation.Horizontal };
        buttons.Children.Add(_play);
        buttons.Children.Add(Small("⤢", expand, "Phóng to (Enter / bấm đúp)"));
        buttons.Children.Add(Small("✕", close, "Tắt phim (Esc)"));
        DockPanel.SetDock(buttons, Dock.Right);
        bar.Children.Add(buttons);
        bar.Children.Add(_title);
        bar.MouseLeftButtonDown += (_, e) =>
        {
            if (e.ClickCount == 2) expand();
            else DragMove();
        };

        var root = new DockPanel();
        DockPanel.SetDock(bar, Dock.Bottom);
        root.Children.Add(bar);
        root.Children.Add(Host);
        Content = root;

        PreviewKeyDown += (_, e) =>
        {
            switch (e.Key)
            {
                case Key.Space or Key.MediaPlayPause:
                    toggle();
                    break;
                case Key.Enter:
                    expand();
                    break;
                case Key.Escape:
                    close();
                    break;
                default:
                    return;
            }
            e.Handled = true;
        };
        // Góc dưới phải vùng làm việc của màn hình chính
        var wa = SystemParameters.WorkArea;
        Left = wa.Right - Width - 24;
        Top = wa.Bottom - Height - 24;
    }

    public void Update(string title, bool? paused)
    {
        _title.Text = title;
        _play.Content = paused == false ? "⏸" : "▶";
    }
}
