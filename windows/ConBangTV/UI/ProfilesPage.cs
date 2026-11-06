using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.UI;

/// Người xem (bản TV: Cài đặt → Người xem…): mỗi người một lịch sử / Xem tiếp / theo dõi / trang phim; khoá bằng mã PIN;
/// giới hạn giờ xem (cần mã PIN phụ huynh). Khi đã có giới hạn giờ xem, thêm / xoá người xem cũng cần mã PIN phụ huynh.
public sealed class ProfilesPage : Page
{
    public ProfilesPage()
    {
        MenuKey = "profile";
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    public override void Returned() => Build();

    private void Refocus(string? text)
    {
        if (text == null) return;
        Dispatcher.BeginInvoke(() => KeyNav.Targets(Body).OfType<Button>().FirstOrDefault(b => b.Content as string == text)?.Focus(),
            DispatcherPriority.Loaded);
    }

    private void Rebuild()
    {
        var text = (System.Windows.Input.Keyboard.FocusedElement as Button)?.Content as string;
        Build();
        Main.RebuildMenu();
        Refocus(text);
    }

    private void Build()
    {
        Body.Children.Clear();
        Body.Children.Add(Ui.Header("Người xem"));
        Body.Children.Add(Ui.Dim("Mỗi người một lịch sử, Xem tiếp, chỗ đang xem, phim theo dõi và trang phim riêng. Khoá bằng mã PIN để trẻ " +
                                 "em khỏi vào nhầm; giới hạn giờ xem mỗi ngày / giờ đi ngủ cần mã PIN phụ huynh. Đồng bộ trong nhà ghép người " +
                                 "xem cùng tên (và người đầu tiên của mỗi máy).", 14, new Thickness(0, 0, 0, 14)));
        var list = Prefs.Profiles;
        var cur = Prefs.ProfileId;
        foreach (var p in list)
        {
            var row = new WrapPanel { Margin = new Thickness(0, 0, 0, 6) };
            var color = Prefs.ProfileColors[Math.Clamp(p.Color, 0, Prefs.ProfileColors.Length - 1)];
            row.Children.Add(new Border
            {
                Width = 40, Height = 40, CornerRadius = new CornerRadius(20), Margin = new Thickness(0, 2, 12, 10),
                Background = new SolidColorBrush(Color.FromRgb((byte)(color >> 16), (byte)(color >> 8), (byte)color)),
                Child = new TextBlock
                {
                    Text = p.Initial, FontSize = 18, FontWeight = FontWeights.Bold, Foreground = Brushes.White,
                    HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
                },
            });
            var name = Ui.Text(p.Name + (p.Locked ? " 🔒" : "") + (p.Id == cur ? "  (đang xem)" : ""), 17, weight: FontWeights.SemiBold, wrap: false,
                margin: new Thickness(0, 9, 14, 10));
            name.MinWidth = 170;
            row.Children.Add(name);
            var pp = p;
            if (p.Id != cur) row.Children.Add(Ui.Pill("Dùng", () => _ = Use(pp)));
            row.Children.Add(Ui.Pill("Đổi tên", () => _ = Rename(pp)));
            row.Children.Add(Ui.Pill("Màu", () => _ = PickColor(pp)));
            row.Children.Add(Ui.Pill(p.Locked ? "Bỏ khoá" : "Khoá bằng mã PIN", () => _ = ToggleLock(pp)));
            row.Children.Add(Ui.Pill($"Giới hạn: {WatchLimit.Summary(p.Id)}", () => _ = Limits(pp)));
            if (list.Count > 1) row.Children.Add(Ui.Pill("Xoá", () => _ = Delete(pp)));
            Body.Children.Add(row);
        }
        if (list.Count < Prefs.MaxProfiles) Body.Children.Add(Ui.Row(Ui.Pill("＋  Thêm người xem", () => _ = Add())));

        Body.Children.Add(Ui.Text("Hỏi «Ai đang xem?» khi mở app", 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 18, 0, 8)));
        Body.Children.Add(Ui.Row(
            Ui.Pill("Bật", () => { Prefs.AskProfile = true; Rebuild(); }, Prefs.AskProfile ? "active" : null, 90),
            Ui.Pill("Tắt", () => { Prefs.AskProfile = false; Rebuild(); }, !Prefs.AskProfile ? "active" : null, 90)));
        Body.Children.Add(Ui.Dim("Khi có từ 2 người xem; hỏi lại khi rời app hơn 30 phút. Người có khoá luôn phải nhập mã PIN.", 13.5));

        Body.Children.Add(Ui.Text("Mã PIN phụ huynh", 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 18, 0, 8)));
        var pr = new WrapPanel();
        if (!Prefs.HasParentPin)
        {
            pr.Children.Add(Ui.Pill("Đặt mã PIN phụ huynh", async () =>
            {
                if (await Main.CheckParent("")) Rebuild();
            }));
        }
        else
        {
            pr.Children.Add(Ui.Pill("Đổi mã PIN phụ huynh", async () =>
            {
                if (!await Main.CheckParent("Nhập mã PIN phụ huynh hiện tại.")) return;
                var np = await Main.AskNewPin("Mã PIN phụ huynh mới");
                if (np == null) return;
                Prefs.SetParentPin(np);
                Main.Toast("Đã đổi mã PIN phụ huynh.");
            }));
            pr.Children.Add(Ui.Pill("Xoá mã PIN phụ huynh", async () =>
            {
                if (Prefs.AnyLimits)
                {
                    Main.Toast("Đang có giới hạn giờ xem — bỏ hết giới hạn trước rồi mới xoá được mã phụ huynh.");
                    return;
                }
                if (!await Main.CheckParent("Nhập mã PIN phụ huynh để xoá.")) return;
                Prefs.SetParentPin(null);
                Rebuild();
            }));
        }
        Body.Children.Add(pr);
        Body.Children.Add(Ui.Dim("Dùng để đặt giới hạn giờ xem và cho xem thêm khi hết giờ. Khác mã PIN khoá từng người xem.", 13.5));
    }

    private async Task Use(Profile p)
    {
        if (p.Locked)
        {
            var pin = await Main.AskPin($"Mã PIN của {p.Name}", "Nhập mã PIN 4 số.");
            if (pin == null) return;
            if (!Prefs.CheckPin(p, pin))
            {
                Main.Toast("Sai mã PIN.");
                return;
            }
        }
        Main.SwitchTo(p);
    }

    private static async Task<string?> AskName(string title, string current)
    {
        var box = new TextBox { Text = current, MinWidth = 300, MaxLength = 24 };
        box.KeyDown += (_, e) =>
        {
            if (e.Key != System.Windows.Input.Key.Enter) return;
            e.Handled = true;
            Main.CloseDialog(0);
        };
        Main.Dispatcher.BeginInvoke(DispatcherPriority.Input, () =>
        {
            box.Focus();
            box.SelectAll();
        });
        var r = await Main.Ask(title, "", new[] { "Lưu", "Huỷ" }, box);
        var name = box.Text.Trim();
        return r == 0 && name.Length > 0 ? name : null;
    }

    private async Task Add()
    {
        if (Prefs.AnyLimits && !await Main.CheckParent("Đang có giới hạn giờ xem — nhập mã PIN phụ huynh để thêm người xem.")) return;
        var name = await AskName("Thêm người xem", "");
        if (name == null) return;
        Prefs.AddProfile(name);
        Main.Toast($"Đã thêm {name} — chọn «Dùng» để chuyển sang.");
        Rebuild();
        SyncService.SyncSoon();
    }

    private async Task Rename(Profile p)
    {
        if (!await Unlock(p)) return;
        var name = await AskName("Đổi tên người xem", p.Name);
        if (name == null) return;
        Prefs.RenameProfile(p.Id, name);
        Rebuild();
    }

    private async Task PickColor(Profile p)
    {
        var tiles = new WrapPanel();
        for (var i = 0; i < Prefs.ProfileColors.Length; i++)
        {
            var c = Prefs.ProfileColors[i];
            var idx = i;
            var b = Ui.Pill(Prefs.ProfileColorNames[i], () => Main.CloseDialog(100 + idx), i == p.Color ? "active" : null, 120);
            b.BorderBrush = new SolidColorBrush(Color.FromRgb((byte)(c >> 16), (byte)(c >> 8), (byte)c));
            b.Foreground = new SolidColorBrush(Color.FromRgb((byte)(c >> 16), (byte)(c >> 8), (byte)c));
            tiles.Children.Add(b);
        }
        var r = await Main.Ask($"Màu của {p.Name}", "", new[] { "Huỷ" }, tiles);
        if (r < 100) return;
        Prefs.SetProfileColor(p.Id, r - 100);
        Rebuild();
    }

    /// Người có khoá: hỏi mã PIN của người đó (hoặc mã phụ huynh) trước khi sửa.
    private static async Task<bool> Unlock(Profile p)
    {
        if (!p.Locked) return true;
        var pin = await Main.AskPin($"Mã PIN của {p.Name}", "Nhập mã PIN của người xem này (hoặc mã PIN phụ huynh).");
        if (pin == null) return false;
        if (Prefs.CheckPin(p, pin) || (Prefs.HasParentPin && Prefs.CheckParentPin(pin))) return true;
        Main.Toast("Sai mã PIN.");
        return false;
    }

    private async Task ToggleLock(Profile p)
    {
        if (p.Locked)
        {
            if (!await Unlock(p)) return;
            Prefs.SetProfilePin(p.Id, null);
            Main.Toast($"Đã bỏ khoá {p.Name}.");
        }
        else
        {
            var pin = await Main.AskNewPin($"Khoá {p.Name} bằng mã PIN");
            if (pin == null) return;
            Prefs.SetProfilePin(p.Id, pin);
            Main.Toast($"Đã khoá {p.Name} — chọn người này phải nhập mã PIN.");
        }
        Rebuild();
    }

    private async Task Limits(Profile p)
    {
        if (!await Main.CheckParent($"Nhập mã PIN phụ huynh để đặt giới hạn giờ xem cho {p.Name}.")) return;
        Main.Push(new LimitsPage(p.Id));
    }

    private async Task Delete(Profile p)
    {
        if (!await Unlock(p)) return;
        if (Prefs.AnyLimits && !await Main.CheckParent("Đang có giới hạn giờ xem — nhập mã PIN phụ huynh để xoá người xem.")) return;
        if (!await Main.Confirm($"Xoá người xem «{p.Name}»?", "Xoá cả lịch sử, Xem tiếp, phim theo dõi và cài đặt riêng của người này.", "Xoá")) return;
        var wasCurrent = p.Id == Prefs.ProfileId;
        if (wasCurrent) Main.Player?.Close();
        Prefs.DeleteProfile(p.Id);
        if (wasCurrent)
        {
            Main.SetCategories(new());
            Main.Toast($"Đang xem: {Prefs.CurrentProfile.Name}.");
        }
        Rebuild();
    }
}

