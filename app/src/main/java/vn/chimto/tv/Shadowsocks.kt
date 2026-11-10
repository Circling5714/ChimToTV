package vn.chimto.tv

import java.io.EOFException
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.HttpURLConnection
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import java.net.SocketTimeoutException
import java.net.URL
import java.net.URLDecoder
import java.net.UnknownHostException
import java.security.GeneralSecurityException
import java.security.MessageDigest
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec
import kotlin.math.abs

/**
 * Máy chủ Shadowsocks (Cài đặt → Proxy → Kiểu: Shadowsocks), vd lối vào Shadowsocks của 3X-UI / Xray.
 * password: SS 2022 = các khoá base64 cách nhau bằng ":" (3X-UI luôn nhiều người dùng: "khoá-máy-chủ:khoá-người-dùng");
 * kiểu cũ (aes-256-gcm, chacha20-ietf-poly1305…) = mật khẩu.
 */
data class SsConfig(
    val host: String, val port: Int, val method: String, val password: String,
    /** true: máy chủ phân giải tên miền (gửi tên miền); false: thiết bị tự phân giải rồi gửi IP. */
    val remoteDns: Boolean = true,
    /** true: chỉ trang phim đi qua máy chủ; video / ảnh / cập nhật đi thẳng. */
    val sitesOnly: Boolean = false,
)

/** Kiểu mã hoá Shadowsocks hỗ trợ (TCP). keySize = số byte khoá = số byte salt. */
enum class SsMethod(val id: String, val keySize: Int, val chacha: Boolean, val v2022: Boolean) {
    B3_AES_256("2022-blake3-aes-256-gcm", 32, false, true),
    B3_AES_128("2022-blake3-aes-128-gcm", 16, false, true),
    B3_CHACHA("2022-blake3-chacha20-poly1305", 32, true, true),
    AES_256("aes-256-gcm", 32, false, false),
    AES_128("aes-128-gcm", 16, false, false),
    CHACHA("chacha20-ietf-poly1305", 32, true, false);

    companion object {
        val DEFAULT = B3_AES_256

        /** Theo tên (không phân biệt hoa / thường); "chacha20-poly1305" của Xray / 3X-UI = chacha20-ietf-poly1305. */
        fun of(name: String): SsMethod? {
            val n = name.trim().lowercase()
            if (n.isEmpty()) return DEFAULT
            if (n == "chacha20-poly1305") return CHACHA
            return entries.firstOrNull { it.id == n }
        }

        val names: String get() = entries.joinToString(", ") { it.id }
    }
}

/** Lỗi Shadowsocks: câu báo cho người xem (sai khoá, máy chủ không trả lời…). */
class SsError(message: String) : IOException(message)

/** Đọc link ss:// (SIP002, cả kiểu 3X-UI cho SS 2022: "ss://method:khoá-máy-chủ:khoá-người-dùng@host:cổng?type=tcp#tên"). */
object SsLink {
    /** Sai thì IllegalArgumentException với câu báo cho người xem. */
    fun parse(link: String, remoteDns: Boolean = true, sitesOnly: Boolean = false): SsConfig {
        var s = link.trim()
        require(s.startsWith("ss://", ignoreCase = true)) { "Link Shadowsocks phải bắt đầu bằng ss://" }
        s = s.substring(5).substringBefore('#')
        val query = s.substringAfter('?', "")
        s = s.substringBefore('?').trimEnd('/')
        for (p in query.split('&')) {
            val k = p.substringBefore('=').lowercase()
            val v = decode(p.substringAfter('=', "")).lowercase()
            when {
                k == "type" && v.isNotEmpty() && v != "tcp" -> throw IllegalArgumentException(
                    "Link dùng truyền tải «$v» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Transmission = TCP, không TLS)."
                )
                k == "security" && v.isNotEmpty() && v != "none" -> throw IllegalArgumentException(
                    "Link dùng «security=$v» — app chỉ hỗ trợ Shadowsocks qua TCP thường (3X-UI: Security = None)."
                )
                k == "plugin" && v.isNotEmpty() -> throw IllegalArgumentException("Link dùng plugin (${v.substringBefore(';')}) — app chưa hỗ trợ plugin.")
                k == "headertype" && v.isNotEmpty() && v != "none" -> throw IllegalArgumentException(
                    "Link dùng ngụy trang HTTP — app chưa hỗ trợ (3X-UI: TCP header = None)."
                )
            }
        }
        // Kiểu cũ: ss://base64(method:pass@host:port)
        if ('@' !in s) s = String(SsBase64.decode(s) ?: throw IllegalArgumentException("Link ss:// không đọc được."), Charsets.UTF_8)
        val user = s.substringBeforeLast('@')
        val hostPort = s.substringAfterLast('@')
        val info = if (':' in user || '%' in user) decode(user)
        else SsBase64.decode(user)?.let { String(it, Charsets.UTF_8) }?.takeIf { ':' in it } ?: decode(user)
        require(':' in info) { "Link ss:// thiếu kiểu mã hoá / khoá." }
        val method = info.substringBefore(':')
        val password = info.substringAfter(':')
        val (host, port) = splitHostPort(hostPort) ?: throw IllegalArgumentException("Link ss:// thiếu máy chủ / cổng.")
        val m = SsMethod.of(method) ?: throw IllegalArgumentException("Kiểu mã hoá «$method» chưa hỗ trợ. App hỗ trợ: ${SsMethod.names}.")
        Shadowsocks.keys(m, password) // kiểm tra khoá ngay lúc dán
        return SsConfig(host, port, m.id, password, remoteDns, sitesOnly)
    }

