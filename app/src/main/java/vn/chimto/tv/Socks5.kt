package vn.chimto.tv

import java.io.DataInputStream
import java.io.IOException
import java.io.OutputStream
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.UnknownHostException
import java.util.Collections
import java.util.concurrent.Executors
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLException
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory

/** Proxy SOCKS5 thật mà người xem nhập trong Cài đặt. */
data class SocksConfig(
    val host: String, val port: Int, val user: String = "", val pass: String = "",
    /** true: proxy phân giải tên miền (gửi tên miền); false: thiết bị tự phân giải rồi gửi IP cho proxy. */
    val remoteDns: Boolean = true,
    /** true: chỉ trang phim (trong danh sách) đi qua proxy; video / ảnh CDN, GitHub… đi thẳng mạng nhà. */
    val sitesOnly: Boolean = false,
)

/**
 * Cài đặt của cổng nội bộ: proxy SOCKS5 (null = đi thẳng) và DNS mã hoá cho các tên miền thiết bị tự phân giải
 * (null = DNS của thiết bị / nhà mạng).
 */
/** split: Vượt chặn nhà mạng (mức 1: chia nhỏ gói bắt tay); disorder: mức 2 — thêm đảo thứ tự (byte đầu gửi với TTL 1). */
data class RouteConfig(val socks: SocksConfig?, val doh: Doh.Server?, val split: Boolean = false, val disorder: Boolean = false)

/** Gói bắt tay sắp gửi: tên trang, có đi qua proxy không, có đảo thứ tự không (Socks5Bridge.writeHello). */
internal data class Hello(val host: String, val viaProxy: Boolean, val disorder: Boolean)

/** Lỗi khi mở kết nối; code = mã trả lời SOCKS5 gửi lại bên gọi; viaProxy = lỗi ở phía proxy (để báo người xem). */
class SocksError(val code: Int, message: String, val viaProxy: Boolean = true) : IOException(message)

/** Vị trí đầu tiên của name (chữ thường) trong b[from until to], không phân biệt hoa / thường. */
internal fun find(name: ByteArray, b: ByteArray, from: Int, to: Int): Int? {
    var s = from
    while (s + name.size <= to) {
        var k = 0
        while (k < name.size) {
            val c = b[s + k].toInt() and 0xFF
            if ((if (c in 0x41..0x5A) c or 0x20 else c) != (name[k].toInt() and 0xFF)) break
            k++
        }
        if (k == name.size) return s
        s++
    }
    return null
}

/** Phía khách SOCKS5 (RFC 1928, đăng nhập theo RFC 1929): đi qua proxy thật tới một đích. */
object Socks5 {
    private val REPLIES = mapOf(
        1 to "lỗi chung", 2 to "proxy không cho phép", 3 to "mạng không tới được", 4 to "máy đích không tới được",
        5 to "máy đích từ chối", 6 to "hết thời gian", 7 to "lệnh không hỗ trợ", 8 to "kiểu địa chỉ không hỗ trợ",
    )

    /**
     * Kết nối tới host:port qua proxy c. ip != null: đích đã là IP. DNS của proxy: gửi tên miền để proxy phân giải;
     * DNS của thiết bị: tự phân giải ở đây rồi gửi IP.
     */
    fun connect(c: SocksConfig, host: String, ip: InetAddress?, port: Int, resolve: (String) -> InetAddress = ::resolve): Socket {
        val s = Socket()
        try {
            s.connect(InetSocketAddress(c.host, c.port), 10_000)
        } catch (e: Exception) {
            s.close()
            throw SocksError(1, if (e is UnknownHostException) "không tìm thấy proxy ${c.host}" else "không kết nối được proxy ${c.host}:${c.port}")
        }
        try {
            s.tcpNoDelay = true
            s.keepAlive = true
            s.soTimeout = 15_000
            handshake(s, c, host, ip, port, resolve)
            s.soTimeout = 0
            return s
        } catch (e: IOException) {
            s.close()
            throw e as? SocksError ?: SocksError(1, "proxy ${c.host}:${c.port} ngắt kết nối (${e.message})")
        }
    }

