namespace ConBangTV.Core;

/// Máy đã ghép (lưu trong prefs.json, khoá sync_peers — lõi đọc / ghi).
public sealed class SyncPeer
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string Platform { get; set; } = "";
    public string Token { get; set; } = "";
    public string? Host { get; set; }
    public int? Port { get; set; }
    public long? Last { get; set; }
    public string? Status { get; set; }
}

/// Máy tìm thấy trong mạng nhà (mDNS _conbangtv._tcp).
public sealed record SyncFound(string Id, string Name, string Platform, string Host, int Port);

/// Đồng bộ trong nhà với TV / iPad / Mac / Linux (ghép bằng mã 4 số, cổng 8791 — bản Debug 8792, HMAC, dấu xoá): chạy trong
/// lõi Rust (linux/core: sync_service.rs, sync_data.rs) — cùng giao thức, cùng khoá dữ liệu với các bản khác. Sự kiện của lõi
/// (dữ liệu vừa gộp, trạng thái, lời báo) gọi từ luồng của lõi — bên nhận tự chuyển sang luồng giao diện.
public static class SyncService
{
    public const string ServiceType = "_conbangtv._tcp";
    public const string Platform = "windows";

    public static int Port => Native.Sync<int>("sync.port");

    public static string DeviceName => Environment.MachineName;

    /// Vừa gộp dữ liệu của máy khác (lịch sử, theo dõi, người xem…): giao diện tải lại trang đang mở.
    public static event Action? DataChanged;

    /// Trạng thái đổi (đang đồng bộ, máy vừa ghép, mã ghép…).
    public static event Action? StateChanged;

    /// Lời báo ngắn cho người xem («Đã đồng bộ với …»).
    public static event Action<string>? Message;

    static SyncService()
    {
        Native.Event += (name, data) =>
        {
            switch (name)
            {
                case "sync.dataChanged":
                    Prefs.InvalidateCaches();
                    DataChanged?.Invoke();
                    break;
                case "sync.stateChanged":
                    StateChanged?.Invoke();
                    break;
                case "sync.message":
                    Message?.Invoke(data?.GetValue<string>() ?? "");
                    break;
            }
        };
    }

    private sealed class HostPort
    {
        public string Host = "";
        public int Port;
    }

    public static string? PairCode => Native.Sync<string?>("sync.pairCode");

    public static List<SyncFound> Found => Native.Sync<List<SyncFound>>("sync.found");

    public static bool Busy => Native.Sync<bool>("sync.busy");

    public static bool Listening => Native.Sync<bool>("sync.listening");

    public static List<SyncPeer> Peers => Native.Sync<List<SyncPeer>>("sync.peers");

    public static void Unpair(string id) => Native.SyncVoid("sync.unpair", new { id });

    /// Mở app: mở cổng nhận đồng bộ (đã ghép máy nào thì đồng bộ luôn).
    public static void AppStarted() => Native.SyncVoid("sync.appStarted");

    public static void Start() => Native.SyncVoid("sync.start");

    public static void Stop() => Native.SyncVoid("sync.stop");

    /// Mở màn ghép: hiện mã 4 số, quảng bá máy này trong mạng nhà.
    public static void OpenPairing() => Native.SyncVoid("sync.openPairing");

    public static void ClosePairing() => Native.SyncVoid("sync.closePairing");

    public static Task<List<SyncFound>> Browse() => Native.Call<List<SyncFound>>("sync.browse");

    /// "192.168.1.5" / "192.168.1.5:8792" / "[fe80::1%eth0]:8791" → (host, cổng); sai → null.
    public static (string Host, int Port)? ParseAddress(string input) =>
        Native.Sync<HostPort?>("sync.parseAddress", new { input }) is { } hp ? (hp.Host, hp.Port) : null;

    /// Ghép với máy kia bằng mã 4 số: lỗi → câu báo, được → null.
    public static Task<string?> Pair(string host, int port, string code) => Native.Call<string?>("sync.pair", new { host, port, code });

    /// Đồng bộ sau vài giây (gộp nhiều lần đổi liền nhau) — chỉ khi bật tự đồng bộ và đã ghép máy.
    public static void SyncSoon(double seconds = 3) => Native.SyncVoid("sync.soon", new { seconds });

    public static Task SyncAll(bool manual) => Native.CallNode("sync.all", new { manual });
}
