using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Chi tiết phim: ảnh, thông tin, nút Xem tiếp / Xem từ đầu / Theo dõi, các bản (Vietsub / Thuyết Minh) và danh sách tập.
public sealed class DetailPage : Page
{
    private readonly string _url;
    private readonly Film? _hint;
    private FilmDetail? _d;
    private EpisodeGroup? _group;
    /// Các nguồn có phim này (mở từ trang chủ tổng hợp) — có từ 2 nguồn thì có nút đổi nguồn.
    private readonly List<FeedItem> _links;
    /// Các hàng «Phim liên quan» (Related) — tải sau khi trang phim đã hiện (null = chưa xong).
    private List<Related.Row>? _related;
    private bool _relatedLoading;
    private WrapPanel? _buttons;
    private StackPanel? _relatedPanel;

    public DetailPage(string url, Film? hint = null, List<FeedItem>? links = null)
    {
        _url = url;
        _hint = hint;
        _links = links ?? new();
    }

    protected override async Task Load()
    {
        Loading(_hint != null ? $"Đang tải «{_hint.Title}»…" : "Đang tải phim…");
        var d = Site.CachedDetail(_url) ?? await Site.Detail(_url);
        _d = d;
        if (Prefs.FollowOf(d.Url) != null && d.LatestEpisode is { } last) Prefs.UpdateFollow(d.Url, d.EpisodeCount, last.Title, false);
        HomeFeed.MarkSeen(d); // trang chủ tổng hợp: hết báo «tập mới» tới khi có tập sau
        var h = Prefs.HistoryFor(d.Url);
        _group = (h != null ? d.GroupOf(h.EpUrl) : null) ?? d.Groups.FirstOrDefault();
        Build(true);
    }

    public override void Returned()
    {
        if (_d == null) return;
        var h = Prefs.HistoryFor(_d.Url);
        if (h != null && _d.GroupOf(h.EpUrl) is { } g) _group = g;
        Build(false);
    }

