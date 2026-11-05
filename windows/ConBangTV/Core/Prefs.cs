using System.IO;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace ConBangTV.Core;

/// Cài đặt + lịch sử xem (bản Android: Prefs.kt, iPad: Prefs.swift — cùng tên khoá để sau này đồng bộ trong nhà).
/// Lưu trong một tệp JSON ở %LOCALAPPDATA%\ConBangTV\prefs.json; ghi lại sau mỗi lần đổi (gộp các lần đổi liền nhau).
public static class Prefs
{
    public const string DefaultBase = "https://yanhh3d.ee";

    /// Các trang mẫu có sẵn trong danh sách lần đầu cài.
    public static readonly string[] PresetSites =
    {
        DefaultBase, "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee", "https://phimmois.fun", "https://mephim.ddns.net",
    };

    private const int MaxHistory = 60;
    private const int MaxPositions = 400;

    public static string DataDir { get; private set; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ConBangTV");

    private static readonly object Lock = new();
    private static JsonObject _store = new();
    private static string _file = "";
    private static Timer? _saveTimer;

    /// Tên khoá trong JSON viết kiểu camelCase như bản Android / iPad (filmUrl, epTitle…), để sau này đồng bộ được.
    private static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };

    /// Gọi một lần lúc mở app (thử nghiệm: thư mục riêng).
    public static void Load(string? dir = null)
    {
        lock (Lock)
        {
            if (dir != null) DataDir = dir;
            Directory.CreateDirectory(DataDir);
            _file = Path.Combine(DataDir, "prefs.json");
            try
            {
                _store = File.Exists(_file) ? JsonNode.Parse(File.ReadAllText(_file)) as JsonObject ?? new() : new();
            }
            catch (Exception)
            {
                // Tệp hỏng (mất điện lúc ghi): giữ bản hỏng để xem lại, bắt đầu mới
                try { File.Copy(_file, _file + ".bad", true); } catch (Exception) { }
                _store = new();
            }
            _historyCache = null;
            _positionsCache = null;
        }
    }

    private static void Changed()
    {
        _saveTimer ??= new Timer(_ => Flush());
        _saveTimer.Change(400, Timeout.Infinite);
    }

    /// Ghi ngay (lúc thoát app).
    public static void Flush()
    {
        string text;
        lock (Lock)
        {
            if (_file.Length == 0) return;
            text = _store.ToJsonString(new JsonSerializerOptions { WriteIndented = false });
        }
        try
        {
            var tmp = _file + ".tmp";
            File.WriteAllText(tmp, text);
            File.Move(tmp, _file, true);
        }
        catch (Exception)
        {
        }
    }

    // ---- Đọc / ghi khoá ----

    private static T? Get<T>(string key)
    {
        lock (Lock)
        {
            if (!_store.TryGetPropertyValue(key, out var n) || n == null) return default;
            try
            {
                return n.Deserialize<T>(Json);
            }
            catch (Exception)
            {
                return default;
            }
        }
    }

    private static void Set<T>(string key, T? value)
    {
        lock (Lock)
        {
            if (value == null) _store.Remove(key);
            else _store[key] = JsonSerializer.SerializeToNode(value, Json);
        }
        Changed();
    }

    private static bool Has(string key)
    {
        lock (Lock) return _store.ContainsKey(key);
    }

    public static void Remove(string key) => Set<object>(key, null);

    /// Khoá riêng của người xem (bản này chưa có hồ sơ: người xem đầu tiên dùng khoá gốc, như bản TV).
    private static string K(string key) => key;

    // ---- Trang phim ----

    /// Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách.
    public static string BaseUrl
    {
        get => Get<string>(K("base_url")) ?? DefaultBase;
        set
        {
            Set(K("base_url"), value);
            if (!Sites.Contains(value)) Sites = Sites.Append(value).ToList();
        }
    }

    /// Danh sách trang phim để chọn nhanh (mặc định: các trang mẫu).
    public static List<string> Sites
    {
        get => Get<List<string>>("sites") ?? PresetSites.ToList();
        set => Set("sites", value.Distinct().ToList());
    }

