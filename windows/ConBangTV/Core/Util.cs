using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text;
using System.Text.RegularExpressions;

namespace ConBangTV.Core;

// Tiện ích nhỏ dùng chung: regex, chuỗi, thời gian, chạy có giới hạn thời gian (bản iPad: Util.swift).

/// Biểu thức chính quy viết sẵn trong code.
public sealed class Rx
{
    public readonly Regex Re;

    public Rx(string pattern, RegexOptions options = RegexOptions.None)
    {
        Re = new Regex(pattern, options | RegexOptions.CultureInvariant);
    }

    /// Nhóm i của lần khớp đầu tiên (nhóm 0 = cả chuỗi khớp); không khớp → null; nhóm không khớp → "".
    public string? Group(int i, string s)
    {
        var m = Re.Match(s);
        return m.Success ? m.Groups[i].Value : null;
    }

    public string[]? Groups(string s)
    {
        var m = Re.Match(s);
        if (!m.Success) return null;
        return m.Groups.Cast<System.Text.RegularExpressions.Group>().Select(g => g.Value).ToArray();
    }

    public bool Matches(string s) => Re.IsMatch(s);

    public IEnumerable<Match> All(string s, int from = 0)
    {
        if (from > s.Length) yield break;
        for (var m = Re.Match(s, from); m.Success; m = m.NextMatch()) yield return m;
    }

    public string Replace(string s, string replacement) => Re.Replace(s, replacement);
}

public static class Str
{
    public static string Trimmed(this string s) => s.Trim();

    /// null khi rỗng (sau khi bỏ khoảng trắng).
    public static string? NonEmpty(this string? s)
    {
        var t = s?.Trim();
        return string.IsNullOrEmpty(t) ? null : t;
    }

    public static string TrimEndChar(this string s, char ch) => s.TrimEnd(ch);

    /// Phần sau lần xuất hiện cuối của ký tự (không có thì cả chuỗi), như substringAfterLast của Kotlin.
    public static string AfterLast(this string s, char ch)
    {
        var i = s.LastIndexOf(ch);
        return i < 0 ? s : s[(i + 1)..];
    }

    public static string Before(this string s, string sep)
    {
        var i = s.IndexOf(sep, StringComparison.Ordinal);
        return i < 0 ? s : s[..i];
    }

    /// Mã hoá cho tham số trong địa chỉ (giống URLEncoder của Java: dấu cách → +).
    public static string FormEncoded(this string s)
    {
        var sb = new StringBuilder();
        foreach (var b in Encoding.UTF8.GetBytes(s))
        {
            var c = (char)b;
            if (b < 128 && (char.IsAsciiLetterOrDigit(c) || c is '-' or '.' or '_' or '*')) sb.Append(c);
            else if (c == ' ') sb.Append('+');
            else sb.Append('%').Append(b.ToString("X2"));
        }
        return sb.ToString();
    }

    /// Gộp khoảng trắng liên tiếp thành một dấu cách (như text() của jsoup).
    public static string CollapseSpaces(this string s) => Regex.Replace(s, @"\s+", " ").Trim();
}

public static class Util
{
    /// "1:05", "1:02:03"
    public static string FormatTime(double seconds)
    {
        var s = (int)Math.Max(0, double.IsFinite(seconds) ? seconds : 0);
        int h = s / 3600, m = s % 3600 / 60, sec = s % 60;
        return h > 0 ? $"{h}:{m:00}:{sec:00}" : $"{m}:{sec:00}";
    }

    /// Thời gian hiện tại (ms kể từ 1970), như System.currentTimeMillis().
    public static long NowMs() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

    /// Chạy việc với giới hạn thời gian; quá hạn thì báo lỗi.
    public static async Task<T> WithTimeout<T>(double seconds, Func<Task<T>> work)
    {
        var task = work();
        var done = await Task.WhenAny(task, Task.Delay(TimeSpan.FromSeconds(seconds)));
        if (done != task) throw new AppError($"Quá {(int)seconds} giây không trả lời");
        return await task;
    }

    /// Bắt tay HTTPS hỏng / treo: hay là nhà mạng chặn theo tên trang → gợi ý bật Vượt chặn nhà mạng.
    private static string UnblockHint() =>
        Prefs.UnblockLevel == 0 ? "\nNếu nhà mạng chặn trang này: bật Cài đặt → Vượt chặn nhà mạng." : "";

    /// Câu báo lỗi dễ hiểu cho người xem.
    public static string Friendly(Exception e)
    {
        while (e is AggregateException { InnerException: not null } ag) e = ag.InnerException!;
        switch (e)
        {
            case AppError a: return a.Message;
            case SocksError sk: return "Lỗi proxy: " + sk.Message + ".";
            case Http.HttpError h: return h.Message;
            case TaskCanceledException or TimeoutException: return "Trang không trả lời (quá thời gian chờ)." + UnblockHint();
            case HttpRequestException { InnerException: SocksError se2 }:
                return "Lỗi proxy: " + se2.Message + ".";
            case HttpRequestException hr:
                var inner = hr.InnerException;
                while (inner?.InnerException != null && inner is not SocketException) inner = inner.InnerException;
                if (inner is SocketException se)
                {
                    return se.SocketErrorCode switch
                    {
                        SocketError.HostNotFound or SocketError.NoData => "Không tìm thấy tên miền (trang đổi tên miền hoặc bị chặn?).",
                        SocketError.ConnectionRefused => "Không kết nối được tới trang.",
                        SocketError.ConnectionReset => "Mất kết nối giữa chừng.",
                        SocketError.TimedOut => "Trang không trả lời (quá thời gian chờ).",
                        SocketError.NetworkUnreachable or SocketError.NetworkDown => "Không có mạng.",
                        _ => $"Lỗi mạng ({se.SocketErrorCode}).",
                    };
                }
                if (hr.InnerException is System.Security.Authentication.AuthenticationException ||
                    hr.InnerException is IOException)
                    return "Lỗi kết nối bảo mật / mất kết nối." + UnblockHint();
                return "Lỗi mạng: " + hr.Message + Net.Note();
            default: return e.Message;
        }
    }
}
