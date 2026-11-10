using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace ConBangTV.Core;

/// Máy chủ Shadowsocks (Cài đặt → Proxy → Kiểu: Shadowsocks), vd lối vào Shadowsocks của 3X-UI / Xray (bản TV:
/// Shadowsocks.kt). Password: SS 2022 = các khoá base64 cách nhau bằng ":" (3X-UI luôn nhiều người dùng:
/// "khoá-máy-chủ:khoá-người-dùng"); kiểu cũ (aes-256-gcm, chacha20-ietf-poly1305…) = mật khẩu.
public sealed record SsConfig(string Host, int Port, string Method, string Password, bool RemoteDns = true, bool SitesOnly = false);

/// Kiểu mã hoá hỗ trợ (TCP). KeySize = số byte khoá = số byte salt.
public sealed record SsMethod(string Id, int KeySize, bool Chacha, bool Is2022)
{
    public static readonly SsMethod B3Aes256 = new("2022-blake3-aes-256-gcm", 32, false, true);
    public static readonly SsMethod B3Aes128 = new("2022-blake3-aes-128-gcm", 16, false, true);
    public static readonly SsMethod B3Chacha = new("2022-blake3-chacha20-poly1305", 32, true, true);
    public static readonly SsMethod Aes256 = new("aes-256-gcm", 32, false, false);
    public static readonly SsMethod Aes128 = new("aes-128-gcm", 16, false, false);
    public static readonly SsMethod ChachaIetf = new("chacha20-ietf-poly1305", 32, true, false);

    public static readonly SsMethod[] All = { B3Aes256, B3Aes128, B3Chacha, Aes256, Aes128, ChachaIetf };
    public static SsMethod Default => B3Aes256;

    /// Theo tên (không phân biệt hoa / thường; trống = mặc định); "chacha20-poly1305" của Xray / 3X-UI = chacha20-ietf-poly1305.
    public static SsMethod? Of(string? name)
    {
        var n = (name ?? "").Trim().ToLowerInvariant();
        if (n.Length == 0) return Default;
        if (n == "chacha20-poly1305") return ChachaIetf;
        return All.FirstOrDefault(m => m.Id == n);
    }

    public static string Names => string.Join(", ", All.Select(m => m.Id));

    /// Máy có hỗ trợ không (ChaCha20-Poly1305 cần Windows 11 / Windows 10 bản mới).
    public bool Supported => !Chacha || ChaCha20Poly1305.IsSupported;
}

/// Lỗi Shadowsocks: câu báo cho người xem (sai khoá, máy chủ không trả lời…).
public sealed class SsError : IOException
{
    public SsError(string message) : base(message) { }
}

