using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Trang chủ. Kiểu tổng hợp (từ 1.5, mặc định): phim từ mọi nguồn trong danh sách trang, gộp phim trùng, xếp hàng theo lịch
/// sử xem của người xem (HomeFeed) — hiện ngay trang chủ lần trước, tải mới mọi nguồn (mỗi nguồn tối đa 6 giây) rồi thay vào;
/// nguồn lỗi / chậm ghi ở dòng trạng thái. Kiểu theo trang: Xem tiếp, phim theo dõi có tập mới, rồi các nhóm phim của trang.
public sealed class HomePage : Page
{
    private Home? _home;
    private string _base = "";
    private bool _mix;
    private bool _kids;
    /// Cài đặt các hàng lúc tải (thứ tự / bật tắt + hàng tự thêm + nguồn tắt): đổi thì tải lại.
    private string _rows = "";
    private HomeFeed.Snapshot? _snap;

    /// Số phim tối đa một dải vuốt ngang (còn lại xem ở «Xem thêm»).
    private const int StripMax = 12;

    private static string RowsSignature() =>
        string.Join(",", Prefs.HomeRows) + "|" + string.Join(",", Prefs.HomeCustom.Select(c => $"{c.Id};{c.Topic};{c.Type};{c.Country};{c.Genre};{c.Lang}")) +
        "|" + string.Join(",", Prefs.HomeSrcOff.OrderBy(x => x));
    private long _feedAt;
    private int _run;
    private TextBlock? _note;

    public HomePage()
    {
        MenuKey = "home";
    }

