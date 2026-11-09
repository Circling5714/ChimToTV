using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;
using System.Windows.Media;
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
            SyncService.Stop();
            Prefs.Flush();
        };
        Deactivated += (_, _) => _deactivatedAt = Util.NowMs();
        Activated += (_, _) =>
        {
            _player?.Reactivate();
            // Rời app hơn 30 phút (không xem phim): hỏi lại ai đang xem
            if (_deactivatedAt > 0 && Util.NowMs() - _deactivatedAt > 30 * 60_000 && Player == null && Prefs.Profiles.Count > 1 &&
                (Prefs.AskProfile || Prefs.CurrentProfile.Locked))
                _ = PickProfile(!Prefs.CurrentProfile.Locked);
            // Quay lại app (vd vừa xem trên tivi): đồng bộ, tối đa 1 phút một lần
            if (Util.NowMs() - _lastActiveSync > 60_000)
            {
                _lastActiveSync = Util.NowMs();
                SyncService.SyncSoon(1);
            }
        };
        StateChanged += (_, _) =>
        {
            // Thu nhỏ cửa sổ app lúc đang xem: phim chạy tiếp ở cửa sổ nhỏ
            if (WindowState == WindowState.Minimized && Player != null && Prefs.MiniOnMinimize) _player!.EnterMini();
            _player?.Follow();
        };
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
        SyncService.DataChanged += () => Dispatcher.BeginInvoke(SyncApplied);
        SyncService.Message += m => Dispatcher.BeginInvoke(() => Toast(m));
        SyncService.AppStarted();
        // «Ai đang xem?» khi mở app (từ 2 người xem), hoặc người đang dùng có khoá
        if (Prefs.CurrentProfile.Locked || (Prefs.AskProfile && Prefs.Profiles.Count > 1))
            Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () => _ = PickProfile(!Prefs.CurrentProfile.Locked));
    }

    private long _deactivatedAt;

    private long _lastActiveSync;

    /// Đồng bộ trong nhà vừa đổi dữ liệu máy này: vẽ lại menu và trang đang mở (không đụng trang đang có ô nhập).
    private void SyncApplied()
    {
        RebuildMenu();
        if (Player == null && Current is { } p and not SyncPage and not SearchPage and not ProxyPage and not SitesPage)
        {
            var focus = Keyboard.FocusedElement as FrameworkElement;
            p.Returned();
            if (focus is { IsVisible: true }) focus.Focus();
        }
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
        // Kéo cửa sổ về trong màn hình (lần trước nằm sát mép dưới / màn hình phụ đã rút ra / đổi độ phân giải: phần ngoài màn
        // hình không vẽ — trông như cửa sổ trắng một nửa)
        double vl = SystemParameters.VirtualScreenLeft, vt = SystemParameters.VirtualScreenTop;
        double vw = SystemParameters.VirtualScreenWidth, vh = SystemParameters.VirtualScreenHeight;
        WindowStartupLocation = WindowStartupLocation.Manual;
        Width = Math.Min(Math.Max(MinWidth, b[2]), vw);
        Height = Math.Min(Math.Max(MinHeight, b[3]), vh);
        Left = Math.Clamp(b[0], vl, Math.Max(vl, vl + vw - Width));
        Top = Math.Clamp(b[1], vt, Math.Max(vt, vt + vh - Height));
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
        // Toàn màn hình không có thanh tiêu đề (không nút thu nhỏ / đóng): menu có thêm mục thoát toàn màn hình
        if (MenuPanel.Children.Count > 0) RebuildMenu();
    }

    public void ToggleFullscreen() => SetFullscreen(!_fullscreen);

    // ---- Menu ----

    private IEnumerable<MenuEntry> MenuEntries()
    {
        if (_player is { IsMini: true } mini)
        {
            yield return new("mini-expand", "⤢  Phóng to phim đang xem", () => mini.ExitMini(true));
            yield return new("mini-close", "✕  Tắt phim thu nhỏ", () => mini.Close());
        }
        var me = Prefs.CurrentProfile;
        if (_fullscreen) yield return new("windowed", "⤡  Thoát toàn màn hình (F11)", () => SetFullscreen(false));
        yield return new("profile", $"👤  Người xem: {me.Name}{(me.Locked ? " 🔒" : "")}", () => _ = PickProfile(true));
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
        yield return new("sync", "⇄  Đồng bộ trong nhà", () => ShowRoot(new SyncPage()));
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
        SiteName.Text = Site.DisplayName(Site.HostOf(Prefs.BaseUrl) ?? Prefs.BaseUrl);
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

    /// Thay trang trên cùng (vd đổi nguồn ở trang phim): Esc vẫn về trang trước đó, giữ ô đang chọn ở đó.
    public void Replace(Page p)
    {
        if (_stack.Count == 0)
        {
            Push(p);
            return;
        }
        _stack[^1] = p;
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
        // Trang gốc: Back về menu (như bản TV); đã ở menu mà đang toàn màn hình → Esc thoát toàn màn hình (như trình duyệt)
        if (Keyboard.FocusedElement is not Button fb || !_menuButtons.Contains(fb)) FocusMenu();
        else if (_fullscreen)
        {
            SetFullscreen(false);
            Toast("Đã thoát toàn màn hình (F11 để bật lại).");
        }
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
        RebuildMenu();
    }

    /// Trình phát đã đóng (người xem thoát): về trang trước, cập nhật tập đã xem / xem tiếp.
    public void PlayerClosed()
    {
        SyncService.SyncSoon(1);
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

    public PlayerView? Player => _player is { IsOpen: true, IsMini: false } ? _player : null;

    /// Trình phát đang mở (kể cả ở cửa sổ nhỏ).
    public PlayerView? OpenPlayer => _player is { IsOpen: true } ? _player : null;

    /// Phim đang chạy ở cửa sổ nhỏ: cửa sổ app trở lại các trang.
    public void PlayerMinimized()
    {
        PlayerHost.Visibility = Visibility.Collapsed;
        Shell.Visibility = Visibility.Visible;
        RebuildMenu();
        if (WindowState != WindowState.Minimized)
        {
            Activate();
            if (Current is { } p)
                Dispatcher.BeginInvoke(DispatcherPriority.Loaded, () =>
                {
                    if (p.LastFocus is { IsVisible: true } f) KeyNav.FocusAndShow(f);
                    else FocusMenu();
                });
        }
    }

    /// Phóng to phim từ cửa sổ nhỏ.
    public void PlayerRestored()
    {
        if (WindowState == WindowState.Minimized) WindowState = WindowState.Normal;
        if (Current is { } cur) cur.LastFocus = Keyboard.FocusedElement as FrameworkElement;
        PlayerHost.Visibility = Visibility.Visible;
        Shell.Visibility = Visibility.Hidden;
        Activate();
        RebuildMenu();
    }

    /// Hết giờ xem: người lớn nhập mã PIN phụ huynh để cho xem thêm (rồi mở lại đúng tập, xem tiếp), hoặc thôi.
    public async Task LimitReached(WatchLimit.Block b, string film, string ep)
    {
        if (WindowState == WindowState.Minimized) WindowState = WindowState.Normal;
        Activate();
        var who = Prefs.CurrentProfile.Name;
        var why = b == WatchLimit.Block.Bedtime
            ? $"Đã tới giờ đi ngủ của {who} (từ {WatchLimit.BedtimeLabel(Prefs.Bedtime())} tới 6 giờ sáng)."
            : $"{who} đã xem hết {WatchLimit.DailyLabel(Prefs.DailyLimit())} của hôm nay.";
        while (true)
        {
            var r = await Ask("Hết giờ xem", why + "\nNgười lớn nhập mã PIN phụ huynh để cho xem thêm.", new[] { "Cho xem thêm…", "Thoát" }, focus: 1);
            if (r != 0) return;
            var pin = await AskPin("Mã PIN phụ huynh", "Nhập mã PIN phụ huynh (4 số) để cho xem thêm.");
            if (pin == null) continue;
            if (!Prefs.CheckParentPin(pin))
            {
                Toast("Sai mã PIN phụ huynh.");
                continue;
            }
            var labels = WatchLimit.Grants.Select(m => $"Thêm {WatchLimit.DailyLabel(m)}").Append("Tới 6 giờ sáng").ToArray();
            var g = await Ask("Cho xem thêm bao lâu?", $"Người xem: {who}.", labels);
            if (g < 0) continue;
            if (g < WatchLimit.Grants.Length) WatchLimit.Grant(WatchLimit.Grants[g]);
            else WatchLimit.GrantUntilMorning();
            Toast("Đã cho xem thêm.");
            Play(film, ep);
            return;
        }
    }

    // ---- Người xem ----

    /// Nhập mã PIN 4 số (đủ 4 số là tự xong). null = huỷ.
    public async Task<string?> AskPin(string title, string text)
    {
        var box = new PasswordBox { MaxLength = Prefs.PinLength, MinWidth = 160, HorizontalAlignment = HorizontalAlignment.Left,
            Style = (Style)FindResource("PassBox"), FontSize = 26 };
        box.PasswordChanged += (_, _) =>
        {
            if (box.Password.Length == Prefs.PinLength && box.Password.All(char.IsDigit)) CloseDialog(0);
        };
        box.KeyDown += (_, e) =>
        {
            if (e.Key != Key.Enter) return;
            e.Handled = true;
            CloseDialog(0);
        };
        Dispatcher.BeginInvoke(DispatcherPriority.Input, () => box.Focus());
        var r = await Ask(title, text, new[] { "OK", "Huỷ" }, box);
        var pin = box.Password;
        return r == 0 && pin.Length == Prefs.PinLength && pin.All(char.IsDigit) ? pin : null;
    }

    /// Đặt mã PIN mới (nhập hai lần). null = huỷ / không khớp.
    public async Task<string?> AskNewPin(string title)
    {
        var a = await AskPin(title, "Nhập mã PIN mới (4 số).");
        if (a == null) return null;
        var b = await AskPin(title, "Nhập lại mã PIN để xác nhận.");
        if (b == a) return a;
        if (b != null) Toast("Hai lần nhập không khớp.");
        return null;
    }

    /// Hỏi mã PIN phụ huynh (chưa có thì đặt mới). true = đúng / vừa đặt.
    public async Task<bool> CheckParent(string why)
    {
        if (!Prefs.HasParentPin)
        {
            var np = await AskNewPin("Đặt mã PIN phụ huynh");
            if (np == null) return false;
            Prefs.SetParentPin(np);
            Toast("Đã đặt mã PIN phụ huynh — nhớ mã này, quên thì phải xoá dữ liệu app.");
            return true;
        }
        var pin = await AskPin("Mã PIN phụ huynh", why);
        if (pin == null) return false;
        if (Prefs.CheckParentPin(pin)) return true;
        Toast("Sai mã PIN phụ huynh.");
        return false;
    }

    private bool _picking;

    /// «Ai đang xem?»: chọn người xem (người có khoá phải nhập mã PIN). cancellable = false: người đang dùng có khoá →
    /// phải chọn được ai đó mới vào app.
    public async Task PickProfile(bool cancellable)
    {
        if (_picking) return;
        _picking = true;
        // Bắt buộc chọn (người đang dùng có khoá): nền đen kín, không lộ lịch sử của người đó phía sau
        var dim = DialogLayer.Background;
        if (!cancellable) DialogLayer.Background = (Brush)FindResource("Bg");
        try
        {
            while (true)
            {
                var list = Prefs.Profiles;
                var tiles = new WrapPanel { Margin = new Thickness(0, 6, 0, 0) };
                for (var i = 0; i < list.Count; i++)
                {
                    var idx = i;
                    tiles.Children.Add(ProfileTile(list[i], () => CloseDialog(100 + idx)));
                }
                var buttons = cancellable ? new[] { "Quản lý người xem…", "Huỷ" } : new[] { "Quản lý người xem…" };
                var cur = list.FindIndex(p => p.Id == Prefs.ProfileId);
                Dispatcher.BeginInvoke(DispatcherPriority.ContextIdle, () =>
                {
                    if (cur >= 0 && cur < tiles.Children.Count) ((Button)tiles.Children[cur]).Focus();
                });
                var r = await Ask("Ai đang xem?", "", buttons, tiles);
                if (r == 0)
                {
                    if (cancellable || !Prefs.CurrentProfile.Locked)
                    {
                        ShowRoot(new ProfilesPage());
                        return;
                    }
                    Toast("Chọn người xem trước.");
                    continue;
                }
                if (r < 100)
                {
                    if (!cancellable && Prefs.CurrentProfile.Locked) continue;
                    return;
                }
                var p = list[r - 100];
                if (p.Locked && !(cancellable && p.Id == Prefs.ProfileId))
                {
                    var pin = await AskPin($"Mã PIN của {p.Name}", "Nhập mã PIN 4 số.");
                    if (pin == null) continue;
                    if (!Prefs.CheckPin(p, pin))
                    {
                        Toast("Sai mã PIN.");
                        continue;
                    }
                }
                SwitchTo(p);
                return;
            }
        }
        finally
        {
            _picking = false;
            DialogLayer.Background = dim;
        }
    }

    /// Đổi sang người xem p: đóng phim đang xem, về Trang chủ (trang phim / lịch sử của người đó).
    public void SwitchTo(Profile p)
    {
        if (p.Id == Prefs.ProfileId) return;
        _player?.Close(false);
        WatchLimit.Flush();
        WatchLimit.Reset();
        Prefs.SwitchProfile(p.Id);
        PlayerHost.Visibility = Visibility.Collapsed;
        Shell.Visibility = Visibility.Visible;
        _categories = new();
        RebuildMenu();
        ShowRoot(new HomePage());
        Toast($"Đang xem: {p.Name}.");
    }

    /// Ô người xem: vòng tròn màu + chữ cái đầu, tên, 🔒 nếu có khoá.
    public static Button ProfileTile(Profile p, Action onClick)
    {
        var color = Prefs.ProfileColors[Math.Clamp(p.Color, 0, Prefs.ProfileColors.Length - 1)];
        var circle = new Border
        {
            Width = 84, Height = 84, CornerRadius = new CornerRadius(42), HorizontalAlignment = HorizontalAlignment.Center,
            Background = new SolidColorBrush(Color.FromRgb((byte)(color >> 16), (byte)(color >> 8), (byte)color)),
            Child = new TextBlock
            {
                Text = p.Initial, FontSize = 38, FontWeight = FontWeights.Bold, Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
            },
        };
        var panel = new StackPanel { Width = 120 };
        panel.Children.Add(circle);
        panel.Children.Add(new TextBlock
        {
            Text = p.Name + (p.Locked ? " 🔒" : ""), FontSize = 16, Margin = new Thickness(0, 8, 0, 0), TextAlignment = TextAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        var b = new Button { Content = panel, Padding = new Thickness(10), Margin = new Thickness(0, 0, 12, 12), Background = Brushes.Transparent };
        b.Click += (_, _) => onClick();
        return b;
    }

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
        // ▲ / ▼ trong trang chỉ đi trong trang (không nhảy sang menu bên trái khi hàng dưới cách xa vì có tiêu đề)
        var area = root == Shell && dir is KeyNav.Dir.Up or KeyNav.Dir.Down && KeyNav.IsInside(focused, PageHost) ? PageHost : root;
        var next = KeyNav.Next(area, focused, dir);
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

    public void CloseDialog(int result)
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
