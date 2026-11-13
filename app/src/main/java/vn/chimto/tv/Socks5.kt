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
 * Cài đặt của cổng nội bộ: proxy SOCKS5 hoặc máy chủ Shadowsocks (cả hai null = đi thẳng); dns = DNS mã hoá (Doh: DoH
 * rồi DoT, lỗi hết thì DNS của thiết bị) cho các tên miền thiết bị tự phân giải, false = DNS của thiết bị / nhà mạng.
 * split: Vượt chặn nhà mạng (mức 1: chia nhỏ gói bắt tay); disorder: mức 2 — thêm đảo thứ tự (byte đầu gửi với TTL 1).
 */
data class RouteConfig(
    val socks: SocksConfig?, val dns: Boolean, val split: Boolean = false, val disorder: Boolean = false, val ss: SsConfig? = null,
) {
    /** Đang dùng proxy (SOCKS5 hoặc Shadowsocks). */
    val proxied: Boolean get() = socks != null || ss != null

    /** Proxy chỉ cho trang phim (null = không có proxy). */
    val sitesOnly: Boolean? get() = socks?.sitesOnly ?: ss?.sitesOnly
}

/** Gói bắt tay sắp gửi: tên trang, có đi qua proxy không, có đảo thứ tự không (Socks5Bridge.writeHello). */
internal data class Hello(val host: String, val viaProxy: Boolean, val disorder: Boolean)

/** Lỗi khi mở kết nối; code = mã trả lời SOCKS5 gửi lại bên gọi; viaProxy = lỗi ở phía proxy (để báo người xem). */
class SocksError(val code: Int, message: String, val viaProxy: Boolean = true) : IOException(message)

/**
 * Tên miền gốc, để nhớ theo trang chứ không theo từng tên con: img.phimmoi.net → phimmoi.net, www.abc.com.vn → abc.com.vn.
 * Địa chỉ IP giữ nguyên.
 */
internal fun baseDomain(host: String): String {
    val h = host.lowercase().trimEnd('.')
    if (h.contains(':') || h.all { it.isDigit() || it == '.' }) return h
    val labels = h.split('.')
    if (labels.size <= 2) return h
    // Tên miền quốc gia hai cấp (com.vn, co.uk…): giữ ba nhãn cuối
    val three = labels.last().length == 2 && labels[labels.size - 2] in SECOND_LEVEL
    return labels.takeLast(if (three) 3 else 2).joinToString(".")
}

private val SECOND_LEVEL = setOf("com", "net", "org", "edu", "gov", "co", "ac", "info", "biz", "name", "pro", "int", "health")

/**
 * Vượt chặn tự lên Mức 2 (11/2026): đang Mức 1 mà trang không trả lời gói bắt tay (nhà mạng đã ghép được các mẩu và chặn)
 * thì cổng nội bộ tự thử lại theo Mức 2 (Socks5Bridge.opening); lọt thì nhớ trang (tên miền gốc) dùng Mức 2 trong 7 ngày —
 * các kết nối sau đi Mức 2 ngay, khỏi chờ. Nhà mạng thôi chặn thì sau 7 ngày trang tự về Mức 1.
 */
object Level2Memo {
    const val KEEP_MS = 7L * 24 * 3600 * 1000
    private val hosts = java.util.concurrent.ConcurrentHashMap<String, Long>()

    /** Lưu xuống máy (AppProxy gắn vào Prefs); null = chỉ giữ trong bộ nhớ (test). */
    @Volatile var save: ((Map<String, Long>) -> Unit)? = null

    fun load(saved: Map<String, Long>, now: Long = System.currentTimeMillis()) {
        hosts.clear()
        saved.forEach { (h, t) -> if (now - t in 0 until KEEP_MS) hosts[h] = t }
    }

    fun needs(host: String, now: Long = System.currentTimeMillis()): Boolean =
        hosts[baseDomain(host)]?.let { now - it in 0 until KEEP_MS } == true

    fun remember(host: String, now: Long = System.currentTimeMillis()) {
        hosts[baseDomain(host)] = now
        save?.invoke(HashMap(hosts))
    }

