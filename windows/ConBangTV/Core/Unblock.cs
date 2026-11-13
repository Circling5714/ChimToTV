using System.Diagnostics;
using System.IO;
using System.Net.Sockets;
using System.Text;

namespace ConBangTV.Core;

/// «Vượt chặn nhà mạng» (bản TV: writeHello trong Socks5.kt). Nhà mạng chặn trang phim theo tên trang trong gói bắt tay TLS
/// (SNI); gửi riêng byte đầu tiên của gói bắt tay, rồi tên trang thành từng mẩu 2 byte là qua được (mức 1). Mức 2 gửi byte
/// đầu với TTL 1 (gói đó mất ở chặng đầu, máy gửi lại sau → máy chặn thấy gói đảo thứ tự). Kết nối mở ở Net.Connect;
/// HttpClient dùng HelloSplit, trình phát (cổng SOCKS5 nội bộ) gọi WriteHello.
///
/// Mức 2 trên Windows khác bản TV / iPad (đo 12/11/2026): Linux gửi lại đúng 1 byte bị mất, còn Windows gửi lại MỌI byte
/// chưa được xác nhận trong một gói — gửi cả gói bắt tay rồi mới để Windows gửi lại thì gói gửi lại chứa nguyên gói bắt
/// tay (có tên trang) → bị chặn, tệ hơn mức 1. Nên ở đây: byte 0 (TTL 1), rồi byte 1, 2, 3 mỗi byte một gói (máy chủ báo
/// thiếu 3 lần → Windows gửi lại ngay 4 byte đầu, chưa có tên trang), CHỜ máy chủ xác nhận, rồi mới gửi phần còn lại như
/// mức 1. Máy chặn vẫn thấy gói đảo thứ tự, và không gói nào chứa tên trang nguyên vẹn.
public static class Unblock
{
    public static int Level => Prefs.UnblockLevel;

    /// Gói đầu của bên gọi là gói bắt tay TLS (ClientHello).
    public static bool IsTlsHello(ReadOnlySpan<byte> b) => b.Length > 5 && b[0] == 0x16 && b[1] == 0x03;

    private static readonly byte[][] HttpMethods =
        new[] { "GET ", "POST ", "HEAD ", "PUT ", "DELETE ", "OPTIONS ", "PATCH " }.Select(Encoding.ASCII.GetBytes).ToArray();

    /// Gói đầu là một yêu cầu HTTP thường (không mã hoá).
    public static bool IsHttpRequest(ReadOnlySpan<byte> b)
    {
        foreach (var m in HttpMethods)
            if (b.Length > m.Length && b.StartsWith(m)) return true;
        return false;
    }

    /// Cách tách gói đầu. Gói bắt tay TLS — mức 1: byte đầu riêng, tên trang từng mẩu 2 byte, phần còn lại. Yêu cầu HTTP thường
    /// (từ 11/2026 — trước chỉ tách TLS): byte đầu riêng, tới giữa chữ «Host», phần còn lại của «Host: », giá trị Host từng mẩu
    /// 2 byte, phần còn lại. Mức 2: byte 0 (Ttl1 = gửi với TTL 1), byte 1, 2, 3 từng gói, Hold = gửi xong mẩu này thì chờ máy
    /// chủ xác nhận hết rồi mới gửi tiếp; phần còn lại như mức 1. Gói khác / tắt → cả gói một lần.
    public static List<(int Start, int Length, bool Ttl1, bool Hold)> HelloChunks(ReadOnlySpan<byte> buf, string host, int level)
    {
        var chunks = new List<(int, int, bool, bool)>();
        var tls = IsTlsHello(buf);
        if ((!tls && !IsHttpRequest(buf)) || level <= 0)
        {
            chunks.Add((0, buf.Length, false, false));
            return chunks;
        }
        var i = 1;
        if (level >= 2)
        {
            chunks.Add((0, 1, true, false));
            chunks.Add((1, 1, false, false));
            chunks.Add((2, 1, false, false));
            chunks.Add((3, 1, false, true));
            i = 4;
        }
        else
        {
            chunks.Add((0, 1, false, false));
        }
        void Add(int start, int end)
        {
            if (end > i && end > start) chunks.Add((Math.Max(start, i), end - Math.Max(start, i), false, false));
            i = Math.Max(i, end);
        }
        void Pieces(int start, int end)
        {
            for (var k = start; k < end; k += 2) Add(k, Math.Min(k + 2, end));
        }
        if (tls)
        {
            var name = Encoding.ASCII.GetBytes(host.ToLowerInvariant());
            var at = name.Length >= 2 ? buf[i..].IndexOf(name) : -1;
            if (at >= 0)
            {
                var start = at + i;
                Add(i, start);
                Pieces(start, start + name.Length);
            }
        }
        else
        {
            var at = IndexOfIgnoreCase(buf, "\r\nhost:"u8);
            if (at >= 0)
            {
                var k = at + 2; // chữ «Host»
                var v = k + 5;
                while (v < buf.Length && (buf[v] == ' ' || buf[v] == '\t')) v++;
                var e = v;
                while (e < buf.Length && buf[e] != '\r' && buf[e] != '\n') e++;
                Add(i, k + 2);
                Add(k + 2, v);
                Pieces(v, e);
            }
        }
        Add(i, buf.Length);
        return chunks;
    }