    private fun handshake(s: Socket, c: SocksConfig, host: String, ip: InetAddress?, port: Int, resolve: (String) -> InetAddress) {
        val i = DataInputStream(s.getInputStream())
        val o = s.getOutputStream()
        val login = c.user.isNotEmpty()
        o.write(if (login) byteArrayOf(5, 2, 0, 2) else byteArrayOf(5, 1, 0))
        if (i.readUnsignedByte() != 5) throw SocksError(1, "${c.host}:${c.port} không phải proxy SOCKS5")
        when (i.readUnsignedByte()) {
            0 -> Unit
            2 -> {
                if (!login) throw SocksError(1, "proxy cần tên đăng nhập / mật khẩu")
                val u = c.user.toByteArray()
                val p = c.pass.toByteArray()
                if (u.size > 255 || p.size > 255) throw SocksError(1, "tên đăng nhập / mật khẩu quá dài (tối đa 255 byte)")
                o.write(byteArrayOf(1, u.size.toByte()) + u + byteArrayOf(p.size.toByte()) + p)
                i.readUnsignedByte()
                if (i.readUnsignedByte() != 0) throw SocksError(1, "proxy báo sai tên đăng nhập / mật khẩu")
            }
            else -> throw SocksError(1, if (login) "proxy không nhận cách đăng nhập này" else "proxy cần tên đăng nhập / mật khẩu")
        }
        o.write(byteArrayOf(5, 1, 0) + address(c, host, ip, resolve) + byteArrayOf((port shr 8).toByte(), port.toByte()))
        if (i.readUnsignedByte() != 5) throw SocksError(1, "proxy trả lời sai giao thức")
        val rep = i.readUnsignedByte()
        i.readUnsignedByte()
        when (i.readUnsignedByte()) { // địa chỉ proxy gắn cho kết nối — không dùng
            1 -> i.readFully(ByteArray(4))
            3 -> i.readFully(ByteArray(i.readUnsignedByte()))
            4 -> i.readFully(ByteArray(16))
        }
        i.readUnsignedShort()
        // Mã 2 khi có đăng nhập: nhiều proxy (vd 3proxy) nhận mọi mật khẩu ở bước đăng nhập rồi mới từ chối ở đây —
        // thường là sai tên đăng nhập / mật khẩu (đo trên proxy nhà của người dùng)
        if (rep == 2 && login) {
            throw SocksError(rep, "proxy không cho mở $host:$port — thường do sai tên đăng nhập / mật khẩu (kiểm tra chữ hoa / chữ thường), hoặc proxy chặn trang này")
        }
        if (rep != 0) throw SocksError(rep, "proxy không mở được $host:$port (${REPLIES[rep] ?: "mã $rep"})")
    }

    private fun address(c: SocksConfig, host: String, ip: InetAddress?, resolve: (String) -> InetAddress): ByteArray {
        val a = ip ?: if (c.remoteDns) null else resolve(host)
        return when {
            a is Inet4Address -> byteArrayOf(1) + a.address
            a != null -> byteArrayOf(4) + a.address
            else -> host.toByteArray(Charsets.US_ASCII).let { byteArrayOf(3, it.size.toByte()) + it }
        }
    }

    /** DNS của thiết bị; ưu tiên IPv4 (nhiều mạng / TV box không có IPv6). */
    fun resolve(host: String): InetAddress = try {
        InetAddress.getAllByName(host).let { all -> all.firstOrNull { it is Inet4Address } ?: all.first() }
    } catch (e: UnknownHostException) {
        throw SocksError(4, "thiết bị không phân giải được $host")
    }

