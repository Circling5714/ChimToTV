package vn.chimto.tv

import android.util.Log
import java.io.BufferedInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ConcurrentLinkedDeque
import java.util.concurrent.ExecutionException
import java.util.concurrent.FutureTask
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLException
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory

/**
 * DNS mã hoá (từ 2.11 bật sẵn): hỏi địa chỉ tên miền qua DoH (RFC 8484) rồi DoT (RFC 7858), mỗi giao thức lần lượt
 * NextDNS (hồ sơ riêng của app — mã trong keys/nextdns.txt, không có trên git) → Cloudflare → Google; lỗi hết thì bên gọi
 * dùng DNS của thiết bị — app không bao giờ hỏng vì DNS mã hoá bị chặn. Nhà mạng không đọc / sửa được truy vấn DNS (vẫn
 * thấy tên trang lúc bắt tay TLS và IP đích — Vượt chặn lo phần tên trang).
 *
 * Máy chủ gọi bằng IP (khỏi phải hỏi DNS để tìm máy chủ DNS), kiểm chứng chỉ theo tên máy chủ, đi thẳng (không qua cổng
 * nội bộ / proxy). Từ 2.11.1:
 * - NextDNS dùng máy chủ gần nhất: IP anycast từ mạng Viettel bị đưa sang Hồng Kông (đo 11/2026: hỏi lạnh ~210 ms, máy
 *   chủ Hà Nội ~150 ms) → lần đầu hỏi chính NextDNS tên «dns.nextdns.io» (NextDNS trả máy chủ gần IP của mạng này nhất),
 *   không được thì hỏi DNS của thiết bị; nhớ 6 giờ; anycast vẫn đứng sau để dự phòng.
 * - Giữ kết nối TLS tới máy chủ DNS để dùng lại (DoT; DoH bằng HTTP/1.1 keep-alive): mở kết nối mới tốn ~140–220 ms,
 *   dùng lại chỉ ~20–40 ms. Kết nối nghỉ quá 90 giây thì bỏ (NextDNS / Cloudflare DoH / Google DoH giữ ≥ 100 giây); kết nối cũ bị máy chủ đóng thì mở kết nối mới ngay.
 * Máy chủ lỗi thì nghỉ 2 phút. Trả lời «không có tên miền» (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi
 * máy chủ sau. Bản debug ghi mỗi lần hỏi (logcat ChimToDoh, kèm IP máy chủ và thời gian).
 */
object Doh {
    enum class Kind { DOH, DOT }

    /** steer = NextDNS: IP lấy theo máy chủ gần nhất (nextPops) rồi mới tới ips cố định (anycast). */
    class Server(val label: String, val kind: Kind, val host: String, val path: String, val ips: List<String>, val steer: Boolean = false) {
        val port get() = if (kind == Kind.DOT) 853 else 443
    }

    private val NEXT_IPS = listOf("45.90.28.0", "45.90.30.0")
    private val GOOGLE_IPS = listOf("8.8.8.8", "8.8.4.4")

    /** Thứ tự hỏi: DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy (không có mã NextDNS thì bỏ NextDNS). */
    fun chain(nextId: String): List<Server> {
        val next = nextId.trim()
        return listOfNotNull(
            next.takeIf { it.isNotEmpty() }?.let { Server("DoH NextDNS", Kind.DOH, "dns.nextdns.io", "/$it", NEXT_IPS, steer = true) },
            Server("DoH Cloudflare", Kind.DOH, "dns.cloudflare.com", "/dns-query", listOf("104.16.248.249", "104.16.249.249")),
            Server("DoH Google", Kind.DOH, "dns.google", "/dns-query", GOOGLE_IPS),
            next.takeIf { it.isNotEmpty() }?.let { Server("DoT NextDNS", Kind.DOT, "$it.dns.nextdns.io", "", NEXT_IPS, steer = true) },
            Server("DoT Cloudflare", Kind.DOT, "1dot1dot1dot1.cloudflare-dns.com", "", listOf("1.1.1.1", "1.0.0.1")),
            Server("DoT Google", Kind.DOT, "dns.google", "", GOOGLE_IPS),
        )
    }

    val servers: List<Server> by lazy { chain(BuildConfig.NEXTDNS_ID) }

