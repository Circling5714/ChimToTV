using System.IO;
using ConBangTV.Core;
using Xunit;

// Prefs / WatchLimit là tĩnh (một kho cho cả app) → các test chạy lần lượt
[assembly: CollectionBehavior(DisableTestParallelization = true)]

namespace ConBangTV.Tests;

/// Mỗi test một thư mục dữ liệu riêng.
public abstract class PrefsTest : IDisposable
{
    private readonly string _dir = Path.Combine(Path.GetTempPath(), "conbang-test-" + Guid.NewGuid().ToString("N"));

    protected PrefsTest()
    {
        Prefs.Load(_dir);
    }

    public void Dispose()
    {
        WatchLimit.Clock = Util.NowMs;
        WatchLimit.Reset();
        try { Directory.Delete(_dir, true); } catch (Exception) { }
    }

    protected static HistoryItem History(string slug) => new()
    {
        FilmUrl = $"https://yanhh3d.ee/{slug}", Title = slug, EpTitle = "Tập 1", EpUrl = $"https://yanhh3d.ee/{slug}/tap-1", Time = 1,
    };

    protected static readonly FilmDetail Film = new()
    {
        Url = "https://yanhh3d.ee/tien-nghich", Title = "Tiên Nghịch",
        Groups = new() { new EpisodeGroup("Vietsub", Enumerable.Range(1, 3).Select(i => new Episode($"Tập {i}", $"https://yanhh3d.ee/tien-nghich/tap-{i}")).ToList()) },
    };
}

/// Người xem (hồ sơ) — cùng ca với ProfilesTests.swift / ProfilesTest.kt.
public class ProfilesTests : PrefsTest
{
    [Fact]
    public void FirstProfileKeepsOldKeys()
    {
        Prefs.AddHistory(History("tien-nghich"));
        Prefs.SavePosition("https://yanhh3d.ee/tien-nghich/tap-1", 120, Film.Url);
        Prefs.BaseUrl = "https://phimmoiw.fun";
        Assert.Single(Prefs.Profiles);
        Assert.Equal("", Prefs.ProfileId);
        Assert.Equal("Cả nhà", Prefs.CurrentProfile.Name);
        Assert.Single(Prefs.History);
        Assert.Equal(120, Prefs.PositionOf("https://yanhh3d.ee/tien-nghich/tap-1"));
        Assert.Equal("https://phimmoiw.fun", Prefs.BaseUrl);
    }

    [Fact]
    public void EachProfileHasOwnHistoryFollowsAndSettings()
    {
        Prefs.AddHistory(History("tien-nghich"));
        Prefs.SavePosition("https://yanhh3d.ee/tien-nghich/tap-1", 120, Film.Url);
        Prefs.FollowFilm(Film);
        Prefs.BaseUrl = "https://hoathinh3d.ee";
        Prefs.SeekStep = 30;
        Prefs.SetServer("hoathinh3d.ee", "tm", "Thuyết Minh");
        Prefs.SetIntro(Film.Url, 0, 90);

        var bo = Prefs.AddProfile("  Bố ");
        Assert.Equal("Bố", bo.Name);
        Assert.Equal("p1", bo.Id);
        Assert.Equal("", Prefs.ProfileId); // thêm chưa chuyển sang
        Prefs.SwitchProfile(bo.Id);

        Assert.Empty(Prefs.History);
        Assert.Equal(0, Prefs.PositionOf("https://yanhh3d.ee/tien-nghich/tap-1"));
        Assert.Empty(Prefs.WatchedEpisodes());
        Assert.Empty(Prefs.Follows);
        Assert.Equal("https://hoathinh3d.ee", Prefs.BaseUrl);
        Assert.Equal(30, Prefs.SeekStep);
        Assert.Equal("tm", Prefs.ServerType("hoathinh3d.ee"));
        Assert.Equal(90, Prefs.Intro(Film.Url)?.End); // mốc nhạc mở đầu: dùng chung

        Prefs.AddHistory(History("vu-than"));
        Prefs.BaseUrl = "https://mephim.ddns.net";
        Prefs.SetServer("hoathinh3d.ee", "vs", "Vietsub");
        Prefs.SeekStep = 10;
        Prefs.SwitchProfile("");
        Assert.Equal(new[] { "tien-nghich" }, Prefs.History.Select(h => h.Title));
        Assert.Equal(120, Prefs.PositionOf("https://yanhh3d.ee/tien-nghich/tap-1"));
        Assert.Single(Prefs.Follows);
        Assert.Equal("https://hoathinh3d.ee", Prefs.BaseUrl);
        Assert.Equal("tm", Prefs.ServerType("hoathinh3d.ee"));
        Assert.Equal(30, Prefs.SeekStep);

        // Mở lại app: nhớ người xem lần trước
        Prefs.SwitchProfile(bo.Id);
        Prefs.Flush();
        Prefs.Load(Prefs.DataDir);
        Assert.Equal(bo.Id, Prefs.ProfileId);
        Assert.Equal(new[] { "vu-than" }, Prefs.History.Select(h => h.Title));
    }