    /// Trang tự chuyển sang tên miền mới: thay tên cũ trong danh sách (giữ chỗ), rồi dùng tên mới.
    public static void ReplaceSite(string old, string now)
    {
        Sites = Sites.Select(s => s == old ? now : s).ToList();
        BaseUrl = now;
    }

    public static void AddSite(string url)
    {
        if (!Sites.Contains(url)) Sites = Sites.Append(url).ToList();
    }

    /// Bỏ trang khỏi danh sách (còn ít nhất một trang); đang dùng trang đó thì sang trang đầu danh sách.
    public static bool RemoveSite(string url)
    {
        var rest = Sites.Where(s => s != url).ToList();
        if (rest.Count == 0) return false;
        Sites = rest;
        if (BaseUrl == url) BaseUrl = rest[0];
        return true;
    }

    /// Trang mẫu không còn trong danh sách (người xem đã xoá) — để thêm lại nhanh.
    public static List<string> MissingPresets
    {
        get
        {
            var list = Sites;
            return PresetSites.Where(p => !list.Contains(p)).ToList();
        }
    }

    /// Server người xem chọn lần gần nhất trên từng trang (vd "tiktik").
    public static string? ServerType(string? host) => Get<string>(K($"server_type@{host ?? ""}"));

    public static void SetServer(string? host, string? type, string? label)
    {
        Set(K($"server_type@{host ?? ""}"), type);
        Set(K($"server_label@{host ?? ""}"), label);
    }

    public static int SeekStep
    {
        get => Has(K("seek_step")) ? Get<int>(K("seek_step")) : 10;
        set => Set(K("seek_step"), value);
    }

    public static bool AutoNext
    {
        get => !Has(K("auto_next")) || Get<bool>(K("auto_next"));
        set => Set(K("auto_next"), value);
    }

    /// Chuẩn hoá chuỗi người dùng gõ ("yanhh3d.ee", "https://yanhh3d.ee/") → "https://yanhh3d.ee"; sai thì null.
    public static string? NormalizeUrl(string input)
    {
        var s = input.Trim().TrimEnd('/');
        if (s.Length == 0 || s.Contains(' ')) return null;
        if (!s.StartsWith("http://", StringComparison.OrdinalIgnoreCase) && !s.StartsWith("https://", StringComparison.OrdinalIgnoreCase))
            s = "https://" + s;
        if (!Uri.TryCreate(s, UriKind.Absolute, out var u) || !u.Host.Contains('.')) return null;
        return s;
    }

    // ---- Giao diện (riêng bản Windows) ----

    /// Cỡ chữ / cỡ giao diện: 1.0 (máy tính), 1.25 / 1.5 (tivi ngồi xa).
    public static double UiScale
    {
        get
        {
            var v = Get<double>("ui_scale");
            return v == 0 ? 1 : Math.Clamp(v, 0.8, 2);
        }
        set => Set("ui_scale", value);
    }

    /// Mở app là vào toàn màn hình (máy cắm tivi).
    public static bool StartFullscreen
    {
        get => Get<bool>("start_fullscreen");
        set => Set("start_fullscreen", value);
    }

    /// Vị trí / cỡ cửa sổ lần trước: [trái, trên, rộng, cao, phóng to (1/0)].
    public static double[]? WindowBounds
    {
        get => Get<double[]>("window_bounds");
        set => Set("window_bounds", value);
    }

    /// Vượt chặn nhà mạng (xem Unblock): 0 = tắt, 1 = tách gói bắt tay, 2 = tách + gửi đảo thứ tự. Cùng khoá bản TV.
    public static int UnblockLevel
    {
        get => Get<bool>("unblock") ? (Get<int>("unblock_level") == 2 ? 2 : 1) : 0;
        set
        {
            Set("unblock", value > 0);
            if (value > 0) Set("unblock_level", value);
        }
    }

    // ---- Proxy SOCKS5 (xem Net) — cùng khoá bản TV / iPad ----

    public static bool ProxyOn
    {
        get => Get<bool>("proxy_on");
        set => Set("proxy_on", value);
    }

