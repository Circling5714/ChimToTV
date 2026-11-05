using System.IO;
namespace ConBangTV.Core;

/// Nhật ký nhỏ: ghi vào %LOCALAPPDATA%\ConBangTV\log.txt (giữ ~500 KB) + vòng 2 000 dòng trong bộ nhớ (cổng gỡ lỗi /log).
public static class Log
{
    private static readonly object Lock = new();
    private static readonly List<(int Seq, string Text)> Lines = new();
    private static int _seq;
    private static string? _file;

    public static void Start()
    {
        _file = Path.Combine(Prefs.DataDir, "log.txt");
        try
        {
            if (File.Exists(_file) && new FileInfo(_file).Length > 500_000)
            {
                File.Move(_file, _file + ".1", true);
            }
        }
        catch (Exception)
        {
        }
    }

    public static void D(string tag, string message)
    {
        var line = $"{DateTime.Now:HH:mm:ss.fff} {tag}: {message}";
        lock (Lock)
        {
            _seq++;
            Lines.Add((_seq, line));
            if (Lines.Count > 2000) Lines.RemoveRange(0, Lines.Count - 2000);
            try
            {
                if (_file != null) File.AppendAllText(_file, $"{DateTime.Now:yyyy-MM-dd} {line}\n");
            }
            catch (Exception)
            {
            }
        }
        System.Diagnostics.Debug.WriteLine(line);
    }

    /// Các dòng có số thứ tự &gt; since; trả kèm số thứ tự cuối để lần sau hỏi tiếp.
    public static (int Last, List<string> Lines) Since(int since)
    {
        lock (Lock) return (_seq, Lines.Where(l => l.Seq > since).Select(l => l.Text).ToList());
    }
}