/// Đọc link ss:// (SIP002, cả kiểu 3X-UI cho SS 2022: "ss://method:khoá-máy-chủ:khoá-người-dùng@host:cổng?type=tcp#tên").
public static class SsLink
{
    /// Sai thì SsError với câu báo cho người xem.
    public static SsConfig Parse(string link, bool remoteDns = true, bool sitesOnly = false)
    {
        var s = link.Trim();
        if (!s.StartsWith("ss://", StringComparison.OrdinalIgnoreCase)) throw new SsError("Link Shadowsocks phải bắt đầu bằng ss://");
        s = s[5..];
        var hash = s.IndexOf('#');
        if (hash >= 0) s = s[..hash];
        var query = "";
        var q = s.IndexOf('?');
        if (q >= 0)
        {
            query = s[(q + 1)..];
            s = s[..q];
        }
        s = s.TrimEnd('/');
        foreach (var p in query.Split('&', StringSplitOptions.RemoveEmptyEntries))
        {
            var eq = p.IndexOf('=');
            var k = (eq >= 0 ? p[..eq] : p).ToLowerInvariant();
            var v = (eq >= 0 ? Decode(p[(eq + 1)..]) : "").ToLowerInvariant();
            if (k == "type" && v.Length > 0 && v != "tcp")
                throw new SsError($"Link dùng truyền tải «{v}» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Transmission = TCP, không TLS).");
            if (k == "security" && v.Length > 0 && v != "none")
                throw new SsError($"Link dùng «security={v}» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Security = None).");
            if (k == "plugin" && v.Length > 0) throw new SsError($"Link dùng plugin ({v.Split(';')[0]}) — app chưa hỗ trợ plugin.");
            if (k == "headertype" && v.Length > 0 && v != "none")
                throw new SsError("Link dùng ngụy trang HTTP — app chưa hỗ trợ (3X-UI: TCP header = None).");
        }
        // Kiểu cũ: ss://base64(method:pass@host:port)
        if (!s.Contains('@')) s = SsBase64.Decode(s) is { } d ? Encoding.UTF8.GetString(d) : throw new SsError("Link ss:// không đọc được.");
        var at = s.LastIndexOf('@');
        if (at < 0) throw new SsError("Link ss:// thiếu máy chủ / cổng.");
        var user = s[..at];
        var hostPort = s[(at + 1)..];
        string info;
        if (user.Contains(':') || user.Contains('%')) info = Decode(user);
        else if (SsBase64.Decode(user) is { } b && Encoding.UTF8.GetString(b) is var t && t.Contains(':')) info = t;
        else info = Decode(user);
        var colon = info.IndexOf(':');
        if (colon < 0) throw new SsError("Link ss:// thiếu kiểu mã hoá / khoá.");
        var method = info[..colon];
        var password = info[(colon + 1)..];
        var hp = SplitHostPort(hostPort) ?? throw new SsError("Link ss:// thiếu máy chủ / cổng.");
        var m = SsMethod.Of(method) ?? throw new SsError($"Kiểu mã hoá «{method}» chưa hỗ trợ. App hỗ trợ: {SsMethod.Names}.");
        Shadowsocks.Keys(m, password); // kiểm tra khoá ngay lúc dán
        return new SsConfig(hp.Host, hp.Port, m.Id, password, remoteDns, sitesOnly);
    }

    /// "host:cổng" hoặc "[ipv6]:cổng" → (host, cổng).
    public static (string Host, int Port)? SplitHostPort(string s)
    {
        var t = s.Trim();
        string host, port;
        if (t.StartsWith('['))
        {
            var close = t.IndexOf(']');
            if (close < 0 || close + 1 >= t.Length || t[close + 1] != ':') return null;
            host = t[1..close];
            port = t[(close + 2)..];
        }
        else
        {
            var c = t.LastIndexOf(':');
            if (c < 0) return null;
            host = t[..c];
            port = t[(c + 1)..];
        }
        if (host.Length == 0 || !int.TryParse(port, out var p) || p is < 1 or > 65535) return null;
        return (host, p);
    }

    private static string Decode(string s)
    {
        try
        {
            return Uri.UnescapeDataString(s);
        }
        catch (Exception)
        {
            return s;
        }
    }
}

/// Base64 thường lẫn base64url, có hoặc không dấu "=" (khoá 3X-UI, link SIP002). null = không phải base64.
public static class SsBase64
{
    public static byte[]? Decode(string s)
    {
        var t = s.Trim().TrimEnd('=');
        if (t.Length == 0) return null;
        var out_ = new List<byte>(t.Length * 3 / 4);
        int acc = 0, bits = 0;
        foreach (var ch in t)
        {
            int v;
            if (ch is >= 'A' and <= 'Z') v = ch - 'A';
            else if (ch is >= 'a' and <= 'z') v = ch - 'a' + 26;
            else if (ch is >= '0' and <= '9') v = ch - '0' + 52;
            else if (ch is '+' or '-') v = 62;
            else if (ch is '/' or '_') v = 63;
            else return null;
            acc = (acc << 6) | v;
            bits += 6;
            if (bits >= 8)
            {
                bits -= 8;
                out_.Add((byte)((acc >> bits) & 0xFF));
            }
        }
        if (bits >= 6) return null;
        return out_.ToArray();
    }
}