    /** "host:cổng" hoặc "[ipv6]:cổng" → (host, cổng). */
    fun splitHostPort(s: String): Pair<String, Int>? {
        val t = s.trim()
        val host: String
        val port: String
        if (t.startsWith('[')) {
            host = t.substring(1).substringBefore(']')
            port = t.substringAfter("]:", "")
        } else {
            host = t.substringBeforeLast(':')
            port = t.substringAfterLast(':', "")
        }
        val p = port.toIntOrNull() ?: return null
        if (host.isEmpty() || p !in 1..65535) return null
        return host to p
    }

    private fun decode(s: String): String = try {
        URLDecoder.decode(s.replace("+", "%2B"), "UTF-8")
    } catch (e: IllegalArgumentException) {
        s
    }
}

/** Base64 thường lẫn base64url, có hoặc không dấu "=" (khoá 3X-UI, link SIP002). null = không phải base64. */
internal object SsBase64 {
    fun decode(s: String): ByteArray? {
        val t = s.trim().trimEnd('=')
        if (t.isEmpty()) return null
        val out = java.io.ByteArrayOutputStream(t.length * 3 / 4)
        var acc = 0
        var bits = 0
        for (ch in t) {
            val v = when (ch) {
                in 'A'..'Z' -> ch - 'A'
                in 'a'..'z' -> ch - 'a' + 26
                in '0'..'9' -> ch - '0' + 52
                '+', '-' -> 62
                '/', '_' -> 63
                else -> return null
            }
            acc = (acc shl 6) or v
            bits += 6
            if (bits >= 8) {
                bits -= 8
                out.write((acc shr bits) and 0xFF)
            }
        }
        if (bits >= 6) return null // thừa một ký tự lẻ
        return out.toByteArray()
    }
}

/**
 * BLAKE3 — chỉ đầu vào ≤ 1024 byte (một chunk) và đầu ra ≤ 32 byte: đủ cho khoá của Shadowsocks 2022 (khoá + salt ≤ 64 byte).
 * Kiểm bằng bộ số chuẩn của BLAKE3 (test_vectors.json) trong ShadowsocksTest.
 */
