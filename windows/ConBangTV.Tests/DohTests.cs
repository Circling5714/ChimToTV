using System.Net;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Vượt chặn bằng DNS (11/2026): DNS chặn trả 0.0.0.0 / :: → hỏi lại qua DNS mã hoá — cùng ca với Socks5BridgeTest.kt.
public class DohTests
{
    [Fact]
    public void BlockedAnswers()
    {
        foreach (var a in new[] { "0.0.0.0", "::", "127.0.0.1", "::1" }) Assert.True(Doh.Blocked(IPAddress.Parse(a)), a);
        foreach (var a in new[] { "104.21.35.220", "2606:4700:3035::6815:23dc", "10.0.0.1" }) Assert.False(Doh.Blocked(IPAddress.Parse(a)), a);
    }

    [Fact]
    public void ParseTakesFirstARecord()
    {
        var json = """
            {"Status":0,"Answer":[{"name":"www.phim.test","type":5,"TTL":300,"data":"phim.test."},
            {"name":"phim.test","type":1,"TTL":75,"data":"104.21.35.220"},{"name":"phim.test","type":1,"TTL":75,"data":"172.67.180.59"}]}
            """;
        Assert.Equal(IPAddress.Parse("104.21.35.220"), Doh.Parse(json, out var ttl));
        Assert.Equal(75, ttl);
        // Không có bản ghi A / chỉ có địa chỉ chặn → null
        Assert.Null(Doh.Parse("""{"Status":3}""", out _));
        Assert.Null(Doh.Parse("""{"Answer":[{"type":1,"TTL":10,"data":"0.0.0.0"}]}""", out _));
    }

    [Fact]
    public async Task NoLookupForIpOrLocalhost()
    {
        // Địa chỉ IP / localhost: không hỏi DNS mã hoá (null = dùng như thường)
        Assert.Null(await Doh.Unblocked("127.0.0.1", CancellationToken.None));
        Assert.Null(await Doh.Unblocked("localhost", CancellationToken.None));
        Assert.Null(await Doh.Unblocked("[::1]", CancellationToken.None));
    }
}