/// BLAKE3 — chỉ đầu vào ≤ 1024 byte (một chunk) và đầu ra ≤ 32 byte: đủ cho khoá của Shadowsocks 2022. Kiểm bằng bộ số chuẩn
/// của BLAKE3 trong ShadowsocksTests.
public static class Blake3
{
    private static readonly uint[] Iv = { 0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19 };
    private static readonly int[] Perm = { 2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8 };

    public static byte[] Hash(byte[] input, int outLen = 32) => Root(Iv, input, 0, outLen);

    public static byte[] DeriveKey(string context, byte[] material, int outLen = 32)
    {
        var ck = Root(Iv, Encoding.UTF8.GetBytes(context), 32, 32);
        var words = new uint[8];
        for (var i = 0; i < 8; i++) words[i] = BitConverter.ToUInt32(ck, i * 4);
        return Root(words, material, 64, outLen);
    }

    private static byte[] Root(uint[] key, byte[] input, uint flags, int outLen)
    {
        if (input.Length > 1024 || outLen is < 1 or > 32) throw new ArgumentException("BLAKE3: chỉ hỗ trợ đầu vào ≤ 1024 byte");
        var cv = key;
        var blocks = Math.Max(1, (input.Length + 63) / 64);
        var m = new uint[16];
        for (var b = 0; b < blocks; b++)
        {
            var start = b * 64;
            var len = Math.Min(64, input.Length - start);
            Array.Clear(m);
            for (var i = 0; i < len; i++) m[i >> 2] |= (uint)input[start + i] << (8 * (i & 3));
            var f = flags;
            if (b == 0) f |= 1;
            if (b == blocks - 1) f |= 2 | 8;
            cv = Compress(cv, m, (uint)Math.Max(0, len), f);
        }
        var r = new byte[outLen];
        for (var i = 0; i < outLen; i++) r[i] = (byte)(cv[i >> 2] >> (8 * (i & 3)));
        return r;
    }

    private static uint[] Compress(uint[] cv, uint[] block, uint len, uint flags)
    {
        var s = new uint[] { cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7], Iv[0], Iv[1], Iv[2], Iv[3], 0, 0, len, flags };
        var m = (uint[])block.Clone();
        for (var r = 0; r < 7; r++)
        {
            G(s, 0, 4, 8, 12, m[0], m[1]); G(s, 1, 5, 9, 13, m[2], m[3]);
            G(s, 2, 6, 10, 14, m[4], m[5]); G(s, 3, 7, 11, 15, m[6], m[7]);
            G(s, 0, 5, 10, 15, m[8], m[9]); G(s, 1, 6, 11, 12, m[10], m[11]);
            G(s, 2, 7, 8, 13, m[12], m[13]); G(s, 3, 4, 9, 14, m[14], m[15]);
            if (r < 6)
            {
                var p = new uint[16];
                for (var i = 0; i < 16; i++) p[i] = m[Perm[i]];
                m = p;
            }
        }
        var o = new uint[8];
        for (var i = 0; i < 8; i++) o[i] = s[i] ^ s[i + 8];
        return o;
    }

    private static uint Rotr(uint x, int n) => (x >> n) | (x << (32 - n));

    private static void G(uint[] s, int a, int b, int c, int d, uint x, uint y)
    {
        s[a] = s[a] + s[b] + x;
        s[d] = Rotr(s[d] ^ s[a], 16);
        s[c] = s[c] + s[d];
        s[b] = Rotr(s[b] ^ s[c], 12);
        s[a] = s[a] + s[b] + y;
        s[d] = Rotr(s[d] ^ s[a], 8);
        s[c] = s[c] + s[d];
        s[b] = Rotr(s[b] ^ s[c], 7);
    }
}

/// AEAD một chiều của phiên: khoá phiên + nonce 12 byte little-endian, tăng sau mỗi lần mã hoá / giải mã.
public sealed class SsAead : IDisposable
{
    private readonly AesGcm? _aes;
    private readonly ChaCha20Poly1305? _chacha;
    private readonly byte[] _nonce = new byte[12];

