using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Trang chủ: Xem tiếp, phim theo dõi có tập mới, rồi các nhóm phim của trang.
public sealed class HomePage : Page
{
    private Home? _home;
    private string _base = "";

    public HomePage()
    {
        MenuKey = "home";
    }

    protected override async Task Load()
    {
        _base = Prefs.BaseUrl;
        Loading($"Đang tải {Site.HostOf(_base)}…");
        var home = await Site.HomeOf(_base);
        if (home.MovedTo is { } moved && Prefs.NormalizeUrl(moved) is { } m && m != _base)
        {
            Prefs.ReplaceSite(_base, m);
            Main.Toast($"Trang đã chuyển sang {Site.HostOf(m)} — app đã đổi theo.");
            _base = m;
        }
        _home = home;
        Main.SetCategories(home.Categories);
        Build();
    }

    private void Build()
    {
        var home = _home;
        if (home == null) return;
        Body.Children.Clear();
        var history = Prefs.History.Take(12).ToList();
        if (history.Count > 0)
        {
            Body.Children.Add(Ui.Section("Xem tiếp", history.Select(h =>
                (UIElement)FilmCard(new Film(h.FilmUrl, h.Title, h.Poster, null), () => Main.Play(h.FilmUrl, h.EpUrl, true), h.EpTitle)),
                "Enter: xem tiếp ngay"));
        }
        var fresh = Prefs.Follows.Where(f => f.NewCount > 0).ToList();
        if (fresh.Count > 0)
        {
            Body.Children.Add(Ui.Section("Phim theo dõi có tập mới", fresh.Select(f =>
                (UIElement)FilmCard(new Film(f.FilmUrl, f.Title, f.Poster, f.Latest.NonEmpty()), fresh: $"+{f.NewCount}"))));
        }
        foreach (var s in home.Sections)
            Body.Children.Add(Ui.Section(s.Title, s.Films.Select(f => (UIElement)FilmCard(f))));
        if (home.Sections.Count == 0)
        {
            Body.Children.Add(Ui.Text("Trang không có phim nào", 22, weight: FontWeights.SemiBold, margin: new Thickness(0, 30, 0, 8)));
            Body.Children.Add(Ui.Dim("Có thể trang đã đổi giao diện, hoặc nhà mạng chặn trang này (trang chặn không có phim). Thử trang khác:", 15,
                new Thickness(0, 0, 0, 16)));
            Body.Children.Add(Ui.Pill("🌐  Đổi trang phim", () => Main.ShowRoot(new SitesPage())));
        }
    }

    public override void Returned()
    {
        if (Prefs.BaseUrl != _base)
        {
            _ = Start();
            return;
        }
        Build();
    }
}

/// Danh sách phim của một mục (thể loại, quốc gia…): tự tải trang sau khi cuộn tới cuối.
public sealed class ListPage : Page
{
    private readonly string _title;
    private readonly string _url;
    private string? _next;
    private bool _loadingMore;
    private WrapPanel? _grid;
    private Button? _more;
    private readonly HashSet<string> _seen = new();

    public ListPage(string title, string url)
    {
        _title = title;
        _url = url;
        Scroller.ScrollChanged += (_, e) =>
        {
            if (_next != null && !_loadingMore && e.VerticalOffset + e.ViewportHeight > e.ExtentHeight - 700) _ = More();
        };
    }

    protected override async Task Load()
    {
        Loading($"Đang tải {_title}…");
        _seen.Clear();
        var page = await Site.List(_url);
        Body.Children.Clear();
        Body.Children.Add(Ui.Header(_title));
        if (page.Sections.Count > 0)
        {
            foreach (var s in page.Sections) Body.Children.Add(Ui.Section(s.Title, s.Films.Select(f => (UIElement)FilmCard(f))));
            return;
        }
        if (page.Films.Count == 0)
        {
            Body.Children.Add(Ui.Dim("Không có phim nào.", 16));
            return;
        }
        _grid = new WrapPanel();
        Body.Children.Add(_grid);
        Add(page);
    }

    private void Add(FilmPage page)
    {
        if (_grid == null) return;
        if (_more != null) Body.Children.Remove(_more);
        foreach (var f in page.Films)
            if (_seen.Add(f.Url)) _grid.Children.Add(FilmCard(f));
        _next = page.NextUrl;
        if (_next != null)
        {
            _more = Ui.Pill("Tải thêm phim", () => _ = More());
            _more.HorizontalAlignment = HorizontalAlignment.Left;
            Body.Children.Add(_more);
        }
    }