internal object Blake3 {
    private val IV = intArrayOf(
        0x6A09E667, 0xBB67AE85.toInt(), 0x3C6EF372, 0xA54FF53A.toInt(), 0x510E527F, 0x9B05688C.toInt(), 0x1F83D9AB, 0x5BE0CD19,
    )
    private val PERM = intArrayOf(2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
    private const val CHUNK_START = 1
    private const val CHUNK_END = 2
    private const val ROOT = 8
    private const val DERIVE_KEY_CONTEXT = 32
    private const val DERIVE_KEY_MATERIAL = 64

    fun hash(input: ByteArray, outLen: Int = 32): ByteArray = root(IV, input, 0, outLen)

    fun deriveKey(context: String, material: ByteArray, outLen: Int = 32): ByteArray {
        val ck = root(IV, context.toByteArray(Charsets.UTF_8), DERIVE_KEY_CONTEXT, 32)
        return root(IntArray(8) { le32(ck, it * 4) }, material, DERIVE_KEY_MATERIAL, outLen)
    }

    private fun root(key: IntArray, input: ByteArray, flags: Int, outLen: Int): ByteArray {
        require(input.size <= 1024 && outLen in 1..32)
        var cv = key
        val blocks = maxOf(1, (input.size + 63) / 64)
        val m = IntArray(16)
        for (b in 0 until blocks) {
            val start = b * 64
            val len = minOf(64, input.size - start)
            m.fill(0)
            for (i in 0 until len) m[i shr 2] = m[i shr 2] or ((input[start + i].toInt() and 0xFF) shl (8 * (i and 3)))
            var f = flags
            if (b == 0) f = f or CHUNK_START
            if (b == blocks - 1) f = f or CHUNK_END or ROOT
            cv = compress(cv, m, len, f)
        }
        return ByteArray(outLen) { (cv[it shr 2] ushr (8 * (it and 3))).toByte() }
    }

    /** Hàm nén (bộ đếm = 0: chỉ một chunk); trả 8 từ đầu (chaining value / đầu ra gốc). */
    private fun compress(cv: IntArray, block: IntArray, len: Int, flags: Int): IntArray {
        val s = intArrayOf(cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7], IV[0], IV[1], IV[2], IV[3], 0, 0, len, flags)
        var m = block.copyOf()
        for (r in 0 until 7) {
            g(s, 0, 4, 8, 12, m[0], m[1]); g(s, 1, 5, 9, 13, m[2], m[3])
            g(s, 2, 6, 10, 14, m[4], m[5]); g(s, 3, 7, 11, 15, m[6], m[7])
            g(s, 0, 5, 10, 15, m[8], m[9]); g(s, 1, 6, 11, 12, m[10], m[11])
            g(s, 2, 7, 8, 13, m[12], m[13]); g(s, 3, 4, 9, 14, m[14], m[15])
            if (r < 6) m = IntArray(16) { m[PERM[it]] }
        }
        return IntArray(8) { s[it] xor s[it + 8] }
    }

    private fun g(s: IntArray, a: Int, b: Int, c: Int, d: Int, x: Int, y: Int) {
        s[a] = s[a] + s[b] + x
        s[d] = (s[d] xor s[a]).rotateRight(16)
        s[c] = s[c] + s[d]
        s[b] = (s[b] xor s[c]).rotateRight(12)
        s[a] = s[a] + s[b] + y
        s[d] = (s[d] xor s[a]).rotateRight(8)
        s[c] = s[c] + s[d]
        s[b] = (s[b] xor s[c]).rotateRight(7)
    }

    private fun le32(b: ByteArray, i: Int) =
        (b[i].toInt() and 0xFF) or ((b[i + 1].toInt() and 0xFF) shl 8) or ((b[i + 2].toInt() and 0xFF) shl 16) or (b[i + 3].toInt() shl 24)
}

/** AEAD một chiều của phiên: khoá phiên + nonce 12 byte little-endian, tăng sau mỗi lần mã hoá / giải mã. */
internal class SsAead(m: SsMethod, key: ByteArray) {
    private val chacha = m.chacha
    private val cipher: Cipher = if (chacha) Shadowsocks.chachaCipher() else Shadowsocks.aesGcm()
    private val spec = SecretKeySpec(key, if (chacha) "ChaCha20" else "AES")
    private val nonce = ByteArray(12)

    fun seal(p: ByteArray, off: Int, len: Int, out: ByteArray, outOff: Int): Int {
        cipher.init(Cipher.ENCRYPT_MODE, spec, params())
        val n = cipher.doFinal(p, off, len, out, outOff)
        inc()
        return n
    }

    /** Sai thẻ xác thực (sai khoá / dữ liệu hỏng) → GeneralSecurityException. */
    fun open(c: ByteArray, off: Int, len: Int, out: ByteArray, outOff: Int): Int {
        cipher.init(Cipher.DECRYPT_MODE, spec, params())
        val n = cipher.doFinal(c, off, len, out, outOff)
        inc()
        return n
    }

    private fun params() = if (chacha) IvParameterSpec(nonce.copyOf()) else GCMParameterSpec(128, nonce.copyOf())