/// Giới hạn giờ xem của một người xem (đã nhập mã PIN phụ huynh): số phút mỗi ngày, giờ đi ngủ.
public sealed class LimitsPage : Page
{
    private readonly string _id;

    public LimitsPage(string id)
    {
        _id = id;
        MenuKey = "profile";
    }

    protected override Task Load()
    {
        Build();
        return Task.CompletedTask;
    }

    private void Build()
    {
        var p = Prefs.Profiles.FirstOrDefault(x => x.Id == _id);
        if (p == null) return;
        Body.Children.Clear();
        Body.Children.Add(Ui.Header($"Giới hạn giờ xem — {p.Name}"));
        Body.Children.Add(Ui.Dim("Đếm thời gian phim thật sự chạy (dừng / đang tải không tính). Hết giờ: phim dừng, xem thêm phải nhập mã PIN " +
                                 "phụ huynh. Tính theo từng máy.", 14, new Thickness(0, 0, 0, 10)));
        Body.Children.Add(Ui.Text("Mỗi ngày được xem", 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 10, 0, 8)));
        var d = new WrapPanel();
        foreach (var m in WatchLimit.Daily)
        {
            var mm = m;
            d.Children.Add(Ui.Pill(WatchLimit.DailyLabel(m), () => Set(() => Prefs.SetDailyLimit(_id, mm)), Prefs.DailyLimit(_id) == m ? "active" : null, 100));
        }
        Body.Children.Add(d);
        Body.Children.Add(Ui.Text("Giờ đi ngủ (không xem được tới 6 giờ sáng)", 16, weight: FontWeights.SemiBold, margin: new Thickness(0, 14, 0, 8)));
        var b = new WrapPanel();
        foreach (var m in WatchLimit.Bedtimes)
        {
            var mm = m;
            b.Children.Add(Ui.Pill(WatchLimit.BedtimeLabel(m), () => Set(() => Prefs.SetBedtime(_id, mm)), Prefs.Bedtime(_id) == m ? "active" : null, 90));
        }
        Body.Children.Add(b);
        if (_id == Prefs.ProfileId && WatchLimit.Remaining() is { } r)
            Body.Children.Add(Ui.Dim($"Hôm nay còn {(r > 0 ? WatchLimit.MinutesText(r) : "0 phút")}.", 14, new Thickness(0, 10, 0, 0)));
        Body.Children.Add(Ui.Row(Ui.Pill("Xong", () => Main.Back())));
    }

    private void Set(Action change)
    {
        var text = (System.Windows.Input.Keyboard.FocusedElement as Button)?.Content as string;
        change();
        Build();
        if (text != null)
            Dispatcher.BeginInvoke(() => KeyNav.Targets(Body).OfType<Button>().FirstOrDefault(x => x.Content as string == text)?.Focus(),
                DispatcherPriority.Loaded);
    }
}
