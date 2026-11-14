using System.Globalization;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Threading;
using ConBangTV.Core;

namespace ConBangTV.Player;

/// Trình phát native (thử nghiệm, như bản Linux 1.1): libmpv (libmpv-2.dll — gói trong .exe, build.ps1 tải bản dựng của
/// shinchiro/mpv-winbuild-cmake, kiểm SHA-256) vẽ thẳng vào một cửa sổ con bằng Direct3D 11, giải mã bằng card đồ hoạ khi được
/// (hwdec=auto-safe → d3d11va). mpv chỉ tải từ cổng phát nội bộ của lõi Rust (media_proxy.rs: DNS mã hoá, vượt chặn, proxy, bỏ
/// quảng cáo như hls.html, cắt phần ngụy trang PNG, nối khúc MP4). Thanh điều khiển vẫn ở cửa sổ phủ (PlayerOverlay) như với
/// WebView2. Trạng thái / lệnh theo đúng giao thức của player-agent.js (t, d, paused, ended, waiting, ready, err; toggle, play,
/// pause, seekTo, rate, fit) để PlayerView giữ nguyên mọi logic (xem tiếp, nhạc mở đầu, đổi server, giới hạn giờ…).
public sealed class NativeVideo : IDisposable
{
    private const string Dll = "libmpv-2";

    // mpv_event_id
    private const int EventNone = 0, EventShutdown = 1, EventLog = 2, EventStartFile = 6, EventEndFile = 7, EventFileLoaded = 8,
        EventPlaybackRestart = 21;
    private const int EndError = 4;
    private const int FormatString = 1, FormatFlag = 3, FormatInt64 = 4, FormatDouble = 5;