    private fun inc() {
        for (i in nonce.indices) {
            nonce[i]++
            if (nonce[i] != 0.toByte()) break
        }
    }
}

/**
 * Luồng gửi tới máy chủ (mã hoá). Phần đầu (salt, phần nhận dạng người dùng, địa chỉ đích) gửi chung một lần ghi với dữ
 * liệu đầu tiên của bên gọi; start() gửi phần đầu không kèm dữ liệu (thêm đệm ngẫu nhiên) khi bên gọi chưa nói gì.
 */
internal class SsOutput(private val raw: OutputStream, private val m: SsMethod, private val keys: List<ByteArray>, private val target: ByteArray) :
    OutputStream() {
    val salt: ByteArray = Shadowsocks.random(m.keySize)
    private val aead = SsAead(m, Shadowsocks.subkey(m, keys.last(), salt))
    private val maxChunk = if (m.v2022) 0xFFFF else 0x3FFF
    // Dùng lại giữa các chunk (TV yếu: bớt cấp phát / dọn rác khi đang phát video)
    private var obuf = ByteArray(0)
    private val lenPlain = ByteArray(2)
    var started = false
        private set

    override fun write(b: Int) = write(byteArrayOf(b.toByte()), 0, 1)

    override fun write(b: ByteArray, off: Int, len: Int) {
        var o = off
        var n = len
        if (!started) {
            val k = header(b, o, n)
            o += k
            n -= k
        }
        while (n > 0) {
            val k = minOf(n, maxChunk)
            val need = 2 + 16 + k + 16
            if (obuf.size < need) obuf = ByteArray(maxOf(need, 16 * 1024 + 34))
            lenPlain[0] = (k shr 8).toByte()
            lenPlain[1] = k.toByte()
            aead.seal(lenPlain, 0, 2, obuf, 0)
            aead.seal(b, o, k, obuf, 18)
            raw.write(obuf, 0, need)
            o += k
            n -= k
        }
    }

    /** Gửi phần đầu ngay (chưa có dữ liệu của bên gọi). */
    fun start() {
        if (!started) header(ByteArray(0), 0, 0)
    }

    override fun flush() = raw.flush()

    override fun close() = raw.close()

    private fun seal(p: ByteArray, out: ByteArray, at: Int): Int = aead.seal(p, 0, p.size, out, at)

    /** Ghi phần đầu kèm tối đa một phần dữ liệu; trả số byte dữ liệu đã gửi kèm. */
    private fun header(b: ByteArray, off: Int, len: Int): Int {
        started = true
        val buf = java.io.ByteArrayOutputStream(m.keySize + 128 + len)
        buf.write(salt)
        val sent: Int
        if (m.v2022) {
            // Phần nhận dạng (EIH): mỗi khoá trước khoá người dùng mã hoá 16 byte đầu BLAKE3(khoá kế tiếp) bằng AES-ECB
            for (i in 0 until keys.size - 1) {
                val sub = Blake3.deriveKey("shadowsocks 2022 identity subkey", keys[i] + salt, m.keySize)
                val ecb = Cipher.getInstance("AES/ECB/NoPadding")
                ecb.init(Cipher.ENCRYPT_MODE, SecretKeySpec(sub, "AES"))
                buf.write(ecb.doFinal(Blake3.hash(keys[i + 1], 16)))
            }
            val padding = if (len < 900) Shadowsocks.rng.nextInt(900) + 1 else 0
            sent = minOf(len, 0xFFFF - target.size - 2 - padding)
            val varLen = target.size + 2 + padding + sent
            val fixed = java.nio.ByteBuffer.allocate(11).put(0).putLong(System.currentTimeMillis() / 1000).putShort(varLen.toShort()).array()
            val vh = ByteArray(varLen)
            System.arraycopy(target, 0, vh, 0, target.size)
            vh[target.size] = (padding shr 8).toByte()
            vh[target.size + 1] = padding.toByte()
            System.arraycopy(Shadowsocks.random(padding), 0, vh, target.size + 2, padding)
            System.arraycopy(b, off, vh, target.size + 2 + padding, sent)
            val out = ByteArray(11 + 16 + varLen + 16)
            seal(fixed, out, 0)
            aead.seal(vh, 0, varLen, out, 27)
            buf.write(out)
        } else {
            // Kiểu cũ: chunk đầu = địa chỉ đích + dữ liệu đầu
            sent = minOf(len, maxChunk - target.size)
            val first = target + b.copyOfRange(off, off + sent)
            val out = ByteArray(2 + 16 + first.size + 16)
            seal(byteArrayOf((first.size shr 8).toByte(), first.size.toByte()), out, 0)
            aead.seal(first, 0, first.size, out, 18)
            buf.write(out)
        }
        raw.write(buf.toByteArray()) // một lần ghi (chuẩn SS 2022: không để lộ cỡ từng phần)
        return sent
    }
}