    private void Build(bool focus)
    {
        var d = _d!;
        Body.Children.Clear();
        var top = new Grid();
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var poster = new Border
        {
            Width = 230, Height = 345, CornerRadius = new CornerRadius(12), Background = Ui.Res("Surface2"),
            Margin = new Thickness(0, 0, 30, 20), VerticalAlignment = VerticalAlignment.Top, ClipToBounds = true,
        };
        var img = new Image { Stretch = Stretch.UniformToFill };
        poster.Child = img;
        ImageLoader.SetUrl(img, d.Poster ?? _hint?.Poster);
        top.Children.Add(poster);

        var info = new StackPanel { MaxWidth = 980, HorizontalAlignment = HorizontalAlignment.Left };
        Grid.SetColumn(info, 1);
        info.Children.Add(Ui.Text(d.Title, 30, weight: FontWeights.Bold));
        if (d.OtherTitle != null) info.Children.Add(Ui.Dim(d.OtherTitle, 17, new Thickness(0, 4, 0, 0)));
        if (d.Info != null) info.Children.Add(Ui.Dim(d.Info, 15, new Thickness(0, 10, 0, 0)));
        info.Children.Add(Ui.Dim(Site.HostOf(d.Url) ?? "", 13, new Thickness(0, 6, 0, 0)));

        var buttons = new WrapPanel { Margin = new Thickness(0, 18, 0, 0) };
        _buttons = buttons;
        var movie = IsMovie(d);
        Button? main = null;
        var h = Prefs.HistoryFor(d.Url);
        var eps = _group?.Episodes ?? new List<Episode>();
        if (h != null && d.GroupOf(h.EpUrl) != null)
        {
            var pos = Prefs.PositionOf(h.EpUrl);
            var label = Prefs.Finished(h.EpUrl) ? $"▶  Xem tiếp (sau {h.EpTitle})" : $"▶  Xem tiếp {h.EpTitle}" + (pos > 0 ? $" ({Util.FormatTime(pos)})" : "");
            main = Ui.Pill(label, () => Main.Play(d.Url, h.EpUrl, true), "active");
            buttons.Children.Add(main);
        }
        if (eps.Count > 0)
        {
            var first = Ui.Pill(eps.Count == 1 ? "▶  Xem phim" : $"▶  Xem từ {eps[0].Title}", () => Main.Play(d.Url, eps[0].Url), main == null ? "active" : null);
            buttons.Children.Add(first);
            main ??= first;
            if (eps.Count > 2) buttons.Children.Add(Ui.Pill($"Mới nhất: {eps[^1].Title}", () => Main.Play(d.Url, eps[^1].Url)));
        }
        // Phim có ở nhiều nguồn (trang chủ tổng hợp): «Nguồn: … ▾» → chọn nguồn khác (mở lại trang phim)
        if (_links.Count >= 2) buttons.Children.Add(Ui.Pill($"⇄  Nguồn: {SourceName(_url)} ▾", () => _ = PickSource()));
        var followed = Prefs.FollowOf(d.Url) != null;
        buttons.Children.Add(Ui.Pill(followed ? "★  Đang theo dõi" : "☆  Theo dõi", () =>
        {
            if (Prefs.FollowOf(d.Url) != null)
            {
                Prefs.Unfollow(d.Url);
                Main.Toast("Đã bỏ theo dõi.");
            }
            else
            {
                Prefs.FollowFilm(d);
                Main.Toast("Đã theo dõi — có tập mới sẽ hiện ở Trang chủ và mục Theo dõi.");
            }
            Main.RebuildMenu();
            SyncService.SyncSoon();
            Build(false);
        }));
        if (!movie && _related is { Count: > 0 }) buttons.Children.Add(RelatedButton());
        buttons.Children.Add(Ui.Pill("↗  Mở trên trình duyệt", () => Ui.OpenBrowser(d.Url)));
        info.Children.Add(buttons);
        if (d.Description != null)
        {
            var desc = Ui.Text(d.Description, 15.5, Ui.Res("TextDim"), margin: new Thickness(0, 10, 0, 0));
            desc.LineHeight = 23;
            info.Children.Add(desc);
        }
        top.Children.Add(info);
        Body.Children.Add(top);

        if (d.Groups.Count > 1)
        {
            Body.Children.Add(Ui.Header("Bản", new Thickness(0, 8, 0, 12)));
            var groups = new WrapPanel();
            foreach (var g in d.Groups)
            {
                groups.Children.Add(Ui.Pill($"{g.Name} ({g.Episodes.Count})", () =>
                {
                    _group = g;
                    Build(false);
                }, g == _group ? "active" : null));
            }
            Body.Children.Add(groups);
        }

        if (d.Groups.Count == 0)
        {
            Body.Children.Add(Ui.Dim("Phim này chưa có tập nào.", 16, new Thickness(0, 10, 0, 0)));
        }
        else if (!movie)
        {
            Body.Children.Add(Ui.Header(eps.Count > 1 ? $"Danh sách tập ({eps.Count})" : "Tập phim", new Thickness(0, 14, 0, 12)));
            var watched = Prefs.WatchedEpisodes();
            var grid = new WrapPanel();
            foreach (var e in eps)
            {
                var isLast = h?.EpUrl == e.Url;
                var text = (Prefs.Finished(e.Url) ? "✓ " : "") + e.Title;
                var b = Ui.Pill(text, () => Main.Play(d.Url, e.Url), isLast ? "active" : watched.Contains(e.Url) ? "watched" : null, 104);
                b.ToolTip = e.Title;
                grid.Children.Add(b);
            }
            Body.Children.Add(grid);
        }
        if (_related is { Count: > 0 }) Body.Children.Add(RelatedPanel(_related));
        if (focus && main != null)
            Dispatcher.BeginInvoke(() => KeyNav.FocusAndShow(main), DispatcherPriority.Loaded);
        if (_related == null && !_relatedLoading)
        {
            _relatedLoading = true;
            _ = LoadRelated(d);
        }
    }