    protected override async Task Load()
    {
        _mix = Prefs.HomeMixOn;
        _kids = Prefs.Kids;
        _base = Prefs.BaseUrl;
        _rows = RowsSignature();
        if (_mix)
        {
            await LoadMix();
            return;
        }
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

    // ---- Kiểu tổng hợp ----

    private async Task LoadMix()
    {
        var run = ++_run;
        var pid = Prefs.ProfileId;
        var shown = _snap != null; // làm mới: giữ trang đang hiện tới khi có bản mới
        if (!shown)
        {
            Loading("Đang tải trang chủ…");
            if (await Task.Run(() => HomeFeed.Last(pid)) is { } last && run == _run) shown = Show(last);
        }
        var plan = await Task.Run(() => HomeFeed.MakePlan());
        if (run != _run) return;
        SetNote($"Đang làm mới {plan.Src.Count} nguồn…");
        if (!shown)
        {
            Loading($"Đang tải trang chủ từ {plan.Src.Count} nguồn…");
            if (await Task.Run(() => HomeFeed.Cached(plan)) is { } c && run == _run) shown = Show(c);
        }
        var snap = await Task.Run(() => HomeFeed.Load(plan));
        if (run != _run) return;
        _feedAt = Util.NowMs();
        if (!Show(snap) && !shown) ShowError("Không tải được phim từ nguồn nào.\n" + snap.Note);
        _ = FollowLate(run, snap);
    }

    /// Nguồn chậm về muộn: cứ 4 giây xem bản lưu của chúng đã mới chưa, có thì dựng lại; thôi sau 5 lần.
    private async Task FollowLate(int run, HomeFeed.Snapshot snap)
    {
        for (var i = 0; i < 5 && snap.Pending; i++)
        {
            await Task.Delay(4000);
            if (run != _run) return;
            var next = await Task.Run(() => HomeFeed.Late(snap));
            if (run != _run) return;
            if (next != null)
            {
                Show(next);
                snap = next;
            }
        }
        if (run == _run && snap.Pending) SetNote(snap.NoteGaveUp);
    }

    private void SetNote(string text)
    {
        if (_note != null) _note.Text = (_kids ? "Hồ sơ trẻ em · " : "") + text;
    }

    /// Hiện một lần dựng (giữ ô đang chọn). false nếu không có gì để hiện.
    private bool Show(HomeFeed.Snapshot s)
    {
        _snap = s;
        if (s.MovedTo is { } m && m.Old == Prefs.BaseUrl && Prefs.NormalizeUrl(m.New) is { } nu)
        {
            Prefs.ReplaceSite(m.Old, nu);
            _base = nu;
            Main.Toast($"Trang đã chuyển sang {Site.HostOf(nu)} — app đã đổi theo.");
        }
        if (s.Categories is { Count: > 0 } cats) Main.SetCategories(cats);
        var focus = (Keyboard.FocusedElement as UIElement)?.Uid;
        var any = BuildMix();
        if (!string.IsNullOrEmpty(focus)) FocusUid(focus);
        return any;
    }

    private void FocusUid(string uid) =>
        Dispatcher.BeginInvoke(() =>
        {
            if (KeyNav.Targets(Body).FirstOrDefault(x => x.Uid == uid) is { } el) KeyNav.FocusAndShow(el);
        }, DispatcherPriority.Loaded);


    /// Một hàng vuốt ngang của trang chủ tổng hợp; rowId: id hàng (nhấn phải / phím Menu / M trên phim hoặc tên hàng → sửa nhanh).
    private UIElement Strip(string title, string rowId, List<UIElement> cards, Action? more = null)
    {
        var t = Ui.Text(title, 21, weight: FontWeights.SemiBold, wrap: false);
        t.MouseRightButtonUp += (_, e) =>
        {
            e.Handled = true;
            _ = RowMenu(rowId);
        };
        foreach (var c in cards.OfType<Button>()) MenuOn(c, rowId);
        return Ui.Strip(t, cards, more);
    }

    /// Nhấn phải / phím Menu (Apps) / Shift + F10 / M trên ô → sửa nhanh hàng.
    private void MenuOn(Button b, string rowId)
    {
        b.MouseRightButtonUp += (_, e) =>
        {
            e.Handled = true;
            _ = RowMenu(rowId);
        };
        b.PreviewKeyDown += (_, e) =>
        {
            var k = e.Key == Key.System ? e.SystemKey : e.Key;
            if (k is not (Key.Apps or Key.M) && !(k == Key.F10 && Keyboard.Modifiers == ModifierKeys.Shift)) return;
            e.Handled = true;
            _ = RowMenu(rowId);
        };
    }

    /// <summary>
    /// Sửa nhanh một hàng trang chủ: ẩn, ▲ chuyển lên / ▼ chuyển xuống (qua hàng đang hiện kề nó), «Các hàng…», «Nguồn…».
    /// Hồ sơ trẻ em: hỏi mã PIN phụ huynh trước (nếu đã đặt).
    /// </summary>
    private async Task RowMenu(string rowId)
    {
        if (Prefs.Kids && Prefs.HasParentPin && !await Main.CheckParent("Nhập mã PIN phụ huynh để sửa trang chủ.")) return;
        var arranged = _snap != null && _mix ? HomeFeed.ArrangeSlots(_snap.Rows) : new();
        string title, slot;
        if (rowId == "new-eps") (title, slot) = ("Có tập mới", rowId);
        else if (rowId == "continue") (title, slot) = ("Xem tiếp", rowId);
        else
        {
            var r = arranged.FirstOrDefault(x => x.Row.Id == rowId);
            if (r.Row == null || r.Slot == "topics") return;
            (title, slot) = (r.Row.Title, r.Slot);
        }
        var shown = arranged.Select(x => x.Slot).ToList();
        var prefs = HomeFeed.RowPrefs();
        var at = arranged.FindIndex(x => x.Row.Id == rowId);
        var next = at < 0 ? null : (at + 1 < arranged.Count ? arranged[at + 1] : at > 0 ? arranged[at - 1] : default).Row;
        var opts = new List<(string Label, Action Run)>
        {
            ("Ẩn hàng này", () =>
            {
                Apply(HomeFeed.Hide(prefs, slot), next?.Cards.FirstOrDefault()?.Film.Url);
                Main.Toast($"Đã ẩn hàng «{title}» — bật lại trong Cài đặt → Các hàng trên trang chủ.");
            }),
        };
        if (HomeFeed.MoveShown(prefs, shown, slot, true) is { } up) opts.Add(("▲  Chuyển lên", () => Apply(up, null)));
        if (HomeFeed.MoveShown(prefs, shown, slot, false) is { } down) opts.Add(("▼  Chuyển xuống", () => Apply(down, null)));
        opts.Add(("☰  Các hàng…", () => Main.Push(new HomeRowsPage())));
        if (_mix) opts.Add(("Nguồn…", () => Main.Push(new HomeSourcesPage())));
        var focus = (Keyboard.FocusedElement as UIElement)?.Uid;
        var i = await Main.Ask($"Hàng «{title}»", "", opts.Select(o => o.Label).Append("Huỷ").ToArray());
        if (i < 0 || i >= opts.Count) return;
        _pendingFocus = focus;
        opts[i].Run();
    }

    /// Ô cần chọn lại sau khi sửa nhanh (uid ô đang chọn lúc mở bảng).
    private string? _pendingFocus;

    /// Lưu cài đặt các hàng rồi xếp lại từ lần tải gần nhất (không tải lại); focus: ô cần chọn (null = ô đang chọn).
    private void Apply(List<(string Id, bool On)> prefs, string? focus)
    {
        HomeFeed.SaveRows(prefs);
        _rows = RowsSignature();
        var uid = focus ?? _pendingFocus;
        if (_mix) BuildMix(); else Build();
        if (!string.IsNullOrEmpty(uid)) FocusUid(uid);
        else Dispatcher.BeginInvoke(() => FocusFirst(), DispatcherPriority.Loaded);
    }

    /// Dựng trang chủ tổng hợp từ lần tải gần nhất: Có tập mới, Xem tiếp, (chọn chủ đề), mỗi hàng một dải vuốt ngang (tới
    /// StripMax phim, «Xem thêm» hẹp cuối dải nếu còn).
    private bool BuildMix()
    {
        var s = _snap;
        if (s == null) return false;
        Body.Children.Clear();
        _note = Ui.Dim((_kids ? "Hồ sơ trẻ em · " : "") + s.Note, 13.5, new Thickness(0, 0, 0, 10));
        Body.Children.Add(_note);
        var count = 0;

        // Có tập mới: phim theo dõi + phim đang xem dở có tập mới ở bất kỳ nguồn nào (đã mở phim từ lúc dựng trang thì thôi)
        var follows = Prefs.Follows.Where(f => f.NewCount > 0).ToList();
        var followed = Prefs.Follows.Select(f => KkPhim.Norm(f.Title)).ToHashSet();
        var auto = s.Fresh.Where(a => (Prefs.SeenEp(a.Key) ?? -1) < a.Ep && !followed.Contains(a.Key)).ToList();
        // «Có tập mới» / «Xem tiếp» người xem tắt được (Cài đặt → Trang chủ → Các hàng)
        var on = HomeFeed.RowPrefs().Where(x => x.On).Select(x => x.Id).ToHashSet();
        if (follows.Count + auto.Count > 0 && on.Contains("new-eps"))
        {
            Body.Children.Add(Strip("Có tập mới", "new-eps", follows.Select(f => (UIElement)Tagged(FilmCard(new Film(f.FilmUrl, f.Title, f.Poster, f.Latest.NonEmpty()), fresh: $"+{f.NewCount}"), f.FilmUrl))
                .Concat(auto.Select(a => (UIElement)CardButton(a.Card))).ToList()));
            count++;
        }
        var history = on.Contains("continue") ? Prefs.History.Take(StripMax).ToList() : new();
        if (history.Count > 0)
        {
            Body.Children.Add(Strip("Xem tiếp", "continue", history.Select(h =>
                (UIElement)Tagged(FilmCard(new Film(h.FilmUrl, h.Title, h.Poster, null), () => Main.Play(h.FilmUrl, h.EpUrl, true), h.EpTitle), "resume:" + h.FilmUrl)).ToList()));
            count++;
        }
        // Người xem chưa có lịch sử: chọn chủ đề
        if (Prefs.History.Count == 0 && !_kids)
        {
            var chosen = Prefs.HomeTopics;
            Body.Children.Add(Ui.Section("Chọn vài chủ đề bạn thích — trang chủ sẽ gợi ý theo đó", HomeFeed.Topics.Select(t =>
            {
                var b = Ui.Pill((chosen.Contains(t.Id) ? "✓ " : "") + t.Label, () => ToggleTopic(t.Id), chosen.Contains(t.Id) ? "active" : null, 200);
                b.Margin = new Thickness(0, 0, 12, 12);
                b.Uid = "chip:" + t.Id;
                return (UIElement)b;
            })));
        }
        // Thứ tự / bật tắt theo cài đặt hiện tại của người xem (bản lưu dựng trước khi đổi cũng xếp đúng ngay)
        foreach (var r in HomeFeed.Arrange(s.Rows))
        {
            var n = r.Cards.Count;
            var cards = r.Cards.Take(StripMax).Select(c => (UIElement)CardButton(c)).ToList();
            Action? more = null;
            if (n > StripMax || r.More != null)
            {
                var row = r;
                more = () => OpenMore(row);
                // Hàng có danh sách nhiều trang của nguồn: không ghi số phim (mới tải trang đầu)
                var tile = Ui.MoreTile(more, r.More == null ? $"{n} phim" : null);
                tile.Uid = "more:" + r.Id;
                cards.Add(tile);
            }
            Body.Children.Add(Strip(r.Title, r.Id, cards, more));
            count++;
        }
        return count > 0;
    }

    private static Button Tagged(Button b, string uid)
    {
        b.Uid = uid;
        return b;
    }

    /// Nhãn nguồn («3 nguồn», «có ở nguonc», «bản lưu») ở dòng phụ dưới tên phim (thẻ hẹp: góc trên đã có nhãn tập).
    private Button CardButton(Card c) => Tagged(FilmCard(c.Film, () => Main.Push(new DetailPage(c.Film.Url, c.Film, c.Links)), sub: c.Chip), c.Film.Url);

    /// «Xem thêm»: cả hàng (danh sách có phân trang của nguồn nếu có, không thì mọi phim đã gộp). Esc / Backspace quay lại.
    private void OpenMore(FeedRow r)
    {
        if (r.More != null) Main.Push(new ListPage(r.Title, r.More) { MenuKey = "home" });
        else Main.Push(new CardsPage(r.Title, r.Cards));
    }

    /// Chọn / bỏ một chủ đề rồi tải các hàng theo chủ đề.
    private void ToggleTopic(string id)
    {
        var set = Prefs.HomeTopics;
        if (!set.Remove(id)) set.Add(id);
        Prefs.HomeTopics = set;
        BuildMix();
        FocusUid("chip:" + id);
        _ = Start();
    }

    // ---- Kiểu theo trang (như trước 1.5) ----

    private void Build()
    {
        var home = _home;
        if (home == null) return;
        Body.Children.Clear();
        var on = HomeFeed.RowPrefs().Where(x => x.On).Select(x => x.Id).ToHashSet();
        var history = on.Contains("continue") ? Prefs.History.Take(12).ToList() : new();
        if (history.Count > 0)
        {
            Body.Children.Add(Ui.Section("Xem tiếp", history.Select(h =>
                (UIElement)FilmCard(new Film(h.FilmUrl, h.Title, h.Poster, null), () => Main.Play(h.FilmUrl, h.EpUrl, true), h.EpTitle)),
                "Enter: xem tiếp ngay"));
        }
        var fresh = on.Contains("new-eps") ? Prefs.Follows.Where(f => f.NewCount > 0).ToList() : new();
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
        // Đổi kiểu trang chủ / hồ sơ trẻ em / trang phim trong Cài đặt → tải lại
        if (Prefs.HomeMixOn != _mix || Prefs.Kids != _kids || Prefs.BaseUrl != _base || RowsSignature() != _rows)
        {
            _snap = null;
            _ = Start();
            return;
        }
        if (!_mix)
        {
            Build();
            return;
        }
        // Mở lâu (quay lại sau 20 phút): làm mới; không thì dựng lại hàng đầu (xem xong / vừa mở phim có tập mới)
        if (Util.NowMs() - _feedAt > 20 * 60_000)
        {
            _ = Start();
            return;
        }
        var uid = LastFocus?.Uid;
        BuildMix();
        LastFocus = null;
        // Ô mới chỉ tìm được sau khi xếp chỗ; chạy trước lệnh chọn ô của MainWindow.Back (cùng mức Loaded, xếp hàng trước)
        if (!string.IsNullOrEmpty(uid))
            Dispatcher.BeginInvoke(() =>
            {
                if (KeyNav.Targets(Body).FirstOrDefault(x => x.Uid == uid) is { } el) LastFocus = el;
            }, DispatcherPriority.Loaded);
    }
}

/// Cả một hàng của trang chủ tổng hợp («Xem thêm» của hàng gộp nhiều nguồn): phim đã gộp, kèm số nguồn.
public sealed class CardsPage : Page
{
    private readonly string _title;
    private readonly List<Card> _cards;

    public CardsPage(string title, List<Card> cards)
    {
        _title = title;
        _cards = cards;
        MenuKey = "home";
    }

    protected override Task Load()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header(_title));
        var grid = new WrapPanel();
        foreach (var c in _cards) grid.Children.Add(FilmCard(c.Film, () => Main.Push(new DetailPage(c.Film.Url, c.Film, c.Links)), sub: c.Chip));
        Body.Children.Add(grid);
        return Task.CompletedTask;
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
