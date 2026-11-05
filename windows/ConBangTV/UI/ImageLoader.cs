using System.IO;
using System.Net;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using ConBangTV.Core;
using SkiaSharp;

namespace ConBangTV.UI;

/// Tải ảnh bìa: bộ nhớ (400 ảnh gần nhất) → ổ đĩa (%LOCALAPPDATA%\ConBangTV\img, giữ 14 ngày) → mạng (6 ảnh cùng lúc).
/// Ảnh WebP giải mã bằng Skia (Windows không phải máy nào cũng có bộ giải mã WebP); ảnh khác dùng WIC của Windows.
public static class ImageLoader
{
    public const int DecodeWidth = 330;

    private static readonly HttpClient Client = new(new SocketsHttpHandler
    {
        AutomaticDecompression = DecompressionMethods.All,
        ConnectTimeout = TimeSpan.FromSeconds(10),
        MaxConnectionsPerServer = 6,
        ConnectCallback = Net.HttpConnect,
    })
    {
        Timeout = TimeSpan.FromSeconds(20),
    };

    private static readonly SemaphoreSlim Gate = new(6);
    private static readonly Dictionary<string, ImageSource> Memory = new();
    private static readonly LinkedList<string> Order = new();
    private static readonly Dictionary<string, Task<ImageSource?>> Pending = new();
    private static string Dir => Path.Combine(Prefs.DataDir, "img");

    public static readonly DependencyProperty UrlProperty = DependencyProperty.RegisterAttached(
        "Url", typeof(string), typeof(ImageLoader), new PropertyMetadata(null, OnUrlChanged));

    public static void SetUrl(Image img, string? url) => img.SetValue(UrlProperty, url);

    public static string? GetUrl(Image img) => (string?)img.GetValue(UrlProperty);

    private static async void OnUrlChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is not Image img) return;
        img.Source = null;
        var url = e.NewValue as string;
        if (string.IsNullOrEmpty(url)) return;
        var src = await Load(url);
        if (GetUrl(img) == url) img.Source = src;
    }

    /// Dọn ảnh cũ trên ổ đĩa (lúc mở app, chạy nền).
    public static void Cleanup() => Task.Run(() =>
    {
        try
        {
            if (!Directory.Exists(Dir)) return;
            var cut = DateTime.UtcNow.AddDays(-14);
            foreach (var f in new DirectoryInfo(Dir).EnumerateFiles())
                if (f.LastWriteTimeUtc < cut) f.Delete();
        }
        catch (Exception)
        {
        }
    });

    public static Task<ImageSource?> Load(string url)
    {
        lock (Memory)
        {
            if (Memory.TryGetValue(url, out var hit))
            {
                Order.Remove(url);
                Order.AddFirst(url);
                return Task.FromResult<ImageSource?>(hit);
            }
            if (Pending.TryGetValue(url, out var p)) return p;
            var t = Fetch(url);
            Pending[url] = t;
            return t;
        }
    }

    private static async Task<ImageSource?> Fetch(string url)
    {
        ImageSource? img = null;
        try
        {
            var file = Path.Combine(Dir, Hash(url));
            byte[]? bytes = null;
            if (File.Exists(file)) bytes = await File.ReadAllBytesAsync(file);
            if (bytes == null)
            {
                await Gate.WaitAsync();
                try
                {
                    using var req = new HttpRequestMessage(HttpMethod.Get, url);
                    req.Headers.TryAddWithoutValidation("User-Agent", Http.UserAgent);
                    req.Headers.TryAddWithoutValidation("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8");
                    // Nhiều CDN ảnh chỉ cho trang của mình lấy ảnh (chống lấy trộm theo Referer)
                    req.Headers.TryAddWithoutValidation("Referer", Prefs.BaseUrl + "/");
                    using var resp = await Client.SendAsync(req);
                    if (!resp.IsSuccessStatusCode) return null;
                    bytes = await resp.Content.ReadAsByteArrayAsync();
                }
                finally
                {
                    Gate.Release();
                }
                try
                {
                    Directory.CreateDirectory(Dir);
                    await File.WriteAllBytesAsync(file, bytes);
                }
                catch (Exception)
                {
                }
            }
            img = await Task.Run(() => Decode(bytes));
            return img;
        }
        catch (Exception)
        {
            return null;
        }
        finally
        {
            lock (Memory)
            {
                Pending.Remove(url);
                if (img != null)
                {
                    Memory[url] = img;
                    Order.AddFirst(url);
                    while (Order.Count > 400)
                    {
                        Memory.Remove(Order.Last!.Value);
                        Order.RemoveLast();
                    }
                }
            }
        }
    }

    public static ImageSource? Decode(byte[] bytes)
    {
        if (bytes.Length < 12) return null;
        var webp = bytes[0] == 'R' && bytes[1] == 'I' && bytes[2] == 'F' && bytes[3] == 'F' &&
                   bytes[8] == 'W' && bytes[9] == 'E' && bytes[10] == 'B' && bytes[11] == 'P';
        if (!webp)
        {
            try
            {
                var bi = new BitmapImage();
                bi.BeginInit();
                bi.CacheOption = BitmapCacheOption.OnLoad;
                bi.CreateOptions = BitmapCreateOptions.IgnoreColorProfile;
                bi.DecodePixelWidth = DecodeWidth;
                bi.StreamSource = new MemoryStream(bytes);
                bi.EndInit();
                bi.Freeze();
                return bi;
            }
            catch (Exception)
            {
                // WIC không đọc được → thử Skia
            }
        }
        try
        {
            using var bmp = SKBitmap.Decode(bytes);
            if (bmp == null) return null;
            var w = Math.Min(DecodeWidth, bmp.Width);
            var h = Math.Max(1, (int)Math.Round((double)bmp.Height * w / bmp.Width));
            using var scaled = bmp.Resize(new SKImageInfo(w, h, SKColorType.Bgra8888, SKAlphaType.Premul),
                new SKSamplingOptions(SKFilterMode.Linear, SKMipmapMode.Linear));
            if (scaled == null) return null;
            var bs = BitmapSource.Create(w, h, 96, 96, PixelFormats.Pbgra32, null, scaled.GetPixels(), scaled.ByteCount, scaled.RowBytes);
            bs.Freeze();
            return bs;
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static string Hash(string url) => Convert.ToHexString(SHA1.HashData(Encoding.UTF8.GetBytes(url)));
}