/** Luồng nhận từ máy chủ (giải mã). Lỗi phần đầu → SsError với câu dễ hiểu (sai khoá / máy chủ không trả lời…). */
internal class SsInput(private val raw: InputStream, private val m: SsMethod, private val key: ByteArray, private val requestSalt: ByteArray) :
    InputStream() {
    private var aead: SsAead? = null
    private var buf = ByteArray(0)
    private var pos = 0
    private var end = 0
    // Dùng lại giữa các chunk: chunk độ dài (mã hoá / giải mã) và bản mã của chunk dữ liệu
    private val lc = ByteArray(18)
    private val lp = ByteArray(18) // Conscrypt đòi chỗ chứa ≥ cỡ bản mã (kể cả thẻ 16 byte)
    private var cbuf = ByteArray(0)

    override fun read(): Int {
        val one = ByteArray(1)
        return if (read(one, 0, 1) < 0) -1 else one[0].toInt() and 0xFF
    }

    override fun read(b: ByteArray, off: Int, len: Int): Int {
        if (len == 0) return 0
        while (pos >= end) if (!fill()) return -1
        val n = minOf(len, end - pos)
        System.arraycopy(buf, pos, b, off, n)
        pos += n
        return n
    }

    override fun available() = end - pos

    override fun close() = raw.close()

    /** Chunk tiếp theo vào buf; false = máy chủ đóng kết nối (hết dữ liệu). */
    private fun fill(): Boolean {
        val a = aead ?: return start()
        if (!readFully(lc, 18, eofOk = true)) return false
        try {
            a.open(lc, 0, 18, lp, 0)
        } catch (e: GeneralSecurityException) {
            throw SsError("dữ liệu từ máy chủ Shadowsocks bị hỏng")
        }
        payload(a, ((lp[0].toInt() and 0xFF) shl 8) or (lp[1].toInt() and 0xFF))
        return true
    }

    private fun payload(a: SsAead, n: Int) {
        Shadowsocks.stats.incrementAndGet(0)
        Shadowsocks.stats.addAndGet(1, n.toLong())
        if (cbuf.size < n + 16) cbuf = ByteArray(maxOf(n + 16, 16 * 1024 + 16))
        readFully(cbuf, n + 16, eofOk = false)
        if (buf.size < n + 16) buf = ByteArray(maxOf(n + 16, 16 * 1024 + 16))
        try {
            a.open(cbuf, 0, n + 16, buf, 0)
        } catch (e: GeneralSecurityException) {
            throw SsError("dữ liệu từ máy chủ Shadowsocks bị hỏng")
        }
        pos = 0
        end = n
    }

    private fun start(): Boolean {
        val salt = ByteArray(m.keySize)
        // Máy chủ không nhận ra khoá thì đóng / ngắt (RST) / im lặng — chưa có trả lời nào là coi như bị từ chối
        val got = try {
            readFully(salt, salt.size, eofOk = true)
        } catch (e: IOException) {
            throw SsError(Shadowsocks.noReply(m))
        }
        if (!got) throw SsError(Shadowsocks.noReply(m))
        val a = SsAead(m, Shadowsocks.subkey(m, key, salt))
        aead = a
        if (!m.v2022) return fill()
        val hl = 1 + 8 + m.keySize + 2
        val c = ByteArray(hl + 16)
        readFully(c, c.size, eofOk = false)
        val h = ByteArray(c.size)
        try {
            a.open(c, 0, c.size, h, 0)
        } catch (e: GeneralSecurityException) {
            throw SsError("không giải mã được trả lời của máy chủ Shadowsocks — sai khoá hoặc sai kiểu mã hoá")
        }
        if (h[0].toInt() != 1 || !h.copyOfRange(9, 9 + m.keySize).contentEquals(requestSalt)) {
            throw SsError("máy chủ Shadowsocks trả lời sai giao thức")
        }
        val ts = java.nio.ByteBuffer.wrap(h, 1, 8).long
        val skew = System.currentTimeMillis() / 1000 - ts
        if (abs(skew) > 30) throw SsError("giờ trên máy lệch ${abs(skew)} giây so với máy chủ (Shadowsocks 2022 cho lệch tối đa 30 giây)")
        payload(a, ((h[hl - 2].toInt() and 0xFF) shl 8) or (h[hl - 1].toInt() and 0xFF))
        return true
    }

    private fun readFully(b: ByteArray, size: Int, eofOk: Boolean): Boolean {
        var n = 0
        while (n < size) {
            val k = raw.read(b, n, size - n)
            if (k < 0) {
                if (n == 0 && eofOk) return false
                throw EOFException("máy chủ Shadowsocks ngắt kết nối giữa chừng")
            }
            n += k
        }
        return true
    }
}