    fun all(): Map<String, Long> = HashMap(hosts)

    fun clear() {
        hosts.clear()
        save?.invoke(emptyMap())
    }
}

/** Gói đầu của bên gọi là gói bắt tay TLS (ClientHello). */
internal fun isTlsHello(b: ByteArray, n: Int) = n > 5 && b[0] == 0x16.toByte() && b[1] == 0x03.toByte()

private val HTTP_METHODS = listOf("GET ", "POST ", "HEAD ", "PUT ", "DELETE ", "OPTIONS ", "PATCH ").map { it.toByteArray(Charsets.US_ASCII) }

/** Gói đầu là một yêu cầu HTTP thường (không mã hoá). */
internal fun isHttpRequest(b: ByteArray, n: Int) =
    HTTP_METHODS.any { m -> n > m.size && m.indices.all { b[it] == m[it] } }

/**
 * Cách cắt gói đầu của bên gọi để nhà mạng không đọc được tên trang: danh sách đoạn (đầu, dài), mỗi đoạn gửi thành một gói
 * TCP (tcpNoDelay). Gói bắt tay TLS: byte đầu riêng, phần trước tên trang, tên trang thành từng mẩu 2 byte, phần còn lại.
 * Yêu cầu HTTP thường (từ 11/2026 — trước chỉ cắt TLS): byte đầu riêng, tới giữa chữ «Host», phần còn lại của «Host: »,
 * giá trị Host thành từng mẩu 2 byte, phần còn lại. Gói khác: một đoạn (gửi nguyên).
 */