    public static string ProxyHost
    {
        get => Get<string>("proxy_host") ?? "";
        set => Set("proxy_host", value.Trim());
    }

    public static int ProxyPort
    {
        get => Has("proxy_port") ? Get<int>("proxy_port") : 1080;
        set => Set("proxy_port", value);
    }

    public static string ProxyUser
    {
        get => Get<string>("proxy_user") ?? "";
        set => Set("proxy_user", value);
    }

    /// Mật khẩu lưu đã mã hoá (DPAPI, chỉ tài khoản Windows này giải được) — khoá riêng của bản Windows.
    public static string ProxyPass
    {
        get => Secret.Unprotect(Get<string>("proxy_pass_dpapi"));
        set => Set("proxy_pass_dpapi", Secret.Protect(value));
    }

    /// DNS của proxy (gửi tên miền cho proxy) — mặc định; tắt = máy tự phân giải.
    public static bool ProxyRemoteDns
    {
        get => !Has("proxy_remote_dns") || Get<bool>("proxy_remote_dns");
        set => Set("proxy_remote_dns", value);
    }

    /// Chỉ trang phim đi qua proxy (video / ảnh CDN đi thẳng).
    public static bool ProxySitesOnly
    {
        get => Get<bool>("proxy_sites_only");
        set => Set("proxy_sites_only", value);
    }

    /// Proxy đang bật và đủ thông tin (null = đi thẳng).
    public static SocksConfig? ProxyConfig()
    {
        if (!ProxyOn || ProxyHost.Length == 0 || ProxyPort is <= 0 or > 65535) return null;
        return new SocksConfig(ProxyHost, ProxyPort, ProxyUser, ProxyUser.Length > 0 ? ProxyPass : "", ProxyRemoteDns, ProxySitesOnly);
    }

    // ---- Cập nhật ----

    public static bool AutoUpdate
    {
        get => !Has("auto_update") || Get<bool>("auto_update");
        set => Set("auto_update", value);
    }

    public static long LastUpdateCheck
    {
        get => Get<long>("last_update_check");
        set => Set("last_update_check", value);
    }

    public static void SnoozeUpdate(string version, long until)
    {
        Set("snooze_version", version);
        Set("snooze_until", until);
    }

    public static bool IsUpdateSnoozed(string version) =>
        Get<string>("snooze_version") == version && Util.NowMs() < Get<long>("snooze_until");

    // ---- Lịch sử xem ----

    private static List<HistoryItem>? _historyCache;

    public static List<HistoryItem> History
    {
        get
        {
            lock (Lock) return (_historyCache ??= Get<List<HistoryItem>>(K("history")) ?? new()).ToList();
        }
    }

    private static void SaveHistory(List<HistoryItem> list)
    {
        lock (Lock) _historyCache = list;
        Set(K("history"), list);
    }

    public static HistoryItem? HistoryFor(string filmUrl) => History.FirstOrDefault(h => h.FilmUrl == filmUrl);

    public static void AddHistory(HistoryItem item)
    {
        lock (Lock) SaveHistory(new[] { item }.Concat(History.Where(h => h.FilmUrl != item.FilmUrl)).Take(MaxHistory).ToList());
    }

    public static void ClearHistory()
    {
        lock (Lock)
        {
            SaveHistory(new());
            SavePositions(new());
        }
    }

    /// Xoá một phim khỏi lịch sử, kèm chỗ đang xem / đánh dấu đã xem của các tập phim đó.
    public static void RemoveHistory(string filmUrl)
    {
        lock (Lock)
        {
            var item = HistoryFor(filmUrl);
            var eps = Positions().Where(p => p.Film == filmUrl).Select(p => p.Ep).ToHashSet();
            if (item != null) eps.Add(item.EpUrl);
            SavePositions(Positions().Where(p => !eps.Contains(p.Ep)).ToList());
            SaveHistory(History.Where(h => h.FilmUrl != filmUrl).ToList());
        }
    }

    // ---- Phim theo dõi ----

    public static List<Follow> Follows
    {
        get => Get<List<Follow>>(K("follows")) ?? new();
        set => Set(K("follows"), value);
    }