    public SsAead(SsMethod m, byte[] key)
    {
        if (m.Chacha) _chacha = new ChaCha20Poly1305(key);
        else _aes = new AesGcm(key, 16);
    }

    /// Bản mã + thẻ 16 byte.
    public byte[] Seal(ReadOnlySpan<byte> p)
    {
        var o = new byte[p.Length + 16];
        if (_aes != null) _aes.Encrypt(_nonce, p, o.AsSpan(0, p.Length), o.AsSpan(p.Length));
        else _chacha!.Encrypt(_nonce, p, o.AsSpan(0, p.Length), o.AsSpan(p.Length));
        Inc();
        return o;
    }

    /// Sai thẻ xác thực (sai khoá / dữ liệu hỏng) → CryptographicException.
    public byte[] Open(ReadOnlySpan<byte> c)
    {
        var o = new byte[c.Length - 16];
        if (_aes != null) _aes.Decrypt(_nonce, c[..^16], c[^16..], o);
        else _chacha!.Decrypt(_nonce, c[..^16], c[^16..], o);
        Inc();
        return o;
    }

    private void Inc()
    {
        for (var i = 0; i < 12; i++)
            if (++_nonce[i] != 0) break;
    }

    public void Dispose()
    {
        _aes?.Dispose();
        _chacha?.Dispose();
    }
}

/// Kết nối qua máy chủ Shadowsocks (luồng đã mã hoá chồng trên ổ cắm). Phần đầu (salt, phần nhận dạng người dùng, địa chỉ
/// đích) gửi chung một lần ghi với dữ liệu đầu tiên của bên gọi; StartAsync gửi phần đầu không kèm dữ liệu (thêm đệm ngẫu
/// nhiên) khi bên gọi chưa nói gì.
public sealed class SsStream : Stream
{
    private readonly Socket _socket;
    private readonly NetworkStream _raw;
    private readonly SsMethod _m;
    private readonly byte[][] _keys;
    private readonly byte[] _target;
    private readonly byte[] _salt;
    private readonly SsAead _enc;
    private SsAead? _dec;
    private byte[] _plain = Array.Empty<byte>();
    private int _plainPos;
    private readonly SemaphoreSlim _writeLock = new(1, 1);

    public bool Started { get; private set; }
    public Socket Socket => _socket;

    public SsStream(Socket socket, SsMethod m, byte[][] keys, byte[] target)
    {
        _socket = socket;
        _raw = new NetworkStream(socket, true);
        _m = m;
        _keys = keys;
        _target = target;
        _salt = RandomNumberGenerator.GetBytes(m.KeySize);
        _enc = new SsAead(m, Shadowsocks.Subkey(m, keys[^1], _salt));
    }

    public override bool CanRead => true;
    public override bool CanSeek => false;
    public override bool CanWrite => true;
    public override long Length => throw new NotSupportedException();
    public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
    public override void Flush() { }
    public override Task FlushAsync(CancellationToken ct) => Task.CompletedTask;
    public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
    public override void SetLength(long value) => throw new NotSupportedException();

    // ---- Gửi ----

    public override void Write(byte[] buffer, int offset, int count) => WriteAsync(buffer.AsMemory(offset, count)).AsTask().GetAwaiter().GetResult();

    public override Task WriteAsync(byte[] buffer, int offset, int count, CancellationToken ct) => WriteAsync(buffer.AsMemory(offset, count), ct).AsTask();

    public override async ValueTask WriteAsync(ReadOnlyMemory<byte> data, CancellationToken ct = default)
    {
        await _writeLock.WaitAsync(ct);
        try
        {
            var d = data;
            if (!Started)
            {
                var k = await Header(d, ct);
                d = d[k..];
            }
            var max = _m.Is2022 ? 0xFFFF : 0x3FFF;
            while (d.Length > 0)
            {
                var k = Math.Min(d.Length, max);
                var len = _enc.Seal(new[] { (byte)(k >> 8), (byte)k });
                var body = _enc.Seal(d.Span[..k]);
                var o = new byte[len.Length + body.Length];
                len.CopyTo(o, 0);
                body.CopyTo(o, len.Length);
                await _raw.WriteAsync(o, ct);
                d = d[k..];
            }
        }
        finally
        {
            _writeLock.Release();
        }
    }

