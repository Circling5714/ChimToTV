namespace ConBangTV.Core;

/// Giới hạn giờ xem theo người xem (Cài đặt → Người xem → Giới hạn giờ xem): số phút xem mỗi ngày và giờ đi ngủ (từ giờ đó
/// tới 6 giờ sáng không xem được). Đếm thời gian phim thật sự chạy của người đang xem; hết giờ thì trình phát dừng phim, xem
/// thêm phải có mã PIN phụ huynh. Bản Android: WatchLimit.kt, iPad: WatchLimit.swift (cùng cách tính, cùng tên khoá).
public static class WatchLimit
{
    /// Hết giờ đi ngủ lúc 6:00 sáng.
    public const int Morning = 6 * 60;

    /// Lựa chọn số phút mỗi ngày (0 = không giới hạn) và giờ đi ngủ (phút trong ngày, -1 = không).
    public static readonly int[] Daily = { 0, 30, 45, 60, 90, 120, 180 };
    public static readonly int[] Bedtimes = { -1, 19 * 60 + 30, 20 * 60, 20 * 60 + 30, 21 * 60, 21 * 60 + 30, 22 * 60, 23 * 60 };

    /// Cho xem thêm (phút) khi hết giờ.
    public static readonly int[] Grants = { 15, 30, 60 };

    public enum Block { Daily, Bedtime }

    /// Đồng hồ (ms; test thay được).
    public static Func<long> Clock = Util.NowMs;

    /// Giây xem chưa ghi xuống (ghi mỗi 30 giây và khi rời trình phát).
    private static double _pending;
    private static long _lastTick;

    private static DateTime Local(long now) => DateTimeOffset.FromUnixTimeMilliseconds(now).LocalDateTime;

    public static string Day(long now) => Local(now).ToString("yyyy-MM-dd", System.Globalization.CultureInfo.InvariantCulture);

    private static int MinuteOfDay(long now)
    {
        var d = Local(now);
        return d.Hour * 60 + d.Minute;
    }

    /// Giây người đang xem đã xem hôm nay (kể cả phần chưa ghi).
    public static int UsedToday(long? at = null)
    {
        var now = at ?? Clock();
        return (Prefs.WatchDay == Day(now) ? Prefs.WatchSeconds : 0) + (int)_pending;
    }

    /// Giây còn được xem hôm nay; null = không giới hạn theo ngày.
    public static int? Remaining(long? at = null)
    {
        var now = at ?? Clock();
        var limit = Prefs.DailyLimit();
        if (limit <= 0) return null;
        var extra = Prefs.WatchDay == Day(now) ? Prefs.WatchExtra : 0;
        return (limit + extra) * 60 - UsedToday(now);
    }

    /// Người đang xem có bị chặn không (giờ đi ngủ trước, rồi hết phút trong ngày); null = xem được.
    public static Block? Blocked(long? at = null)
    {
        var now = at ?? Clock();
        if (now < Prefs.UnblockUntil) return null;
        var bed = Prefs.Bedtime();
        if (bed >= 0)
        {
            var m = MinuteOfDay(now);
            if (m >= bed || m < Morning) return Block.Bedtime;
        }
        return Remaining(now) is { } r && r <= 0 ? Block.Daily : null;
    }

    /// Gọi mỗi lần đọc trạng thái trình phát: phim đang chạy thì cộng thời gian từ lần trước (bỏ khoảng hở > 3 giây).
    public static void Tick(bool playing, long? at = null)
    {
        var now = at ?? Clock();
        var dt = (now - _lastTick) / 1000.0;
        _lastTick = now;
        if (!playing || dt <= 0 || dt > 3) return;
        _pending += dt;
        if (_pending >= 30) Flush(now);
    }

    /// Ghi phần đã đếm (rời trình phát, đổi người xem, sang ngày mới).
    public static void Flush(long? at = null)
    {
        var now = at ?? Clock();
        var d = Day(now);
        if (Prefs.WatchDay != d)
        {
            Prefs.WatchDay = d;
            Prefs.WatchSeconds = 0;
            Prefs.WatchExtra = 0;
        }
        var whole = (int)_pending;
        if (whole > 0) Prefs.WatchSeconds += whole;
        _pending -= whole;
    }

    /// Bỏ phần đếm chưa ghi (đổi người xem — đã Flush trước).
    public static void Reset()
    {
        _pending = 0;
        _lastTick = 0;
    }

    /// Cho xem thêm (đã nhập mã phụ huynh): đang giờ đi ngủ → mở khoá thêm từng ấy phút; hết phút trong ngày → cộng thêm.
    public static void Grant(int minutes, long? at = null)
    {
        var now = at ?? Clock();
        Flush(now);
        if (Blocked(now) == Block.Bedtime) Prefs.UnblockUntil = now + minutes * 60_000L;
        else Prefs.WatchExtra += minutes;
    }

    /// Không giới hạn tới 6 giờ sáng (hôm nay nếu đang trước 6 giờ, không thì sáng mai).
    public static void GrantUntilMorning(long? at = null)
    {
        var now = Local(at ?? Clock());
        var six = now.Date.AddHours(Morning / 60);
        if (six <= now) six = six.AddDays(1);
        Prefs.UnblockUntil = new DateTimeOffset(six).ToUnixTimeMilliseconds();
    }

    /// "30 phút", "1 giờ", "1 giờ 30 phút", "Không giới hạn".
    public static string DailyLabel(int minutes)
    {
        if (minutes <= 0) return "Không giới hạn";
        if (minutes < 60) return $"{minutes} phút";
        if (minutes % 60 == 0) return $"{minutes / 60} giờ";
        return $"{minutes / 60} giờ {minutes % 60} phút";
    }

    /// "21:00" hoặc "Không".
    public static string BedtimeLabel(int minute) => minute < 0 ? "Không" : $"{minute / 60}:{minute % 60:00}";

    /// Giây còn lại → "25 phút" (làm tròn lên, tối thiểu 1 phút).
    public static string MinutesText(int seconds) => DailyLabel(Math.Max(1, (seconds + 59) / 60));

    /// Tóm tắt giới hạn của một người xem, vd "1 giờ/ngày, ngủ lúc 21:00".
    public static string Summary(string id)
    {
        var parts = new List<string>();
        int d = Prefs.DailyLimit(id), b = Prefs.Bedtime(id);
        if (d > 0) parts.Add($"{DailyLabel(d)}/ngày");
        if (b >= 0) parts.Add($"ngủ lúc {BedtimeLabel(b)}");
        return parts.Count == 0 ? "không giới hạn" : string.Join(", ", parts);
    }
}
