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

    /// Cách tách gói bắt tay TLS. Mức 1: byte đầu riêng, tên trang từng mẩu 2 byte, phần còn lại. Mức 2: byte 0 (Ttl1 = gửi
    /// với TTL 1), byte 1, 2, 3 từng gói, Hold = gửi xong mẩu này thì chờ máy chủ xác nhận hết rồi mới gửi tiếp; phần còn
    /// lại như mức 1. Không phải gói bắt tay / tắt → cả gói một lần.
    public static List<(int Start, int Length, bool Ttl1, bool Hold)> HelloChunks(ReadOnlySpan<byte> buf, string host, int level)
    {
        var chunks = new List<(int, int, bool, bool)>();
        if (buf.Length <= 5 || buf[0] != 0x16 || buf[1] != 0x03 || level <= 0)
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
        var name = Encoding.ASCII.GetBytes(host.ToLowerInvariant());
        var at = name.Length >= 2 ? buf[i..].IndexOf(name) : -1;
        if (at >= 0)
        {
            var start = at + i;
            if (start > i) chunks.Add((i, start - i, false, false));
            for (var k = start; k < start + name.Length; k += 2) chunks.Add((k, Math.Min(2, start + name.Length - k), false, false));
            i = start + name.Length;
        }
        if (i < buf.Length) chunks.Add((i, buf.Length - i, false, false));
        return chunks;
    }

    /// Gửi gói bắt tay đã tách (TCP_NODELAY: mỗi lần gửi đi thành một gói riêng). Qua proxy: không đảo thứ tự (proxy gửi
    /// lại theo thứ tự), chờ 40 ms sau byte đầu để proxy kịp gửi riêng (như bản TV). Mức 2 cần hỏi được trạng thái TCP của
    /// socket (để biết lúc nào byte đầu đã được gửi lại xong) — không hỏi được thì gửi như mức 1.
    public static void WriteHello(Socket s, ReadOnlySpan<byte> buf, string host, int level, bool viaProxy = false)
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

    /// Luồng của HttpClient: lần ghi đầu (gói bắt tay của SslStream) được tách, sau đó như thường.
    public sealed class HelloSplit : Stream
    {
        private readonly NetworkStream _inner;
        private readonly Socket _socket;
        private readonly string _host;
        private readonly int _level;
        private readonly bool _viaProxy;
        private bool _sent;

        public HelloSplit(NetworkStream inner, Socket socket, string host, int level, bool viaProxy)
        {
            _inner = inner;
            _socket = socket;
            _host = host;
            _level = level;
            _viaProxy = viaProxy;
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
        public override int Read(byte[] buffer, int offset, int count) => _inner.Read(buffer, offset, count);
        public override ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken ct = default) => _inner.ReadAsync(buffer, ct);

        public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken ct) =>
            _inner.ReadAsync(buffer, offset, count, ct);

        public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
        public override void SetLength(long value) => throw new NotSupportedException();

        public override void Write(byte[] buffer, int offset, int count) => Write(buffer.AsSpan(offset, count));

        public override void Write(ReadOnlySpan<byte> buffer)
        {
            if (!_sent)
            {
                _sent = true;
                WriteHello(_socket, buffer, _host, _level, _viaProxy);
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