    /** Tên các nhà cung cấp theo thứ tự, để hiện trong Cài đặt: "NextDNS → Cloudflare → Google". */
    val providers: String get() = servers.map { it.label.substringAfter(' ') }.distinct().joinToString(" → ")

    private const val CONNECT_MS = 2_500
    private const val READ_MS = 3_000
    private const val DOWN_MS = 120_000L
    private const val IDLE_MS = 90_000L
    private const val MAX_IDLE = 4
    private const val POPS_MS = 6 * 3_600_000L

    /** Địa chỉ đã hỏi; addr null = tên miền không có (NXDOMAIN). */
    private class Entry(val addr: InetAddress?, val until: Long)

    private val cache = ConcurrentHashMap<String, Entry>()
    private val downUntil = ConcurrentHashMap<String, Long>()
    private val nextIp = ConcurrentHashMap<String, Int>()
    private val random = SecureRandom()
    private val IPV4 = Regex("""^\d{1,3}(\.\d{1,3}){3}$""")

    /** Kết quả hỏi một máy chủ (máy chủ lỗi thì ném lỗi). */
    sealed class Answer {
        /** all = mọi bản ghi A (theo thứ tự máy chủ trả); addr = cái đầu. */
        class Found(val addr: InetAddress, val ttl: Int, val all: List<InetAddress> = listOf(addr)) : Answer()
        /** NXDOMAIN: tên miền không có (hoặc hồ sơ DNS chặn) — không hỏi máy chủ khác. */
        object NoDomain : Answer()
        /** Có tên miền nhưng không có bản ghi A (vd chỉ có IPv6) — để DNS của thiết bị thử. */
        object NoIpv4 : Answer()
        /** SERVFAIL / REFUSED… — hỏi máy chủ sau. */
        object Failed : Answer()
    }

    /**
     * Địa chỉ IPv4 của host qua chuỗi DNS mã hoá (nhớ theo TTL, 30 giây – 10 phút). null = mọi máy chủ đều lỗi / không có
     * bản ghi A → bên gọi dùng DNS của thiết bị. Tên miền không có → SocksError 4.
     */
    fun resolve(host: String, servers: List<Server> = this.servers): InetAddress? {
        val key = host.lowercase().trimEnd('.')
        if (IPV4.matches(key) || key == "localhost") return InetAddress.getByName(key)
        cache[key]?.takeIf { it.until > System.currentTimeMillis() }?.let { return it.addr ?: throw noDomain(key) }
        // Nhiều kết nối cùng lúc tới một tên miền (ảnh bìa, API…): chỉ hỏi một lần, các kết nối kia chờ kết quả đó
        val job = FutureTask { lookup(key, servers) }
        val running = inflight.putIfAbsent(key, job)
        if (running == null) {
            try {
                job.run()
            } finally {
                inflight.remove(key, job)
            }
        }
        return try {
            (running ?: job).get()
        } catch (e: ExecutionException) {
            throw e.cause ?: e
        }
    }

    private val inflight = ConcurrentHashMap<String, FutureTask<InetAddress?>>()

    private fun noDomain(name: String) = SocksError(4, "không tìm thấy tên miền $name", viaProxy = false)

    private fun lookup(key: String, servers: List<Server>): InetAddress? {
        for (s in servers) {
            if ((downUntil[s.label] ?: 0L) > System.currentTimeMillis()) continue
            val ips = ipsFor(s)
            val ip = ips[(nextIp[s.label] ?: 0) % ips.size]
            val t0 = System.currentTimeMillis()
            val answer = try {
                ask(s, ip, key)
            } catch (e: Exception) {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP kế của nó
                downUntil[s.label] = System.currentTimeMillis() + DOWN_MS
                nextIp[s.label] = (nextIp[s.label] ?: 0) + 1
                if (BuildConfig.DEBUG) Log.d("ChimToDoh", "${s.label} · $ip lỗi: ${e.javaClass.simpleName} ${e.message}")
                continue
            }
            when (answer) {
                is Answer.Found -> {
                    cache[key] = Entry(answer.addr, System.currentTimeMillis() + answer.ttl.coerceIn(30, 600) * 1000L)
                    if (BuildConfig.DEBUG) {
                        Log.d("ChimToDoh", "$key → ${answer.addr.hostAddress} (${s.label} · $ip, ${System.currentTimeMillis() - t0} ms)")
                    }
                    return answer.addr
                }
                Answer.NoDomain -> {
                    cache[key] = Entry(null, System.currentTimeMillis() + 30_000) // nhớ 30 giây: kết nối khác khỏi hỏi lại
                    throw noDomain(key)
                }
                Answer.NoIpv4 -> return null
                Answer.Failed -> continue
            }
        }
        return null
    }

