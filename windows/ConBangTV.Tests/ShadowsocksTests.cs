using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using ConBangTV.Core;
using Xunit;

namespace ConBangTV.Tests;

/// Shadowsocks (11/2026): BLAKE3, khoá, link ss://, kết nối thật trên 127.0.0.1 với máy chủ thử viết riêng theo đặc tả — cùng ca
/// với ShadowsocksTest.kt / ShadowsocksTests.swift.
public class ShadowsocksTests
{
    private static string Hex(byte[] b) => Convert.ToHexString(b).ToLowerInvariant();
    private static byte[] Input(int n) => Enumerable.Range(0, n).Select(i => (byte)(i % 251)).ToArray();
    private static string B64(byte[] b) => Convert.ToBase64String(b);

    // Bộ số chuẩn của BLAKE3 (BLAKE3-team/BLAKE3 test_vectors.json — 32 byte đầu của hash / derive_key)
    [Fact]
    public void Blake3OfficialVectors()
    {
        const string ctx = "BLAKE3 2019-12-27 16:29:52 test vectors context";
        var cases = new (int N, string H, string D)[]
        {
            (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d"),
            (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213", "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c"),
            (63, "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b", "b6451e30b953c206e34644c6803724e9d2725e0893039cfc49584f991f451af3"),
            (64, "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98", "a5c4a7053fa86b64746d4bb688d06ad1f02a18fce9afd3e818fefaa7126bf73e"),
            (65, "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee", "51fd05c3c1cfbc8ed67d139ad76f5cf8236cd2acd26627a30c104dfd9d3ff8a8"),
            (1023, "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11", "74a16c1c3d44368a86e1ca6df64be6a2f64cce8f09220787450722d85725dea5"),
            (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7", "7356cd7720d5b66b6d0697eb3177d9f8d73a4a5c5e968896eb6a689684302706"),
        };
        foreach (var (n, h, d) in cases)
        {
            Assert.Equal(h, Hex(Blake3.Hash(Input(n))));
            Assert.Equal(d, Hex(Blake3.DeriveKey(ctx, Input(n))));
        }
        Assert.Equal(cases[2].H[..32], Hex(Blake3.Hash(Input(63), 16)));
        Assert.Equal(cases[3].D[..32], Hex(Blake3.DeriveKey(ctx, Input(64), 16)));
    }

    // Số kiểm bằng Python (hashlib MD5 + cryptography HKDF-SHA1)
    [Fact]
    public void LegacyKeyDerivation()
    {
        var k = Shadowsocks.EvpBytesToKey(Encoding.UTF8.GetBytes("test-password"), 32);
        Assert.Equal("dfb450efddbb5387197c84460623675b69f2cebcd8ef520a3dfddef7c3d540b2", Hex(k));
        Assert.Equal("dfb450efddbb5387197c84460623675b", Hex(Shadowsocks.EvpBytesToKey(Encoding.UTF8.GetBytes("test-password"), 16)));
        Assert.Equal("7b6ca12d45ef79467432717aaa6a6ae6e1dd046a505fb2ebaaac602210919595",
            Hex(Shadowsocks.Subkey(SsMethod.Aes256, k, Enumerable.Range(0, 32).Select(i => (byte)i).ToArray())));
    }

    [Fact]
    public void Base64BothAlphabets()
    {
        var raw = Enumerable.Range(0, 32).Select(i => (byte)(i * 37 + 251)).ToArray();
        Assert.Equal(raw, SsBase64.Decode(B64(raw)));
        Assert.Equal(raw, SsBase64.Decode(B64(raw).Replace('+', '-').Replace('/', '_').TrimEnd('=')));
        Assert.Null(SsBase64.Decode("abc!"));
        Assert.Null(SsBase64.Decode(""));
    }

    private static readonly byte[] ServerPsk = Enumerable.Range(0, 32).Select(i => (byte)(i + 1)).ToArray();
    private static readonly byte[] UserPsk = Enumerable.Range(0, 32).Select(i => (byte)(100 + i)).ToArray();

    [Fact]
    public void Parse3xUi2022Link()
    {
        var link = $"ss://2022-blake3-aes-256-gcm:{Uri.EscapeDataString(B64(ServerPsk))}:{Uri.EscapeDataString(B64(UserPsk))}@vpn.example.com:24443?type=tcp#Nh%C3%A0";
        var c = SsLink.Parse(link);
        Assert.Equal(new SsConfig("vpn.example.com", 24443, "2022-blake3-aes-256-gcm", $"{B64(ServerPsk)}:{B64(UserPsk)}"), c);
        var keys = Shadowsocks.Keys(SsMethod.B3Aes256, c.Password);
        Assert.Equal(ServerPsk, keys[0]);
        Assert.Equal(UserPsk, keys[1]);
    }

    [Fact]
    public void ParseSip002AndOldLinks()
    {
        static string Url(string s) => Convert.ToBase64String(Encoding.UTF8.GetBytes(s)).Replace('+', '-').Replace('/', '_').TrimEnd('=');
        Assert.Equal(new SsConfig("1.2.3.4", 8388, "aes-256-gcm", "mật khẩu"), SsLink.Parse("ss://" + Url("aes-256-gcm:mật khẩu") + "@1.2.3.4:8388/?type=tcp#x"));
        Assert.Equal(new SsConfig("host.vn", 443, "chacha20-ietf-poly1305", "pa:ss"),
            SsLink.Parse("ss://" + B64(Encoding.UTF8.GetBytes("chacha20-poly1305:pa:ss@host.vn:443")) + "#tên"));
        var c = SsLink.Parse("ss://" + Url($"2022-blake3-aes-128-gcm:{B64(new byte[16])}") + "@[2001:db8::1]:8443");
        Assert.Equal(("2001:db8::1", 8443), (c.Host, c.Port));
    }

    [Theory]
    [InlineData("?type=ws&path=%2F", "ws")]
    [InlineData("?type=tcp&security=tls", "tls")]
    [InlineData("?plugin=obfs-local%3Bobfs%3Dhttp", "plugin")]
    public void ParseRejectsTransports(string query, string word)
    {
        var e = Assert.Throws<SsError>(() => SsLink.Parse($"ss://2022-blake3-aes-256-gcm:{B64(ServerPsk)}@h:1{query}"));
        Assert.Contains(word, e.Message);
    }

    [Fact]
    public void ParseRejectsBadKeys()
    {
        Assert.Contains("32 byte", Assert.Throws<SsError>(() => SsLink.Parse($"ss://2022-blake3-aes-256-gcm:{B64(new byte[16])}@h:1")).Message);
        Assert.Contains("chưa hỗ trợ", Assert.Throws<SsError>(() => SsLink.Parse("ss://rc4-md5:abc@h:1")).Message);
        Assert.Contains("nhiều người dùng",
            Assert.Throws<SsError>(() => SsLink.Parse($"ss://2022-blake3-chacha20-poly1305:{B64(ServerPsk)}:{B64(UserPsk)}@h:1")).Message);
    }

    // ---- Máy chủ thử (viết riêng theo đặc tả, không dùng SsStream) ----

    private sealed class TestServer
    {
        private readonly SsMethod _m;
        private readonly byte[]? _identity;
        private readonly byte[][] _users;
        private readonly TcpListener _l = new(IPAddress.Loopback, 0);
        public readonly TaskCompletionSource<(string Target, byte[] First)> Seen = new();
        public int Port => ((IPEndPoint)_l.LocalEndpoint).Port;

        public TestServer(SsMethod m, byte[]? identity, params byte[][] users)
        {
            _m = m;
            _identity = identity;
            _users = users;
            _l.Start();
            _ = Task.Run(async () =>
            {
                using var s = await _l.AcceptSocketAsync();
                try
                {
                    await Serve(new NetworkStream(s, true));
                }
                catch (Exception e)
                {
                    Seen.TrySetException(e);
                }
                _l.Stop();
            });
        }

        private static byte[] Nonce(ulong n)
        {
            var b = new byte[12];
            for (var i = 0; i < 8; i++) b[i] = (byte)(n >> (8 * i));
            return b;
        }

        private byte[] Open(byte[] key, ulong n, byte[] c)
        {
            var p = new byte[c.Length - 16];
            if (_m.Chacha) new ChaCha20Poly1305(key).Decrypt(Nonce(n), c[..^16], c[^16..], p);
            else new AesGcm(key, 16).Decrypt(Nonce(n), c[..^16], c[^16..], p);
            return p;
        }

        private byte[] Seal(byte[] key, ulong n, byte[] p)
        {
            var c = new byte[p.Length + 16];
            if (_m.Chacha) new ChaCha20Poly1305(key).Encrypt(Nonce(n), p, c.AsSpan(0, p.Length), c.AsSpan(p.Length));
            else new AesGcm(key, 16).Encrypt(Nonce(n), p, c.AsSpan(0, p.Length), c.AsSpan(p.Length));
            return c;
        }

        private byte[] Sub(byte[] psk, byte[] salt) => _m.Is2022
            ? Blake3.DeriveKey("shadowsocks 2022 session subkey", psk.Concat(salt).ToArray(), _m.KeySize)
            : HKDF.DeriveKey(HashAlgorithmName.SHA1, psk, _m.KeySize, salt, Encoding.ASCII.GetBytes("ss-subkey"));

        private static async Task<byte[]?> Read(Stream s, int n)
        {
            var b = new byte[n];
            var got = 0;
            while (got < n)
            {
                var r = await s.ReadAsync(b.AsMemory(got));
                if (r == 0) return got == 0 ? null : throw new EndOfStreamException();
                got += r;
            }
            return b;
        }

        private static int AddrLen(byte[] v) => v[0] switch { 1 => 7, 4 => 19, _ => 4 + v[1] };

        private static string Describe(byte[] a)
        {
            var port = a[^2] << 8 | a[^1];
            var host = a[0] == 3 ? Encoding.ASCII.GetString(a, 2, a[1]) : new IPAddress(a[1..^2]).ToString();
            return $"{host}:{port}";
        }

        private async Task Serve(Stream s)
        {
            var salt = (await Read(s, _m.KeySize))!;
            var psk = _users[0];
            if (_identity != null)
            {
                using var aes = Aes.Create();
                aes.Key = Blake3.DeriveKey("shadowsocks 2022 identity subkey", _identity.Concat(salt).ToArray(), _m.KeySize);
                var h = aes.DecryptEcb((await Read(s, 16))!, PaddingMode.None);
                var u = _users.FirstOrDefault(x => Blake3.Hash(x, 16).SequenceEqual(h));
                if (u == null) return; // người dùng lạ: đóng
                psk = u;
            }
            var key = Sub(psk, salt);
            ulong rn = 0;
            byte[] addr, first;
            if (_m.Is2022)
            {
                var f = Open(key, rn++, (await Read(s, 27))!);
                Assert.Equal(0, f[0]);
                ulong ts = 0;
                for (var i = 1; i <= 8; i++) ts = ts << 8 | f[i];
                Assert.True(Math.Abs(DateTimeOffset.UtcNow.ToUnixTimeSeconds() - (long)ts) <= 30);
                var v = Open(key, rn++, (await Read(s, (f[9] << 8 | f[10]) + 16))!);
                var al = AddrLen(v);
                addr = v[..al];
                var pad = v[al] << 8 | v[al + 1];
                Assert.True(pad > 0 || v.Length > al + 2);
                first = v[(al + 2 + pad)..];
            }
            else
            {
                var l = Open(key, rn++, (await Read(s, 18))!);
                var v = Open(key, rn++, (await Read(s, (l[0] << 8 | l[1]) + 16))!);
                var al = AddrLen(v);
                addr = v[..al];
                first = v[al..];
            }
            Seen.TrySetResult((Describe(addr), first));
            var rsalt = RandomNumberGenerator.GetBytes(_m.KeySize);
            var wkey = Sub(psk, rsalt);
            ulong wn = 0;
            var reply = Encoding.ASCII.GetBytes("echo:").Concat(first).ToArray();
            var o = new MemoryStream();
            o.Write(rsalt);
            if (_m.Is2022)
            {
                var h = new byte[1 + 8 + _m.KeySize + 2];
                h[0] = 1;
                var ts = (ulong)DateTimeOffset.UtcNow.ToUnixTimeSeconds();
                for (var i = 0; i < 8; i++) h[1 + i] = (byte)(ts >> (8 * (7 - i)));
                salt.CopyTo(h, 9);
                h[^2] = (byte)(reply.Length >> 8);
                h[^1] = (byte)reply.Length;
                o.Write(Seal(wkey, wn++, h));
            }
            else o.Write(Seal(wkey, wn++, new[] { (byte)(reply.Length >> 8), (byte)reply.Length }));
            o.Write(Seal(wkey, wn++, reply));
            await s.WriteAsync(o.ToArray());
            while (await Read(s, 18) is { } lc)
            {
                var l = Open(key, rn++, lc);
                var data = Open(key, rn++, (await Read(s, (l[0] << 8 | l[1]) + 16))!);
                var a = Seal(wkey, wn++, new[] { (byte)(data.Length >> 8), (byte)data.Length });
                var b = Seal(wkey, wn++, data);
                await s.WriteAsync(a.Concat(b).ToArray());
            }
        }
    }

    private static async Task<byte[]> ReadAll(Stream s, int n)
    {
        var b = new byte[n];
        await s.ReadExactlyAsync(b);
        return b;
    }

    private static async Task RoundTrip(SsMethod m, string password, TestServer server, string initial)
    {
        var c = new SsConfig("127.0.0.1", server.Port, m.Id, password);
        using var t = new CancellationTokenSource(10_000);
        await using var s = await Shadowsocks.Connect(c, "phim.example", null, 443, t.Token);
        var first = Encoding.UTF8.GetBytes(initial);
        await s.WriteAsync(first, t.Token);
        var seen = await server.Seen.Task.WaitAsync(t.Token);
        Assert.Equal("phim.example:443", seen.Target);
        Assert.Equal(first, seen.First);
        Assert.Equal(Encoding.ASCII.GetBytes("echo:").Concat(first).ToArray(), await ReadAll(s, 5 + first.Length));
        // Chunk lớn (quá 0x3FFF với kiểu cũ → chia nhiều chunk)
        var big = Enumerable.Range(0, 40_000).Select(i => (byte)(i % 97)).ToArray();
        await s.WriteAsync(big, t.Token);
        Assert.Equal(big, await ReadAll(s, big.Length));
        s.Socket.Shutdown(SocketShutdown.Send);
        Assert.Equal(0, await s.ReadAsync(new byte[1], t.Token));
    }

    [Fact]
    public async Task Ss2022MultiUserRoundTrip()
    {
        var s = new TestServer(SsMethod.B3Aes256, ServerPsk, Enumerable.Repeat((byte)7, 32).ToArray(), UserPsk);
        await RoundTrip(SsMethod.B3Aes256, $"{B64(ServerPsk)}:{B64(UserPsk)}", s, "GET / HTTP/1.1\r\n\r\n");
    }

    [Fact]
    public async Task Ss2022Aes128SingleUser()
    {
        var k = Enumerable.Range(0, 16).Select(i => (byte)(i * 3)).ToArray();
        await RoundTrip(SsMethod.B3Aes128, B64(k), new TestServer(SsMethod.B3Aes128, null, k), "xin chào");
    }

    [Fact]
    public async Task LegacyAeadRoundTrip()
    {
        foreach (var m in new[] { SsMethod.Aes256, SsMethod.Aes128, SsMethod.ChachaIetf }.Where(m => m.Supported))
        {
            var key = Shadowsocks.EvpBytesToKey(Encoding.UTF8.GetBytes("mật khẩu"), m.KeySize);
            await RoundTrip(m, "mật khẩu", new TestServer(m, null, key), $"legacy {m.Id}");
        }
    }

    [Fact]
    public async Task WrongUserKeyReportsClearError()
    {
        var server = new TestServer(SsMethod.B3Aes256, ServerPsk, UserPsk);
        var c = new SsConfig("127.0.0.1", server.Port, SsMethod.B3Aes256.Id, $"{B64(ServerPsk)}:{B64(Enumerable.Repeat((byte)9, 32).ToArray())}");
        using var t = new CancellationTokenSource(10_000);
        await using var s = await Shadowsocks.Connect(c, "phim.example", null, 443, t.Token);
        await s.WriteAsync(Encoding.ASCII.GetBytes("hello"), t.Token);
        var e = await Assert.ThrowsAsync<SsError>(async () => await s.ReadAsync(new byte[1], t.Token));
        Assert.Contains("sai khoá", e.Message);
    }

    /// Thử với máy chủ thật (3X-UI / Xray…): chỉ chạy khi có biến môi trường SS_TEST_LINK (một hay nhiều link ss://, cách nhau
    /// bằng dấu cách). Không đưa link thật vào code.
    [Fact]
    public async Task RealServerWhenGiven()
    {
        var links = (Environment.GetEnvironmentVariable("SS_TEST_LINK") ?? "").Split(' ', StringSplitOptions.RemoveEmptyEntries);
        foreach (var l in links)
        {
            var ip = await Shadowsocks.Check(SsLink.Parse(l));
            Assert.False(string.IsNullOrEmpty(ip));
        }
    }
}