/** Kết nối đã mở tới đích: đi thẳng / qua SOCKS5 thì luồng của socket; qua Shadowsocks thì luồng đã mã hoá. */
class Upstream(val socket: Socket, val input: InputStream, val output: OutputStream) {
    constructor(s: Socket) : this(s, s.getInputStream(), s.getOutputStream())
}

/** Phía khách Shadowsocks (TCP): SS 2022 (SIP022, có phần nhận dạng người dùng cho máy chủ nhiều người dùng) + AEAD cũ. */
object Shadowsocks {
    internal val rng = SecureRandom()

    /** Đếm cho cổng gỡ lỗi: [số chunk nhận, số byte nhận]. */
    internal val stats = java.util.concurrent.atomic.AtomicLongArray(2)

    internal fun random(n: Int) = ByteArray(n).also { rng.nextBytes(it) }

    /**
     * Khoá từ mật khẩu. SS 2022: mỗi phần (cách nhau bằng ":") là khoá base64 đúng cỡ — phần cuối là khoá người dùng, các
     * phần trước là khoá nhận dạng (khoá máy chủ của 3X-UI). Kiểu cũ: EVP_BytesToKey(MD5). Sai → IllegalArgumentException.
     */
    fun keys(m: SsMethod, password: String): List<ByteArray> {
        require(password.isNotEmpty()) { "Chưa nhập khoá / mật khẩu Shadowsocks." }
        if (!m.v2022) return listOf(evpBytesToKey(password.toByteArray(Charsets.UTF_8), m.keySize))
        val parts = password.split(':').map { it.trim() }
        require(parts.size == 1 || !m.chacha) {
            "${m.id} không dùng được với máy chủ nhiều người dùng (khoá dạng «khoá-máy-chủ:khoá-người-dùng») — chọn 2022-blake3-aes-256-gcm."
        }
        return parts.mapIndexed { i, p ->
            val k = SsBase64.decode(p)
            val which = if (parts.size == 1) "Khoá" else if (i == parts.size - 1) "Khoá người dùng" else "Khoá máy chủ"
            require(k != null && k.size == m.keySize) {
                "$which không đúng: ${m.id} cần khoá base64 dài ${m.keySize} byte (${(m.keySize + 2) / 3 * 4} ký tự, vd ${"A".repeat(4)}…=)."
            }
            k
        }
    }

    internal fun subkey(m: SsMethod, key: ByteArray, salt: ByteArray): ByteArray =
        if (m.v2022) Blake3.deriveKey("shadowsocks 2022 session subkey", key + salt, m.keySize)
        else hkdfSha1(key, salt, "ss-subkey".toByteArray(), m.keySize)

    internal fun evpBytesToKey(pass: ByteArray, size: Int): ByteArray {
        val md5 = MessageDigest.getInstance("MD5")
        val out = java.io.ByteArrayOutputStream()
        var prev = ByteArray(0)
        while (out.size() < size) {
            md5.update(prev)
            md5.update(pass)
            prev = md5.digest()
            out.write(prev)
        }
        return out.toByteArray().copyOf(size)
    }

