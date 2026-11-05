using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Sockets;
using System.Text;

namespace ConBangTV.Core;

/// «Vượt chặn nhà mạng» (bản TV: writeHello trong Socks5.kt). Nhà mạng chặn trang phim theo tên trang trong gói bắt tay TLS
/// (SNI); gửi riêng byte đầu tiên của gói bắt tay, rồi tên trang thành từng mẩu 2 byte là qua được (mức 1). Mức 2 gửi byte
/// đầu với TTL 1 (gói đó mất ở chặng đầu, máy gửi lại sau → máy chặn thấy gói đảo thứ tự).
/// - Phần app tự tải (trang, API, ảnh bìa): HttpClient kết nối qua Connect() bên dưới.
/// - Trình phát (WebView2): đi qua cổng chuyển tiếp nội bộ 127.0.0.1 (proxy HTTP CONNECT) cũng tách gói như vậy.
public static class Unblock
{
    public static int Level => Prefs.UnblockLevel;

    // ---- HttpClient ----

    /// ConnectCallback của SocketsHttpHandler: kết nối thường; bật vượt chặn thì gói bắt tay đầu tiên được tách.
    public static async ValueTask<Stream> Connect(SocketsHttpConnectionContext ctx, CancellationToken ct)
    {
        var s = new Socket(SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            await s.ConnectAsync(ctx.DnsEndPoint, ct);
        }
        catch
        {
            s.Dispose();
            throw;
        }
        var ns = new NetworkStream(s, true);
        var level = Level;
        return level > 0 && ctx.DnsEndPoint.Port == 443 ? new HelloSplit(ns, s, ctx.DnsEndPoint.Host, level) : ns;
    }

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

    /// Gửi gói bắt tay đã tách (TCP_NODELAY: mỗi lần gửi đi thành một gói riêng).
    public static void WriteHello(Socket s, ReadOnlySpan<byte> buf, string host, int level)
    {
        foreach (var (start, len, ttl1) in HelloChunks(buf, host, level))
        {
            if (!ttl1)
            {
                SendAll(s, buf.Slice(start, len));
                continue;
            }
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
    private sealed class HelloSplit : Stream
    {
        private readonly NetworkStream _inner;
        private readonly Socket _socket;
        private readonly string _host;
        private readonly int _level;
        private bool _sent;

        public HelloSplit(NetworkStream inner, Socket socket, string host, int level)
        {
            _inner = inner;
            _socket = socket;
            _host = host;
            _level = level;
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
                WriteHello(_socket, buffer, _host, _level);
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

    // ---- Cổng chuyển tiếp cho WebView2 ----

    private static TcpListener? _listener;

    /// Cổng của proxy nội bộ (0 = chưa chạy).
    public static int ProxyPort { get; private set; }

    /// Mở proxy nội bộ (gọi trước khi tạo WebView2 khi đang bật vượt chặn).
    public static int StartProxy()
    {
        if (_listener != null) return ProxyPort;
        _listener = new TcpListener(IPAddress.Loopback, 0);
        _listener.Start(256);
        ProxyPort = ((IPEndPoint)_listener.LocalEndpoint).Port;
        _ = Task.Run(async () =>
        {
            while (true)
            {
                TcpClient c;
                try
                {
                    c = await _listener.AcceptTcpClientAsync();
                }
                catch (Exception)
                {
                    return;
                }
                _ = Task.Run(() => Serve(c));
            }
        });
        Log.D("unblock", $"Proxy nội bộ 127.0.0.1:{ProxyPort}, mức {Level}");
        return ProxyPort;
    }

    private static async Task Serve(TcpClient client)
    {
        using var c = client;
        Socket? up = null;
        try
        {
            c.NoDelay = true;
            var cs = c.GetStream();
            var (head, extra) = await ReadHead(cs);
            if (head == null) return;
            var firstEnd = head.IndexOf("\r\n", StringComparison.Ordinal);
            var first = firstEnd < 0 ? head : head[..firstEnd];
            var parts = first.Split(' ');
            if (parts.Length < 3) return;
            string host;
            int port;
            byte[] toSend;
            var connect = parts[0].Equals("CONNECT", StringComparison.OrdinalIgnoreCase);
            if (connect)
            {
                var hp = parts[1];
                var colon = hp.LastIndexOf(':');
                host = colon > 0 ? hp[..colon].Trim('[', ']') : hp;
                port = colon > 0 && int.TryParse(hp[(colon + 1)..], out var p) ? p : 443;
                toSend = extra;
            }
            else
            {
                // Yêu cầu http:// thường (dạng địa chỉ đầy đủ): đổi về đường dẫn rồi chuyển tiếp nguyên vẹn
                if (!Uri.TryCreate(parts[1], UriKind.Absolute, out var u)) return;
                host = u.Host;
                port = u.Port;
                var rest = firstEnd < 0 ? "\r\n\r\n" : head[firstEnd..];
                toSend = Encoding.ASCII.GetBytes($"{parts[0]} {u.PathAndQuery} {parts[2]}{rest}").Concat(extra).ToArray();
            }
            up = new Socket(SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
            await up.ConnectAsync(new DnsEndPoint(host, port));
            if (connect) await cs.WriteAsync(Encoding.ASCII.GetBytes("HTTP/1.1 200 Connection Established\r\n\r\n"));
            var us = new NetworkStream(up, false);
            var level = Level;
            var helloPending = connect && port == 443 && level > 0;
            if (toSend.Length > 0)
            {
                if (helloPending)
                {
                    WriteHello(up, toSend, host, level);
                    helloPending = false;
                }
                else await us.WriteAsync(toSend);
            }
            var toUp = Task.Run(async () =>
            {
                var buf = new byte[32 * 1024];
                int n;
                while ((n = await cs.ReadAsync(buf)) > 0)
                {
                    if (helloPending)
                    {
                        helloPending = false;
                        WriteHello(up, buf.AsSpan(0, n), host, level);
                    }
                    else await us.WriteAsync(buf.AsMemory(0, n));
                }
                up.Shutdown(SocketShutdown.Send);
            });
            var toClient = us.CopyToAsync(cs, 64 * 1024);
            await Task.WhenAny(toUp, toClient);
            await Task.WhenAny(Task.WhenAll(toUp, toClient), Task.Delay(2000));
        }
        catch (Exception)
        {
        }
        finally
        {
            up?.Dispose();
        }
    }

    /// Đọc phần đầu yêu cầu tới hết dòng trống; trả kèm phần dữ liệu đọc lố.
    private static async Task<(string? Head, byte[] Extra)> ReadHead(NetworkStream s)
    {
        var buf = new byte[16 * 1024];
        var len = 0;
        while (len < buf.Length)
        {
            var n = await s.ReadAsync(buf.AsMemory(len));
            if (n <= 0) return (null, Array.Empty<byte>());
            len += n;
            var end = buf.AsSpan(0, len).IndexOf("\r\n\r\n"u8);
            if (end >= 0)
            {
                var head = Encoding.ASCII.GetString(buf, 0, end + 4);
                return (head, buf.AsSpan(end + 4, len - end - 4).ToArray());
            }
        }
        return (null, Array.Empty<byte>());
    }
}