    [Fact]
    public void DeleteRemovesOnlyThatProfileData()
    {
        Prefs.AddHistory(History("tien-nghich"));
        var me = Prefs.AddProfile("Mẹ");
        Prefs.SwitchProfile(me.Id);
        Prefs.AddHistory(History("vu-than"));
        Prefs.SetServer("yanhh3d.ee", "v1", "VIP 1");
        Prefs.FollowFilm(Film);
        Prefs.SetIntro(Film.Url, 0, 80);

        Prefs.DeleteProfile(me.Id);
        Assert.Equal("", Prefs.ProfileId);
        Assert.Equal(new[] { "" }, Prefs.Profiles.Select(p => p.Id));
        Assert.Equal(new[] { "tien-nghich" }, Prefs.History.Select(h => h.Title));
        Assert.Empty(Prefs.Follows);
        Assert.Equal(80, Prefs.Intro(Film.Url)?.End);
        Prefs.DeleteProfile("");
        Assert.Single(Prefs.Profiles);
    }

    [Fact]
    public void PinLockStoresOnlyHash()
    {
        var bo = Prefs.AddProfile("Bố");
        Assert.False(bo.Locked);
        Assert.True(Prefs.CheckPin(bo, ""));
        Prefs.SetProfilePin(bo.Id, "1234");
        var locked = Prefs.Profiles.First(p => p.Id == bo.Id);
        Assert.True(locked.Locked);
        Assert.DoesNotContain("1234", locked.Pin);
        Assert.True(Prefs.CheckPin(locked, "1234"));
        Assert.False(Prefs.CheckPin(locked, "4321"));
        // Cùng cách băm bản TV / iPad (đồng bộ / chuyển máy về sau không phải đặt lại)
        Assert.Equal(Prefs.PinHash("p1", "1234"), locked.Pin);
        Prefs.SetProfilePin(bo.Id, null);
        Assert.Null(Prefs.Profiles.First(p => p.Id == bo.Id).Pin);
    }

    [Fact]
    public void NewProfilesGetFreeColorsAndIds()
    {
        var a = Prefs.AddProfile("A");
        var b = Prefs.AddProfile("B");
        Assert.Equal(new[] { 0, 1, 2 }, Prefs.Profiles.Select(p => p.Color));
        Assert.Equal(new[] { "", "p1", "p2" }, Prefs.Profiles.Select(p => p.Id));
        Prefs.DeleteProfile(a.Id);
        var c = Prefs.AddProfile("C");
        Assert.Equal("p1", c.Id);
        Assert.Equal(1, c.Color);
        Prefs.RenameProfile(b.Id, "Bé Na");
        Prefs.SetProfileColor(b.Id, 6);
        var nb = Prefs.Profiles.First(p => p.Id == b.Id);
        Assert.Equal(("Bé Na", 6), (nb.Name, nb.Color));
    }

    [Fact]
    public void SyncCarriesEveryProfile()
    {
        Prefs.AddHistory(History("tien-nghich"));
        var na = Prefs.AddProfile("Bé Na");
        var payload = SyncService.Payload();
        Assert.Equal(new[] { "Cả nhà", "Bé Na" }, payload.Profiles.Select(p => p.Name));
        // Máy kia (TV) có người xem "bé na" (khác id) xem phim x → vào đúng người xem ở máy này
        var remote = new SyncPayload
        {
            Device = "tv", Profiles = new()
            {
                new SyncProfile { Id = "", Name = "Cả nhà" },
                new SyncProfile { Id = "p7", Name = " bé na ", History = new() { new SyncHistory("x", "X", "", "Tập 2", "x/2", 50) } },
            },
            Sites = Prefs.SyncSitesData,
        };
        Assert.True(SyncService.Absorb(remote));
        Assert.Equal(new[] { "tien-nghich" }, Prefs.History.Select(h => h.Title));
        Prefs.SwitchProfile(na.Id);
        Assert.Equal(new[] { "X" }, Prefs.History.Select(h => h.Title));
    }
}

/// Giới hạn giờ xem — cùng ca với WatchLimitTests.swift.
public class WatchLimitTests : PrefsTest
{
    private long _now;

    public WatchLimitTests()
    {
        WatchLimit.Reset();
        WatchLimit.Clock = () => _now;
    }

    private static long At(int h, int m, int day = 5) => new DateTimeOffset(new DateTime(2026, 11, day, h, m, 0, DateTimeKind.Local)).ToUnixTimeMilliseconds();

