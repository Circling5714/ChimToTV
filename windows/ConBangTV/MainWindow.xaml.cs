using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using ConBangTV.Core;
using ConBangTV.Player;
using ConBangTV.UI;
using Page = ConBangTV.UI.Page;

namespace ConBangTV;

/// Cửa sổ chính: menu bên trái (như bản TV), trang nội dung bên phải, trình phát phủ cả cửa sổ.
/// Đi bằng chuột hoặc phím mũi tên / remote (KeyNav); Esc / Backspace / phím Back của remote = quay lại.
public partial class MainWindow : Window
{
    private sealed record MenuEntry(string Key, string Title, Action Open);

    private readonly List<Page> _stack = new();
    private readonly List<Button> _menuButtons = new();
    private List<Category> _categories = new();
    private string _activeMenu = "home";
    private PlayerView? _player;
    private readonly DispatcherTimer _toastTimer = new() { Interval = TimeSpan.FromSeconds(4) };
    private TaskCompletionSource<int>? _dialog;
    private FrameworkElement? _focusBeforeDialog;

    // Toàn màn hình
    private bool _fullscreen;
    private WindowState _prevState;

    public MainWindow()
    {
        InitializeComponent();
        _toastTimer.Tick += (_, _) =>
        {
            _toastTimer.Stop();
            ToastBox.Visibility = Visibility.Collapsed;
        };
        PreviewKeyDown += OnPreviewKeyDown;
        PreviewMouseDown += (_, e) =>
        {
            if (e.ChangedButton == MouseButton.XButton1 && _player == null && DialogLayer.Visibility != Visibility.Visible)
            {
                e.Handled = true;
                Back();
            }
        };
        LoadBounds();
        ApplyScale();
        Loaded += (_, _) => Start();
        Closing += (_, _) =>
        {
            SaveBounds();
            _player?.Close(false);
            Prefs.Flush();
        };
        Activated += (_, _) => _player?.Reactivate();
        StateChanged += (_, _) => _player?.Follow();
        LocationChanged += (_, _) => _player?.Follow();
        SizeChanged += (_, _) => _player?.Follow();
    }

    private void Start()
    {
        if (Prefs.StartFullscreen) SetFullscreen(true);
        ImageLoader.Cleanup();
        RebuildMenu();
        ShowRoot(new HomePage());
#if DEBUG
        DebugServer.Start(this);
#endif
        _ = CheckUpdate(false);
    }

    // ---- Cỡ giao diện / cửa sổ ----

    public void ApplyScale()
    {
        var s = Prefs.UiScale;
        Shell.LayoutTransform = Math.Abs(s - 1) < 0.01 ? Transform.Identity : new ScaleTransform(s, s);
        DialogBox.LayoutTransform = Shell.LayoutTransform;
        ToastBox.LayoutTransform = Shell.LayoutTransform;
    }

    private void LoadBounds()
    {
        var b = Prefs.WindowBounds;
        if (b is not { Length: 5 }) return;
        var area = SystemParameters.VirtualScreenWidth;
        if (b[0] < SystemParameters.VirtualScreenLeft - 50 || b[0] > SystemParameters.VirtualScreenLeft + area - 100) return;
        WindowStartupLocation = WindowStartupLocation.Manual;
        Left = b[0];
        Top = b[1];
        Width = Math.Max(MinWidth, b[2]);
        Height = Math.Max(MinHeight, b[3]);
        if (b[4] > 0) WindowState = WindowState.Maximized;
    }

    private void SaveBounds()
    {
        if (_fullscreen) return;
        var r = WindowState == WindowState.Normal ? new Rect(Left, Top, Width, Height) : RestoreBounds;
        Prefs.WindowBounds = new[] { r.Left, r.Top, r.Width, r.Height, WindowState == WindowState.Maximized ? 1.0 : 0 };
    }

    public bool IsFullscreen => _fullscreen;

    public void SetFullscreen(bool on)
    {
        if (on == _fullscreen) return;
        _fullscreen = on;
        if (on)
        {
            _prevState = WindowState;
            WindowStyle = WindowStyle.None;
            ResizeMode = ResizeMode.NoResize;
            if (WindowState == WindowState.Maximized) WindowState = WindowState.Normal; // để phủ cả thanh tác vụ
            WindowState = WindowState.Maximized;
        }
        else
        {
            WindowStyle = WindowStyle.SingleBorderWindow;
            ResizeMode = ResizeMode.CanResize;
            WindowState = _prevState == WindowState.Maximized ? WindowState.Maximized : WindowState.Normal;
        }
        _player?.Follow();
    }