    // ---- NextDNS: máy chủ gần nhất ----

    @Volatile private var pops: List<String> = emptyList()
    @Volatile private var popsUntil = 0L
    private val popsLock = Any()

    /** IP để gọi máy chủ s: NextDNS = máy chủ gần nhất rồi anycast; còn lại = IP cố định. */
    fun ipsFor(s: Server): List<String> = if (!s.steer) s.ips else (nextPops() + s.ips).distinct()

    private fun nextPops(): List<String> {
        if (System.currentTimeMillis() < popsUntil) return pops
        synchronized(popsLock) {
            if (System.currentTimeMillis() < popsUntil) return pops
            val found = discoverPops()
            pops = found
            popsUntil = System.currentTimeMillis() + if (found.isNotEmpty()) POPS_MS else 600_000L
            if (BuildConfig.DEBUG) Log.d("ChimToDoh", "NextDNS gần nhất: ${found.ifEmpty { listOf("(không tìm được — dùng anycast)") }}")
            return pops
        }
    }

    /** Hỏi chính NextDNS (anycast, đã mã hoá) tên «dns.nextdns.io» — NextDNS trả máy chủ gần IP của mạng này; không được thì DNS máy. */
    private fun discoverPops(): List<String> {
        val any = servers.firstOrNull { it.steer && it.kind == Kind.DOH }
            ?: servers.firstOrNull { it.steer } ?: return emptyList()
        for (ip in any.ips) {
            val got = try {
                ask(any, ip, "dns.nextdns.io")
            } catch (e: Exception) {
                null
            }
            if (got is Answer.Found) return got.all.filterNot { Socks5.blockedAnswer(it) }.mapNotNull { it.hostAddress }.distinct()
        }
        return try {
            InetAddress.getAllByName("dns.nextdns.io").filterIsInstance<Inet4Address>().filterNot { Socks5.blockedAnswer(it) }
                .mapNotNull { it.hostAddress }.distinct()
        } catch (e: Exception) {
            emptyList()
        }
    }

    /** Chỉ bản debug (cổng 8790 /dohfail): coi các máy chủ này như bị chặn, để thử chuỗi dự phòng. */
    @Volatile var debugFail: Set<String> = emptySet()

    /** Hỏi máy chủ s ở địa chỉ ip. */
    fun ask(s: Server, ip: String, name: String): Answer {
        if (BuildConfig.DEBUG && s.label in debugFail) throw IOException("giả bị chặn")
        val id = if (s.kind == Kind.DOT) random.nextInt(0x10000) else 0 // DoH khuyên id 0 (dễ lưu đệm)
        val reply = exchange(s, ip, DnsWire.query(name, id)) ?: return Answer.Failed
        return DnsWire.answer(reply, id)
    }

    // ---- Kết nối dùng lại ----

    private class Conn(val key: String, val sock: SSLSocket) {
        val input = BufferedInputStream(sock.inputStream)
        val output: OutputStream = sock.outputStream
        @Volatile var lastUsed = System.currentTimeMillis()
        fun close() {
            runCatching { sock.close() }
        }
    }

    private val idle = ConcurrentHashMap<String, ConcurrentLinkedDeque<Conn>>()

    private fun take(key: String): Conn? {
        val q = idle[key] ?: return null
        while (true) {
            val c = q.pollFirst() ?: return null
            if (System.currentTimeMillis() - c.lastUsed < IDLE_MS && !c.sock.isClosed) return c
            c.close()
        }
    }

    private fun give(c: Conn) {
        c.lastUsed = System.currentTimeMillis()
        val q = idle.getOrPut(c.key) { ConcurrentLinkedDeque() }
        if (q.size >= MAX_IDLE) c.close() else q.addFirst(c)
    }

