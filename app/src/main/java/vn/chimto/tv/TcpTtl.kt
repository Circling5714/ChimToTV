package vn.chimto.tv

import android.system.Os
import android.system.OsConstants
import java.io.FileDescriptor
import java.io.FileInputStream
import java.net.Socket

/**
 * TTL (IPv4) / hop limit (IPv6) của một socket TCP — cho Vượt chặn mức 2 (Socks5Bridge.writeHello): gửi byte đầu của gói
 * bắt tay với TTL 1 để nó chết ở router đầu tiên, rồi trả TTL về mặc định. Không cần quyền root (app đặt TTL trên socket
 * của chính mình). Android chạy socket IPv4 bằng socket AF_INET6 (địa chỉ ::ffff:a.b.c.d) nên đặt cả hai tầng. Trên Linux,
 * giá trị -1 = trả về mặc định của hệ thống (khỏi phải đọc giá trị cũ — getsockopt không có trong SDK công khai).
 *
 * Lấy fd: Android 7+ luồng đọc của Socket là FileInputStream (getFD = fd của chính socket, không được đóng). Không dùng
 * ParcelFileDescriptor.fromSocket rồi đóng: trên Android 9 nó KHÔNG nhân bản fd như tài liệu viết — đóng là đóng luôn
 * socket (đo trên TX3: «Socket closed», EBADF); detachFd cũng không được (vô hiệu luôn FileDescriptor của socket). Android
 * 5–6 (luồng đọc chưa là FileInputStream): gọi Socket.getFileDescriptor$ bằng reflection — chưa có hạn chế API ẩn.
 */
object TcpTtl {
    /** Đặt TTL / hop limit (-1 = mặc định); true nếu ít nhất một tầng đặt được (test JVM: false). */
    fun set(s: Socket, ttl: Int): Boolean = try {
        val fd = fdOf(s) ?: throw IllegalStateException("không lấy được fd")
        val a = runCatching { Os.setsockoptInt(fd, OsConstants.IPPROTO_IP, OsConstants.IP_TTL, ttl) }
        val b = runCatching { Os.setsockoptInt(fd, OsConstants.IPPROTO_IPV6, OsConstants.IPV6_UNICAST_HOPS, ttl) }
        if (BuildConfig.DEBUG) android.util.Log.d("ChimToTtl", "ttl=$ttl ipv4=${a.exceptionOrNull() ?: "ok"} ipv6=${b.exceptionOrNull() ?: "ok"}")
        a.isSuccess || b.isSuccess
    } catch (e: Throwable) {
        if (BuildConfig.DEBUG) android.util.Log.d("ChimToTtl", "không đặt được TTL: $e")
        false
    }

    private fun fdOf(s: Socket): FileDescriptor? {
        (s.getInputStream() as? FileInputStream)?.fd?.takeIf { it.valid() }?.let { return it }
        return Socket::class.java.getMethod("getFileDescriptor\$").invoke(s) as? FileDescriptor
    }
}