    public void ToggleFullscreen() => SetFullscreen(!_fullscreen);

    // ---- Menu ----

    private IEnumerable<MenuEntry> MenuEntries()
    {
        yield return new("home", "🏠  Trang chủ", () => ShowRoot(new HomePage()));
        yield return new("search", "🔍  Tìm kiếm", () => ShowRoot(new SearchPage()));
        yield return new("history", "🕘  Xem tiếp / Lịch sử", () => ShowRoot(new HistoryPage()));
        var fresh = Prefs.Follows.Count(f => f.NewCount > 0);
        yield return new("follows", "★  Theo dõi" + (fresh > 0 ? $"  ({fresh} mới)" : ""), () => ShowRoot(new FollowsPage()));
        foreach (var c in _categories)
            yield return new("cat:" + c.Url, c.Title, () => ShowRoot(new ListPage(c.Title, c.Url) { MenuKey = "cat:" + c.Url }));
        var b = Prefs.BaseUrl;
        if (NextSite.IsKnown(b) || ApiSite.IsKnown(b))
        {
            yield return new("genres", "Thể loại…", () => ShowRoot(new FilterPage(true)));
            yield return new("countries", "Quốc gia…", () => ShowRoot(new FilterPage(false)));
        }
        yield return new("sites", "🌐  Đổi trang phim", () => ShowRoot(new SitesPage()));
        yield return new("settings", "⚙  Cài đặt", () => ShowRoot(new SettingsPage()));
    }

    public void SetCategories(List<Category> list)
    {
        _categories = list;
        RebuildMenu();
    }

    public void RebuildMenu()
    {
        var focused = Keyboard.FocusedElement is Button fb && _menuButtons.Contains(fb) ? (string?)fb.CommandParameter : null;
        MenuPanel.Children.Clear();
        _menuButtons.Clear();
        SiteName.Text = Site.HostOf(Prefs.BaseUrl) ?? Prefs.BaseUrl;
        var i = 0;
        foreach (var m in MenuEntries())
        {
            if (m.Key.StartsWith("cat:") && i++ == 0)
                MenuPanel.Children.Add(new Border { Height = 1, Background = Ui.Res("Surface3"), Margin = new Thickness(16, 8, 16, 8) });
            if (m.Key == "sites")
                MenuPanel.Children.Add(new Border { Height = 1, Background = Ui.Res("Surface3"), Margin = new Thickness(16, 8, 16, 8) });
            var b = new Button
            {
                Style = (Style)FindResource("MenuItem"),
                Content = new TextBlock { Text = m.Title, TextTrimming = TextTrimming.CharacterEllipsis },
                Tag = m.Key == _activeMenu ? "active" : null,
                CommandParameter = m.Key,
            };
            b.Click += (_, _) => m.Open();
            _menuButtons.Add(b);
            MenuPanel.Children.Add(b);
        }
        if (focused != null) _menuButtons.FirstOrDefault(x => (string)x.CommandParameter == focused)?.Focus();
    }

    private void SetActiveMenu(string key)
    {
        _activeMenu = key;
        foreach (var b in _menuButtons) b.Tag = (string)b.CommandParameter == key ? "active" : null;
    }

    public void FocusMenu()
    {
        var b = _menuButtons.FirstOrDefault(x => (string)x.CommandParameter == _activeMenu) ?? _menuButtons.FirstOrDefault();
        if (b != null) KeyNav.FocusAndShow(b);
    }

    // ---- Trang ----

    public Page? Current => _stack.LastOrDefault();

    /// Mở trang gốc (từ menu): bỏ các trang đang mở.
    public void ShowRoot(Page p)
    {
        _stack.Clear();
        Push(p);
    }

    public void Push(Page p)
    {
        if (Current is { } cur) cur.LastFocus = Keyboard.FocusedElement as FrameworkElement;
        _stack.Add(p);
        SetActiveMenu(p.MenuKey);
        PageHost.Content = p;
        _ = p.Start();
    }