internal fun splitPlan(b: ByteArray, n: Int, host: String): List<Pair<Int, Int>> {
    val out = ArrayList<Pair<Int, Int>>()
    fun add(start: Int, end: Int) { if (end > start) out += start to end - start }
    fun chunks(start: Int, end: Int) { var k = start; while (k < end) { add(k, minOf(k + 2, end)); k += 2 } }
    when {
        isTlsHello(b, n) -> {
            add(0, 1)
            val name = host.lowercase().toByteArray(Charsets.US_ASCII)
            val s = if (name.size >= 2) find(name, b, 1, n) else null
            if (s == null) {
                add(1, n)
            } else {
                add(1, s)
                chunks(s, s + name.size)
                add(s + name.size, n)
            }
        }
        isHttpRequest(b, n) -> {
            add(0, 1)
            val h = find("\r\nhost:".toByteArray(Charsets.US_ASCII), b, 0, n)
            if (h == null) {
                add(1, n)
            } else {
                val k = h + 2 // chữ «Host»
                var v = k + 5
                while (v < n && (b[v] == ' '.code.toByte() || b[v] == '\t'.code.toByte())) v++
                var e = v
                while (e < n && b[e] != '\r'.code.toByte() && b[e] != '\n'.code.toByte()) e++
                add(1, k + 2)
                add(k + 2, v)
                chunks(v, e)
                add(e, n)
            }
        }
        else -> add(0, n)
    }
    return out
}

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

    /**
     * DNS chặn trang (của nhà mạng, hay bộ lọc quảng cáo trong mạng nhà) thường trả 0.0.0.0 / :: (có nơi 127.x / ::1):
     * địa chỉ trỏ về chính máy chứ không phải trang thật — kết nối bị từ chối ngay.
     */
    fun blockedAnswer(a: InetAddress) = a.isAnyLocalAddress || a.isLoopbackAddress

    /**
     * Xét mọi địa chỉ trong câu trả lời, không chỉ địa chỉ app chọn: có một địa chỉ chặn là cả câu trả lời không dùng được.
     * Đo DNS Viettel 13/11/2026: tên miền bị chặn chỉ trả 127.0.0.1.
     */
    fun blockedAnswer(all: List<InetAddress>) = all.any(::blockedAnswer)

    /** Địa chỉ DNS thiết bị đã trả → tên miền gốc đầu tiên nhận địa chỉ đó (tối đa 512 địa chỉ, cũ nhất bỏ trước). */
    private val answeredFor = object : LinkedHashMap<String, String>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, String>?) = size > 512
    }

    /**
     * Trang cảnh báo của nhà mạng: DNS chặn trả cùng một địa chỉ (máy chủ trang cảnh báo) cho mọi trang bị chặn. Câu trả lời
     * có địa chỉ đã trả cho một tên miền gốc khác thì đáng ngờ → Vượt chặn hỏi lại DNS mã hoá cho chắc. Hai trang dùng chung
     * CDN cũng trùng địa chỉ — khi đó DNS mã hoá trả địa chỉ đúng như thường, không hại gì.
     */
    @Synchronized
    fun sharedAnswer(host: String, all: List<InetAddress>): Boolean {
        val base = baseDomain(host)
        var shared = false
        for (a in all) {
            val k = a.hostAddress ?: continue
            val prev = answeredFor[k]
            when {
                k in suspects -> shared = true
                prev == null -> answeredFor[k] = base
                prev != base -> {
                    suspects += k // từ giờ trang nào nhận địa chỉ này (kể cả trang đầu tiên) cũng hỏi lại DNS mã hoá
                    shared = true
                }
            }
        }
        return shared
    }

    /** Địa chỉ đã thấy trả cho hai tên miền gốc khác nhau. */
    private val suspects = HashSet<String>()

    @Synchronized
    internal fun forgetAnswers() {
        answeredFor.clear()
        suspects.clear()
    }

    /**
     * Chỉ bản debug (cổng 8790 /dnsblock): coi DNS thiết bị trả debugDnsBlockIp (mặc định 0.0.0.0) cho các tên miền này,
     * để thử Vượt chặn. Đặt địa chỉ công khai (vd 203.0.113.9) để giả trang cảnh báo của nhà mạng.
     */
    @Volatile var debugDnsBlock: Set<String> = emptySet()
    @Volatile var debugDnsBlockIp: InetAddress = InetAddress.getByAddress(ByteArray(4))

    /**
     * Chỉ bản debug (cổng 8790 /unblockhang): ở Mức 1, gửi nguyên gói bắt tay tới các trang này — như nhà mạng ghép được
     * các mẩu — để thử tự lên Mức 2 với chính máy soi của nhà mạng (trang đang bị chặn theo tên).
     */
    @Volatile var debugWholeHello: Set<String> = emptySet()

    /** DNS của thiết bị: mọi địa chỉ. */
    fun resolveAll(host: String): List<InetAddress> = try {
        InetAddress.getAllByName(host).toList().ifEmpty { throw UnknownHostException(host) }
    } catch (e: UnknownHostException) {
        throw SocksError(4, "thiết bị không phân giải được $host")
    }

    /** Ưu tiên IPv4 (nhiều mạng / TV box không có IPv6). */
    fun pick(all: List<InetAddress>): InetAddress = all.firstOrNull { it is Inet4Address } ?: all.first()

    /** DNS của thiết bị. */
    fun resolve(host: String): InetAddress = pick(resolveAll(host))

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
            val u = try {
                route(c, host, ip, port)
            } catch (e: SocksError) {
                if (e.viaProxy) noteError(e.message)
                return reply(o, e.code)
            }
            up = u.socket
            open += up
            reply(o, 0)
            client.soTimeout = 0
            val viaProxy = c.proxied && (c.sitesOnly == false || isSite(host))
            // Qua Shadowsocks: nhà mạng chỉ thấy dữ liệu đã mã hoá, không đọc được tên trang — khỏi chia nhỏ
            val hello = if (c.split && (c.ss == null || !viaProxy)) Hello(host, viaProxy, c.disorder) else null
            if (hello == null) return relay(client, u, null)
            val (ready, pending) = opening(client, u, hello, c, host, ip, port) ?: return
            if (ready !== u) {
                up = ready.socket
                open += up
            }
            relay(client, ready, pending)
        } catch (e: Exception) {
            // bên gọi đóng giữa chừng, hoặc proxy vừa bị tắt — bỏ qua (không để lỗi lọt ra làm sập app)
        } finally {
            close(client)
            up?.let { close(it) }
        }
    }

    private fun route(c: RouteConfig, host: String, ip: InetAddress?, port: Int): Upstream {
        val socks = c.socks
        if (socks != null && (!socks.sitesOnly || isSite(host))) {
            return Upstream(Socks5.connect(socks, host, ip, port) { h -> dns(c, h) })
        }
        val ss = c.ss
        if (ss != null && (!ss.sitesOnly || isSite(host))) {
            return Shadowsocks.connect(ss, host, ip, port) { h -> dns(c, h) }
        }
        // Đi thẳng: lỗi DNS như kết nối thẳng thường (không phải lỗi proxy)
        val addr = ip ?: try {
            dns(c, host)
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
        return Upstream(s)
    }

    /**
     * Địa chỉ cho tên miền thiết bị tự phân giải. DNS mã hoá bật (mặc định): hỏi Doh, lỗi hết thì DNS của thiết bị.
     * DNS mã hoá tắt mà đang Vượt chặn: DNS của thiết bị không trả lời hoặc trả địa chỉ chặn (0.0.0.0…) thì vẫn hỏi lại qua
     * DNS mã hoá — vd ảnh bìa CLB Phim Xưa (clbphimxua.com) bị bộ lọc DNS trong nhà chặn; chia nhỏ gói bắt tay thì không đủ.
     */
    private fun dns(c: RouteConfig, host: String): InetAddress {
        val name = host.lowercase().trimEnd('.')
        if (name == "localhost") return Socks5.resolve(host)
        if (c.dns) Doh.resolve(name)?.let { return it }
        else if (!c.split) return Socks5.resolve(host)
        val all = try {
            if (BuildConfig.DEBUG && name in Socks5.debugDnsBlock) listOf(Socks5.debugDnsBlockIp) else Socks5.resolveAll(host)
        } catch (e: SocksError) {
            if (!c.dns) Doh.resolve(name)?.let { return it }
            throw e
        }
        val device = Socks5.pick(all)
        if (c.dns) return device
        // Địa chỉ chặn (0.0.0.0, 127.x…) hoặc đáng ngờ (đã trả cho trang khác — kiểu trang cảnh báo) → hỏi lại DNS mã hoá
        if (!Socks5.blockedAnswer(all) && !Socks5.sharedAnswer(name, all)) return device
        return Doh.resolve(name) ?: device
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
    private fun relay(a: Socket, b: Upstream, hello: Hello?) {
        val back = pool.submit { pipe(b.socket, b.input, a, a.getOutputStream(), null) }
        pipe(a, a.getInputStream(), b.socket, b.output, hello)
        runCatching { back.get() }
    }

    private fun pipe(from: Socket, i: java.io.InputStream, to: Socket, o: OutputStream, hello: Hello?) {
        val buf = ByteArray(16 * 1024)
        try {
            var first = hello
            if (o is SsOutput) {
                // Shadowsocks: phần đầu gửi cùng dữ liệu đầu tiên của bên gọi (HTTPS: gói bắt tay). Bên gọi im quá 0,3 s
                // (giao thức máy chủ nói trước) thì gửi phần đầu trước để máy chủ mở kết nối tới đích
                from.soTimeout = 300
                val n = try {
                    i.read(buf)
                } catch (e: java.net.SocketTimeoutException) {
                    0
                }
                from.soTimeout = 0
                if (n > 0) o.write(buf, 0, n) else if (n == 0) o.start()
                if (n < 0) {
                    to.shutdownOutput()
                    to.soTimeout = 60_000
                    return
                }
            }
            while (true) {
                val n = i.read(buf)
                if (n < 0) break
                val h = first
                if (h != null) {
                    first = null
                    writeSplit(to, o, buf, h, splitPlan(buf, n, h.host))
                    continue
                }
                o.write(buf, 0, n)
            }
            // Hết dữ liệu: báo bên kia (đóng nửa ghi); chiều ngược lại chờ tối đa 60 s im lặng rồi thôi
            to.shutdownOutput()
            to.soTimeout = 60_000
        } catch (e: Exception) {
            if (hello != null) debugLog?.invoke("chiều gửi lỗi: $e")
            if (e is SsError) noteError(e.message)
            close(from)
            close(to)
        }
    }

    /**
     * Gói đầu của bên gọi (đi kèm Vượt chặn): đọc trọn (gói bắt tay TLS có thể dài hơn một lần đọc), cắt nhỏ (splitPlan) rồi
     * gửi. Đi thẳng, gói bắt tay TLS, đang Mức 1: gửi xong mà trang im quá firstReplyMs hoặc kết nối bị cắt trước khi trả
     * byte nào (nhà mạng đã ghép được các mẩu và chặn) → mở kết nối mới, gửi lại theo Mức 2; lọt thì nhớ trang (Level2Memo).
     * Bên gọi chưa nhận byte nào của trang nên gửi lại không sai. Trả về (kết nối dùng tiếp, gói còn chờ cắt — khi bên gọi
     * chưa gửi gì vì giao thức để máy chủ nói trước); null = không mở được.
     */
    private fun opening(client: Socket, u: Upstream, h: Hello, c: RouteConfig, host: String, ip: InetAddress?, port: Int): Pair<Upstream, Hello?>? {
        val buf = ByteArray(17 * 1024)
        val n = readFirst(client, buf)
        if (n < 0) return null
        if (n == 0) return u to h
        val plan = splitPlan(buf, n, host)
        if (h.viaProxy || !isTlsHello(buf, n)) {
            writeSplit(u.socket, u.output, buf, h, plan)
            return u to null
        }
        val level2 = h.disorder || Level2Memo.needs(host)
        if (BuildConfig.DEBUG && !level2 && host.lowercase().trimEnd('.') in Socks5.debugWholeHello) {
            u.output.write(buf, 0, n) // giả nhà mạng ghép được các mẩu: gửi nguyên gói
        } else {
            writeSplit(u.socket, u.output, buf, h.copy(disorder = level2), plan)
        }
        if (firstReply(u, client, if (level2) LEVEL2_REPLY_MS else firstReplyMs)) return u to null
        if (level2) return null
        // Mức 1 không lọt: kết nối mới, gửi lại theo Mức 2
        close(u.socket)
        val u2 = try {
            route(c, host, ip, port)
        } catch (e: SocksError) {
            return null
        }
        open += u2.socket
        val applied = writeSplit(u2.socket, u2.output, buf, h.copy(disorder = true), plan)
        val ok = firstReply(u2, client, LEVEL2_REPLY_MS)
        debugLog?.invoke("vượt chặn: $host không trả lời ở mức 1 → mức 2: " + (if (ok) "lọt" else "vẫn không được") +
            if (applied) "" else " (không đặt được TTL)")
        if (!ok) {
            close(u2.socket)
            return null
        }
        if (applied) {
            Level2Memo.remember(host)
            autoLevel2++
        }
        return u2 to null
    }

    /** Đọc gói đầu của bên gọi: im quá 0,3 s thì 0 (máy chủ nói trước); gói bắt tay TLS thì đọc trọn bản ghi đầu. */
    private fun readFirst(client: Socket, buf: ByteArray): Int {
        val i = client.getInputStream()
        try {
            client.soTimeout = 300
            var n = try {
                i.read(buf)
            } catch (e: java.net.SocketTimeoutException) {
                return 0
            }
            if (n >= 5 && buf[0] == 0x16.toByte() && buf[1] == 0x03.toByte()) {
                val need = minOf(buf.size, 5 + ((buf[3].toInt() and 0xFF) shl 8 or (buf[4].toInt() and 0xFF)))
                client.soTimeout = 2_000
                while (n < need) {
                    val m = try {
                        i.read(buf, n, need - n)
                    } catch (e: java.net.SocketTimeoutException) {
                        break
                    }
                    if (m < 0) break
                    n += m
                }
            }
            return n
        } finally {
            client.soTimeout = 0
        }
    }

    /** Chờ trang trả lời gói bắt tay (tối đa ms); có thì chuyển ngay cho bên gọi. false = trang im / cắt kết nối. */
    private fun firstReply(u: Upstream, client: Socket, ms: Int): Boolean {
        val buf = ByteArray(16 * 1024)
        val m = try {
            u.socket.soTimeout = ms
            u.input.read(buf)
        } catch (e: IOException) {
            -1
        } finally {
            runCatching { u.socket.soTimeout = 0 }
        }
        if (m <= 0) return false
        client.getOutputStream().write(buf, 0, m) // lỗi ở đây: bên gọi đã đóng → handle đóng hết
        return true
    }

    /**
     * Vượt chặn theo tên trang (SNI): nhà mạng đọc tên trang trong gói bắt tay HTTPS đầu tiên (TLS ClientHello) để chặn
     * (vd phimmoir.fun — kết nối được nhưng bắt tay treo 15 giây rồi đứt). Đo trên mạng nhà: gửi nguyên hoặc cắt 2–3 mẩu
     * lớn đều bị chặn; gửi riêng byte đầu là lọt. Gửi từng đoạn của splitPlan — mỗi lần ghi là một gói (tcpNoDelay). Qua
     * proxy: chờ chút sau byte đầu để proxy cũng gửi nó thành gói riêng.
     *
     * Mức 2 (disorder, đi thẳng): đoạn đầu (1 byte) gửi với TTL 1 — gói chết ở router đầu tiên (trong nhà), không tới nhà
     * mạng; phần còn lại đi bình thường, trang nhận rồi giữ chờ phần thiếu; hệ điều hành thấy byte đầu chưa được xác nhận thì
     * tự gửi lại với TTL đã trả về bình thường → byte đầu tới sau. Máy soi của nhà mạng thấy gói thiếu đầu, sai thứ tự. Đi
     * qua proxy thì không dùng (proxy nhận đủ theo thứ tự rồi mới gửi tiếp — cách mức 1 có tác dụng hơn). Gói không cắt
     * được (một đoạn) thì gửi nguyên. Trả về true nếu đã gửi đảo thứ tự.
     */
    private fun writeSplit(to: Socket, o: OutputStream, buf: ByteArray, h: Hello, plan: List<Pair<Int, Int>>): Boolean {
        var applied = false
        plan.forEachIndexed { idx, (start, len) ->
            if (idx == 0 && plan.size > 1 && h.disorder && !h.viaProxy && setTtl(to, 1)) {
                try {
                    o.write(buf, start, len) // tcpNoDelay: hệ điều hành gửi ngay trong lúc ghi, với TTL 1
                } finally {
                    setTtl(to, -1) // gói gửi lại (và mọi gói sau) đi với TTL mặc định
                }
                applied = true
                disorderCount++
            } else {
                o.write(buf, start, len)
            }
            if (idx == 0 && plan.size > 1 && h.viaProxy) Thread.sleep(40)
        }
        return applied
    }

    /** Nhật ký gỡ lỗi (bản debug gắn Log.d). */
    var debugLog: ((String) -> Unit)? = null

    /** Chờ trang trả lời gói bắt tay ở Mức 1 bao lâu thì coi là bị chặn và thử Mức 2 (test đặt ngắn). */
    internal var firstReplyMs = 4_000

    /** Đặt TTL cho kết nối (test thay — trên máy tính không đặt được). */
    internal var setTtl: (Socket, Int) -> Boolean = TcpTtl::set

    /** Số gói bắt tay đã gửi đảo thứ tự (mức 2) — để thử / gỡ lỗi. */
    @Volatile var disorderCount = 0
        private set

    /** Số lần tự lên Mức 2 vì trang không trả lời ở Mức 1 — để thử / gỡ lỗi. */
    @Volatile var autoLevel2 = 0
        private set

    private companion object {
        /** Ở Mức 2 (cách cuối cùng): chờ lâu hơn lúc nhà mạng giữ kết nối treo (đo: 15 giây). */
        const val LEVEL2_REPLY_MS = 20_000
    }

    private fun close(s: Socket) {
        open -= s
        runCatching { s.close() }
    }
}
