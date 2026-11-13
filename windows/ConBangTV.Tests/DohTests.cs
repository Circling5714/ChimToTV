using System.IO;
using System.Net;
using System.Text;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// DNS mã hoá (11/2026): gói DNS, trả lời HTTP, thứ tự máy chủ, Vượt chặn DNS — cùng ca với DohTest.kt / Socks5Tests.swift.
public class DohTests
{
    private static byte[] Hex(string s) => Convert.FromHexString(s.Replace(" ", ""));

    [Fact]
    public void BlockedAnswers()
    {
        foreach (var a in new[] { "0.0.0.0", "::", "127.0.0.1", "::1" }) Assert.True(Doh.Blocked(IPAddress.Parse(a)), a);
        foreach (var a in new[] { "104.21.35.220", "2606:4700:3035::6815:23dc", "10.0.0.1" }) Assert.False(Doh.Blocked(IPAddress.Parse(a)), a);
    }

    /// Trang cảnh báo của nhà mạng: cùng một địa chỉ cho hai tên miền gốc khác nhau → đáng ngờ, kể cả với trang đầu tiên.
    [Fact]
    public void SameAnswerForDifferentSitesIsSuspicious()
    {
        Doh.ForgetAnswers();
        var warn = new[] { IPAddress.Parse("203.0.113.9") };
        Assert.False(Doh.SharedAnswer("phimmoi.test", warn));
        Assert.False(Doh.SharedAnswer("img.phimmoi.test", warn)); // cùng tên miền gốc
        Assert.True(Doh.SharedAnswer("www.kkphim.test", warn));
        Assert.True(Doh.SharedAnswer("phimmoi.test", warn)); // từ giờ trang đầu tiên cũng hỏi lại DNS mã hoá
        Assert.False(Doh.SharedAnswer("khac.test", new[] { IPAddress.Parse("198.51.100.7") }));
        Doh.ForgetAnswers();
    }

    [Fact]
    public void QueryForA()
    {
        Assert.Equal("000001000001000000000000016102626300" + "00010001", Convert.ToHexString(DnsWire.Query("a.bc")).ToLowerInvariant());
    }

    [Fact]
    public void AnswerWithCnameAndCompression()
    {
        var reply = Hex("0000 8180 0001 0003 0000 0000" +
                        "03 777777 01 78 02 766e 00 0001 0001" +
                        "c00c 0005 0001 00000e10 0004 c010 c010" +
                        "c010 0001 0001 0000012c 0004 68 15 23 dc" +
                        "c010 0001 0001 0000003c 0004 ac 43 b4 3b");
        var a = Assert.IsType<Doh.Answer.Found>(DnsWire.Answer(reply, 0));
        Assert.Equal(IPAddress.Parse("104.21.35.220"), a.Addr);
        Assert.Equal(60, a.Ttl);
        Assert.Equal(new[] { "104.21.35.220", "172.67.180.59" }, a.All.Select(x => x.ToString())); // mọi bản ghi A
    }

    [Fact]
    public void NoDomainNoIpv4AndWrongId()
    {
        Assert.IsType<Doh.Answer.NoDomain>(DnsWire.Answer(Hex("0000 8183 0001 0000 0000 0000 01 78 00 0001 0001"), 0));
        var empty = Hex("0000 8180 0001 0000 0000 0000 01 78 00 0001 0001");
        Assert.IsType<Doh.Answer.NoIpv4>(DnsWire.Answer(empty, 0));
        Assert.IsType<Doh.Answer.Failed>(DnsWire.Answer(empty, 0x1234)); // DoT: id khác → bỏ
        Assert.IsType<Doh.Answer.Failed>(DnsWire.Answer(Hex("0000 8182 0001 0000 0000 0000 01 78 00 0001 0001"), 0));
    }