    /// Vị trí của name (chữ thường) trong b, không phân biệt hoa / thường; -1 = không có.
    private static int IndexOfIgnoreCase(ReadOnlySpan<byte> b, ReadOnlySpan<byte> name)
    {
        for (var s = 0; s + name.Length <= b.Length; s++)
        {
            var k = 0;
            while (k < name.Length && (b[s + k] is >= (byte)'A' and <= (byte)'Z' ? b[s + k] | 0x20 : b[s + k]) == name[k]) k++;
            if (k == name.Length) return s;
        }
        return -1;
    }

    /// Gửi gói đầu đã tách (TCP_NODELAY: mỗi lần gửi đi thành một gói riêng). Qua proxy: không đảo thứ tự (proxy gửi lại theo
    /// thứ tự), chờ 40 ms sau byte đầu để proxy kịp gửi riêng (như bản TV). Mức 2 cần hỏi được trạng thái TCP của socket (để
    /// biết lúc nào byte đầu đã được gửi lại xong) — không hỏi được thì gửi như mức 1. Trả về mức đã gửi thật.
    public static int WriteHello(Socket s, ReadOnlySpan<byte> buf, string host, int level, bool viaProxy = false)
    {
        if (level >= 2 && (viaProxy || InFlight(s) == null)) level = 1;
        var first = true;
        foreach (var (start, len, ttl1, hold) in HelloChunks(buf, host, level))
        {
            if (ttl1) SendLowTtl(s, buf.Slice(start, len));
            else SendAll(s, buf.Slice(start, len));
            if (hold) WaitAcked(s);
            if (first && viaProxy && len < buf.Length) Thread.Sleep(40);
            first = false;
        }
        return level;
    }

    // ---- Tự lên Mức 2 ----

    /// Ở Mức 1, chờ trang trả lời gói bắt tay bao lâu thì coi là bị chặn và thử Mức 2 (test đặt ngắn).
    public static int FirstReplyMs { get; set; } = 4000;

    /// Ở Mức 2 (cách cuối cùng): chờ lâu hơn lúc nhà mạng giữ kết nối treo (đo: 15 giây).
    public const int Level2ReplyMs = 20_000;

    /// Đọc gói đầu của bên gọi; gói bắt tay TLS thì đọc trọn bản ghi đầu (có thể dài hơn một lần đọc). 0 = bên gọi đã đóng.
    public static async Task<int> ReadFirst(Stream s, byte[] buf, CancellationToken ct)
    {
        var n = await s.ReadAsync(buf, ct);
        if (n >= 5 && buf[0] == 0x16 && buf[1] == 0x03)
        {
            var need = Math.Min(buf.Length, 5 + (buf[3] << 8 | buf[4]));
            using var t = CancellationTokenSource.CreateLinkedTokenSource(ct);
            t.CancelAfter(2000);
            try
            {
                while (n < need)
                {
                    var m = await s.ReadAsync(buf.AsMemory(n, need - n), t.Token);
                    if (m <= 0) break;
                    n += m;
                }
            }
            catch (OperationCanceledException) when (!ct.IsCancellationRequested)
            {
            }
        }
        return n;
    }