    private async Task More()
    {
        if (_next == null || _loadingMore) return;
        _loadingMore = true;
        if (_more != null) _more.Content = "Đang tải thêm…";
        var hadFocus = _more?.IsKeyboardFocused == true;
        try
        {
            var page = await Site.List(_next);
            var before = _grid?.Children.Count ?? 0;
            Add(page);
            if (hadFocus && _grid != null && _grid.Children.Count > before) KeyNav.FocusAndShow((FrameworkElement)_grid.Children[before]);
        }
        catch (Exception e)
        {
            Main.Toast("Không tải thêm được: " + Util.Friendly(e));
            if (_more != null) _more.Content = "Tải thêm phim";
        }
        finally
        {
            _loadingMore = false;
        }
    }

    public override void ScrollEdge(KeyNav.Dir dir)
    {
        base.ScrollEdge(dir);
        if (dir == KeyNav.Dir.Down) _ = More();
    }
}

/// Tìm phim: trên mọi trang trong danh sách (như bản TV) hoặc chỉ trang đang dùng.
public sealed class SearchPage : Page
{
    private static string _lastQuery = "";
    private static bool _allSites = true;
    private readonly TextBox _box = new() { MinWidth = 380, MaxWidth = 560 };
    private readonly StackPanel _results = new();
    private readonly Button _scope;
    private int _token;

    public SearchPage()
    {
        MenuKey = "search";
        _box.Text = _lastQuery;
        _box.KeyDown += (_, e) =>
        {
            if (e.Key != Key.Enter) return;
            e.Handled = true;
            _ = Search();
        };
        _scope = Ui.Pill("", () =>
        {
            _allSites = !_allSites;
            UpdateScope();
            if (_box.Text.Trim().Length > 0) _ = Search();
        });
        UpdateScope();
    }

    private void UpdateScope() => _scope.Content = _allSites ? "Tìm trên: mọi trang" : $"Tìm trên: {Site.HostOf(Prefs.BaseUrl)}";

    protected override Task Load()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Tìm kiếm"));
        var row = new WrapPanel();
        _box.Margin = new Thickness(0, 0, 10, 10);
        row.Children.Add(_box);
        row.Children.Add(Ui.Pill("🔍  Tìm", () => _ = Search()));
        row.Children.Add(_scope);
        Body.Children.Add(row);
        Body.Children.Add(Ui.Dim("Gõ tên phim (có dấu hoặc không) rồi nhấn Enter.", 14, new Thickness(0, 0, 0, 18)));
        Body.Children.Add(_results);
        Dispatcher.BeginInvoke(() =>
        {
            _box.Focus();
            _box.SelectAll();
        }, DispatcherPriority.Loaded);
        if (_lastQuery.Length > 0 && _results.Children.Count == 0) _ = Search();
        return Task.CompletedTask;
    }

    public override FrameworkElement? FirstTarget() => _box;

    private async Task Search()
    {
        var q = _box.Text.Trim();
        if (q.Length == 0) return;
        _lastQuery = q;
        var t = ++_token;
        _results.Children.Clear();
        _results.Children.Add(Ui.Dim(_allSites ? $"Đang tìm «{q}» trên {Prefs.Sites.Count} trang…" : $"Đang tìm «{q}»…", 16));
        try
        {
            var page = _allSites ? await Site.SearchAll(q) : await Site.Search(q);
            if (t != _token) return;
            _results.Children.Clear();
            var sections = page.Sections.Count > 0 ? page.Sections : new List<FilmSection> { new("Kết quả", page.Films) };
            sections = sections.Where(s => s.Films.Count > 0).ToList();
            if (sections.Count == 0)
            {
                _results.Children.Add(Ui.Dim($"Không tìm thấy phim nào cho «{q}».", 16));
                return;
            }
            foreach (var s in sections) _results.Children.Add(Ui.Section(s.Title, s.Films.Select(f => (UIElement)FilmCard(f))));
        }
        catch (Exception e)
        {
            if (t != _token) return;
            _results.Children.Clear();
            _results.Children.Add(Ui.Dim("Lỗi: " + Util.Friendly(e), 16));
        }
    }
}

