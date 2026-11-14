using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace ConBangTV.Core;

/// Lõi Rust dùng chung với bản Linux (conbang_core.dll — dựng từ linux/ffi, lõi ở linux/core): đọc trang, trang chủ tổng hợp,
/// kho cài đặt (prefs.json), mạng (cổng SOCKS nội bộ, DNS mã hoá, vượt chặn, proxy), đồng bộ, cổng phát nội bộ.
/// Gọi bằng JSON: <see cref="Call{T}"/> chạy nền (kết quả qua callback từ luồng của lõi), <see cref="Sync{T}"/> chờ ngay (hàm
/// nhanh — cài đặt…). Lỗi của lõi → <see cref="AppError"/> với lời báo đã dịch sẵn (util::friendly của lõi).
public static class Native
{
    private const string Dll = "conbang_core";

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void Callback(ulong id, IntPtr json);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern int cbtv_init([MarshalAs(UnmanagedType.LPUTF8Str)] string dataDir, Callback cb);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern void cbtv_call(ulong id, [MarshalAs(UnmanagedType.LPUTF8Str)] string method,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string args);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr cbtv_call_sync([MarshalAs(UnmanagedType.LPUTF8Str)] string method,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string args);

    [DllImport(Dll, CallingConvention = CallingConvention.Cdecl)]
    private static extern void cbtv_free(IntPtr p);

    /// Tên trường JSON kiểu camelCase như lõi (serde rename_all = "camelCase").
    public static readonly JsonSerializerOptions Json = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        IncludeFields = true,
    };

    // Giữ sống suốt đời app (lõi gọi lại bất cứ lúc nào)
    private static readonly Callback Cb = OnCallback;
    private static readonly ConcurrentDictionary<ulong, Pending> Calls = new();
    private static long _next;
    private static bool _ready;

    /// Đã mở lõi chưa.
    public static bool Ready => _ready;

    private sealed record Pending(TaskCompletionSource<JsonNode?> Tcs, Action<JsonNode?>? Part);

    /// Sự kiện của lõi (tên, dữ liệu) — gọi từ luồng của lõi, bên nhận tự chuyển sang luồng giao diện nếu cần.
    public static event Action<string, JsonNode?>? Event;

    /// Mở lõi (một lần, trước mọi lời gọi khác).
    public static void Init(string dataDir)
    {
        if (_ready) return;
        if (cbtv_init(dataDir, Cb) != 0) throw new InvalidOperationException("Không mở được lõi app (conbang_core.dll)");
        _ready = true;
    }

    private static void OnCallback(ulong id, IntPtr p)
    {
        JsonObject? o;
        try
        {
            o = JsonNode.Parse(Marshal.PtrToStringUTF8(p) ?? "null") as JsonObject;
        }
        catch (JsonException)
        {
            o = null;
        }
        if (id == 0)
        {
            if (o?["event"]?.GetValue<string>() is { } name)
            {
                try
                {
                    Event?.Invoke(name, o["data"]?.DeepClone());
                }
                catch (Exception)
                {
                }
            }
            return;
        }
        if (o != null && o.ContainsKey("part"))
        {
            if (Calls.TryGetValue(id, out var pend)) pend.Part?.Invoke(o["part"]?.DeepClone());
            return;
        }
        if (!Calls.TryRemove(id, out var c)) return;
        if (o == null) c.Tcs.TrySetException(new AppError("Lõi app trả lời sai"));
        else if (o["err"] is { } e) c.Tcs.TrySetException(new AppError(e.GetValue<string>()));
        else c.Tcs.TrySetResult(o["ok"]?.DeepClone());
    }

    private static string Args(object? args) => args == null ? "null" : JsonSerializer.Serialize(args, Json);

    /// Gọi chạy nền; part = nhận từng phần (hàm báo tiến độ / trả dần).
    public static Task<JsonNode?> CallNode(string method, object? args = null, Action<JsonNode?>? part = null)
    {
        var id = (ulong)Interlocked.Increment(ref _next);
        var tcs = new TaskCompletionSource<JsonNode?>(TaskCreationOptions.RunContinuationsAsynchronously);
        Calls[id] = new Pending(tcs, part);
        cbtv_call(id, method, Args(args));
        return tcs.Task;
    }

    public static async Task<T> Call<T>(string method, object? args = null, Action<JsonNode?>? part = null) =>
        To<T>(await CallNode(method, args, part).ConfigureAwait(false));

    public static Task CallVoid(string method, object? args = null) => CallNode(method, args);

    /// Gọi hàm nhanh, chờ kết quả ngay.
    public static JsonNode? SyncNode(string method, object? args = null)
    {
        var p = cbtv_call_sync(method, Args(args));
        if (p == IntPtr.Zero) throw new AppError("Lõi app không trả lời");
        string s;
        try
        {
            s = Marshal.PtrToStringUTF8(p) ?? "null";
        }
        finally
        {
            cbtv_free(p);
        }
        var o = JsonNode.Parse(s) as JsonObject ?? throw new AppError("Lõi app trả lời sai");
        if (o["err"] is { } e) throw new AppError(e.GetValue<string>());
        return o["ok"];
    }

    public static T Sync<T>(string method, object? args = null) => To<T>(SyncNode(method, args));

    public static void SyncVoid(string method, object? args = null) => SyncNode(method, args);

    public static T To<T>(JsonNode? n) => n == null ? default! : n.Deserialize<T>(Json)!;
}