    /// Gửi phần đầu ngay (chưa có dữ liệu của bên gọi).
    public async Task StartAsync(CancellationToken ct = default)
    {
        await _writeLock.WaitAsync(ct);
        try
        {
            if (!Started) await Header(ReadOnlyMemory<byte>.Empty, ct);
        }
        finally
        {
            _writeLock.Release();
        }
    }

    /// Ghi phần đầu kèm tối đa một phần dữ liệu; trả số byte dữ liệu đã gửi kèm.
    private async Task<int> Header(ReadOnlyMemory<byte> b, CancellationToken ct)
    {
        Started = true;
        using var o = new MemoryStream();
        o.Write(_salt);
        int sent;
        if (_m.Is2022)
        {
            // Phần nhận dạng (EIH): mỗi khoá trước khoá người dùng mã hoá 16 byte đầu BLAKE3(khoá kế tiếp) bằng AES-ECB
            for (var i = 0; i < _keys.Length - 1; i++)
            {
                var sub = Blake3.DeriveKey("shadowsocks 2022 identity subkey", _keys[i].Concat(_salt).ToArray(), _m.KeySize);
                using var aes = Aes.Create();
                aes.Key = sub;
                o.Write(aes.EncryptEcb(Blake3.Hash(_keys[i + 1], 16), PaddingMode.None));
            }
            var padding = b.Length < 900 ? RandomNumberGenerator.GetInt32(1, 901) : 0;
            sent = Math.Min(b.Length, 0xFFFF - _target.Length - 2 - padding);
            var varLen = _target.Length + 2 + padding + sent;
            var fixedH = new byte[11];
            fixedH[0] = 0;
            var ts = (ulong)DateTimeOffset.UtcNow.ToUnixTimeSeconds();
            for (var i = 0; i < 8; i++) fixedH[1 + i] = (byte)(ts >> (8 * (7 - i)));
            fixedH[9] = (byte)(varLen >> 8);
            fixedH[10] = (byte)varLen;
            var vh = new byte[varLen];
            _target.CopyTo(vh, 0);
            vh[_target.Length] = (byte)(padding >> 8);
            vh[_target.Length + 1] = (byte)padding;
            RandomNumberGenerator.Fill(vh.AsSpan(_target.Length + 2, padding));
            b.Span[..sent].CopyTo(vh.AsSpan(_target.Length + 2 + padding));
            o.Write(_enc.Seal(fixedH));
            o.Write(_enc.Seal(vh));
        }
        else
        {
            // Kiểu cũ: chunk đầu = địa chỉ đích + dữ liệu đầu
            sent = Math.Min(b.Length, 0x3FFF - _target.Length);
            var first = new byte[_target.Length + sent];
            _target.CopyTo(first, 0);
            b.Span[..sent].CopyTo(first.AsSpan(_target.Length));
            o.Write(_enc.Seal(new[] { (byte)(first.Length >> 8), (byte)first.Length }));
            o.Write(_enc.Seal(first));
        }
        await _raw.WriteAsync(o.ToArray(), ct); // một lần ghi (chuẩn SS 2022: không để lộ cỡ từng phần)
        return sent;
    }

    // ---- Nhận ----

    public override int Read(byte[] buffer, int offset, int count) => ReadAsync(buffer.AsMemory(offset, count)).AsTask().GetAwaiter().GetResult();