    internal fun hkdfSha1(key: ByteArray, salt: ByteArray, info: ByteArray, size: Int): ByteArray {
        val mac = Mac.getInstance("HmacSHA1")
        mac.init(SecretKeySpec(salt, "HmacSHA1"))
        val prk = mac.doFinal(key)
        mac.init(SecretKeySpec(prk, "HmacSHA1"))
        val out = java.io.ByteArrayOutputStream()
        var t = ByteArray(0)
        var i = 1
        while (out.size() < size) {
            mac.update(t)
            mac.update(info)
            mac.update(i.toByte())
            t = mac.doFinal()
            out.write(t)
            i++
        }
        return out.toByteArray().copyOf(size)
    }

    /**
     * AES-GCM. Android: chỉ định thẳng Conscrypt ("AndroidOpenSSL") — Cipher tự chọn nhà cung cấp thì mỗi lần init lại dò lại,
     * đo trên TV box: 126 → 87 µs mỗi lần (mỗi chunk 2 lần). JVM (test): mặc định.
     */
    internal fun aesGcm(): Cipher = try {
        Cipher.getInstance("AES/GCM/NoPadding", "AndroidOpenSSL")
    } catch (e: GeneralSecurityException) {
        Cipher.getInstance("AES/GCM/NoPadding")
    }

    /** ChaCha20-Poly1305: JVM gọi "ChaCha20-Poly1305", Android 9+ (Conscrypt) gọi "ChaCha20/Poly1305/NoPadding". */
    internal fun chachaCipher(): Cipher = try {
        Cipher.getInstance("ChaCha20/Poly1305/NoPadding", "AndroidOpenSSL")
    } catch (e: GeneralSecurityException) {
        try {
            Cipher.getInstance("ChaCha20/Poly1305/NoPadding")
        } catch (e: GeneralSecurityException) {
            Cipher.getInstance("ChaCha20-Poly1305")
        }
    }

    /** Máy có hỗ trợ kiểu mã hoá này không (ChaCha20 cần Android 9 trở lên). */
    fun supported(m: SsMethod): Boolean = !m.chacha || runCatching { chachaCipher() }.isSuccess

    internal fun noReply(m: SsMethod) = "máy chủ Shadowsocks không trả lời — thường do sai khoá / sai kiểu mã hoá" +
        (if (m.v2022) ", hoặc giờ trên máy lệch quá 30 giây" else "") + " (kiểm tra lại link ss:// trong 3X-UI)"

    /** Địa chỉ đích dạng SOCKS5 (ATYP + địa chỉ + cổng). */
    internal fun target(host: String, ip: InetAddress?, port: Int, remoteDns: Boolean, resolve: (String) -> InetAddress): ByteArray {
        val a = ip ?: if (remoteDns) null else resolve(host)
        val addr = when {
            a is Inet4Address -> byteArrayOf(1) + a.address
            a != null -> byteArrayOf(4) + a.address
            else -> host.toByteArray(Charsets.US_ASCII).let {
                if (it.size > 255) throw SocksError(1, "tên miền quá dài")
                byteArrayOf(3, it.size.toByte()) + it
            }
        }
        return addr + byteArrayOf((port shr 8).toByte(), port.toByte())
    }

    /**
     * Mở kết nối tới host:port qua máy chủ c. Chưa gửi gì: phần đầu đi cùng dữ liệu đầu tiên (SsOutput). Máy chủ không xác
     * nhận kết nối (giao thức không có bước này) — sai khoá chỉ lộ ra khi đọc trả lời (SsInput báo SsError).
     */
    fun connect(c: SsConfig, host: String, ip: InetAddress?, port: Int, resolve: (String) -> InetAddress = Socks5::resolve): Upstream {
        val m = SsMethod.of(c.method) ?: throw SocksError(1, "kiểu mã hoá Shadowsocks «${c.method}» chưa hỗ trợ")
        if (!supported(m)) throw SocksError(1, "máy này không có ${m.id} (cần Android 9 trở lên) — đổi sang 2022-blake3-aes-256-gcm")
        val keys = try {
            keyCache(m, c.password)
        } catch (e: IllegalArgumentException) {
            throw SocksError(1, e.message ?: "khoá Shadowsocks không đúng")
        }
        val t = target(host, ip, port, c.remoteDns, resolve)
        val s = Socket()
        try {
            s.connect(InetSocketAddress(c.host, c.port), 10_000)
        } catch (e: Exception) {
            s.close()
            throw SocksError(1, if (e is UnknownHostException) "không tìm thấy máy chủ Shadowsocks ${c.host}" else "không kết nối được máy chủ Shadowsocks ${c.host}:${c.port}")
        }
        s.tcpNoDelay = true
        s.keepAlive = true
        val out = SsOutput(s.getOutputStream(), m, keys, t)
        return Upstream(s, SsInput(java.io.BufferedInputStream(s.getInputStream(), 32 * 1024), m, keys.last(), out.salt), out)
    }

