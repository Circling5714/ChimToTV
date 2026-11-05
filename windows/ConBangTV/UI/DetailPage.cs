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

    public DetailPage(string url, Film? hint = null)
    {
        _url = url;
        _hint = hint;
    }

    protected override async Task Load()
    {
        Loading(_hint != null ? $"Đang tải «{_hint.Title}»…" : "Đang tải phim…");
        var d = Site.CachedDetail(_url) ?? await Site.Detail(_url);
        _d = d;
        if (Prefs.FollowOf(d.Url) != null && d.LatestEpisode is { } last) Prefs.UpdateFollow(d.Url, d.EpisodeCount, last.Title, false);
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
        else
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
        if (focus && main != null)
            Dispatcher.BeginInvoke(() => KeyNav.FocusAndShow(main), DispatcherPriority.Loaded);
    }

    public override FrameworkElement? FirstTarget() => KeyNav.First(Body);
}
