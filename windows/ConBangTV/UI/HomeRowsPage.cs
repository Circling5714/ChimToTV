using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Cài đặt → Trang chủ → «Các hàng trên trang chủ» (riêng từng người xem; bản Android: HomeRowsUi.kt, iPad: HomeRowsView.swift):
/// bật / tắt từng hàng, ▲ ▼ đổi thứ tự, «＋ Thêm hàng…» (chủ đề có sẵn hoặc loại phim + nước + thể loại + ngôn ngữ, tối đa
/// HomeFeed.CustomMax), ✕ xoá hàng tự thêm, «Khôi phục mặc định» (giữ hàng tự thêm). Đổi là lưu ngay; quay về trang chủ thì xếp
/// lại, tải hàng vừa bật / vừa thêm. «Có tập mới» / «Xem tiếp» chỉ bật / tắt (luôn ở đầu trang).
public sealed class HomeRowsPage : Page
{
    /// Hàng vừa thêm ở trang «Thêm hàng» (quay lại thì chọn sẵn hàng đó).
    internal static string? Added;

    public HomeRowsPage()
    {
        MenuKey = "settings";
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    public override void Returned()
    {
        var added = Added;
        Added = null;
        Build(added != null ? "t:" + added : "add");
    }

    private static bool Kids => Prefs.Kids;

    private static bool Visible(string id) =>
        id.StartsWith("custom-") || (Kids ? !HomeFeed.AdultRows.Contains(id) : !HomeFeed.KidsRows.Contains(id));

    private static bool Movable(string id) => Visible(id) && !HomeFeed.HeadRows.Contains(id);

    private static string Label(string id)
    {
        if (id.StartsWith("custom-"))
            return Prefs.HomeCustom.FirstOrDefault(c => c.Key == id) is { } c ? "＋ " + HomeFeed.CustomTitle(c) : id;
        var l = HomeFeed.Rows.FirstOrDefault(x => x.Id == id).Label ?? id;
        return l.Replace("Vì …", $"Vì {Prefs.CurrentProfile.Name}");
    }

    private static void Save(IEnumerable<(string Id, bool On)> p) => Prefs.HomeRows = p.Select(x => x.On ? x.Id : "-" + x.Id).ToList();

    private void Toggle(string id)
    {
        Save(HomeFeed.RowPrefs().Select(x => x.Id == id ? (x.Id, !x.On) : x));
        Build("t:" + id);
    }

    private void Move(string id, bool up)
    {
        var p = HomeFeed.RowPrefs();
        var i = p.FindIndex(x => x.Id == id);
        if (i < 0) return;
        var range = up ? Enumerable.Range(0, i).Reverse() : Enumerable.Range(i + 1, p.Count - i - 1);
        var j = range.Where(k => Movable(p[k].Id)).DefaultIfEmpty(-1).First();
        if (j < 0) return;
        (p[i], p[j]) = (p[j], p[i]);
        Save(p);
        Build((up ? "u:" : "d:") + id);
    }

    private async Task Remove(string id)
    {
        if (Prefs.HomeCustom.FirstOrDefault(c => c.Key == id) is not { } c) return;
        if (!await Main.Confirm($"Xoá hàng «{HomeFeed.CustomTitle(c)}»?", "", "Xoá")) return;
        Prefs.HomeCustom = Prefs.HomeCustom.Where(x => x.Key != id).ToList();
        Save(HomeFeed.RowPrefs().Where(x => x.Id != id));
        Build("add");
    }

    private static Button Cell(Grid g, int col, string text, string uid, bool enabled, Action onClick)
    {
        var b = Ui.Pill(text, onClick, minWidth: 58);
        b.Uid = uid;
        b.IsEnabled = enabled;
        b.Opacity = enabled ? 1 : 0.3;
        b.Margin = new Thickness(0, 0, 10, 0);
        Grid.SetColumn(b, col);
        g.Children.Add(b);
        return b;
    }

    private void Build(string? focus = null)
    {
        Body.Children.Clear();
        var who = Prefs.Profiles.Count >= 2 ? $" — {Prefs.CurrentProfile.Name}" : "";
        Body.Children.Add(Ui.Header($"Các hàng trên trang chủ{who}"));
        Body.Children.Add(Ui.Dim("Hàng tắt thì không tải. «Có tập mới» và «Xem tiếp» luôn ở đầu trang; hàng nào không đủ phim thì tự ẩn." +
                                 (Kids ? " Hồ sơ trẻ em: chỉ các hàng hoạt hình hợp trẻ em." : ""), 13.5, new Thickness(0, 0, 0, 14)));
        var shown = HomeFeed.RowPrefs().Where(x => Visible(x.Id)).ToList();
        var ordered = shown.Where(x => HomeFeed.HeadRows.Contains(x.Id)).Concat(shown.Where(x => !HomeFeed.HeadRows.Contains(x.Id))).ToList();
        var others = ordered.Where(x => !HomeFeed.HeadRows.Contains(x.Id)).ToList();
        foreach (var (id, on) in ordered)
        {
            var g = new Grid { Margin = new Thickness(0, 0, 0, 8), HorizontalAlignment = HorizontalAlignment.Left };
            g.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(560) });
            for (var c = 0; c < 3; c++) g.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var t = Ui.Pill((on ? "✓  " : "✗  ") + Label(id) + (on ? "" : "  (tắt)"), () => Toggle(id));
            t.Uid = "t:" + id;
            t.HorizontalAlignment = HorizontalAlignment.Stretch;
            t.HorizontalContentAlignment = HorizontalAlignment.Left;
            t.Opacity = on ? 1 : 0.6;
            t.Margin = new Thickness(0, 0, 12, 0);
            g.Children.Add(t);
            if (!HomeFeed.HeadRows.Contains(id))
            {
                var at = others.FindIndex(x => x.Id == id);
                Cell(g, 1, "▲", "u:" + id, at > 0, () => Move(id, true));
                Cell(g, 2, "▼", "d:" + id, at < others.Count - 1, () => Move(id, false));
                if (id.StartsWith("custom-")) Cell(g, 3, "✕", "x:" + id, true, () => _ = Remove(id));
            }
            Body.Children.Add(g);
        }
        var n = Prefs.HomeCustom.Count;
        var add = Ui.Pill(n < HomeFeed.CustomMax ? "＋  Thêm hàng…" : $"Đã đủ {HomeFeed.CustomMax} hàng tự thêm", () =>
        {
            if (Prefs.HomeCustom.Count < HomeFeed.CustomMax) Main.Push(new AddHomeRowPage());
        });
        add.Uid = "add";
        add.Opacity = n < HomeFeed.CustomMax ? 1 : 0.5;
        var reset = Ui.Pill("Khôi phục mặc định", () =>
        {
            Prefs.HomeRows = new();
            Build("t:new-eps");
        });
        var row = Ui.Row(add, reset);
        row.Margin = new Thickness(0, 10, 0, 6);
        Body.Children.Add(row);
        Body.Children.Add(Ui.Dim("Hàng tự thêm: chủ đề có sẵn, hoặc tự chọn loại phim, nước, thể loại, ngôn ngữ. «Khôi phục mặc định» trả " +
                                 "thứ tự và bật lại mọi hàng, giữ hàng tự thêm.", 13.5));
        if (focus != null) FocusUid(focus);
    }

    private void FocusUid(string uid) =>
        Dispatcher.BeginInvoke(() =>
        {
            var targets = KeyNav.Targets(Body).ToList();
            var el = targets.FirstOrDefault(x => x.Uid == uid && x.IsEnabled)
                     ?? (uid.Length > 2 ? targets.FirstOrDefault(x => x.Uid == "t:" + uid[2..]) : null);
            if (el != null) KeyNav.FocusAndShow(el);
        }, DispatcherPriority.Loaded);
}