    private void Watch(int seconds)
    {
        for (var i = 0; i < seconds; i++)
        {
            _now += 1000;
            WatchLimit.Tick(true, _now);
        }
    }

    [Fact]
    public void NoLimitNeverBlocks()
    {
        _now = At(23, 30);
        WatchLimit.Tick(false, _now);
        Watch(4000);
        Assert.Null(WatchLimit.Blocked());
        Assert.Null(WatchLimit.Remaining());
    }

    [Fact]
    public void DailyLimitBlocksThenGrantAddsMinutesAndNextDayStartsOver()
    {
        Prefs.SetDailyLimit(Prefs.ProfileId, 30);
        _now = At(15, 0);
        WatchLimit.Tick(false, _now);
        Watch(29 * 60);
        Assert.Null(WatchLimit.Blocked());
        Assert.Equal(60, WatchLimit.Remaining());
        Watch(61);
        Assert.Equal(WatchLimit.Block.Daily, WatchLimit.Blocked());
        WatchLimit.Grant(15);
        Assert.Null(WatchLimit.Blocked());
        Assert.InRange(WatchLimit.Remaining() ?? -1, 14 * 60, 15 * 60);
        _now = At(15, 0, 6);
        WatchLimit.Tick(false, _now);
        Assert.Equal(30 * 60, WatchLimit.Remaining());
    }

    [Fact]
    public void PausedTimeAndGapsDoNotCount()
    {
        Prefs.SetDailyLimit(Prefs.ProfileId, 30);
        _now = At(10, 0);
        for (var i = 0; i < 100; i++)
        {
            _now += 1000;
            WatchLimit.Tick(false, _now);
        }
        _now += 60_000;
        WatchLimit.Tick(true, _now);
        Assert.Equal(30 * 60, WatchLimit.Remaining());
    }

    [Fact]
    public void BedtimeBlocksUntilSixInTheMorning()
    {
        Prefs.SetBedtime(Prefs.ProfileId, 21 * 60);
        _now = At(20, 59);
        Assert.Null(WatchLimit.Blocked());
        _now = At(21, 0);
        Assert.Equal(WatchLimit.Block.Bedtime, WatchLimit.Blocked());
        _now = At(5, 59, 6);
        Assert.Equal(WatchLimit.Block.Bedtime, WatchLimit.Blocked());
        _now = At(6, 0, 6);
        Assert.Null(WatchLimit.Blocked());
    }

    [Fact]
    public void GrantAtBedtimeUnblocksForAWhileOrUntilMorning()
    {
        Prefs.SetBedtime(Prefs.ProfileId, 21 * 60);
        _now = At(21, 30);
        WatchLimit.Grant(30);
        Assert.Null(WatchLimit.Blocked());
        _now = At(22, 1);
        Assert.Equal(WatchLimit.Block.Bedtime, WatchLimit.Blocked());
        WatchLimit.GrantUntilMorning();
        _now = At(5, 59, 6);
        Assert.Null(WatchLimit.Blocked());
        _now = At(21, 0, 6);
        Assert.Equal(WatchLimit.Block.Bedtime, WatchLimit.Blocked());
    }

    [Fact]
    public void LimitsBelongToEachProfile()
    {
        var kid = Prefs.AddProfile("Bé Na");
        Prefs.SetDailyLimit(kid.Id, 45);
        Assert.Null(WatchLimit.Remaining());
        Assert.True(Prefs.AnyLimits);
        Prefs.SwitchProfile(kid.Id);
        _now = At(9, 0);
        Assert.Equal(45 * 60, WatchLimit.Remaining());
        Assert.Equal("45 phút/ngày", WatchLimit.Summary(kid.Id));
        Prefs.SwitchProfile("");
        Prefs.DeleteProfile(kid.Id);
        Assert.False(Prefs.AnyLimits);
    }

    [Fact]
    public void ParentPin()
    {
        Assert.False(Prefs.HasParentPin);
        Prefs.SetParentPin("1234");
        Assert.True(Prefs.HasParentPin);
        Assert.True(Prefs.CheckParentPin("1234"));
        Assert.False(Prefs.CheckParentPin("4321"));
        Prefs.SetParentPin(null);
        Assert.False(Prefs.HasParentPin);
    }

    [Fact]
    public void Labels()
    {
        Assert.Equal("Không giới hạn", WatchLimit.DailyLabel(0));
        Assert.Equal("45 phút", WatchLimit.DailyLabel(45));
        Assert.Equal("2 giờ", WatchLimit.DailyLabel(120));
        Assert.Equal("1 giờ 30 phút", WatchLimit.DailyLabel(90));
        Assert.Equal("21:30", WatchLimit.BedtimeLabel(21 * 60 + 30));
        Assert.Equal("1 phút", WatchLimit.MinutesText(5));
        Assert.Equal("5 phút", WatchLimit.MinutesText(300));
    }
}