    public void Back()
    {
        if (DialogLayer.Visibility == Visibility.Visible)
        {
            CloseDialog(-1);
            return;
        }
        if (_stack.Count > 1)
        {
            _stack.RemoveAt(_stack.Count - 1);
            var p = _stack[^1];
            SetActiveMenu(p.MenuKey);
            PageHost.Content = p;
            p.Returned();
            Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
            {
                if (p.LastFocus is { IsVisible: true } f) KeyNav.FocusAndShow(f);
                else p.FocusFirst();
            });
            return;
        }
        // Trang gốc: Back về menu (như bản TV)
        if (Keyboard.FocusedElement is not Button fb || !_menuButtons.Contains(fb)) FocusMenu();
    }

    public void Busy(string? text)
    {
        LoadingBox.Visibility = text == null ? Visibility.Collapsed : Visibility.Visible;
        LoadingText.Text = text ?? "";
    }

    // ---- Trình phát ----

    /// Mở phim ở tập epUrl (detail có sẵn thì khỏi tải lại). resume: mở từ "Xem tiếp" (tập đã xem hết thì sang tập sau).
    public void Play(string filmUrl, string epUrl, bool resume = false)
    {
        if (Current is { } cur) cur.LastFocus = Keyboard.FocusedElement as FrameworkElement;
        if (_player == null)
        {
            _player = new PlayerView(this);
            PlayerHost.Content = _player;
        }
        PlayerHost.Visibility = Visibility.Visible;
        Shell.Visibility = Visibility.Hidden;
        _player.Open(filmUrl, epUrl, resume);
    }

    /// Trình phát đã đóng (người xem thoát): về trang trước, cập nhật tập đã xem / xem tiếp.
    public void PlayerClosed()
    {
        PlayerHost.Visibility = Visibility.Collapsed;
        Shell.Visibility = Visibility.Visible;
        Activate();
        if (Current is { } p)
        {
            p.Returned();
            Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
            {
                if (p.LastFocus is { IsVisible: true } f) KeyNav.FocusAndShow(f);
                else p.FocusFirst();
            });
        }
        RebuildMenu();
    }

    public PlayerView? Player => _player is { IsOpen: true } ? _player : null;

    // ---- Phím ----

    private void OnPreviewKeyDown(object sender, KeyEventArgs e)
    {
        var key = e.Key == Key.System ? e.SystemKey : e.Key;
        // Trình phát đang mở: phím do trình phát xử lý (cửa sổ phủ thường giữ phím; đây là khi cửa sổ chính được chọn)
        if (Player != null)
        {
            Player.HandleKey(e);
            return;
        }
        if (key == Key.F11 || (key == Key.Enter && Keyboard.Modifiers == ModifierKeys.Alt))
        {
            ToggleFullscreen();
            e.Handled = true;
            return;
        }
        var focused = Keyboard.FocusedElement as FrameworkElement;
        var inText = focused is TextBox or PasswordBox;
        if (key is Key.Escape or Key.BrowserBack || (key == Key.Back && !inText))
        {
            e.Handled = true;
            Back();
            return;
        }
        if (key == Key.BrowserSearch || (key == Key.F && Keyboard.Modifiers == ModifierKeys.Control))
        {
            e.Handled = true;
            ShowRoot(new SearchPage());
            return;
        }
        if (key == Key.BrowserHome)
        {
            e.Handled = true;
            ShowRoot(new HomePage());
            return;
        }
        if (KeyNav.DirOf(key) is not { } dir) return;
        if (focused is PasswordBox && dir is KeyNav.Dir.Left or KeyNav.Dir.Right) return;
        if (focused is TextBox && dir is KeyNav.Dir.Left or KeyNav.Dir.Right)
        {
            var tb = (TextBox)focused!;
            var atEdge = dir == KeyNav.Dir.Left ? tb.CaretIndex == 0 && tb.SelectionLength == 0 : tb.CaretIndex >= tb.Text.Length;
            if (!atEdge) return;
        }
        e.Handled = true;
        Visual root = DialogLayer.Visibility == Visibility.Visible ? DialogBox : Shell;
        if (focused == null || !KeyNav.IsInside(focused, root))
        {
            var first = root == Shell && PageHost.Content is Page pg ? pg.FirstTarget() ?? KeyNav.First(root) : KeyNav.First(root);
            if (first != null) KeyNav.FocusAndShow(first);
            return;
        }
        var next = KeyNav.Next(root, focused, dir);
        if (next != null) KeyNav.FocusAndShow(next);
        else if (dir == KeyNav.Dir.Down || dir == KeyNav.Dir.Up) (PageHost.Content as Page)?.ScrollEdge(dir);
    }

    // ---- Thông báo / hộp thoại ----

    public void Toast(string text, int ms = 4000)
    {
        ToastText.Text = text;
        ToastBox.Visibility = Visibility.Visible;
        _toastTimer.Stop();
        _toastTimer.Interval = TimeSpan.FromMilliseconds(ms);
        _toastTimer.Start();
        _player?.Notice(text, ms);
    }

    /// Hộp thoại: trả về chỉ số nút được bấm (-1 = huỷ / Back). Nút đầu tiên được chọn sẵn.
    public Task<int> Ask(string title, string text, string[] buttons, UIElement? extra = null, int focus = 0)
    {
        _dialog?.TrySetResult(-1);
        _focusBeforeDialog = Keyboard.FocusedElement as FrameworkElement;
        DialogTitle.Text = title;
        DialogText.Text = text;
        DialogText.Visibility = string.IsNullOrEmpty(text) ? Visibility.Collapsed : Visibility.Visible;
        DialogExtra.Content = extra;
        DialogButtons.Children.Clear();
        for (var i = 0; i < buttons.Length; i++)
        {
            var idx = i;
            DialogButtons.Children.Add(Ui.Pill(buttons[i], () => CloseDialog(idx), minWidth: 110));
        }
        DialogLayer.Visibility = Visibility.Visible;
        _dialog = new TaskCompletionSource<int>();
        Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
        {
            if (DialogButtons.Children.Count > focus) ((Button)DialogButtons.Children[focus]).Focus();
        });
        return _dialog.Task;
    }

    public async Task<bool> Confirm(string title, string text, string ok = "Đồng ý", string cancel = "Huỷ") =>
        await Ask(title, text, new[] { ok, cancel }, focus: 1) == 0;

    private void CloseDialog(int result)
    {
        DialogLayer.Visibility = Visibility.Collapsed;
        DialogExtra.Content = null;
        var t = _dialog;
        _dialog = null;
        if (_focusBeforeDialog is { IsVisible: true } f) f.Focus();
        t?.TrySetResult(result);
    }

    // ---- Cập nhật ----

    private bool _updating;

    /// Kiểm tra bản mới (tự động: 2 giờ một lần, bỏ bản người xem đã hoãn; manual: bấm trong Cài đặt).
    public async Task CheckUpdate(bool manual)
    {
        if (_updating) return;
        if (!manual && (!Prefs.AutoUpdate || Util.NowMs() - Prefs.LastUpdateCheck < UpdateCheck.CheckEvery)) return;
        try
        {
            if (manual) Toast("Đang kiểm tra bản mới…");
            var rel = await UpdateCheck.Latest();
            Prefs.LastUpdateCheck = Util.NowMs();
            if (rel == null || !UpdateCheck.IsNewer(rel.Version))
            {
                if (manual) Toast($"Đang dùng bản mới nhất ({UpdateCheck.CurrentVersion}).");
                return;
            }
            if (!manual && Prefs.IsUpdateSnoozed(rel.Version)) return;
            // Đang xem phim thì để lúc khác (lần kiểm sau)
            if (!manual && Player != null) return;
            var summary = UpdateCheck.Summary(rel.Notes);
            var choice = await Ask($"Có bản mới: Côn Bằng TV {rel.Version}", (summary.Length > 0 ? summary + "\n\n" : "") +
                $"Bản đang dùng: {UpdateCheck.CurrentVersion}.", new[] { "Cập nhật ngay", "Để sau" });
            if (choice != 0)
            {
                Prefs.SnoozeUpdate(rel.Version, Util.NowMs() + UpdateCheck.Snooze);
                return;
            }
            if (!UpdateCheck.CanSelfUpdate())
            {
                Toast("Thư mục chứa app không ghi được — mở trang tải bản mới.");
                Ui.OpenBrowser(UpdateCheck.ReleasesPage);
                return;
            }
            _updating = true;
            Toast("Đang tải bản cập nhật… 0%", 600_000);
            var last = -1;
            var file = await UpdateCheck.Download(rel, p =>
            {
                var pct = (int)(p * 100);
                if (pct == last) return;
                last = pct;
                Dispatcher.BeginInvoke(() => Toast($"Đang tải bản cập nhật… {pct}%", 600_000));
            });
            Toast("Đã tải xong — mở lại app với bản mới…");
            Log.D("update", $"Cài bản {rel.Version} từ {file}");
            UpdateCheck.Install(file);
            Application.Current.Shutdown();
        }
        catch (Exception e)
        {
            Log.D("update", "Lỗi: " + e);
            if (manual || _updating) Toast("Không cập nhật được: " + Util.Friendly(e), 8000);
        }
        finally
        {
            _updating = false;
        }
    }
}
