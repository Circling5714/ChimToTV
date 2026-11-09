package vn.chimto.tv

import android.util.Log
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.IOException
import java.io.InputStream
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.security.SecureRandom
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ExecutionException
import java.util.concurrent.FutureTask
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLException
import javax.net.ssl.SSLSocket
import javax.net.ssl.SSLSocketFactory

/**
 * DNS mã hoá (từ 2.10.1 bật sẵn): hỏi địa chỉ tên miền qua DoH (RFC 8484) rồi DoT (RFC 7858), mỗi giao thức lần lượt
 * NextDNS (hồ sơ riêng của app — mã trong keys/nextdns.txt, không có trên git) → Cloudflare → Google; lỗi hết thì bên gọi
 * dùng DNS của thiết bị — app không bao giờ hỏng vì DNS mã hoá bị chặn. Nhà mạng không đọc / sửa được truy vấn DNS (vẫn
 * thấy tên trang lúc bắt tay TLS và IP đích — Vượt chặn lo phần tên trang).
 *
 * Máy chủ gọi bằng IP cố định (khỏi phải hỏi DNS để tìm máy chủ DNS), kiểm chứng chỉ theo tên máy chủ; mỗi lần hỏi một
 * kết nối TLS, đi thẳng (không qua cổng nội bộ / proxy). Máy chủ lỗi thì nghỉ 2 phút. Trả lời «không có tên miền»
 * (NXDOMAIN — vd hồ sơ NextDNS chặn) là trả lời thật: không hỏi máy chủ sau. Bản debug ghi mỗi lần hỏi (logcat ChimToDoh).
 */
object Doh {
    enum class Kind { DOH, DOT }

    class Server(val label: String, val kind: Kind, val host: String, val path: String, val ips: List<String>) {
        val port get() = if (kind == Kind.DOT) 853 else 443
    }

    private val NEXT_IPS = listOf("45.90.28.0", "45.90.30.0")
    private val GOOGLE_IPS = listOf("8.8.8.8", "8.8.4.4")