    /// Chờ trang trả lời (tối đa ms). 0 = trang im / cắt kết nối / lỗi — kiểu nhà mạng chặn.
    public static async Task<int> FirstReply(Stream s, Memory<byte> buf, int ms, CancellationToken ct)
    {
        using var t = CancellationTokenSource.CreateLinkedTokenSource(ct);
        t.CancelAfter(ms);
        try
        {
            return await s.ReadAsync(buf, t.Token);
        }
        catch (OperationCanceledException) when (!ct.IsCancellationRequested)
        {
            return 0;
        }
        catch (IOException)
        {
            return 0;
        }
        catch (SocketException)
        {
            return 0;
        }
        catch (ObjectDisposedException)
        {
            return 0;
        }
    }

    /// Tên miền gốc, để nhớ theo trang chứ không theo từng tên con: img.phimmoi.net → phimmoi.net, www.abc.com.vn → abc.com.vn.
    public static string BaseDomain(string host)
    {
        var h = host.Trim('[', ']').TrimEnd('.').ToLowerInvariant();
        if (System.Net.IPAddress.TryParse(h, out _)) return h;
        var labels = h.Split('.');
        if (labels.Length <= 2) return h;
        var three = labels[^1].Length == 2 && SecondLevel.Contains(labels[^2]);
        return string.Join('.', labels[^(three ? 3 : 2)..]);
    }

    private static readonly HashSet<string> SecondLevel =
        new() { "com", "net", "org", "edu", "gov", "co", "ac", "info", "biz", "name", "pro", "int", "health" };

    /// Vượt chặn tự lên Mức 2 (11/2026, như bản TV): đang Mức 1 mà trang không trả lời gói bắt tay (nhà mạng đã ghép được các
    /// mẩu và chặn) thì mở kết nối mới, gửi lại theo Mức 2; lọt thì nhớ trang (tên miền gốc) dùng Mức 2 trong 7 ngày — các kết
    /// nối sau đi Mức 2 ngay, khỏi chờ. Nhà mạng thôi chặn thì sau 7 ngày tự về Mức 1.
    public static class Level2Memo
    {
        public const long KeepMs = 7L * 24 * 3600 * 1000;
        private static readonly object Lock = new();
        private static Dictionary<string, long>? _hosts;

        /// Lưu xuống máy (mặc định Prefs; test đặt null để chỉ giữ trong bộ nhớ).
        public static bool Persist { get; set; } = true;

        private static Dictionary<string, long> Hosts => _hosts ??= Persist ? Prefs.Level2Hosts : new();

        public static void Load(Dictionary<string, long> saved, long now)
        {
            lock (Lock) _hosts = saved.Where(e => now - e.Value >= 0 && now - e.Value < KeepMs).ToDictionary(e => e.Key, e => e.Value);
        }

        public static bool Needs(string host, long? now = null)
        {
            lock (Lock)
                return Hosts.TryGetValue(BaseDomain(host), out var t) && (now ?? Util.NowMs()) - t is >= 0 and < KeepMs;
        }

        public static void Remember(string host, long? now = null)
        {
            Dictionary<string, long> copy;
            lock (Lock)
            {
                Hosts[BaseDomain(host)] = now ?? Util.NowMs();
                copy = new(Hosts);
            }
            if (Persist) Prefs.Level2Hosts = copy;
        }

        public static Dictionary<string, long> All()
        {
            lock (Lock) return new(Hosts);
        }

        public static void Clear()
        {
            lock (Lock) _hosts = new();
            if (Persist) Prefs.Level2Hosts = new();
        }

        /// Số lần tự lên Mức 2 từ lúc mở app — gỡ lỗi.
        public static int AutoCount;
    }

#if DEBUG
    /// Chỉ bản gỡ lỗi (cổng /unblockhang): ở Mức 1, gửi nguyên gói bắt tay tới các trang này — như nhà mạng ghép được các
    /// mẩu — để thử tự lên Mức 2 với chính máy soi của nhà mạng.
    public static HashSet<string> DebugWholeHello = new();
#endif