    public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken ct) => ReadAsync(buffer.AsMemory(offset, count), ct).AsTask();

    public override async ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken ct = default)
    {
        if (buffer.Length == 0) return 0;
        while (_plainPos >= _plain.Length)
            if (!await Fill(ct)) return 0;
        var n = Math.Min(buffer.Length, _plain.Length - _plainPos);
        _plain.AsSpan(_plainPos, n).CopyTo(buffer.Span);
        _plainPos += n;
        return n;
    }

    /// Chunk tiếp theo; false = máy chủ đóng kết nối (hết dữ liệu).
    private async Task<bool> Fill(CancellationToken ct)
    {
        if (_dec == null) return await Begin(ct);
        var lc = await ReadExact(18, true, ct);
        if (lc == null) return false;
        var l = OpenOrFail(_dec, lc, "dữ liệu từ máy chủ Shadowsocks bị hỏng");
        await Payload(_dec, l[0] << 8 | l[1], ct);
        return true;
    }

    private async Task Payload(SsAead d, int n, CancellationToken ct)
    {
        var c = await ReadExact(n + 16, false, ct);
        _plain = OpenOrFail(d, c!, "dữ liệu từ máy chủ Shadowsocks bị hỏng");
        _plainPos = 0;
    }

    private async Task<bool> Begin(CancellationToken ct)
    {
        // Máy chủ không nhận ra khoá thì đóng / ngắt / im lặng — chưa có trả lời nào là coi như bị từ chối
        byte[]? rs;
        try
        {
            rs = await ReadExact(_m.KeySize, true, ct);
        }
        catch (Exception e) when (e is IOException or SocketException)
        {
            throw new SsError(Shadowsocks.NoReply(_m));
        }
        if (rs == null) throw new SsError(Shadowsocks.NoReply(_m));
        var d = new SsAead(_m, Shadowsocks.Subkey(_m, _keys[^1], rs));
        _dec = d;
        if (!_m.Is2022) return await Fill(ct);
        var hl = 1 + 8 + _m.KeySize + 2;
        var c = await ReadExact(hl + 16, false, ct);
        var h = OpenOrFail(d, c!, "không giải mã được trả lời của máy chủ Shadowsocks — sai khoá hoặc sai kiểu mã hoá");
        if (h[0] != 1 || !h.AsSpan(9, _m.KeySize).SequenceEqual(_salt)) throw new SsError("máy chủ Shadowsocks trả lời sai giao thức");
        ulong ts = 0;
        for (var i = 1; i <= 8; i++) ts = ts << 8 | h[i];
        var skew = Math.Abs(DateTimeOffset.UtcNow.ToUnixTimeSeconds() - (long)ts);
        if (skew > 30) throw new SsError($"giờ trên máy lệch {skew} giây so với máy chủ (Shadowsocks 2022 cho lệch tối đa 30 giây)");
        await Payload(d, h[hl - 2] << 8 | h[hl - 1], ct);
        return true;
    }

    private static byte[] OpenOrFail(SsAead d, byte[] c, string message)
    {
        try
        {
            return d.Open(c);
        }
        catch (CryptographicException)
        {
            throw new SsError(message);
        }
    }

    private async Task<byte[]?> ReadExact(int n, bool eofOk, CancellationToken ct)
    {
        var b = new byte[n];
        var got = 0;
        while (got < n)
        {
            var r = await _raw.ReadAsync(b.AsMemory(got), ct);
            if (r == 0)
            {
                if (got == 0 && eofOk) return null;
                throw new IOException("máy chủ Shadowsocks ngắt kết nối giữa chừng");
            }
            got += r;
        }
        return b;
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _raw.Dispose();
            _enc.Dispose();
            _dec?.Dispose();
        }
        base.Dispose(disposing);
    }
}