/// Xem tiếp / lịch sử xem: Enter xem tiếp ngay; Delete (chuột phải) xoá một phim.
public sealed class HistoryPage : Page
{
    public HistoryPage()
    {
        MenuKey = "history";
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private void Build()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Xem tiếp / Lịch sử"));
        var list = Prefs.History;
        if (list.Count == 0)
        {
            Body.Children.Add(Ui.Dim("Chưa xem phim nào.", 16));
            return;
        }
        Body.Children.Add(Ui.Row(Ui.Pill("🗑  Xoá cả lịch sử", async () =>
        {
            if (!await Main.Confirm("Xoá cả lịch sử xem?", "Xoá danh sách Xem tiếp và chỗ đang xem của mọi tập.", "Xoá hết")) return;
            Prefs.ClearHistory();
            SyncService.SyncSoon();
            Build();
        })));
        Body.Children.Add(Ui.Dim("Enter / bấm: xem tiếp ngay.  Delete hoặc chuột phải: xoá phim khỏi lịch sử.", 14, new Thickness(0, 0, 0, 16)));
        var grid = new WrapPanel();
        foreach (var h in list)
        {
            var when = DateTimeOffset.FromUnixTimeMilliseconds(h.Time).LocalDateTime;
            grid.Children.Add(FilmCard(new Film(h.FilmUrl, h.Title, h.Poster, Site.HostOf(h.FilmUrl)), () => Main.Play(h.FilmUrl, h.EpUrl, true),
                $"{h.EpTitle} · {when:dd/MM HH:mm}", onRemove: async () =>
                {
                    if (!await Main.Confirm($"Xoá «{h.Title}» khỏi lịch sử?", "Xoá cả chỗ đang xem của các tập phim này.", "Xoá")) return;
                    Prefs.RemoveHistory(h.FilmUrl);
                    SyncService.SyncSoon();
                    Build();
                    FocusFirst();
                }));
        }
        Body.Children.Add(grid);
    }

    public override void Returned() => Build();
}

/// Phim theo dõi: số tập mới từ lần xem trước; mở trang là kiểm lại.
public sealed class FollowsPage : Page
{
    private bool _checking;

    public FollowsPage()
    {
        MenuKey = "follows";
    }

    protected override Task Load()
    {
        Build();
        _ = CheckAll();
        return Task.CompletedTask;
    }

    private void Build()
    {
        var focusedUrl = (Keyboard.FocusedElement as FrameworkElement)?.DataContext is Film ff ? ff.Url : null;
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Phim theo dõi"));
        var list = Prefs.Follows;
        if (list.Count == 0)
        {
            Body.Children.Add(Ui.Dim("Chưa theo dõi phim nào. Mở một phim rồi bấm «☆ Theo dõi» để biết khi có tập mới.", 16));
            return;
        }
        Body.Children.Add(Ui.Dim(_checking ? "Đang kiểm tra tập mới…" : "Delete hoặc chuột phải: bỏ theo dõi.", 14, new Thickness(0, 0, 0, 16)));
        var grid = new WrapPanel();
        foreach (var f in list.OrderByDescending(x => x.NewCount > 0))
        {
            var card = FilmCard(new Film(f.FilmUrl, f.Title, f.Poster, f.Latest.NonEmpty()), fresh: f.NewCount > 0 ? $"+{f.NewCount}" : null,
                sub: Site.HostOf(f.FilmUrl), onRemove: async () =>
                {
                    if (!await Main.Confirm($"Bỏ theo dõi «{f.Title}»?", "", "Bỏ theo dõi")) return;
                    Prefs.Unfollow(f.FilmUrl);
                    SyncService.SyncSoon();
                    Build();
                    Main.RebuildMenu();
                });
            grid.Children.Add(card);
            if (f.FilmUrl == focusedUrl) Dispatcher.BeginInvoke(() => card.Focus(), DispatcherPriority.Loaded);
        }
        Body.Children.Add(grid);
    }

    private async Task CheckAll()
    {
        if (_checking) return;
        _checking = true;
        var gate = new SemaphoreSlim(3);
        var tasks = Prefs.Follows.Select(async f =>
        {
            await gate.WaitAsync();
            try
            {
                var d = await Site.Detail(f.FilmUrl);
                if (d.LatestEpisode is { } last) Prefs.UpdateFollow(f.FilmUrl, d.EpisodeCount, last.Title, false);
            }
            catch (Exception)
            {
            }
            finally
            {
                gate.Release();
            }
        });
        await Task.WhenAll(tasks);
        _checking = false;
        if (IsVisible) Build();
        Main.RebuildMenu();
    }

    public override void Returned() => Build();
}

/// Thể loại / Quốc gia đủ danh sách (trang Next.js kiểu PhimMoi, trang API kiểu mephim).
public sealed class FilterPage : Page
{
    private readonly bool _genres;

    public FilterPage(bool genres)
    {
        _genres = genres;
        MenuKey = genres ? "genres" : "countries";
    }

    protected override async Task Load()
    {
        Loading();
        var b = Prefs.BaseUrl;
        var f = ApiSite.IsKnown(b) ? ApiSite.Filters(b) : await NextSite.Filters(b);
        var list = _genres ? f.Genres : f.Countries;
        Body.Children.Clear();
        Body.Children.Add(Ui.Header(_genres ? "Thể loại" : "Quốc gia"));
        var grid = new WrapPanel();
        foreach (var c in list) grid.Children.Add(Ui.Pill(c.Title, () => Main.Push(new ListPage(c.Title, c.Url) { MenuKey = MenuKey }), minWidth: 150));
        Body.Children.Add(grid);
    }
}