    /// Gửi gói bắt tay theo mức (Mức 1 + trang đã nhớ → Mức 2). Trả về mức đã gửi thật.
    public static int SendHello(Socket s, ReadOnlySpan<byte> hello, string host, int level)
    {
        if (level == 1 && Level2Memo.Needs(host)) level = 2;
#if DEBUG
        if (level == 1 && DebugWholeHello.Contains(host.TrimEnd('.').ToLowerInvariant()))
        {
            SendAll(s, hello);
            return 1;
        }
#endif
        return WriteHello(s, hello, host, level);
    }

    /// Gửi với TTL 1 (gói mất ở chặng đầu tiên) rồi trả TTL về như cũ. Không đặt được TTL thì gửi thường.
    private static void SendLowTtl(Socket s, ReadOnlySpan<byte> b)
    {
        short old;
        try
        {
            old = s.Ttl;
            s.Ttl = 1;
        }
        catch (SocketException)
        {
            SendAll(s, b);
            return;
        }
        try
        {
            SendAll(s, b);
        }
        finally
        {
            try { s.Ttl = old; } catch (SocketException) { }
        }
    }

    private const int SioTcpInfo = unchecked((int)0xD8000027);
    private const int HoldMs = 1500;

    /// Số byte đã gửi mà máy chủ chưa xác nhận (SIO_TCP_INFO — Windows 10 1703 trở lên; trường BytesInFlight của
    /// TCP_INFO_v0 nằm ở byte thứ 28). null = không hỏi được.
    public static int? InFlight(Socket s)
    {
        try
        {
            var info = new byte[128];
            var n = s.IOControl(SioTcpInfo, new byte[4], info);
            return n >= 32 ? BitConverter.ToInt32(info, 28) : null;
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// Chờ máy chủ xác nhận hết phần đã gửi: byte đầu mất ở chặng đầu → máy chủ báo thiếu → Windows gửi lại 4 byte đầu →
    /// máy chủ xác nhận (khoảng 2 lượt đi về). Quá HoldMs vẫn chưa xong (mạng rớt gói nặng) thì cứ gửi tiếp.
    private static void WaitAcked(Socket s)
    {
        var sw = Stopwatch.StartNew();
        while (sw.ElapsedMilliseconds < HoldMs && InFlight(s) is > 0) Thread.Sleep(2);
    }

    private static void SendAll(Socket s, ReadOnlySpan<byte> b)
    {
        while (b.Length > 0)
        {
            var n = s.Send(b);
            b = b[n..];
        }
    }

    /// Luồng của HttpClient: lần ghi đầu (gói bắt tay của SslStream, hay yêu cầu HTTP thường) được tách, sau đó như thường.
    /// Đi thẳng, gói bắt tay TLS ở Mức 1: lần đọc đầu chờ trang trả lời tối đa FirstReplyMs; trang im / cắt kết nối → mở kết
    /// nối mới (reconnect), gửi lại gói bắt tay theo Mức 2 rồi đọc tiếp ở kết nối đó (SslStream chưa nhận byte nào nên không
    /// biết gì); lọt thì nhớ trang (Level2Memo).
    public sealed class HelloSplit : Stream
    {
        private NetworkStream _inner;
        private Socket _socket;
        private readonly string _host;
        private readonly int _level;
        private readonly bool _viaProxy;
        private readonly Func<CancellationToken, Task<Socket>>? _reconnect;
        private bool _sent;
        private byte[]? _hello;
        private int _helloLevel;

        public HelloSplit(NetworkStream inner, Socket socket, string host, int level, bool viaProxy,
            Func<CancellationToken, Task<Socket>>? reconnect = null)
        {
            _inner = inner;
            _socket = socket;
            _host = host;
            _level = level;
            _viaProxy = viaProxy;
            _reconnect = reconnect;
        }

        /// Lần ghi đầu: tách và gửi; gói bắt tay TLS đi thẳng thì giữ lại để có thể gửi lại theo Mức 2.
        private void SendFirst(ReadOnlySpan<byte> buffer)
        {
            _sent = true;
            if (_viaProxy || !IsTlsHello(buffer) || _reconnect == null)
            {
                WriteHello(_socket, buffer, _host, _level, _viaProxy);
                return;
            }
            _hello = buffer.ToArray();
            _helloLevel = SendHello(_socket, buffer, _host, _level);
        }

        /// Trả lời đầu của trang, đọc vào bộ đệm riêng: SslStream (.NET 8) hay đọc 0 byte để chờ dữ liệu — không được lấy kết
        /// quả 0 của lần đọc đó làm «trang im».
        private byte[]? _pending;
        private int _pendingAt;
        private int _pendingLen;

        /// Lần đọc đầu sau gói bắt tay: chờ trang trả lời (vào _pending); im ở Mức 1 → thử lại theo Mức 2. 0 = không được.
        private async ValueTask<int> FirstReadAsync(Memory<byte> buffer, CancellationToken ct)
        {
            var hello = _hello!;
            _hello = null;
            var got = new byte[16 * 1024];
            var n = await FirstReply(_inner, got, _helloLevel >= 2 ? Level2ReplyMs : FirstReplyMs, ct);
            if (n <= 0 && _helloLevel < 2)
            {
                Socket? s2;
                try
                {
                    s2 = await _reconnect!(ct);
                }
                catch (Exception) when (!ct.IsCancellationRequested)
                {
                    s2 = null;
                }
                if (s2 != null)
                {
                    var old = _inner;
                    _socket = s2;
                    _inner = new NetworkStream(s2, true);
                    old.Dispose();
                    var applied = WriteHello(s2, hello, _host, 2);
                    n = await FirstReply(_inner, got, Level2ReplyMs, ct);
                    Log.D("net", $"vượt chặn: {_host} không trả lời ở mức 1 → mức 2: {(n > 0 ? "lọt" : "vẫn không được")}" +
                        (applied >= 2 ? "" : " (không hỏi được trạng thái TCP)"));
                    if (n > 0 && applied >= 2)
                    {
                        Level2Memo.Remember(_host);
                        Interlocked.Increment(ref Level2Memo.AutoCount);
                    }
                }
            }
            if (n <= 0) return 0;
            _pending = got;
            _pendingAt = 0;
            _pendingLen = n;
            return TakePending(buffer.Span);
        }

        /// Chép phần trả lời đầu còn giữ sang bộ đệm của bên đọc (bộ đệm 0 byte: chỉ báo «có dữ liệu»).
        private int TakePending(Span<byte> dst)
        {
            var n = Math.Min(dst.Length, _pendingLen);
            _pending.AsSpan(_pendingAt, n).CopyTo(dst);
            _pendingAt += n;
            _pendingLen -= n;
            if (_pendingLen == 0) _pending = null;
            return n;
        }

        public override bool CanRead => true;
        public override bool CanSeek => false;
        public override bool CanWrite => true;
        public override long Length => throw new NotSupportedException();

        public override long Position
        {
            get => throw new NotSupportedException();
            set => throw new NotSupportedException();
        }

        public override void Flush() => _inner.Flush();
        public override Task FlushAsync(CancellationToken ct) => _inner.FlushAsync(ct);

        public override int Read(byte[] buffer, int offset, int count) =>
            _hello != null ? FirstReadAsync(buffer.AsMemory(offset, count), default).AsTask().GetAwaiter().GetResult()
            : _pending != null ? TakePending(buffer.AsSpan(offset, count))
            : _inner.Read(buffer, offset, count);

        public override ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken ct = default) =>
            _hello != null ? FirstReadAsync(buffer, ct)
            : _pending != null ? ValueTask.FromResult(TakePending(buffer.Span))
            : _inner.ReadAsync(buffer, ct);

        public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken ct) =>
            ReadAsync(buffer.AsMemory(offset, count), ct).AsTask();

        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();

        public override void Write(byte[] buffer, int offset, int count) => Write(buffer.AsSpan(offset, count));

        public override void Write(ReadOnlySpan<byte> buffer)
        {
            if (!_sent)
            {
                SendFirst(buffer);
                return;
            }
            _inner.Write(buffer);
        }

        public override ValueTask WriteAsync(ReadOnlyMemory<byte> buffer, CancellationToken ct = default)
        {
            if (!_sent)
            {
                Write(buffer.Span);
                return ValueTask.CompletedTask;
            }
            return _inner.WriteAsync(buffer, ct);
        }

        public override Task WriteAsync(byte[] buffer, int offset, int count, CancellationToken ct) =>
            WriteAsync(buffer.AsMemory(offset, count), ct).AsTask();

        protected override void Dispose(bool disposing)
        {
            if (disposing) _inner.Dispose();
            base.Dispose(disposing);
        }
    }
}