    [StructLayout(LayoutKind.Sequential)]
    private struct MpvEvent
    {
        public int EventId;
        public int Error;
        public ulong ReplyUserdata;
        public IntPtr Data;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MpvEndFile
    {
        public int Reason;
        public int Error;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MpvLog
    {
        public IntPtr Prefix, Level, Text;
        public int LogLevel;
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void Wakeup(IntPtr d);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_create();
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_initialize(IntPtr h);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_terminate_destroy(IntPtr h);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int mpv_set_option_string(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, [MarshalAs(UnmanagedType.LPUTF8Str)] string data);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int mpv_set_property_string(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, [MarshalAs(UnmanagedType.LPUTF8Str)] string data);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int mpv_get_property(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, int format, out double data);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl, EntryPoint = "mpv_get_property")]
    private static extern int mpv_get_property_int(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, int format, out int data);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl, EntryPoint = "mpv_get_property")]
    private static extern int mpv_get_property_long(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string name, int format, out long data);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr mpv_get_property_string(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string name);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_free(IntPtr p);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern int mpv_command(IntPtr h, IntPtr[] args);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_wait_event(IntPtr h, double timeout);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern void mpv_set_wakeup_callback(IntPtr h, Wakeup? cb, IntPtr d);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int mpv_request_log_messages(IntPtr h, [MarshalAs(UnmanagedType.LPUTF8Str)] string level);
    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)] private static extern IntPtr mpv_error_string(int e);

    /// Máy có libmpv không (gói trong .exe — thiếu thì phát bằng WebView2 như trước).
    public static bool Available
    {
        get
        {
            if (_available is { } a) return a;
            try
            {
                _available = NativeLibrary.TryLoad(Dll, typeof(NativeVideo).Assembly, null, out _);
            }
            catch (Exception)
            {
                _available = false;
            }
            if (_available == false) Log.D("native", "Không có libmpv-2.dll — phát bằng WebView2");
            return _available.Value;
        }
    }

    private static bool? _available;

    /// Trạng thái phát (như window.__chimtoState của player-agent.js).
    public sealed record State(double T, double D, bool Paused, bool Ended, bool Waiting, bool Ready, string? Err);

    public readonly MpvHost Host = new();
    private readonly IntPtr _h;
    private readonly Wakeup _wakeup;
    private readonly Dispatcher _ui;
    private int _wakePending;
    private bool _restarted;
    private string? _err;
    private string? _pendingSub;
    private int _logLines;
    private bool _disposed;

    private NativeVideo(IntPtr h, Dispatcher ui)
    {
        _h = h;
        _ui = ui;
        _wakeup = _ =>
        {
            if (Interlocked.Exchange(ref _wakePending, 1) == 0) _ui.BeginInvoke(DispatcherPriority.Background, Drain);
        };
    }

    /// Tạo trình phát (một cái cho cả trình phát của app; tắt phim thì huỷ để trả bộ nhớ). null = không mở được libmpv.
    public static NativeVideo? Create()
    {
        if (!Available) return null;
        try
        {
            var h = mpv_create();
            if (h == IntPtr.Zero) return null;
            var n = new NativeVideo(h, Dispatcher.CurrentDispatcher);
            var mute = Environment.GetEnvironmentVariable("CONBANG_MUTE") == "1" ? "yes" : "no";
            var opts = new (string, string)[]
            {
                ("wid", n.Host.Hwnd.ToInt64().ToString(CultureInfo.InvariantCulture)),
                ("hwdec", "auto-safe"),
                ("config", "no"), ("load-scripts", "no"), ("ytdl", "no"), ("osc", "no"),
                ("input-default-bindings", "no"), ("input-vo-keyboard", "no"), ("input-cursor", "no"), ("cursor-autohide", "no"),
                ("terminal", "no"), ("idle", "yes"),
                // Hết phim: đứng ở khung cuối (eof-reached) để app tự chuyển tập — như <video> ended
                ("keep-open", "yes"),
                ("audio-client-name", "Côn Bằng TV"), ("sub-auto", "no"), ("osd-level", "0"),
                // Bộ đệm ~2 phút như hls.html (BUFFER = 120)
                ("cache", "yes"), ("cache-secs", "120"), ("demuxer-max-bytes", "48MiB"), ("demuxer-max-back-bytes", "16MiB"),
                ("network-timeout", "30"),
                // Đoạn HLS lỗi: tải lại thay vì bỏ qua (cổng nội bộ đã tự thử lại 3 lần)
                ("demuxer-lavf-o", "seg_max_retry=5"),
                ("user-agent", Http.UserAgent), ("mute", mute),
                ("background-color", "#000000"),
            };
            foreach (var (k, v) in opts)
            {
                var r = mpv_set_option_string(h, k, v);
                if (r < 0) Log.D("native", $"tuỳ chọn {k}={v}: {Err(r)}");
            }
            var init = mpv_initialize(h);
            if (init < 0)
            {
                Log.D("native", "mpv_initialize lỗi: " + Err(init));
                mpv_terminate_destroy(h);
                return null;
            }
            mpv_request_log_messages(h, "warn");
            mpv_set_wakeup_callback(h, n._wakeup, IntPtr.Zero);
            Log.D("native", "mpv (libmpv-2.dll) vẽ bằng Direct3D, giải mã bằng card đồ hoạ khi được");
            return n;
        }
        catch (Exception e)
        {
            Log.D("native", "Không mở được libmpv: " + e.Message);
            _available = false;
            return null;
        }
    }

    private static string Err(int code) => Marshal.PtrToStringUTF8(mpv_error_string(code)) ?? code.ToString(CultureInfo.InvariantCulture);

    private int Command(params string[] args)
    {
        var ptrs = new IntPtr[args.Length + 1];
        try
        {
            for (var i = 0; i < args.Length; i++) ptrs[i] = Marshal.StringToCoTaskMemUTF8(args[i]);
            var r = mpv_command(_h, ptrs);
            if (r < 0) Log.D("native", $"lệnh {args[0]}: {Err(r)}");
            return r;
        }
        finally
        {
            foreach (var p in ptrs)
                if (p != IntPtr.Zero) Marshal.FreeCoTaskMem(p);
        }
    }

    private double? GetDouble(string name) => mpv_get_property(_h, name, FormatDouble, out var v) >= 0 ? v : null;

    private bool? GetFlag(string name) => mpv_get_property_int(_h, name, FormatFlag, out var v) >= 0 ? v != 0 : null;

    private long? GetLong(string name) => mpv_get_property_long(_h, name, FormatInt64, out var v) >= 0 ? v : null;

    private string? GetString(string name)
    {
        var p = mpv_get_property_string(_h, name);
        if (p == IntPtr.Zero) return null;
        try
        {
            return Marshal.PtrToStringUTF8(p);
        }
        finally
        {
            mpv_free(p);
        }
    }

    private void Drain()
    {
        Interlocked.Exchange(ref _wakePending, 0);
        if (_disposed) return;
        while (true)
        {
            var ev = Marshal.PtrToStructure<MpvEvent>(mpv_wait_event(_h, 0));
            switch (ev.EventId)
            {
                case EventNone:
                case EventShutdown:
                    return;
                case EventStartFile:
                    _restarted = false;
                    break;
                case EventFileLoaded:
                    if (_pendingSub is { } sub)
                    {
                        _pendingSub = null;
                        Command("sub-add", sub, "select", "Tiếng Việt");
                    }
                    break;
                case EventPlaybackRestart:
                    if (!_restarted)
                    {
                        _restarted = true;
                        Log.D("native", "native: chạy");
                    }
                    break;
                case EventEndFile when ev.Data != IntPtr.Zero:
                    var e = Marshal.PtrToStructure<MpvEndFile>(ev.Data);
                    if (e.Reason == EndError)
                    {
                        _err = Err(e.Error);
                        Log.D("native", "mpv không phát được: " + _err);
                    }
                    break;
                case EventLog when ev.Data != IntPtr.Zero && _logLines < 60:
                    _logLines++;
                    var m = Marshal.PtrToStructure<MpvLog>(ev.Data);
                    Log.D("mpv", $"[{Marshal.PtrToStringUTF8(m.Prefix)}] {Marshal.PtrToStringUTF8(m.Level)}: {Marshal.PtrToStringUTF8(m.Text)?.TrimEnd()}");
                    break;
            }
        }
    }

    /// Phát địa chỉ (của cổng phát nội bộ) từ giây start, kèm phụ đề rời nếu có.
    public void Load(string url, double start, string? sub)
    {
        _err = null;
        _pendingSub = sub;
        _logLines = 0;
        _restarted = false;
        mpv_set_property_string(_h, "start", start > 0 ? start.ToString(CultureInfo.InvariantCulture) : "none");
        mpv_set_property_string(_h, "pause", "no");
        Command("loadfile", url, "replace");
    }

    public void Stop()
    {
        Command("stop");
        _restarted = false;
    }

    public State Read()
    {
        var t = GetDouble("time-pos");
        var seeking = GetFlag("seeking") ?? false;
        return new State(Math.Max(0, t ?? 0), GetDouble("duration") ?? 0, GetFlag("pause") ?? false, GetFlag("eof-reached") ?? false,
            (GetFlag("paused-for-cache") ?? false) || seeking, _restarted && t != null, _err);
    }

    /// Lệnh như chimtoCmd của player-agent.js.
    public void Cmd(string name, double? arg)
    {
        switch (name)
        {
            case "toggle":
                mpv_set_property_string(_h, "pause", GetFlag("pause") == true ? "no" : "yes");
                break;
            case "play":
                mpv_set_property_string(_h, "pause", "no");
                break;
            case "pause":
                mpv_set_property_string(_h, "pause", "yes");
                break;
            case "seekTo" when arg is { } t:
                Command("seek", t.ToString(CultureInfo.InvariantCulture), "absolute");
                break;
            case "rate" when arg is { } r:
                mpv_set_property_string(_h, "speed", r.ToString(CultureInfo.InvariantCulture));
                break;
            // Lấp đầy màn hình: phóng hình cho hết viền đen (cắt bớt hai bên)
            case "fit":
                mpv_set_property_string(_h, "panscan", (arg ?? 0) > 0.5 ? "1.0" : "0.0");
                break;
        }
    }