/// Phía khách Shadowsocks (TCP): SS 2022 (SIP022, có phần nhận dạng người dùng cho máy chủ nhiều người dùng) + AEAD cũ.
public static class Shadowsocks
{
    /// Khoá từ mật khẩu. SS 2022: mỗi phần (cách nhau bằng ":") là khoá base64 đúng cỡ — phần cuối là khoá người dùng, các phần
    /// trước là khoá nhận dạng (khoá máy chủ của 3X-UI). Kiểu cũ: EVP_BytesToKey(MD5). Sai → SsError.
    public static byte[][] Keys(SsMethod m, string password)
    {
        if (password.Length == 0) throw new SsError("Chưa nhập khoá / mật khẩu Shadowsocks.");
        if (!m.Is2022) return new[] { EvpBytesToKey(Encoding.UTF8.GetBytes(password), m.KeySize) };
        var parts = password.Split(':').Select(p => p.Trim()).ToArray();
        if (parts.Length > 1 && m.Chacha)
            throw new SsError($"{m.Id} không dùng được với máy chủ nhiều người dùng (khoá dạng «khoá-máy-chủ:khoá-người-dùng») — chọn 2022-blake3-aes-256-gcm.");
        return parts.Select((p, i) =>
        {
            var which = parts.Length == 1 ? "Khoá" : i == parts.Length - 1 ? "Khoá người dùng" : "Khoá máy chủ";
            var k = SsBase64.Decode(p);
            if (k == null || k.Length != m.KeySize)
                throw new SsError($"{which} không đúng: {m.Id} cần khoá base64 dài {m.KeySize} byte ({(m.KeySize + 2) / 3 * 4} ký tự).");
            return k;
        }).ToArray();
    }

    public static byte[] Subkey(SsMethod m, byte[] key, byte[] salt) => m.Is2022
        ? Blake3.DeriveKey("shadowsocks 2022 session subkey", key.Concat(salt).ToArray(), m.KeySize)
        : HKDF.DeriveKey(HashAlgorithmName.SHA1, key, m.KeySize, salt, Encoding.ASCII.GetBytes("ss-subkey"));

    public static byte[] EvpBytesToKey(byte[] pass, int size)
    {
        var o = new List<byte>();
        var prev = Array.Empty<byte>();
        while (o.Count < size)
        {
            prev = MD5.HashData(prev.Concat(pass).ToArray());
            o.AddRange(prev);
        }
        return o.Take(size).ToArray();
    }

    public static string NoReply(SsMethod m) => "máy chủ Shadowsocks không trả lời — thường do sai khoá / sai kiểu mã hoá" +
        (m.Is2022 ? ", hoặc giờ trên máy lệch quá 30 giây" : "") + " (kiểm tra lại link ss:// trong 3X-UI)";

    /// Địa chỉ đích dạng SOCKS5 (ATYP + địa chỉ + cổng).
    public static byte[] Target(string host, IPAddress? ip, int port)
    {
        byte[] a;
        if (ip != null || IPAddress.TryParse(host, out ip))
        {
            var b = ip.GetAddressBytes();
            a = new[] { (byte)(b.Length == 4 ? 1 : 4) }.Concat(b).ToArray();
        }
        else
        {
            var h = Encoding.ASCII.GetBytes(host);
            if (h.Length > 255) throw new SocksError(1, "tên miền quá dài");
            a = new[] { (byte)3, (byte)h.Length }.Concat(h).ToArray();
        }
        return a.Concat(new[] { (byte)(port >> 8), (byte)port }).ToArray();
    }

    private static readonly object CacheLock = new();
    private static (SsMethod M, string P, byte[][] K)? _cached;

    /// Mở kết nối tới host:port qua máy chủ c (ip: đích đã phân giải; null = gửi tên miền). Chưa gửi gì: phần đầu đi cùng dữ
    /// liệu đầu tiên. Máy chủ không xác nhận kết nối — sai khoá chỉ lộ ra khi đọc trả lời (SsStream báo SsError).
    public static async Task<SsStream> Connect(SsConfig c, string host, IPAddress? ip, int port, CancellationToken ct)
    {
        var m = SsMethod.Of(c.Method) ?? throw new SocksError(1, $"kiểu mã hoá Shadowsocks «{c.Method}» chưa hỗ trợ");
        if (!m.Supported) throw new SocksError(1, $"máy này không có {m.Id} — đổi sang 2022-blake3-aes-256-gcm");
        byte[][] keys;
        lock (CacheLock)
        {
            if (_cached is { } k && k.M == m && k.P == c.Password) keys = k.K;
            else
            {
                try
                {
                    keys = Keys(m, c.Password);
                }
                catch (SsError e)
                {
                    throw new SocksError(1, e.Message);
                }
                _cached = (m, c.Password, keys);
            }
        }
        var t = Target(host, ip, port);
        var s = new Socket(SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            using var to = CancellationTokenSource.CreateLinkedTokenSource(ct);
            to.CancelAfter(10_000);
            await s.ConnectAsync(new DnsEndPoint(c.Host, c.Port), to.Token);
        }
        catch (Exception)
        {
            s.Dispose();
            throw new SocksError(1, $"không kết nối được máy chủ Shadowsocks {c.Host}:{c.Port}");
        }
        return new SsStream(s, m, keys, t);
    }