    /** Một lần hỏi: dùng lại kết nối đang nghỉ (hỏng thì mở mới ngay), không có thì mở kết nối mới. */
    private fun exchange(s: Server, ip: String, query: ByteArray): ByteArray? {
        val key = "${s.label}|$ip"
        take(key)?.let { c ->
            try {
                return roundTrip(s, c, query)
            } catch (e: Exception) {
                // máy chủ đã đóng kết nối cũ — không phải máy chủ lỗi, mở kết nối mới bên dưới
            }
        }
        return roundTrip(s, open(s, ip, key), query)
    }

    private fun open(s: Server, ip: String, key: String): Conn {
        val raw = Socket()
        try {
            raw.connect(InetSocketAddress(InetAddress.getByName(ip), s.port), CONNECT_MS)
            raw.soTimeout = READ_MS
            raw.tcpNoDelay = true
        } catch (e: IOException) {
            raw.close()
            throw e
        }
        val ssl = (SSLSocketFactory.getDefault() as SSLSocketFactory).createSocket(raw, s.host, s.port, true) as SSLSocket
        try {
            ssl.startHandshake()
            if (!HttpsURLConnection.getDefaultHostnameVerifier().verify(s.host, ssl.session)) throw SSLException("chứng chỉ không khớp ${s.host}")
        } catch (e: Exception) {
            runCatching { ssl.close() }
            throw e
        }
        return Conn(key, ssl)
    }

    /** Gửi một câu hỏi trên kết nối c, đọc đúng một trả lời rồi trả kết nối về chỗ nghỉ (lỗi / máy chủ báo đóng thì đóng). */
    private fun roundTrip(s: Server, c: Conn, query: ByteArray): ByteArray? {
        var keep = false
        try {
            c.sock.soTimeout = READ_MS
            if (s.kind == Kind.DOT) {
                c.output.write(byteArrayOf((query.size shr 8).toByte(), query.size.toByte()) + query)
                c.output.flush()
                val i = DataInputStream(c.input)
                val reply = ByteArray(i.readUnsignedShort()).also { i.readFully(it) }
                keep = true
                return reply
            }
            val head = "POST ${s.path} HTTP/1.1\r\nHost: ${s.host}\r\nContent-Type: application/dns-message\r\n" +
                "Accept: application/dns-message\r\nContent-Length: ${query.size}\r\n\r\n"
            c.output.write(head.toByteArray(Charsets.US_ASCII) + query)
            c.output.flush()
            val r = DohHttp.read(c.input)
            keep = r.keepAlive
            return if (r.status == 200) r.body else null
        } finally {
            if (keep) give(c) else c.close()
        }
    }

    fun clearCache() {
        cache.clear()
        downUntil.clear()
    }
}

/** Gói DNS (RFC 1035): hỏi bản ghi A, đọc trả lời. */
object DnsWire {
    fun query(name: String, id: Int = 0): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(byteArrayOf((id shr 8).toByte(), id.toByte(), 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0)) // RD, 1 câu hỏi
        for (label in name.trimEnd('.').split('.')) {
            val b = label.toByteArray(Charsets.US_ASCII)
            require(b.size in 1..63) { "tên miền sai: $name" }
            out.write(b.size)
            out.write(b)
        }
        out.write(byteArrayOf(0, 0, 1, 0, 1)) // hết tên, kiểu A, lớp IN
        return out.toByteArray()
    }

    /** Trả lời → Answer; id khác (DoT) hay gói hỏng → Failed. */
    fun answer(b: ByteArray, id: Int): Doh.Answer {
        if (b.size < 12) return Doh.Answer.Failed
        fun u16(i: Int) = ((b[i].toInt() and 0xFF) shl 8) or (b[i + 1].toInt() and 0xFF)
        if (u16(0) != id) return Doh.Answer.Failed
        val rcode = b[3].toInt() and 0x0F
        if (rcode == 3) return Doh.Answer.NoDomain
        if (rcode != 0) return Doh.Answer.Failed
        var i = 12
        repeat(u16(4)) { i = skipName(b, i) + 4 }
        val all = ArrayList<InetAddress>()
        var ttl = Int.MAX_VALUE
        repeat(u16(6)) {
            i = skipName(b, i)
            if (i + 10 > b.size) return Doh.Answer.Failed
            val type = u16(i)
            val t = ((b[i + 4].toLong() and 0xFF) shl 24) or ((b[i + 5].toLong() and 0xFF) shl 16) or
                ((b[i + 6].toLong() and 0xFF) shl 8) or (b[i + 7].toLong() and 0xFF)
            val len = u16(i + 8)
            i += 10
            if (i + len > b.size) return Doh.Answer.Failed
            if (type == 1 && len == 4) {
                all += InetAddress.getByAddress(b.copyOfRange(i, i + 4)) as Inet4Address
                ttl = minOf(ttl, t.coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
            }
            i += len
        }
        return all.firstOrNull()?.let { Doh.Answer.Found(it, if (ttl == Int.MAX_VALUE) 60 else ttl, all) } ?: Doh.Answer.NoIpv4
    }

    /** Bỏ qua một tên (có thể nén bằng con trỏ 0xC0). */
    private fun skipName(b: ByteArray, from: Int): Int {
        var i = from
        while (i < b.size) {
            val l = b[i].toInt() and 0xFF
            when {
                l == 0 -> return i + 1
                l and 0xC0 == 0xC0 -> return i + 2
                else -> i += 1 + l
            }
        }
        return b.size
    }
}