    /// Phim lẻ (một bản, một tập): không cần ô tập — nút Xem là đủ, hàng phim liên quan lên ngay dưới.
    private static bool IsMovie(FilmDetail d) => d.Groups.Count == 1 && d.EpisodeCount == 1;

    /// Tải các hàng phim liên quan rồi thêm vào trang đang hiện (không dựng lại — giữ ô đang chọn).
    private async Task LoadRelated(FilmDetail d)
    {
        List<Related.Row> rows;
        try { rows = await Related.Load(d.Title, d.Url); }
        catch (Exception) { rows = new(); }
        _related = rows;
        if (rows.Count == 0 || _d != d) return;
        Body.Children.Add(RelatedPanel(rows));
        if (!IsMovie(d) && _buttons != null) _buttons.Children.Insert(Math.Max(0, _buttons.Children.Count - 1), RelatedButton());
    }

    /// Nút «▼ Phim liên quan» (phim bộ): cuộn xuống hàng phim liên quan đầu tiên, chọn phim đầu.
    private Button RelatedButton() => Ui.Pill("▼  Phim liên quan", () =>
    {
        if (_relatedPanel != null && KeyNav.First(_relatedPanel) is { } first) KeyNav.FocusAndShow(first);
    });

    /// Các hàng phim liên quan: dải vuốt ngang như trang chủ (phim ở trang khác ghi tên trang ở góc).
    private StackPanel RelatedPanel(List<Related.Row> rows)
    {
        var host = Site.HostOf(_url);
        var panel = new StackPanel { Margin = new Thickness(0, 18, 0, 0) };
        foreach (var row in rows)
        {
            var cards = row.Films.Select(f =>
            {
                var other = Site.HostOf(f.Url) is { } h && h != host ? Site.Tag(h, Prefs.Sites) : null;
                return (UIElement)FilmCard(f, () => Main.Push(new DetailPage(f.Url, f)), sub: other);
            }).ToList();
            panel.Children.Add(Ui.Strip(Ui.Text(row.Title, 21, weight: FontWeights.SemiBold, wrap: false), cards));
        }
        _relatedPanel = panel;
        return panel;
    }

    private static string SourceName(string url) => Site.HostOf(url) is { } host ? Site.Tag(host, Prefs.Sites) : url;

    /// Hộp chọn nguồn (kiểu hộp thoại của app, đi bằng phím mũi tên): chọn nguồn khác thì mở trang phim ở nguồn đó.
    private async Task PickSource()
    {
        var list = new StackPanel();
        Button? other = null;
        for (var i = 0; i < _links.Count; i++)
        {
            var l = _links[i];
            var idx = i;
            var cur = l.Url == _url;
            var b = Ui.Pill(SourceName(l.Url) + (l.Badge != null ? $" — {l.Badge}" : "") + (cur ? "  ✓" : ""),
                () => Main.CloseDialog(100 + idx), cur ? "active" : null, 360);
            b.Margin = new Thickness(0, 0, 0, 10);
            b.HorizontalContentAlignment = HorizontalAlignment.Left;
            list.Children.Add(b);
            if (!cur) other ??= b;
        }
        var ask = Main.Ask("Mở phim ở nguồn khác", "", new[] { "Huỷ" }, list);
        // Chọn sẵn nguồn khác đầu tiên (chạy sau lệnh chọn nút «Huỷ» của hộp thoại)
        if (other != null) _ = Dispatcher.BeginInvoke(() => other.Focus(), DispatcherPriority.Loaded);
        var r = await ask;
        if (r < 100 || r - 100 >= _links.Count) return;
        var link = _links[r - 100];
        if (link.Url == _url) return;
        Main.Replace(new DetailPage(link.Url, new Film(link.Url, link.Title, link.Poster ?? _hint?.Poster, null), _links));
    }

    public override FrameworkElement? FirstTarget() => KeyNav.First(Body);
}