    [Fact]
    public void HttpBodyPlainAndChunked()
    {
        Assert.Equal("abc", Encoding.ASCII.GetString(DohHttp.Body(Encoding.ASCII.GetBytes(
            "HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 3\r\n\r\nabc"))!));
        Assert.Equal("abcde", Encoding.ASCII.GetString(DohHttp.Body(Encoding.ASCII.GetBytes(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n3;x=1\r\ncde\r\n0\r\n\r\n"))!));
        Assert.Null(DohHttp.Body(Encoding.ASCII.GetBytes("HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")));
    }

    [Fact]
    public void ChainOrder()
    {
        Assert.Equal(new[] { "DoH NextDNS", "DoH Cloudflare", "DoH Google", "DoT NextDNS", "DoT Cloudflare", "DoT Google" },
            Doh.Chain("abc123").Select(s => s.Label));
        Assert.Equal("/abc123", Doh.Chain("abc123")[0].Path);
        Assert.Equal("abc123.dns.nextdns.io", Doh.Chain("abc123")[3].Host);
        Assert.Equal(new[] { "DoH Cloudflare", "DoH Google", "DoT Cloudflare", "DoT Google" }, Doh.Chain("").Select(s => s.Label));
    }

    [Fact]
    public async Task DeadServersFallBackToDevice()
    {
        // Mọi máy chủ không tới được (cổng đóng trên máy) → null: bên gọi dùng DNS của máy, không ném lỗi
        var dead = new List<Doh.Server> { new("Chết", Doh.Kind.DoT, "dns.test", "", new[] { "127.0.0.1" }) };
        Assert.Null(await Doh.Resolve("phim.test", CancellationToken.None, dead));
        Assert.Null(await Doh.Resolve("localhost", CancellationToken.None, dead)); // IP / localhost: không hỏi
    }

    [Fact]
    public async Task KeepAliveReadsOneReplyAtATime()
    {
        // Ba trả lời liền nhau trên một kết nối (keep-alive): đọc đúng từng cái; máy chủ báo đóng thì không dùng lại
        var all = string.Concat("HTTP/1.1 200 OK", RN, "Content-Length: 3", RN, RN, "abc",
            "HTTP/1.1 200 OK", RN, "Transfer-Encoding: chunked", RN, RN, "2", RN, "de", RN, "0", RN, RN,
            "HTTP/1.1 200 OK", RN, "Content-Length: 1", RN, "Connection: close", RN, RN, "f");
        var st = new BufferedStream(new MemoryStream(Encoding.ASCII.GetBytes(all)));
        var r1 = await DohHttp.ReadAsync(st, CancellationToken.None);
        var r2 = await DohHttp.ReadAsync(st, CancellationToken.None);
        var r3 = await DohHttp.ReadAsync(st, CancellationToken.None);
        Assert.Equal(("abc", true), (Encoding.ASCII.GetString(r1.Body), r1.KeepAlive));
        Assert.Equal(("de", true), (Encoding.ASCII.GetString(r2.Body), r2.KeepAlive));
        Assert.Equal(("f", false), (Encoding.ASCII.GetString(r3.Body), r3.KeepAlive));
    }

    private const string RN = "\r\n";

    [Fact]
    public async Task OnlyNextDnsIsSteered()
    {
        Assert.Equal(new[] { true, false, false, true, false, false }, Doh.Chain("abc123").Select(s => s.Steer));
        Assert.Equal(new[] { "1.1.1.1", "1.0.0.1" }, await Doh.IpsFor(Doh.Chain("abc123")[4]));
    }

    [Fact]
    public async Task NoLookupForIpOrLocalhost()
    {
        Assert.Null(await Doh.Unblocked("127.0.0.1", CancellationToken.None));
        Assert.Null(await Doh.Unblocked("localhost", CancellationToken.None));
        Assert.Null(await Doh.Unblocked("[::1]", CancellationToken.None));
    }
}

/// Mặc định mới về mạng (1.7.1): Vượt chặn mức 1 + DNS mã hoá, máy đang dùng cũng bật một lần.
public class NetDefaultsTests : PrefsTest
{
    [Fact]
    public void NewInstallAndUpgrade()
    {
        Prefs.ApplyNetDefaults();
        Assert.Equal(1, Prefs.UnblockLevel);
        Assert.True(Prefs.SecureDns);
        // Sau đó người xem tắt thì giữ tắt
        Prefs.UnblockLevel = 0;
        Prefs.SecureDns = false;
        Prefs.ApplyNetDefaults();
        Assert.Equal(0, Prefs.UnblockLevel);
        Assert.False(Prefs.SecureDns);
    }

    [Fact]
    public void UpgradeKeepsLevel2()
    {
        Prefs.UnblockLevel = 2;
        Prefs.ApplyNetDefaults();
        Assert.Equal(2, Prefs.UnblockLevel);
        Assert.True(Prefs.SecureDns);
    }
}