    /**
     * Nút "Kiểm tra proxy": mở HTTPS tới Cloudflare qua proxy (không qua cổng nội bộ, nên thử được trước khi lưu),
     * trả về IP ra ngoài kèm nơi đặt máy, vd "103.1.2.3 (SG)".
     */
    fun check(c: SocksConfig): String {
        val host = "www.cloudflare.com"
        val raw = connect(c.copy(sitesOnly = false), host, null, 443)
        val ssl = (SSLSocketFactory.getDefault() as SSLSocketFactory).createSocket(raw, host, 443, true) as SSLSocket
        ssl.use { s ->
            s.soTimeout = 15_000
            s.startHandshake()
            if (!HttpsURLConnection.getDefaultHostnameVerifier().verify(host, s.session)) throw SSLException("chứng chỉ không khớp $host")
            s.outputStream.write("GET /cdn-cgi/trace HTTP/1.1\r\nHost: $host\r\nUser-Agent: ${Http.UA}\r\nConnection: close\r\n\r\n".toByteArray())
            val text = s.inputStream.readBytes().toString(Charsets.UTF_8)
            val ip = Regex("""(?m)^ip=(\S+)""").find(text)?.groupValues?.get(1) ?: throw IOException("không đọc được kết quả từ $host")
            val loc = Regex("""(?m)^loc=(\S+)""").find(text)?.groupValues?.get(1)
            return ip + (loc?.let { " ($it)" } ?: "")
        }
    }
}

/**
 * Cổng SOCKS5 nội bộ (chỉ nghe 127.0.0.1, không mật khẩu) đứng giữa app và proxy thật — xem AppProxy.
 * config() = proxy của cổng này (đổi cài đặt thì AppProxy mở cổng mới); siteHosts() = tên miền các trang phim, đọc
 * lại mỗi kết nối (chế độ "chỉ trang phim": đích khác đi thẳng). Proxy lỗi thì trả lỗi cho bên gọi, không tự đi thẳng.
 */
class Socks5Bridge(private val config: () -> RouteConfig?, private val siteHosts: () -> Collection<String>) {
    private val server = ServerSocket(0, 64, InetAddress.getByName("127.0.0.1"))
    private val pool = Executors.newCachedThreadPool { r -> Thread(r, "chimto-socks").apply { isDaemon = true } }
    private val open: MutableSet<Socket> = Collections.newSetFromMap(java.util.concurrent.ConcurrentHashMap())

    val port: Int get() = server.localPort

    /** Lỗi proxy gần nhất và thời điểm (ghép vào thông báo lỗi cho người xem). */
    @Volatile var lastError: String? = null
        private set
    @Volatile var lastErrorAt = 0L
        private set

    init {
        pool.execute {
            while (!server.isClosed) {
                val s = try {
                    server.accept()
                } catch (e: IOException) {
                    break
                }
                // Lỗi trong luồng nền của pool làm sập app → mọi lỗi đều phải bắt (vd pool đã đóng lúc tắt proxy)
                try {
                    pool.execute { handle(s) }
                } catch (e: Exception) {
                    close(s)
                }
            }
        }
    }

    /** Tắt proxy: đóng cổng và mọi kết nối đang mở (WebView / Http mở lại kết nối mới theo cài đặt mới). */
    fun close() {
        runCatching { server.close() }
        open.toList().forEach { runCatching { it.close() } }
        pool.shutdown()
    }

