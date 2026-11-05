using System.Runtime.InteropServices;
using System.Text;

namespace ConBangTV.Core;

/// Mã hoá mật khẩu proxy bằng DPAPI của Windows (chỉ tài khoản Windows này trên máy này giải được) — bản TV: Secret.kt
/// (AndroidKeyStore), iPad: Keychain.
public static class Secret
{
    [StructLayout(LayoutKind.Sequential)]
    private struct Blob
    {
        public int Size;
        public IntPtr Data;
    }

    [DllImport("crypt32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CryptProtectData(ref Blob input, string? desc, IntPtr entropy, IntPtr reserved, IntPtr prompt, int flags,
        out Blob output);

    [DllImport("crypt32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool CryptUnprotectData(ref Blob input, IntPtr desc, IntPtr entropy, IntPtr reserved, IntPtr prompt, int flags,
        out Blob output);

    [DllImport("kernel32.dll")]
    private static extern IntPtr LocalFree(IntPtr mem);

    private const int UiForbidden = 0x1;

    public static string Protect(string plain)
    {
        if (plain.Length == 0) return "";
        var r = Run(Encoding.UTF8.GetBytes(plain), true);
        return r == null ? "" : Convert.ToBase64String(r);
    }

    public static string Unprotect(string? stored)
    {
        if (string.IsNullOrEmpty(stored)) return "";
        try
        {
            var r = Run(Convert.FromBase64String(stored), false);
            return r == null ? "" : Encoding.UTF8.GetString(r);
        }
        catch (FormatException)
        {
            return "";
        }
    }

    private static byte[]? Run(byte[] data, bool protect)
    {
        var h = GCHandle.Alloc(data, GCHandleType.Pinned);
        try
        {
            var input = new Blob { Size = data.Length, Data = h.AddrOfPinnedObject() };
            var ok = protect
                ? CryptProtectData(ref input, "ConBangTV", IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, UiForbidden, out var output)
                : CryptUnprotectData(ref input, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, UiForbidden, out output);
            if (!ok) return null;
            try
            {
                var r = new byte[output.Size];
                Marshal.Copy(output.Data, r, 0, output.Size);
                return r;
            }
            finally
            {
                LocalFree(output.Data);
            }
        }
        finally
        {
            h.Free();
        }
    }
}
