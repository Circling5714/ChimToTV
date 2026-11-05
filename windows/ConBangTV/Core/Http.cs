using System.IO;
using System.Net;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;

namespace ConBangTV.Core;

/// Tải trang bằng HttpClient (bản Android: Http.kt, iPad: Http.swift). Tự đi theo chuyển hướng (kể cả đổi tên miền).
public static class Http
{
    public sealed class Response
    {
        /// Địa chỉ cuối cùng (sau chuyển hướng)
        public required string Url;
        public int Code;
        public required byte[] Data;
        public string? ContentType;
        public string Body => Text(Data, ContentType);
    }

    /// Trang trả lời bằng mã lỗi (4xx / 5xx): thử lại cũng vậy.
    public sealed class HttpError : Exception
    {
        public readonly int Code;
        public HttpError(int code) : base($"Trang trả về lỗi HTTP {code}") { Code = code; }
    }

    /// User-Agent chung cho cả app — đặt lại bằng chuỗi của WebView2 (Edge) khi trình duyệt đã chạy, để trang thấy cùng
    /// một trình duyệt.
    public static string UserAgent =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36 Edg/138.0.0.0";

    static Http()
    {
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
    }

    private static readonly HttpClient Client = new(new SocketsHttpHandler
    {
        AllowAutoRedirect = true,
        MaxAutomaticRedirections = 10,
        AutomaticDecompression = DecompressionMethods.All,
        UseCookies = true,
        CookieContainer = new CookieContainer(),
        ConnectTimeout = TimeSpan.FromSeconds(15),
        PooledConnectionLifetime = TimeSpan.FromMinutes(5),
        MaxConnectionsPerServer = 6,
        ConnectCallback = Net.HttpConnect,
    })
    {
        Timeout = TimeSpan.FromSeconds(25),
        DefaultRequestVersion = HttpVersion.Version11,
    };

    public static Task<Response> Get(string url, Dictionary<string, string>? headers = null, CancellationToken ct = default) =>
        Send(() => Request(HttpMethod.Get, url, headers), ct);

    /// POST dạng form (admin-ajax.php của WordPress).
    public static Task<Response> Post(string url, Dictionary<string, string> form, Dictionary<string, string>? headers = null) =>
        Send(() =>
        {
            var r = Request(HttpMethod.Post, url, headers);
            var body = string.Join("&", form.Select(kv => kv.Key.FormEncoded() + "=" + kv.Value.FormEncoded()));
            r.Content = new StringContent(body, Encoding.UTF8);
            r.Content.Headers.ContentType = MediaTypeHeaderValue.Parse("application/x-www-form-urlencoded; charset=UTF-8");
            return r;
        }, default);

    private static HttpRequestMessage Request(HttpMethod method, string url, Dictionary<string, string>? headers)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var u) || (u.Scheme != "http" && u.Scheme != "https"))
            throw new AppError($"Địa chỉ không hợp lệ: {url}");
        var r = new HttpRequestMessage(method, u);
        r.Headers.TryAddWithoutValidation("User-Agent", UserAgent);
        r.Headers.TryAddWithoutValidation("Accept-Language", "vi-VN,vi;q=0.9,en;q=0.8");
        if (headers != null)
            foreach (var (k, v) in headers)
                r.Headers.TryAddWithoutValidation(k, v);
        if (!r.Headers.Contains("Accept"))
            r.Headers.TryAddWithoutValidation("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8");
        return r;
    }

    private static async Task<Response> Send(Func<HttpRequestMessage> make, CancellationToken ct)
    {
        HttpResponseMessage resp;
        try
        {
            resp = await Client.SendAsync(make(), HttpCompletionOption.ResponseContentRead, ct);
        }
        catch (HttpRequestException e) when (e.InnerException is IOException)
        {
            // Kết nối giữ lại từ lần trước bị ngắt ngầm → thử lại một lần
            resp = await Client.SendAsync(make(), HttpCompletionOption.ResponseContentRead, ct);
        }
        using (resp)
        {
            var code = (int)resp.StatusCode;
            if (code >= 400) throw new HttpError(code);
            var data = await resp.Content.ReadAsByteArrayAsync(ct);
            return new Response
            {
                Url = resp.RequestMessage?.RequestUri?.AbsoluteUri ?? "",
                Code = code,
                Data = data,
                ContentType = resp.Content.Headers.ContentType?.ToString(),
            };
        }
    }

    /// Tải tệp lớn (bản cập nhật) vào luồng, báo tiến độ (0..1).
    public static async Task Download(string url, Stream to, Action<double>? progress = null, CancellationToken ct = default)
    {
        using var r = Request(HttpMethod.Get, url, new() { ["Accept"] = "application/octet-stream" });
        using var resp = await Client.SendAsync(r, HttpCompletionOption.ResponseHeadersRead, ct);
        if ((int)resp.StatusCode >= 400) throw new HttpError((int)resp.StatusCode);
        var total = resp.Content.Headers.ContentLength ?? 0;
        await using var input = await resp.Content.ReadAsStreamAsync(ct);
        var buf = new byte[81920];
        long done = 0;
        int n;
        while ((n = await input.ReadAsync(buf, ct)) > 0)
        {
            await to.WriteAsync(buf.AsMemory(0, n), ct);
            done += n;
            if (total > 0) progress?.Invoke((double)done / total);
        }
    }

    /// Giải mã theo charset trong Content-Type (mặc định UTF-8).
    public static string Text(byte[] data, string? contentType)
    {
        var ct = contentType?.ToLowerInvariant();
        var i = ct?.IndexOf("charset=", StringComparison.Ordinal) ?? -1;
        if (i >= 0)
        {
            var name = ct![(i + 8)..].Trim('"', '\'', ' ').Before(";");
            try
            {
                return Encoding.GetEncoding(name).GetString(data);
            }
            catch (ArgumentException)
            {
            }
        }
        return Encoding.UTF8.GetString(data);
    }
}
