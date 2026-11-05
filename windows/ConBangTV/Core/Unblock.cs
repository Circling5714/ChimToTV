using System.IO;
using System.Net.Sockets;
using System.Text;

namespace ConBangTV.Core;

/// «Vượt chặn nhà mạng» (bản TV: writeHello trong Socks5.kt). Nhà mạng chặn trang phim theo tên trang trong gói bắt tay TLS
/// (SNI); gửi riêng byte đầu tiên của gói bắt tay, rồi tên trang thành từng mẩu 2 byte là qua được (mức 1). Mức 2 gửi byte
/// đầu với TTL 1 (gói đó mất ở chặng đầu, máy gửi lại sau → máy chặn thấy gói đảo thứ tự). Kết nối mở ở Net.Connect;
/// HttpClient dùng HelloSplit, trình phát (cổng SOCKS5 nội bộ) gọi WriteHello.
public static class Unblock
{
    public static int Level => Prefs.UnblockLevel;

    /// Cách tách gói bắt tay TLS: byte đầu riêng (mức 2: TTL 1), tên trang từng mẩu 2 byte, phần còn lại. Không phải gói
    /// bắt tay / tắt → cả gói một lần.
    public static List<(int Start, int Length, bool Ttl1)> HelloChunks(ReadOnlySpan<byte> buf, string host, int level)
    {
        var chunks = new List<(int, int, bool)>();
        if (buf.Length <= 5 || buf[0] != 0x16 || buf[1] != 0x03 || level <= 0)
        {
            chunks.Add((0, buf.Length, false));
            return chunks;
        }
        chunks.Add((0, 1, level >= 2));
        var i = 1;
        var name = Encoding.ASCII.GetBytes(host.ToLowerInvariant());
        var at = name.Length >= 2 ? buf[1..].IndexOf(name) : -1;
        if (at >= 0)
        {
            var start = at + 1;
            if (start > i) chunks.Add((i, start - i, false));
            for (var k = start; k < start + name.Length; k += 2) chunks.Add((k, Math.Min(2, start + name.Length - k), false));
            i = start + name.Length;
        }
        if (i < buf.Length) chunks.Add((i, buf.Length - i, false));
        return chunks;
    }

    /// Gửi gói bắt tay đã tách (TCP_NODELAY: mỗi lần gửi đi thành một gói riêng). Qua proxy: không đảo thứ tự (proxy gửi
    /// lại theo thứ tự), chờ 40 ms sau byte đầu để proxy kịp gửi riêng (như bản TV).
    public static void WriteHello(Socket s, ReadOnlySpan<byte> buf, string host, int level, bool viaProxy = false)
    {
        var first = true;
        foreach (var (start, len, ttl1) in HelloChunks(buf, host, level))
        {
            if (ttl1 && !viaProxy)
            {
                var old = s.Ttl;
                try
                {
                    s.Ttl = 1;
                    SendAll(s, buf.Slice(start, len));
                }
                finally
                {
                    s.Ttl = old;
                }
            }
            else
            {
                SendAll(s, buf.Slice(start, len));
            }
            if (first && viaProxy && len < buf.Length) Thread.Sleep(40);
            first = false;
        }
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