    /// Nút "Thử kết nối": hỏi Cloudflare (cdn-cgi/trace, HTTP — chặng mạng nhà → máy chủ đã mã hoá) qua máy chủ c, trả IP ra
    /// ngoài kèm nơi đặt máy, vd "103.1.2.3 (SG)". SS 2022 không trả lời thì xem giờ trên máy có lệch không.
    public static async Task<string> Check(SsConfig c)
    {
        const string host = "www.cloudflare.com";
        using var t = new CancellationTokenSource(20_000);
        await using var s = await Connect(c, host, null, 80, t.Token);
        try
        {
            await s.WriteAsync(Encoding.ASCII.GetBytes($"GET /cdn-cgi/trace HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Mozilla/5.0\r\nConnection: close\r\n\r\n"), t.Token);
            var text = await ReadHttp(s, t.Token);
            var ip = Regex.Match(text, @"(?m)^ip=(\S+)");
            if (!ip.Success) throw new IOException($"không đọc được kết quả từ {host}");
            var loc = Regex.Match(text, @"(?m)^loc=(\S+)");
            return ip.Groups[1].Value + (loc.Success ? $" ({loc.Groups[1].Value})" : "");
        }
        catch (Exception e) when (e is SsError or IOException or OperationCanceledException)
        {
            var m = SsMethod.Of(c.Method) ?? SsMethod.Default;
            if (m.Is2022 && await ClockSkew() is { } skew)
                throw new SsError($"giờ trên máy lệch {skew} giây — Shadowsocks 2022 cần giờ đúng (lệch tối đa 30 giây): chỉnh ngày giờ của máy");
            throw e as SsError ?? new SsError(NoReply(m));
        }
    }

    /// Đọc một trả lời HTTP: phần đầu + đủ Content-Length thì thôi, không chờ máy chủ đóng kết nối (Xray giữ thêm 1 giây sau khi
    /// trang đã đóng — thời gian thử báo ra sẽ sai).
    public static async Task<string> ReadHttp(Stream s, CancellationToken ct)
    {
        using var ms = new MemoryStream();
        var b = new byte[4096];
        int n;
        while ((n = await s.ReadAsync(b, ct)) > 0)
        {
            ms.Write(b, 0, n);
            var text = Encoding.Latin1.GetString(ms.GetBuffer(), 0, (int)ms.Length);
            var h = text.IndexOf("\r\n\r\n", StringComparison.Ordinal);
            if (h < 0) continue;
            var len = Regex.Match(text[..h], @"(?im)^content-length:\s*(\d+)");
            if (len.Success && ms.Length >= h + 4 + int.Parse(len.Groups[1].Value)) break;
        }
        return Encoding.UTF8.GetString(ms.ToArray());
    }

    /// Giờ trên máy lệch bao nhiêu giây so với Cloudflare (đi thẳng); null = không lệch quá 30 giây / không hỏi được.
    private static async Task<long?> ClockSkew()
    {
        try
        {
            using var h = new HttpClient(new SocketsHttpHandler { UseProxy = false }) { Timeout = TimeSpan.FromSeconds(8) };
            var text = await h.GetStringAsync("https://www.cloudflare.com/cdn-cgi/trace");
            var m = Regex.Match(text, @"(?m)^ts=(\d+)");
            if (!m.Success) return null;
            var skew = Math.Abs(DateTimeOffset.UtcNow.ToUnixTimeSeconds() - long.Parse(m.Groups[1].Value));
            return skew > 30 ? skew : null;
        }
        catch (Exception)
        {
            return null;
        }
    }
}