/// «＋ Thêm hàng…»: chủ đề có sẵn, hoặc tự chọn loại phim / nước / thể loại / ngôn ngữ. Hồ sơ trẻ em chỉ hoạt hình.
public sealed class AddHomeRowPage : Page
{
    private string _type = "hoathinh", _country = "", _genre = "", _lang = "";

    public AddHomeRowPage()
    {
        MenuKey = "settings";
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private CustomRow Row => new($"c{Util.NowMs()}", null, _type, _country, _genre, _lang);

    private void Build(string? focus = null)
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Thêm hàng trên trang chủ"));
        var kids = Prefs.Kids;
        var existing = Prefs.HomeCustom;
        var topics = HomeFeed.Topics.Where(t => (!kids || HomeFeed.KidsTopics.Contains(t.Id)) && existing.All(c => c.Topic != t.Id)).ToList();
        if (topics.Count > 0)
        {
            Body.Children.Add(Ui.Text("Chủ đề có sẵn", 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 4, 0, 8)));
            var wrap = new WrapPanel();
            foreach (var t in topics)
            {
                var b = Ui.Pill(t.Label, () => Add(new CustomRow($"t-{t.Id}", t.Id)));
                b.Margin = new Thickness(0, 0, 12, 12);
                wrap.Children.Add(b);
            }
            Body.Children.Add(wrap);
        }
        Body.Children.Add(Ui.Text("Tự chọn", 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 10, 0, 8)));
        var types = (kids ? HomeFeed.Types.Take(1) : HomeFeed.Types).Select(t => (t.Label, t.Id)).ToList();
        var genres = ApiSite.GenreList.Where(g => !kids || !HomeFeed.AdultGenres.Contains(g.Slug)).Select(g => (g.Name, g.Slug));
        Field("Loại phim", types, _type, v => _type = v);
        Field("Nước", new[] { ("Mọi nước", "") }.Concat(ApiSite.CountryList.Select(c => (c.Name, c.Slug))).ToList(), _country, v => _country = v);
        Field("Thể loại", new[] { ("Mọi thể loại", "") }.Concat(genres).ToList(), _genre, v => _genre = v);
        Field("Ngôn ngữ", HomeFeed.Langs.Select(l => (l.Label, l.Id)).ToList(), _lang, v => _lang = v);
        var add = Ui.Pill($"＋  Thêm hàng «{HomeFeed.CustomTitle(Row)}»", () => Add(Row), "active");
        add.Uid = "add";
        add.HorizontalAlignment = HorizontalAlignment.Left;
        add.Margin = new Thickness(0, 8, 0, 10);
        Body.Children.Add(add);
        Body.Children.Add(Ui.Dim("Phim lấy từ KKPhim (lọc được cả bốn mục), không có KKPhim thì VSMOV; phim cùng tên ở các trang khác tự gộp vào.", 13.5));
        if (focus != null)
            Dispatcher.BeginInvoke(() =>
            {
                if (KeyNav.Targets(Body).FirstOrDefault(x => x.Uid == focus) is { } el) KeyNav.FocusAndShow(el);
            }, DispatcherPriority.Loaded);
    }

    /// Một dòng chọn: bấm mở hộp chọn của app (đi bằng phím), chọn xong vẽ lại, giữ tiêu điểm ở dòng đó.
    private void Field(string title, List<(string Name, string Slug)> options, string value, Action<string> set)
    {
        var name = options.FirstOrDefault(o => o.Slug == value).Name ?? value;
        var b = Ui.Pill($"{title}: {name}  ▾", async () =>
        {
            var list = new WrapPanel { MaxWidth = 760 };
            for (var i = 0; i < options.Count; i++)
            {
                var idx = i;
                var p = Ui.Pill(options[i].Name, () => Main.CloseDialog(100 + idx), options[i].Slug == value ? "active" : null);
                p.Margin = new Thickness(0, 0, 10, 10);
                list.Children.Add(p);
            }
            var task = Main.Ask(title, "", new[] { "Huỷ" }, list);
            // Chọn sẵn mục đang dùng (hộp thoại tự chọn nút «Huỷ» trước)
            _ = Dispatcher.BeginInvoke(() =>
            {
                if (list.Children.OfType<Button>().FirstOrDefault(x => (string?)x.Tag == "active") is { } a) a.Focus();
            }, DispatcherPriority.Loaded);
            var r = await task;
            if (r < 100) return;
            set(options[r - 100].Slug);
            Build("f:" + title);
        });
        b.Uid = "f:" + title;
        b.HorizontalAlignment = HorizontalAlignment.Left;
        b.Margin = new Thickness(0, 0, 0, 8);
        Body.Children.Add(b);
    }

    private void Add(CustomRow c)
    {
        var cur = Prefs.HomeCustom;
        if (cur.Any(x => x.Same(c)))
        {
            Main.Toast($"Đã có hàng «{HomeFeed.CustomTitle(c)}».");
            return;
        }
        if (cur.Count >= HomeFeed.CustomMax) return;
        Prefs.HomeCustom = cur.Append(c).ToList();
        HomeRowsPage.Added = c.Key;
        Main.Back();
    }
}