    /** Thứ tự hỏi: DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy (không có mã NextDNS thì bỏ NextDNS). */
    fun chain(nextId: String): List<Server> {
        val next = nextId.trim()
        return listOfNotNull(
            next.takeIf { it.isNotEmpty() }?.let { Server("DoH NextDNS", Kind.DOH, "dns.nextdns.io", "/$it", NEXT_IPS) },
            Server("DoH Cloudflare", Kind.DOH, "dns.cloudflare.com", "/dns-query", listOf("104.16.248.249", "104.16.249.249")),
            Server("DoH Google", Kind.DOH, "dns.google", "/dns-query", GOOGLE_IPS),
            next.takeIf { it.isNotEmpty() }?.let { Server("DoT NextDNS", Kind.DOT, "$it.dns.nextdns.io", "", NEXT_IPS) },
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

    /** Địa chỉ đã hỏi; addr null = tên miền không có (NXDOMAIN). */
    private class Entry(val addr: InetAddress?, val until: Long)

    private val cache = ConcurrentHashMap<String, Entry>()
    private val downUntil = ConcurrentHashMap<String, Long>()
    private val nextIp = ConcurrentHashMap<String, Int>()
    private val random = SecureRandom()
    private val IPV4 = Regex("""^\d{1,3}(\.\d{1,3}){3}$""")

    /** Kết quả hỏi một máy chủ (máy chủ lỗi thì ném lỗi). */
    sealed class Answer {
        class Found(val addr: InetAddress, val ttl: Int) : Answer()
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
            val answer = try {
                ask(s, key)
            } catch (e: Exception) {
                // Máy chủ bị chặn / không trả lời: nghỉ 2 phút, lần sau thử IP còn lại của nó
                downUntil[s.label] = System.currentTimeMillis() + DOWN_MS
                nextIp[s.label] = (nextIp[s.label] ?: 0) + 1
                if (BuildConfig.DEBUG) Log.d("ChimToDoh", "${s.label} lỗi: ${e.javaClass.simpleName} ${e.message}")
                continue
            }
            when (answer) {
                is Answer.Found -> {
                    cache[key] = Entry(answer.addr, System.currentTimeMillis() + answer.ttl.coerceIn(30, 600) * 1000L)
                    if (BuildConfig.DEBUG) Log.d("ChimToDoh", "$key → ${answer.addr.hostAddress} (${s.label})")
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

    /** Chỉ bản debug (cổng 8790 /dohfail): coi các máy chủ này như bị chặn, để thử chuỗi dự phòng. */
    @Volatile var debugFail: Set<String> = emptySet()

    /** Hỏi một máy chủ (một trong các IP của nó). */
    fun ask(s: Server, name: String): Answer {
        if (BuildConfig.DEBUG && s.label in debugFail) throw IOException("giả bị chặn")
        val ip = s.ips[(nextIp[s.label] ?: 0) % s.ips.size]
        val id = if (s.kind == Kind.DOT) random.nextInt(0x10000) else 0 // DoH khuyên id 0 (dễ lưu đệm)
        val reply = exchange(s, ip, DnsWire.query(name, id)) ?: return Answer.Failed
        return DnsWire.answer(reply, id)
    }

    private fun exchange(s: Server, ip: String, query: ByteArray): ByteArray? {
        val raw = Socket()
        try {
            raw.connect(InetSocketAddress(InetAddress.getByName(ip), s.port), CONNECT_MS)
            raw.soTimeout = READ_MS
        } catch (e: IOException) {
            raw.close()
            throw e
        }
        val ssl = (SSLSocketFactory.getDefault() as SSLSocketFactory).createSocket(raw, s.host, s.port, true) as SSLSocket
        ssl.use { t ->
            t.startHandshake()
            if (!HttpsURLConnection.getDefaultHostnameVerifier().verify(s.host, t.session)) throw SSLException("chứng chỉ không khớp ${s.host}")
            val o = t.outputStream
            if (s.kind == Kind.DOT) {
                o.write(byteArrayOf((query.size shr 8).toByte(), query.size.toByte()) + query)
                o.flush()
                val i = DataInputStream(t.inputStream)
                return ByteArray(i.readUnsignedShort()).also { i.readFully(it) }
            }
            val head = "POST ${s.path} HTTP/1.1\r\nHost: ${s.host}\r\nContent-Type: application/dns-message\r\n" +
                "Accept: application/dns-message\r\nContent-Length: ${query.size}\r\nConnection: close\r\n\r\n"
            o.write(head.toByteArray(Charsets.US_ASCII) + query)
            o.flush()
            return DohHttp.body(readAll(t.inputStream))
        }
    }

    private fun readAll(i: InputStream): ByteArray {
        val out = ByteArrayOutputStream()
        val buf = ByteArray(4096)
        while (true) {
            val n = i.read(buf)
            if (n < 0) break
            out.write(buf, 0, n)
            if (out.size() > 70_000) throw IOException("trả lời DNS quá dài")
        }
        return out.toByteArray()
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
        var found: InetAddress? = null
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
                if (found == null) found = InetAddress.getByAddress(b.copyOfRange(i, i + 4)) as Inet4Address
                ttl = minOf(ttl, t.coerceAtMost(Int.MAX_VALUE.toLong()).toInt())
            }
            i += len
        }
        return found?.let { Doh.Answer.Found(it, if (ttl == Int.MAX_VALUE) 60 else ttl) } ?: Doh.Answer.NoIpv4
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

/** Đọc trả lời HTTP/1.1 đã nhận trọn (Connection: close): thân khi mã 200, null nếu mã khác. */
object DohHttp {
    fun body(resp: ByteArray): ByteArray? {
        val sep = (0..resp.size - 4).firstOrNull {
            resp[it] == '\r'.code.toByte() && resp[it + 1] == '\n'.code.toByte() && resp[it + 2] == '\r'.code.toByte() && resp[it + 3] == '\n'.code.toByte()
        } ?: return null
        val head = String(resp, 0, sep, Charsets.ISO_8859_1)
        if (head.lineSequence().first().split(' ').getOrNull(1) != "200") return null
        val body = resp.copyOfRange(sep + 4, resp.size)
        if (Regex("""(?im)^transfer-encoding:\s*chunked""").containsMatchIn(head)) return dechunk(body)
        val len = Regex("""(?im)^content-length:\s*(\d+)""").find(head)?.groupValues?.get(1)?.toIntOrNull()
        return if (len != null && len <= body.size) body.copyOf(len) else body
    }

    private fun dechunk(b: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        var i = 0
        while (i < b.size) {
            val eol = (i until b.size - 1).firstOrNull { b[it] == '\r'.code.toByte() && b[it + 1] == '\n'.code.toByte() } ?: break
            val n = String(b, i, eol - i, Charsets.ISO_8859_1).substringBefore(';').trim().toIntOrNull(16) ?: break
            if (n == 0) break
            val start = eol + 2
            if (start + n > b.size) break
            out.write(b, start, n)
            i = start + n + 2
        }
        return out.toByteArray()
    }
}
