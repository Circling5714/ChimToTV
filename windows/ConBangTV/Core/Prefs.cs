using System.IO;
using System.Security.Cryptography;
using System.Text;
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
        KkSite,
    };

    /// Trang KKPhim (từ 1.3) — dữ liệu lấy thẳng từ API công khai phimapi.com (xem ApiSite).
    public const string KkSite = "https://www.kkphim.com";

    /// Trang mẫu thêm ở bản sau: thêm một lần vào danh sách đã lưu (xoá đi thì thôi, không thêm lại).
    public static readonly string[] AddedSites = { KkSite };

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
            _pidCache = null;
        }
        AddNewPresets();
    }

    /// Trang mẫu thêm ở bản sau (AddedSites): người đã có danh sách trang thì thêm một lần.
    private static void AddNewPresets()
    {
        foreach (var url in AddedSites)
        {
            var key = "preset_added@" + url;
            if (Get<bool>(key)) continue;
            if (Get<List<string>>("sites") is { } list && !list.Contains(url)) Sites = list.Append(url).ToList();
            Set(key, true);
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

#if DEBUG
    /// Cổng gỡ lỗi: đặt một khoá bằng JSON (thử giới hạn giờ xem…).
    public static void DebugSet(string key, string json)
    {
        lock (Lock) _store[key] = JsonNode.Parse(json);
        _historyCache = null;
        _positionsCache = null;
        Changed();
    }
#endif

    // ---- Người xem (hồ sơ) — cùng cách lưu bản TV / iPad ----

    public const int MaxProfiles = 8;
    public const int PinLength = 4;
    public const string FirstProfileName = "Cả nhà";

    /// Màu vòng tròn của người xem (RGB, cùng thứ tự bản Android / iPad).
    public static readonly uint[] ProfileColors = { 0x2BAF85, 0xE5533D, 0x3B82F6, 0xF59E0B, 0x8B5CF6, 0x06B6D4, 0xEC4899, 0x84CC16 };
    public static readonly string[] ProfileColorNames = { "Xanh lá", "Đỏ cam", "Xanh dương", "Vàng", "Tím", "Xanh ngọc", "Hồng", "Xanh nõn chuối" };

    /// Khoá riêng từng người xem (còn lại — danh sách trang, mốc nhạc mở đầu, cập nhật… — dùng chung).
    private static readonly HashSet<string> ProfileKeys = new()
    {
        "history", "positions_v2", "follows", "last_follow_check", "private_mode",
        "hist_del", "follow_del",
        "base_url", "seek_step", "auto_next", "video_fill",
        "limit_min", "bedtime", "watch_day", "watch_sec", "watch_extra", "unblock_until",
    };

    private static readonly string[] ProfilePrefixes = { "server_type@", "server_label@" };

    /// Người thêm sau bắt đầu với trang phim / server ưu tiên / cách xem của người đang dùng.
    private static readonly HashSet<string> CopiedKeys = new() { "base_url", "seek_step", "auto_next", "video_fill" };

    private static string? _pidCache;

    /// Người xem đang dùng ("" = người đầu tiên, giữ khoá cũ không hậu tố).
    public static string ProfileId
    {
        get
        {
            lock (Lock)
            {
                if (_pidCache != null) return _pidCache;
                var saved = Get<string>("profile") ?? "";
                var list = Profiles;
                _pidCache = list.Any(p => p.Id == saved) ? saved : list[0].Id;
                return _pidCache;
            }
        }
    }

    /// Khoá của người xem id (người đầu tiên — id "" — giữ khoá cũ không hậu tố).
    public static string Pk(string id, string key) => id.Length == 0 ? key : $"{key}#{id}";

    /// Khoá của người xem đang dùng.
    private static string K(string key) => Pk(ProfileId, key);

    private static bool IsProfileKey(string b) => ProfileKeys.Contains(b) || ProfilePrefixes.Any(b.StartsWith);

    /// Các khoá riêng đang có của một người xem: (khoá thật, khoá gốc không hậu tố).
    private static List<(string Key, string Base)> OwnKeys(string id)
    {
        lock (Lock)
        {
            var outList = new List<(string, string)>();
            foreach (var key in _store.Select(kv => kv.Key).ToList())
            {
                string? b = id.Length == 0 ? (key.Contains('#') ? null : key) : key.EndsWith("#" + id) ? key[..^(id.Length + 1)] : null;
                if (b != null && IsProfileKey(b)) outList.Add((key, b));
            }
            return outList;
        }
    }

    public static List<Profile> Profiles
    {
        get
        {
            var list = Get<List<Profile>>("profiles") ?? new();
            return list.Count == 0 ? new() { new Profile { Id = "", Name = FirstProfileName, Color = 0 } } : list;
        }
    }

    private static void SaveProfiles(List<Profile> list) => Set("profiles", list);

    /// Người xem đang dùng.
    public static Profile CurrentProfile
    {
        get
        {
            var id = ProfileId;
            var list = Profiles;
            return list.FirstOrDefault(p => p.Id == id) ?? list[0];
        }
    }

    public static void SwitchProfile(string id)
    {
        lock (Lock)
        {
            if (Profiles.All(p => p.Id != id)) return;
            _pidCache = id;
            Set("profile", id);
            _historyCache = null;
            _positionsCache = null;
        }
    }

    /// Thêm người xem (chưa chuyển sang): lịch sử trống; trang phim / server ưu tiên / cách xem chép từ người đang dùng.
    public static Profile AddProfile(string name)
    {
        lock (Lock)
        {
            var list = Profiles;
            var n = 1;
            while (list.Any(p => p.Id == $"p{n}")) n++;
            var id = $"p{n}";
            var used = list.Select(p => p.Color).ToHashSet();
            var color = Enumerable.Range(0, ProfileColors.Length).FirstOrDefault(i => !used.Contains(i), list.Count % ProfileColors.Length);
            foreach (var (key, b) in OwnKeys(ProfileId))
            {
                if (!CopiedKeys.Contains(b) && !ProfilePrefixes.Any(b.StartsWith)) continue;
                if (_store.TryGetPropertyValue(key, out var v)) _store[$"{b}#{id}"] = v?.DeepClone();
            }
            _store[$"base_url#{id}"] = BaseUrl;
            var p = new Profile { Id = id, Name = name.Trim(), Color = color };
            list.Add(p);
            SaveProfiles(list);
            return p;
        }
    }

    public static void RenameProfile(string id, string name)
    {
        lock (Lock)
        {
            var list = Profiles;
            foreach (var p in list.Where(p => p.Id == id)) p.Name = name.Trim();
            SaveProfiles(list);
        }
    }

    public static void SetProfileColor(string id, int color)
    {
        lock (Lock)
        {
            var list = Profiles;
            foreach (var p in list.Where(p => p.Id == id)) p.Color = color;
            SaveProfiles(list);
        }
    }

    /// Đặt mã PIN (4 số); null = tắt khoá.
    public static void SetProfilePin(string id, string? pin)
    {
        lock (Lock)
        {
            var list = Profiles;
            foreach (var p in list.Where(p => p.Id == id)) p.Pin = pin == null ? null : PinHash(id, pin);
            SaveProfiles(list);
        }
    }

    public static bool CheckPin(Profile p, string pin) => p.Pin == null || p.Pin == PinHash(p.Id, pin);

    /// Không lưu mã PIN thật, chỉ lưu mã băm (khoá cho trẻ em khỏi vào nhầm, không phải mật khẩu bảo mật). Cùng cách băm
    /// bản TV / iPad.
    public static string PinHash(string id, string pin) =>
        Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes($"chimto-pin:{id}:{pin}"))).ToLowerInvariant();

    /// Xoá người xem cùng toàn bộ dữ liệu riêng; còn một người thì không xoá được.
    public static void DeleteProfile(string id)
    {
        lock (Lock)
        {
            var list = Profiles;
            if (list.Count <= 1 || list.All(p => p.Id != id)) return;
            foreach (var (key, _) in OwnKeys(id)) _store.Remove(key);
            var rest = list.Where(p => p.Id != id).ToList();
            SaveProfiles(rest);
            if (ProfileId == id) SwitchProfile(rest[0].Id);
        }
        Changed();
    }

    // ---- Giới hạn giờ xem (xem WatchLimit) — cùng tên khoá bản TV / iPad ----

    /// Số phút xem mỗi ngày của người xem id (mặc định người đang dùng; 0 = không giới hạn).
    public static int DailyLimit(string? id = null) => Get<int>(Pk(id ?? ProfileId, "limit_min"));

    public static void SetDailyLimit(string id, int minutes) => Set(Pk(id, "limit_min"), minutes);

    /// Giờ đi ngủ (phút trong ngày, vd 21:00 = 1260; -1 = không): từ giờ đó tới 6 giờ sáng không xem được.
    public static int Bedtime(string? id = null) => Has(Pk(id ?? ProfileId, "bedtime")) ? Get<int>(Pk(id ?? ProfileId, "bedtime")) : -1;

    public static void SetBedtime(string id, int minute) => Set(Pk(id, "bedtime"), minute);

    public static string WatchDay
    {
        get => Get<string>(K("watch_day")) ?? "";
        set => Set(K("watch_day"), value);
    }

    public static int WatchSeconds
    {
        get => Get<int>(K("watch_sec"));
        set => Set(K("watch_sec"), value);
    }

    public static int WatchExtra
    {
        get => Get<int>(K("watch_extra"));
        set => Set(K("watch_extra"), value);
    }

    /// Người đang xem được xem không giới hạn tới thời điểm này (ms) — phụ huynh cho thêm lúc giờ đi ngủ.
    public static long UnblockUntil
    {
        get => Get<long>(K("unblock_until"));
        set => Set(K("unblock_until"), value);
    }

    /// Có ai đang bị giới hạn giờ xem (khi đó thêm / xoá người xem cũng cần mã PIN phụ huynh).
    public static bool AnyLimits => Profiles.Any(p => DailyLimit(p.Id) > 0 || Bedtime(p.Id) >= 0);

    /// Mã PIN phụ huynh (cả nhà): đặt / đổi giới hạn giờ xem, cho xem thêm khi hết giờ.
    public static bool HasParentPin => Get<string>("parent_pin") != null;

    public static void SetParentPin(string? pin)
    {
        if (pin != null) Set("parent_pin", PinHash("parent", pin));
        else Remove("parent_pin");
    }

    public static bool CheckParentPin(string pin) => Get<string>("parent_pin") == PinHash("parent", pin);

    /// Hỏi "Ai đang xem?" khi mở app (từ 2 người xem).
    public static bool AskProfile
    {
        get => !Has("ask_profile") || Get<bool>("ask_profile");
        set => Set("ask_profile", value);
    }

    /// Thêm server dự phòng KKPhim cho các trang tự có trình phát (xem KkPhim). Dùng chung cả máy.
    public static bool KkBackup
    {
        get => !Has("kk_backup") || Get<bool>("kk_backup");
        set => Set("kk_backup", value);
    }

    /// Thu nhỏ cửa sổ app lúc đang xem → phim chạy tiếp ở cửa sổ nhỏ trên màn hình.
    public static bool MiniOnMinimize
    {
        get => !Has("mini_on_minimize") || Get<bool>("mini_on_minimize");
        set => Set("mini_on_minimize", value);
    }

    // ---- Trang phim ----

    /// Trang phim đang dùng (địa chỉ gốc, không có dấu / ở cuối). Đặt trang mới thì tự thêm vào danh sách.
    public static string BaseUrl
    {
        get => Get<string>(K("base_url")) ?? Get<string>("base_url") ?? DefaultBase;
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
        set
        {
            lock (Lock)
            {
                // Đồng bộ trong nhà: ghi lúc thêm / bỏ từng trang
                var old = Sites.ToHashSet();
                var list = value.Distinct().ToList();
                var now = Util.NowMs();
                var add = Stamps("sites_add");
                var del = Stamps("sites_del");
                foreach (var s in list.Where(s => !old.Contains(s))) add[s] = now;
                foreach (var s in old.Where(s => !list.Contains(s))) del[s] = now;
                SetStamps("sites_add", add);
                SetStamps("sites_del", del);
                Set("sites", list);
            }
        }
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
        // Nhà mạng chặn tên miền trần kkphim.com: gõ «kkphim.com» là trang KKPhim mẫu (www)
        if (u.Host.Equals(ApiSite.KkHost, StringComparison.OrdinalIgnoreCase)) return KkSite;
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
            var del = Stamps(K("hist_del"));
            var now = Util.NowMs();
            foreach (var f in History.Select(h => h.FilmUrl).Concat(Positions().Select(p => p.Film)).Where(f => f.Length > 0)) del[f] = now;
            SetStamps(K("hist_del"), del);
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
            var del = Stamps(K("hist_del"));
            del[filmUrl] = Util.NowMs();
            SetStamps(K("hist_del"), del);
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
        lock (Lock)
        {
            var del = Stamps(K("follow_del"));
            del[filmUrl] = Util.NowMs();
            SetStamps(K("follow_del"), del);
            Follows = Follows.Where(f => f.FilmUrl != filmUrl).ToList();
        }
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

    // ---- Đồng bộ trong nhà (SyncData.cs) — cùng khoá bản TV / iPad ----

    /// Mốc thời gian theo khoá (lúc xoá / lúc thêm): {"khoá": ms}.
    public static Dictionary<string, long> Stamps(string key) => Get<Dictionary<string, long>>(key) ?? new();

    public static void SetStamps(string key, Dictionary<string, long> m) => Set(key, SyncMerge.Tombstones(m, new()));

    /// Mã máy (ngẫu nhiên, tạo lần đầu) — máy kia nhận ra máy này qua mã này.
    public static string DeviceId
    {
        get
        {
            lock (Lock)
            {
                if (Get<string>("device_id") is { Length: > 0 } id) return id;
                var n = Guid.NewGuid().ToString().ToLowerInvariant();
                Set("device_id", n);
                return n;
            }
        }
    }

    /// Dữ liệu đồng bộ của người xem p.
    public static SyncProfile SyncProfileData(Profile p)
    {
        lock (Lock)
        {
            var id = p.Id;
            var hist = Get<List<HistoryItem>>(Pk(id, "history")) ?? new();
            var pos = Get<List<Position>>(Pk(id, "positions_v2")) ?? new();
            var fol = Get<List<Follow>>(Pk(id, "follows")) ?? new();
            return new SyncProfile
            {
                Id = id, Name = p.Name,
                History = hist.Select(h => new SyncHistory(h.FilmUrl, h.Title, h.Poster ?? "", h.EpTitle, h.EpUrl, h.Time)).ToList(),
                HistDel = Stamps(Pk(id, "hist_del")),
                Pos = pos.Select(x => new SyncPos(x.Ep, x.Sec, x.Film, x.T ?? 0)).ToList(),
                Follows = fol.Select(f => new SyncFollow(f.FilmUrl, f.Title, f.Poster ?? "", f.Count, f.Latest, f.Seen, f.T ?? 0)).ToList(),
                FollowDel = Stamps(Pk(id, "follow_del")),
            };
        }
    }

    /// Ghi dữ liệu đã gộp cho người xem s.Id. true = có thay đổi.
    public static bool ApplySync(SyncProfile s)
    {
        lock (Lock)
        {
            var p = Profiles.FirstOrDefault(x => x.Id == s.Id);
            if (p == null || SyncProfileData(p).SameAs(s)) return false;
            var id = s.Id;
            Set(Pk(id, "history"), s.History.Select(h => new HistoryItem
            {
                FilmUrl = h.Film, Title = h.Title, Poster = h.Poster.NonEmpty(), EpTitle = h.Ep, EpUrl = h.EpUrl, Time = h.Time,
            }).ToList());
            Set(Pk(id, "positions_v2"), s.Pos.Select(x => new Position { Ep = x.Ep, Sec = x.Sec, Film = x.Film, T = x.T == 0 ? null : x.T }).ToList());
            Set(Pk(id, "follows"), s.Follows.Select(f => new Follow
            {
                FilmUrl = f.Film, Title = f.Title, Poster = f.Poster.NonEmpty(), Count = f.Count, Latest = f.Latest, Seen = f.Seen,
                T = f.T == 0 ? null : f.T,
            }).ToList());
            SetStamps(Pk(id, "hist_del"), s.HistDel);
            SetStamps(Pk(id, "follow_del"), s.FollowDel);
            _historyCache = null;
            _positionsCache = null;
            return true;
        }
    }

    public static SyncSites SyncSitesData => new() { List = Sites, Add = Stamps("sites_add"), Del = Stamps("sites_del") };

    /// Ghi danh sách trang đã gộp (không ghi lại mốc thêm / bỏ như khi người xem tự sửa). true = có thay đổi.
    public static bool ApplySites(SyncSites s)
    {
        lock (Lock)
        {
            if (s.List.Count == 0 || s.SameAs(SyncSitesData)) return false;
            SetStamps("sites_add", s.Add);
            SetStamps("sites_del", s.Del);
            Set("sites", s.List);
            return true;
        }
    }

    /// Máy đã ghép (JSON riêng của bản Windows).
    public static List<SyncPeer> SyncPeers
    {
        get => Get<List<SyncPeer>>("sync_peers") ?? new();
        set => Set("sync_peers", value);
    }

    /// Tự đồng bộ (mở app, thoát phim, theo dõi…).
    public static bool SyncAuto
    {
        get => !Has("sync_auto") || Get<bool>("sync_auto");
        set => Set("sync_auto", value);
    }
}