    private fun handle(client: Socket) {
        open += client
        var up: Socket? = null
        try {
            client.tcpNoDelay = true
            client.soTimeout = 15_000
            val i = DataInputStream(client.getInputStream())
            val o = client.getOutputStream()
            // Lời chào: cổng nội bộ không đòi mật khẩu (WebView không đăng nhập được SOCKS5)
            if (i.readUnsignedByte() != 5) return
            val methods = ByteArray(i.readUnsignedByte()).also { i.readFully(it) }
            if (methods.none { it.toInt() == 0 }) {
                o.write(byteArrayOf(5, 0xFF.toByte()))
                return
            }
            o.write(byteArrayOf(5, 0))
            // Yêu cầu: VER CMD RSV ATYP ĐỊA-CHỈ CỔNG — chỉ hỗ trợ CONNECT (TCP)
            if (i.readUnsignedByte() != 5) return
            val cmd = i.readUnsignedByte()
            i.readUnsignedByte()
            val (host, ip) = when (i.readUnsignedByte()) {
                1 -> InetAddress.getByAddress(ByteArray(4).also { i.readFully(it) }).let { it.hostAddress!! to it }
                3 -> String(ByteArray(i.readUnsignedByte()).also { i.readFully(it) }, Charsets.US_ASCII) to null
                4 -> InetAddress.getByAddress(ByteArray(16).also { i.readFully(it) }).let { it.hostAddress!! to it }
                else -> return reply(o, 8)
            }
            val port = i.readUnsignedShort()
            if (cmd != 1) return reply(o, 7)
            val c = config() ?: return reply(o, 1)
            up = try {
                route(c, host, ip, port)
            } catch (e: SocksError) {
                if (e.viaProxy) noteError(e.message)
                return reply(o, e.code)
            }
            open += up
            reply(o, 0)
            client.soTimeout = 0
            val viaProxy = c.socks != null && (!c.socks.sitesOnly || isSite(host))
            relay(client, up, if (c.split) Hello(host, viaProxy, c.disorder) else null)
        } catch (e: Exception) {
            // bên gọi đóng giữa chừng, hoặc proxy vừa bị tắt — bỏ qua (không để lỗi lọt ra làm sập app)
        } finally {
            close(client)
            up?.let { close(it) }
        }
    }

    private fun route(c: RouteConfig, host: String, ip: InetAddress?, port: Int): Socket {
        val doh = c.doh
        val socks = c.socks
        if (socks != null && (!socks.sitesOnly || isSite(host))) {
            return Socks5.connect(socks, host, ip, port) { h -> doh?.let { Doh.resolve(it, h) } ?: Socks5.resolve(h) }
        }
        // Đi thẳng: DNS mã hoá lỗi thì báo (ghi vào lỗi gần nhất); DNS thiết bị lỗi như kết nối thẳng thường
        val addr = ip ?: doh?.let { Doh.resolve(it, host) } ?: try {
            Socks5.resolve(host)
        } catch (e: SocksError) {
            throw SocksError(e.code, e.message ?: "", viaProxy = false)
        }
        val s = Socket()
        try {
            s.connect(InetSocketAddress(addr, port), 10_000)
        } catch (e: IOException) {
            s.close()
            throw if (e is SocksError) SocksError(e.code, e.message ?: "", viaProxy = false)
            else SocksError(5, "không kết nối thẳng được $host:$port", viaProxy = false)
        }
        s.tcpNoDelay = true
        s.keepAlive = true
        return s
    }

    private fun isSite(host: String): Boolean {
        val h = host.lowercase().trimEnd('.')
        return siteHosts().any { h == it || h.endsWith(".$it") }
    }

    private fun noteError(message: String?) {
        lastError = message
        lastErrorAt = System.currentTimeMillis()
    }

    private fun reply(o: OutputStream, code: Int) = o.write(byteArrayOf(5, code.toByte(), 0, 1, 0, 0, 0, 0, 0, 0))

    /**
     * Chép hai chiều tới khi hết; một chiều lỗi thì đóng cả hai cho chiều kia khỏi treo. hello != null (tên trang, đi qua
     * proxy): gói đầu tiên từ bên gọi (a → b) là gói bắt tay HTTPS thì chia nhỏ (vượt chặn nhà mạng).
     */
    private fun relay(a: Socket, b: Socket, hello: Hello?) {
        val back = pool.submit { pipe(b, a, null) }
        pipe(a, b, hello)
        runCatching { back.get() }
    }

