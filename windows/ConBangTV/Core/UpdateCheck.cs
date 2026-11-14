using System.IO;
using System.Diagnostics;
using System.Reflection;
using System.Security.Cryptography;

namespace ConBangTV.Core;

/// Tự cập nhật từ GitHub Releases (bản Android: Updater.kt, iPad: UpdateCheck.swift). Bản Windows phát hành bằng tag riêng
/// "win-v…" và không bao giờ là bản "latest" (app TV đọc releases/latest và lấy tệp .apk) → duyệt danh sách release, lấy
/// tag win-v* có tệp .exe. Cài: tải tệp mới, đổi tên tệp đang chạy thành .old, đặt tệp mới vào chỗ cũ rồi mở lại app.
public static class UpdateCheck
{
    public const string ReleasesApi = "https://api.github.com/repos/Circling5714/ChimToTV/releases?per_page=30";
    public const string ReleasesPage = "https://github.com/Circling5714/ChimToTV/releases";
    public const string Prefix = "win-v";
    public const long CheckEvery = 2 * 60 * 60 * 1000L;
    public const long Snooze = 24 * 60 * 60 * 1000L;

    /// ApiUrl: link tải cùng tệp qua API (api.github.com/…/releases/assets/&lt;id&gt;) — dùng khi link github.com lỗi.
    public sealed record Release(string Version, string ExeUrl, string Notes, string Tag, string? Sha256, long Size, string? ApiUrl = null);

    /// "1.0.0" → "1.0" (bỏ số 0 cuối cho gọn, khớp tên tag).
    public static string CurrentVersion
    {
        get
        {
            var v = Assembly.GetExecutingAssembly().GetName().Version ?? new Version(1, 0);
            return v.Build > 0 ? $"{v.Major}.{v.Minor}.{v.Build}" : $"{v.Major}.{v.Minor}";
        }
    }

    /// Bản Windows mới nhất (bỏ bản nháp / thử nghiệm); null = chưa có bản nào.
    public static async Task<Release?> Latest(string? api = null)
    {
        // Thử cập nhật trên máy: biến môi trường CONBANG_UPDATE_API trỏ tới danh sách release giả
        api ??= Environment.GetEnvironmentVariable("CONBANG_UPDATE_API") ?? ReleasesApi;
        var r = await Http.Get(api, new() { ["Accept"] = "application/vnd.github+json" });
        if (JsonScan.Parse(r.Data) is not List<object?> list) return null;
        var releases = J.Objects(list).Select(o =>
        {
            var tag = J.Str(o, "tag_name");
            if (!tag.StartsWith(Prefix) || J.Bool(o, "draft") || J.Bool(o, "prerelease")) return null;
            var exe = J.Objects(J.Arr(o, "assets")).FirstOrDefault(a => J.Str(a, "name").EndsWith(".exe", StringComparison.OrdinalIgnoreCase));
            if (exe == null) return null;
            var digest = J.Str(exe, "digest");
            var sha = digest.StartsWith("sha256:") ? digest["sha256:".Length..].ToLowerInvariant() : null;
            long size = exe.TryGetValue("size", out var s) && s is long l ? l : 0;
            var api = J.Str(exe, "url").StartsWith("https://api.github.com/") ? J.Str(exe, "url") : null;
            return new Release(tag[Prefix.Length..], J.Str(exe, "browser_download_url"), J.Str(o, "body"), tag, sha, size, api);
        }).OfType<Release>().ToList();
        return releases.Count == 0 ? null : releases.Aggregate((a, b) => IsNewer(b.Version, a.Version) ? b : a);
    }

    public static bool IsNewer(string remote, string? local = null)
    {
        var a = Parts(remote);
        var b = Parts(local ?? CurrentVersion);
        for (var i = 0; i < Math.Max(a.Length, b.Length); i++)
        {
            var x = i < a.Length ? a[i] : 0;
            var y = i < b.Length ? b[i] : 0;
            if (x != y) return x > y;
        }
        return false;
    }

    private static int[] Parts(string v) => v.Before("-").Before("+").Split('.').Select(p => int.TryParse(p.Trim(), out var n) ? n : 0).ToArray();

    /// Ghi chú phát hành (markdown) → vài dòng chữ thường cho hộp thoại; có mục "Có gì mới" thì chỉ lấy mục đó.
    public static string Summary(string notes)
    {
        var lines = notes.Replace("\r", "").Split('\n').Select(l => l.Trim()).ToList();
        var start = lines.FindIndex(l => l.StartsWith('#') && l.ToLowerInvariant().Contains("có gì mới"));
        var section = lines;
        if (start >= 0)
        {
            section = lines.Skip(start + 1).ToList();
            var end = section.FindIndex(l => l.StartsWith("## "));
            if (end >= 0) section = section.Take(end).ToList();
        }
        var text = string.Join("\n", section
            .Where(l => l.Length > 0 && !l.StartsWith("![") && !l.StartsWith('|') && !l.Contains("SHA-256"))
            .Select(l => l.Trim('#', '-', '*', ' ').Replace("**", "").Replace("`", ""))
            .Where(l => l.Length > 0));
        return text.Length > 600 ? text[..600] : text;
    }

