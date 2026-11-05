using System.Text;
using System.Text.Json;

namespace ConBangTV.Core;

/// Đọc một giá trị JSON bắt đầu tại một vị trí trong chuỗi dài, dừng ngay sau giá trị đó (bản iPad: JSONScan.swift).
/// Dữ liệu Next.js của trang (self.__next_f.push) là văn bản dài lẫn nhiều thứ; app chỉ bóc từng đối tượng cần.
/// Kết quả: Dictionary&lt;string, object?&gt;, List&lt;object?&gt;, string, long, double, bool, null.
public sealed class JsonScan
{
    private readonly string _u;
    private int _i;

    private sealed class Bad : Exception { }

    /// Không đọc được → Fail (khác với giá trị null của JSON).
    public static readonly object Fail = new();

    public static object? Value(string text, int index)
    {
        if (index < 0 || index >= text.Length) return Fail;
        var p = new JsonScan(text, index);
        try
        {
            return p.ParseValue();
        }
        catch (Bad)
        {
            return Fail;
        }
        catch (IndexOutOfRangeException)
        {
            return Fail;
        }
    }

    private JsonScan(string u, int i)
    {
        _u = u;
        _i = i;
    }

    private void Ws()
    {
        while (_i < _u.Length && _u[_i] is ' ' or '\n' or '\r' or '\t') _i++;
    }

    private object? ParseValue()
    {
        Ws();
        if (_i >= _u.Length) throw new Bad();
        switch (_u[_i])
        {
            case '{': return ParseObject();
            case '[': return ParseArray();
            case '"': return ParseString();
            case 't': Literal("true"); return true;
            case 'f': Literal("false"); return false;
            case 'n': Literal("null"); return null;
            default: return ParseNumber();
        }
    }

    private void Literal(string word)
    {
        foreach (var c in word)
        {
            if (_i >= _u.Length || _u[_i] != c) throw new Bad();
            _i++;
        }
    }

    private Dictionary<string, object?> ParseObject()
    {
        _i++;
        var o = new Dictionary<string, object?>();
        Ws();
        if (_i < _u.Length && _u[_i] == '}') { _i++; return o; }
        while (true)
        {
            Ws();
            if (_i >= _u.Length || _u[_i] != '"') throw new Bad();
            var key = ParseString();
            Ws();
            if (_i >= _u.Length || _u[_i] != ':') throw new Bad();
            _i++;
            var v = ParseValue();
            o.TryAdd(key, v);
            Ws();
            if (_i >= _u.Length) throw new Bad();
            if (_u[_i] == ',') { _i++; continue; }
            if (_u[_i] == '}') { _i++; return o; }
            throw new Bad();
        }
    }

    private List<object?> ParseArray()
    {
        _i++;
        var a = new List<object?>();
        Ws();
        if (_i < _u.Length && _u[_i] == ']') { _i++; return a; }
        while (true)
        {
            a.Add(ParseValue());
            Ws();
            if (_i >= _u.Length) throw new Bad();
            if (_u[_i] == ',') { _i++; continue; }
            if (_u[_i] == ']') { _i++; return a; }
            throw new Bad();
        }
    }

    private string ParseString()
    {
        _i++;
        var sb = new StringBuilder();
        while (_i < _u.Length)
        {
            var c = _u[_i++];
            if (c == '"') return sb.ToString();
            if (c != '\\') { sb.Append(c); continue; }
            if (_i >= _u.Length) throw new Bad();
            var e = _u[_i++];
            switch (e)
            {
                case '"': case '\\': case '/': sb.Append(e); break;
                case 'b': sb.Append('\b'); break;
                case 'f': sb.Append('\f'); break;
                case 'n': sb.Append('\n'); break;
                case 'r': sb.Append('\r'); break;
                case 't': sb.Append('\t'); break;
                case 'u':
                    if (_i + 4 > _u.Length) throw new Bad();
                    if (!int.TryParse(_u.AsSpan(_i, 4), System.Globalization.NumberStyles.HexNumber, null, out var v)) throw new Bad();
                    sb.Append((char)v);
                    _i += 4;
                    break;
                default: sb.Append(e); break;
            }
        }
        throw new Bad();
    }

    private object ParseNumber()
    {
        var start = _i;
        var isDouble = false;
        while (_i < _u.Length)
        {
            var c = _u[_i];
            if (c is >= '0' and <= '9' or '-' or '+') { _i++; continue; }
            if (c is '.' or 'e' or 'E') { isDouble = true; _i++; continue; }
            break;
        }
        if (_i == start) throw new Bad();
        var s = _u[start.._i];
        if (!isDouble && long.TryParse(s, out var n)) return n;
        if (double.TryParse(s, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var d)) return d;
        throw new Bad();
    }

    /// JSON chuẩn (API) → cùng kiểu dữ liệu như JsonScan.
    public static object? Parse(byte[] data) => FromElement(JsonDocument.Parse(data).RootElement);

    public static object? Parse(string text) => FromElement(JsonDocument.Parse(text).RootElement);

    public static Dictionary<string, object?>? ParseObject(byte[] data)
    {
        try
        {
            return Parse(data) as Dictionary<string, object?>;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static object? FromElement(JsonElement e)
    {
        switch (e.ValueKind)
        {
            case JsonValueKind.Object:
                var o = new Dictionary<string, object?>();
                foreach (var p in e.EnumerateObject()) o.TryAdd(p.Name, FromElement(p.Value));
                return o;
            case JsonValueKind.Array:
                return e.EnumerateArray().Select(FromElement).ToList();
            case JsonValueKind.String: return e.GetString();
            case JsonValueKind.Number: return e.TryGetInt64(out var n) ? (object)n : e.GetDouble();
            case JsonValueKind.True: return true;
            case JsonValueKind.False: return false;
            default: return null;
        }
    }
}

/// Đọc khoá của đối tượng JSON an toàn (thiếu / null / sai kiểu → mặc định), như optString / optInt của org.json.
public static class J
{
    public static string Str(Dictionary<string, object?>? o, string key) =>
        o != null && o.TryGetValue(key, out var v) && v is string s ? s.Trim() : "";

    public static int Int(Dictionary<string, object?>? o, string key, int def = 0)
    {
        if (o == null || !o.TryGetValue(key, out var v)) return def;
        return v switch
        {
            long n => (int)n,
            double d => (int)d,
            string s => int.TryParse(s, out var i) ? i : def,
            _ => def,
        };
    }

    public static bool Bool(Dictionary<string, object?>? o, string key, bool def = false)
    {
        if (o == null || !o.TryGetValue(key, out var v)) return def;
        return v switch
        {
            bool b => b,
            string s => s == "true" || (s != "false" && def),
            _ => def,
        };
    }

    public static Dictionary<string, object?>? Obj(Dictionary<string, object?>? o, string key) =>
        o != null && o.TryGetValue(key, out var v) ? v as Dictionary<string, object?> : null;

    public static List<object?>? Arr(Dictionary<string, object?>? o, string key) =>
        o != null && o.TryGetValue(key, out var v) ? v as List<object?> : null;

    public static List<Dictionary<string, object?>> Objects(List<object?>? a) =>
        (a ?? new()).OfType<Dictionary<string, object?>>().ToList();

    /// Giá trị dạng chữ của khoá bất kỳ kiểu (số → chữ số), không có → "".
    public static string Any(Dictionary<string, object?>? o, string key)
    {
        if (o == null || !o.TryGetValue(key, out var v) || v == null) return "";
        return v switch
        {
            double d => d.ToString(System.Globalization.CultureInfo.InvariantCulture),
            _ => v.ToString() ?? "",
        };
    }
}