#if DEBUG
    /// Cổng gỡ lỗi: giả lỗi luồng (thử đường tự chuyển sang WebView2).
    public void DebugFail() => _err = "lỗi giả (cổng gỡ lỗi)";
#endif

    /// Cổng gỡ lỗi: khung rơi, cỡ hình, bộ giải mã.
    public string Debug() =>
        $"dropped={GetLong("frame-drop-count") ?? -1} decoderDropped={GetLong("decoder-frame-drop-count") ?? -1} " +
        $"video={GetLong("width") ?? 0}x{GetLong("height") ?? 0} hwdec={GetString("hwdec-current")} vo={GetString("current-vo")} " +
        $"cache={GetDouble("demuxer-cache-duration") ?? 0:0}s";

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        mpv_set_wakeup_callback(_h, null, IntPtr.Zero);
        mpv_terminate_destroy(_h);
        Host.Release();
    }
}

/// Cửa sổ con cho mpv (--wid). Tạo một lần, giữ suốt đời trình phát: chuyển sang cửa sổ phim nhỏ / về cửa sổ app thì WPF dựng lại
/// HwndHost — chỉ đổi cửa sổ cha (SetParent), mpv vẫn vẽ vào đúng cửa sổ đó.
public sealed class MpvHost : HwndHost
{
    private const int WsChild = 0x40000000, WsVisible = 0x10000000, WsClipChildren = 0x02000000, WsClipSiblings = 0x04000000;
    private const int WsPopup = unchecked((int)0x80000000);
    private const int SsBlackRect = 0x4;

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateWindowEx(int exStyle, string cls, string name, int style, int x, int y, int w, int h, IntPtr parent,
        IntPtr menu, IntPtr inst, IntPtr param);

    [DllImport("user32.dll")] private static extern IntPtr SetParent(IntPtr child, IntPtr parent);
    [DllImport("user32.dll")] private static extern bool DestroyWindow(IntPtr h);

    /// Cửa sổ ẩn giữ cửa sổ của mpv lúc chưa / không gắn vào đâu.
    private static IntPtr _parking;

    public readonly IntPtr Hwnd;

    public MpvHost()
    {
        if (_parking == IntPtr.Zero) _parking = CreateWindowEx(0, "static", "", WsPopup, 0, 0, 1, 1, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
        Hwnd = CreateWindowEx(0, "static", "", WsChild | WsVisible | WsClipChildren | WsClipSiblings | SsBlackRect, 0, 0, 16, 9, _parking,
            IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
        Focusable = false;
    }

    protected override HandleRef BuildWindowCore(HandleRef hwndParent)
    {
        SetParent(Hwnd, hwndParent.Handle);
        return new HandleRef(this, Hwnd);
    }

    protected override void DestroyWindowCore(HandleRef hwnd) => SetParent(Hwnd, _parking);

    /// Trình phát đã huỷ: bỏ cửa sổ.
    public void Release()
    {
        if (Hwnd != IntPtr.Zero) DestroyWindow(Hwnd);
    }
}