    /// Tải và kiểm bản mới (đúng cỡ, đúng mã SHA-256 GitHub báo, đúng số hiệu trong tệp) → đường dẫn tệp đã tải.
    public static async Task<string> Download(Release rel, Action<double>? progress = null)
    {
        var dir = Path.Combine(Prefs.DataDir, "update");
        Directory.CreateDirectory(dir);
        foreach (var f in Directory.GetFiles(dir)) TryDelete(f);
        var file = Path.Combine(dir, $"ConBangTV-{rel.Version}.exe");
        // Link github.com lỗi (11/2026: máy chủ GitHub vùng Đông Nam Á trả 504 hàng giờ liền trong khi API vẫn chạy) → tải cùng
        // tệp qua API (Http.Download gửi Accept: application/octet-stream). Tệp từ đường nào cũng phải khớp cỡ + SHA-256 + số bản.
        var sources = new[] { rel.ExeUrl, rel.ApiUrl }.OfType<string>().ToArray();
        Exception? last = null;
        for (var attempt = 0; attempt < 3 && (attempt == 0 || last != null); attempt++)
        {
            // GitHub lỗi thường chỉ vài chục giây: nghỉ 5 s rồi 15 s trước khi thử lại cả hai đường
            if (attempt > 0) await Task.Delay(attempt == 1 ? 5_000 : 15_000);
            foreach (var url in sources)
            {
                try
                {
                    await Http.DownloadTo(url, file, progress);
                    last = null;
                    break;
                }
                catch (Exception e)
                {
                    last = e;
                    Log.D("update", $"Tải lần {attempt + 1} ({url}) lỗi: {e.Message}");
                }
            }
        }
        if (last != null) throw last;
        var info = new FileInfo(file);
        if (rel.Size > 0 && info.Length != rel.Size) throw new AppError("Tải bản cập nhật không đủ (thiếu dữ liệu) — thử lại sau.");
        if (rel.Sha256 != null)
        {
            await using var fs = File.OpenRead(file);
            var hash = Convert.ToHexString(await SHA256.HashDataAsync(fs)).ToLowerInvariant();
            if (hash != rel.Sha256) throw new AppError("Bản cập nhật tải về bị hỏng (sai mã kiểm tra) — thử lại sau.");
        }
        var v = FileVersionInfo.GetVersionInfo(file);
        var pv = v.ProductVersion ?? "";
        if (v.ProductName != "Côn Bằng TV" || IsNewer(pv, rel.Version) || IsNewer(rel.Version, pv))
            throw new AppError("Tệp tải về không phải bản Côn Bằng TV " + rel.Version + ".");
        return file;
    }

    /// Thay tệp đang chạy bằng bản mới rồi mở lại app (Windows cho đổi tên tệp .exe đang chạy).
    public static void Install(string newExe)
    {
        var cur = Environment.ProcessPath ?? throw new AppError("Không biết app đang chạy từ đâu.");
        var old = cur + ".old";
        TryDelete(old);
        File.Move(cur, old);
        try
        {
            File.Copy(newExe, cur, true);
        }
        catch (Exception)
        {
            File.Move(old, cur); // trả lại như cũ
            throw;
        }
        Prefs.Flush();
        Process.Start(new ProcessStartInfo(cur, "--updated") { UseShellExecute = false, WorkingDirectory = Path.GetDirectoryName(cur) });
    }

    /// Lúc mở app: dọn tệp cũ của lần cập nhật trước.
    public static void Cleanup()
    {
        if (Environment.ProcessPath is { } cur) TryDelete(cur + ".old");
    }

    /// Thư mục chứa app ghi được không (để tự thay tệp).
    public static bool CanSelfUpdate()
    {
        var dir = Path.GetDirectoryName(Environment.ProcessPath ?? "");
        if (string.IsNullOrEmpty(dir)) return false;
        try
        {
            var probe = Path.Combine(dir, ".conbang-write-test");
            File.WriteAllText(probe, "");
            File.Delete(probe);
            return true;
        }
        catch (Exception)
        {
            return false;
        }
    }

    private static void TryDelete(string f)
    {
        try { if (File.Exists(f)) File.Delete(f); } catch (Exception) { }
    }
}