/** Trả lời HTTP/1.1 của máy chủ DoH: đọc đúng một trả lời trên kết nối (để dùng lại kết nối). */
object DohHttp {
    class Reply(val status: Int, val body: ByteArray, val keepAlive: Boolean)

    /** Đọc đầu trả lời rồi thân theo Content-Length / chunked; không có độ dài thì đọc tới khi máy chủ đóng. */
    fun read(i: InputStream): Reply {
        val head = ByteArrayOutputStream()
        var last = 0
        while (last != 0x0D0A0D0A) {
            val b = i.read()
            if (b < 0) throw IOException("máy chủ ngắt giữa chừng")
            head.write(b)
            last = (last shl 8) or b
            if (head.size() > 16_384) throw IOException("đầu trả lời quá dài")
        }
        val h = String(head.toByteArray(), Charsets.ISO_8859_1)
        val status = h.lineSequence().first().split(' ').getOrNull(1)?.toIntOrNull() ?: throw IOException("trả lời HTTP sai")
        val close = Regex("""(?im)^connection:\s*close""").containsMatchIn(h)
        if (Regex("""(?im)^transfer-encoding:\s*chunked""").containsMatchIn(h)) return Reply(status, dechunk(i), !close)
        val len = Regex("""(?im)^content-length:\s*(\d+)""").find(h)?.groupValues?.get(1)?.toIntOrNull()
            ?: return Reply(status, readToEnd(i), false)
        if (len > 70_000) throw IOException("trả lời DNS quá dài")
        return Reply(status, ByteArray(len).also { DataInputStream(i).readFully(it) }, !close)
    }

    /** Trả lời đã nhận trọn: thân khi mã 200, null nếu mã khác / hỏng (để test). */
    fun body(resp: ByteArray): ByteArray? =
        runCatching { read(java.io.ByteArrayInputStream(resp)) }.getOrNull()?.takeIf { it.status == 200 }?.body

    private fun line(i: InputStream): String {
        val b = StringBuilder()
        while (true) {
            val c = i.read()
            if (c < 0) throw IOException("máy chủ ngắt giữa chừng")
            if (c == '\n'.code) return b.toString().trimEnd('\r')
            b.append(c.toChar())
            if (b.length > 1024) throw IOException("dòng quá dài")
        }
    }

    private fun dechunk(i: InputStream): ByteArray {
        val out = ByteArrayOutputStream()
        val d = DataInputStream(i)
        while (true) {
            val n = line(i).substringBefore(';').trim().toIntOrNull(16) ?: throw IOException("chunk sai")
            if (n == 0) {
                while (line(i).isNotEmpty()) {
                    // bỏ phần đuôi (trailer)
                }
                return out.toByteArray()
            }
            if (out.size() + n > 70_000) throw IOException("trả lời DNS quá dài")
            out.write(ByteArray(n).also { d.readFully(it) })
            line(i) // CRLF sau mỗi khúc
        }
    }

    private fun readToEnd(i: InputStream): ByteArray {
        val out = ByteArrayOutputStream()
        val buf = ByteArray(4096)
        while (true) {
            val n = i.read(buf)
            if (n < 0) return out.toByteArray()
            out.write(buf, 0, n)
            if (out.size() > 70_000) throw IOException("trả lời DNS quá dài")
        }
    }
}