    @Volatile private var cached: Triple<SsMethod, String, List<ByteArray>>? = null

    private fun keyCache(m: SsMethod, password: String): List<ByteArray> {
        cached?.let { if (it.first == m && it.second == password) return it.third }
        return keys(m, password).also { cached = Triple(m, password, it) }
    }

    /**
     * Nút "Kiểm tra proxy": hỏi Cloudflare (cdn-cgi/trace, HTTP — chặng mạng nhà → máy chủ đã mã hoá) qua máy chủ c,
     * trả IP ra ngoài kèm nơi đặt máy, vd "103.1.2.3 (SG)". SS 2022 không trả lời thì xem giờ trên máy có lệch không.
     */
    fun check(c: SsConfig): String {
        val host = "www.cloudflare.com"
        val up = connect(c.copy(sitesOnly = false), host, null, 80)
        try {
            up.socket.soTimeout = 15_000
            up.output.write("GET /cdn-cgi/trace HTTP/1.1\r\nHost: $host\r\nUser-Agent: ${Http.UA}\r\nConnection: close\r\n\r\n".toByteArray())
            val text = readHttp(up.input)
            val ip = Regex("""(?m)^ip=(\S+)""").find(text)?.groupValues?.get(1) ?: throw IOException("không đọc được kết quả từ $host")
            val loc = Regex("""(?m)^loc=(\S+)""").find(text)?.groupValues?.get(1)
            return ip + (loc?.let { " ($it)" } ?: "")
        } catch (e: IOException) {
            val m = SsMethod.of(c.method)
            if (e is SsError && m?.v2022 == true) clockSkew()?.let { throw SsError("giờ trên máy lệch $it giây — Shadowsocks 2022 cần giờ đúng (lệch tối đa 30 giây): chỉnh ngày giờ của máy") }
            throw if (e is SsError || e is EOFException) SsError(e.message ?: noReply(m ?: SsMethod.DEFAULT))
            else if (e is SocketTimeoutException) SsError(noReply(m ?: SsMethod.DEFAULT)) else e
        } finally {
            runCatching { up.socket.close() }
        }
    }

    /**
     * Đọc một trả lời HTTP: phần đầu + đủ Content-Length thì thôi, không chờ máy chủ đóng kết nối (Xray giữ thêm 1 giây sau khi
     * trang đã đóng — thời gian kiểm tra báo ra sẽ sai).
     */
    internal fun readHttp(i: InputStream): String {
        val buf = java.io.ByteArrayOutputStream()
        val b = ByteArray(4096)
        while (true) {
            val n = i.read(b)
            if (n < 0) break
            buf.write(b, 0, n)
            val s = buf.toString("ISO-8859-1")
            val h = s.indexOf("\r\n\r\n")
            if (h < 0) continue
            val len = Regex("""(?im)^content-length:\s*(\d+)""").find(s.substring(0, h))?.groupValues?.get(1)?.toIntOrNull() ?: continue
            if (buf.size() >= h + 4 + len) break
        }
        return buf.toString("UTF-8")
    }

    /** Giờ trên máy lệch bao nhiêu giây so với Cloudflare (đi thẳng, không qua proxy); null = không lệch quá 30 giây / không hỏi được. */
    private fun clockSkew(): Long? = runCatching {
        val conn = URL("https://www.cloudflare.com/cdn-cgi/trace").openConnection(java.net.Proxy.NO_PROXY) as HttpURLConnection
        conn.connectTimeout = 8000
        conn.readTimeout = 8000
        val text = conn.inputStream.use { it.readBytes().toString(Charsets.UTF_8) }
        val ts = Regex("""(?m)^ts=(\d+)""").find(text)!!.groupValues[1].toLong()
        abs(System.currentTimeMillis() / 1000 - ts).takeIf { it > 30 }
    }.getOrNull()
}