    public static Follow? FollowOf(string filmUrl) => Follows.FirstOrDefault(f => f.FilmUrl == filmUrl);

    public static void FollowFilm(FilmDetail d)
    {
        lock (Lock)
        {
            var n = d.EpisodeCount;
            var f = new Follow
            {
                FilmUrl = d.Url, Title = d.Title, Poster = d.Poster, Count = n, Latest = d.LatestEpisode?.Title ?? "", Seen = n, T = Util.NowMs(),
            };
            Follows = new[] { f }.Concat(Follows.Where(x => x.FilmUrl != d.Url)).ToList();
        }
    }

    public static void Unfollow(string filmUrl)
    {
        lock (Lock) Follows = Follows.Where(f => f.FilmUrl != filmUrl).ToList();
    }

    /// Cập nhật số tập mới nhất; seen = true: người xem vừa mở phim (hết "tập mới").
    public static void UpdateFollow(string filmUrl, int count, string latest, bool seen)
    {
        lock (Lock)
        {
            var list = Follows;
            var f = list.FirstOrDefault(x => x.FilmUrl == filmUrl);
            if (f == null) return;
            var wasSeen = f.Seen;
            f.Count = count;
            f.Latest = latest;
            f.Seen = seen || count < wasSeen ? count : wasSeen;
            Follows = list;
        }
    }

    // ---- Nhạc mở đầu (mốc riêng từng phim, giây) ----

    /// (đầu, hết) nhạc mở đầu; hết = -1 là mới đánh dấu đầu.
    public static (int Start, int End)? Intro(string filmUrl)
    {
        var p = Get<string>($"intro@{filmUrl}")?.Split(',');
        if (p == null || !int.TryParse(p[0], out var s)) return null;
        return (s, p.Length > 1 && int.TryParse(p[1], out var e) ? e : -1);
    }

    public static void SetIntro(string filmUrl, int start, int end) => Set($"intro@{filmUrl}", $"{start},{end}");

    public static void ClearIntro(string filmUrl) => Remove($"intro@{filmUrl}");

    /// Lấp đầy màn hình: video rộng (21:9) phóng cho hết viền đen, cắt bớt hai bên.
    public static bool VideoFill
    {
        get => Get<bool>(K("video_fill"));
        set => Set(K("video_fill"), value);
    }

    // ---- Vị trí đang xem từng tập (giây) ----

    public sealed class Position
    {
        public string Ep { get; set; } = "";
        public int Sec { get; set; }
        public string Film { get; set; } = "";
        public long? T { get; set; }
    }

    /// Thứ tự lưu: cũ nhất trước (quá MaxPositions thì bỏ bớt đầu danh sách).
    private static List<Position>? _positionsCache;

    private static List<Position> Positions()
    {
        lock (Lock) return _positionsCache ??= Get<List<Position>>(K("positions_v2")) ?? new();
    }

    private static void SavePositions(List<Position> list)
    {
        lock (Lock) _positionsCache = list;
        Set(K("positions_v2"), list);
    }

    public static int PositionOf(string epUrl)
    {
        lock (Lock) return Positions().LastOrDefault(p => p.Ep == epUrl)?.Sec ?? 0;
    }

    /// Tập đã xem hết (lưu vị trí 0 khi xem tới ~90 giây cuối).
    public static bool Finished(string epUrl)
    {
        lock (Lock) return Positions().LastOrDefault(p => p.Ep == epUrl)?.Sec == 0;
    }

    public static HashSet<string> WatchedEpisodes()
    {
        lock (Lock) return Positions().Select(p => p.Ep).ToHashSet();
    }

    public static void SavePosition(string epUrl, int seconds, string filmUrl)
    {
        lock (Lock)
        {
            var list = Positions().Where(p => p.Ep != epUrl).ToList();
            list.Add(new Position { Ep = epUrl, Sec = seconds, Film = filmUrl, T = Util.NowMs() });
            if (list.Count > MaxPositions) list.RemoveRange(0, list.Count - MaxPositions);
            SavePositions(list);
        }
    }
}