    private fun pipe(from: Socket, to: Socket, hello: Hello?) {
        val buf = ByteArray(16 * 1024)
        try {
            val i = from.getInputStream()
            val o = to.getOutputStream()
            var first = hello
            while (true) {
                val n = i.read(buf)
                if (n < 0) break
                val h = first
                if (h != null) {
                    first = null
                    writeHello(to, o, buf, n, h)
                    continue
                }
                o.write(buf, 0, n)
            }
            // Hết dữ liệu: báo bên kia (đóng nửa ghi); chiều ngược lại chờ tối đa 60 s im lặng rồi thôi
            to.shutdownOutput()
            to.soTimeout = 60_000
        } catch (e: Exception) {
            if (hello != null) debugLog?.invoke("chiều gửi lỗi: $e")
            close(from)
            close(to)
        }
    }

    /**
     * Vượt chặn theo tên trang (SNI): nhà mạng đọc tên trang trong gói bắt tay HTTPS đầu tiên (TLS ClientHello) để chặn
     * (vd phimmoir.fun — kết nối được nhưng bắt tay treo 15 giây rồi đứt). Đo trên mạng nhà: gửi nguyên hoặc cắt 2–3 mẩu
     * lớn đều bị chặn; gửi riêng byte đầu là lọt. Ở đây: byte đầu riêng, tên trang thành từng mẩu 2 byte, rồi phần còn
     * lại — mỗi lần ghi là một gói (tcpNoDelay). Qua proxy: chờ chút sau byte đầu để proxy cũng gửi nó thành gói riêng.
     * Không phải gói bắt tay TLS thì gửi nguyên.
     *
     * Mức 2 (disorder, đi thẳng): byte đầu gửi với TTL 1 — gói chết ở router đầu tiên (trong nhà), không tới nhà mạng; phần
     * còn lại đi bình thường, trang nhận rồi giữ chờ phần thiếu; hệ điều hành thấy byte đầu chưa được xác nhận thì tự gửi
     * lại với TTL đã trả về bình thường → byte đầu tới sau. Máy soi của nhà mạng thấy gói bắt tay thiếu đầu, sai thứ tự.
     * Đi qua proxy thì không dùng (proxy nhận đủ theo thứ tự rồi mới gửi tiếp — cách mức 1 có tác dụng hơn).
     */
    private fun writeHello(to: Socket, o: OutputStream, buf: ByteArray, n: Int, h: Hello) {
        if (n <= 5 || buf[0] != 0x16.toByte() || buf[1] != 0x03.toByte()) return o.write(buf, 0, n)
        val viaProxy = h.viaProxy
        val host = h.host
        if (h.disorder && !viaProxy && TcpTtl.set(to, 1)) {
            try {
                o.write(buf, 0, 1) // tcpNoDelay: hệ điều hành gửi ngay trong lúc ghi, với TTL 1
            } finally {
                TcpTtl.set(to, -1) // gói gửi lại (và mọi gói sau) đi với TTL mặc định
            }
            disorderCount++
        } else {
            o.write(buf, 0, 1)
        }
        if (viaProxy) Thread.sleep(40)
        var i = 1
        val name = host.lowercase().toByteArray(Charsets.US_ASCII)
        val s = if (name.size >= 2) find(name, buf, 1, n) else null
        if (s != null) {
            if (s > i) o.write(buf, i, s - i)
            var k = s
            while (k < s + name.size) {
                val len = minOf(2, s + name.size - k)
                o.write(buf, k, len)
                k += len
            }
            i = s + name.size
        }
        if (i < n) o.write(buf, i, n - i)
    }

    /** Nhật ký gỡ lỗi (bản debug gắn Log.d). */
    var debugLog: ((String) -> Unit)? = null

    /** Số gói bắt tay đã gửi đảo thứ tự (mức 2) — để thử / gỡ lỗi. */
    @Volatile var disorderCount = 0
        private set

    private fun close(s: Socket) {
        open -= s
        runCatching { s.close() }
    }
}
