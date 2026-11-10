package vn.chimto.tv

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.EOFException
import java.io.OutputStream
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.URLEncoder
import java.util.Base64
import java.util.concurrent.CompletableFuture
import javax.crypto.Cipher
import javax.crypto.spec.SecretKeySpec
import kotlin.concurrent.thread

class ShadowsocksTest {
    private fun hex(b: ByteArray) = b.joinToString("") { "%02x".format(it) }
    private fun unhex(s: String) = ByteArray(s.length / 2) { s.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    private fun input(n: Int) = ByteArray(n) { (it % 251).toByte() }
    private fun b64(b: ByteArray) = Base64.getEncoder().encodeToString(b)

    // Bộ số chuẩn của BLAKE3 (BLAKE3-team/BLAKE3 test_vectors/test_vectors.json — 32 byte đầu của hash / derive_key)
    @Test
    fun blake3OfficialVectors() {
        val ctx = "BLAKE3 2019-12-27 16:29:52 test vectors context"
        val cases = listOf(
            0 to ("af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262" to "2cc39783c223154fea8dfb7c1b1660f2ac2dcbd1c1de8277b0b0dd39b7e50d7d"),
            1 to ("2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213" to "b3e2e340a117a499c6cf2398a19ee0d29cca2bb7404c73063382693bf66cb06c"),
            63 to ("e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b" to "b6451e30b953c206e34644c6803724e9d2725e0893039cfc49584f991f451af3"),
            64 to ("4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98" to "a5c4a7053fa86b64746d4bb688d06ad1f02a18fce9afd3e818fefaa7126bf73e"),
            65 to ("de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee" to "51fd05c3c1cfbc8ed67d139ad76f5cf8236cd2acd26627a30c104dfd9d3ff8a8"),
            1023 to ("10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11" to "74a16c1c3d44368a86e1ca6df64be6a2f64cce8f09220787450722d85725dea5"),
            1024 to ("42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7" to "7356cd7720d5b66b6d0697eb3177d9f8d73a4a5c5e968896eb6a689684302706"),
        )
        for ((n, e) in cases) {
            assertEquals("hash $n", e.first, hex(Blake3.hash(input(n))))
            assertEquals("derive_key $n", e.second, hex(Blake3.deriveKey(ctx, input(n))))
        }
        // Đầu ra ngắn hơn = phần đầu của đầu ra 32 byte (khoá 16 byte của aes-128)
        assertEquals(cases[2].second.first.take(32), hex(Blake3.hash(input(63), 16)))
        assertEquals(cases[3].second.second.take(32), hex(Blake3.deriveKey(ctx, input(64), 16)))
    }

    // Số kiểm bằng Python (hashlib MD5 + cryptography HKDF-SHA1)
    @Test
    fun legacyKeyDerivation() {
        val k = Shadowsocks.evpBytesToKey("test-password".toByteArray(), 32)
        assertEquals("dfb450efddbb5387197c84460623675b69f2cebcd8ef520a3dfddef7c3d540b2", hex(k))
        assertEquals("dfb450efddbb5387197c84460623675b", hex(Shadowsocks.evpBytesToKey("test-password".toByteArray(), 16)))
        val salt = ByteArray(32) { it.toByte() }
        assertEquals(
            "7b6ca12d45ef79467432717aaa6a6ae6e1dd046a505fb2ebaaac602210919595",
            hex(Shadowsocks.subkey(SsMethod.AES_256, k, salt)),
        )
    }

    @Test
    fun base64BothAlphabets() {
        val raw = ByteArray(32) { (it * 37 + 251).toByte() }
        assertArrayEquals(raw, SsBase64.decode(Base64.getEncoder().encodeToString(raw)))
        assertArrayEquals(raw, SsBase64.decode(Base64.getUrlEncoder().withoutPadding().encodeToString(raw)))
        assertEquals(null, SsBase64.decode("abc!"))
        assertEquals(null, SsBase64.decode(""))
    }

    private val serverPsk = ByteArray(32) { (it + 1).toByte() }
    private val userPsk = ByteArray(32) { (100 + it).toByte() }

    @Test
    fun parse3xUi2022Link() {
        // 3X-UI: encodeURIComponent từng phần, nối bằng ":"
        val enc = { s: String -> URLEncoder.encode(s, "UTF-8") }
        val link = "ss://2022-blake3-aes-256-gcm:${enc(b64(serverPsk))}:${enc(b64(userPsk))}@vpn.example.com:24443?type=tcp#Nh%C3%A0"
        val c = SsLink.parse(link)
        assertEquals("vpn.example.com", c.host)
        assertEquals(24443, c.port)
        assertEquals("2022-blake3-aes-256-gcm", c.method)
        assertEquals("${b64(serverPsk)}:${b64(userPsk)}", c.password)
        val keys = Shadowsocks.keys(SsMethod.B3_AES_256, c.password)
        assertArrayEquals(serverPsk, keys[0])
        assertArrayEquals(userPsk, keys[1])
    }

    @Test
    fun parseSip002AndOldLinks() {
        val url = Base64.getUrlEncoder().withoutPadding()
        val a = SsLink.parse("ss://" + url.encodeToString("aes-256-gcm:mật khẩu".toByteArray()) + "@1.2.3.4:8388/?type=tcp#x")
        assertEquals(SsConfig("1.2.3.4", 8388, "aes-256-gcm", "mật khẩu"), a)
        val b = SsLink.parse("ss://" + b64("chacha20-poly1305:pa:ss@host.vn:443".toByteArray()) + "#tên")
        assertEquals(SsConfig("host.vn", 443, "chacha20-ietf-poly1305", "pa:ss"), b)
        val c = SsLink.parse("ss://" + url.encodeToString("2022-blake3-aes-128-gcm:${b64(ByteArray(16))}".toByteArray()) + "@[2001:db8::1]:8443")
        assertEquals("2001:db8::1", c.host)
        assertEquals(8443, c.port)
    }

    @Test
    fun parseRejectsUnsupported() {
        val ok = "ss://2022-blake3-aes-256-gcm:${b64(serverPsk)}@h:1"
        for ((link, word) in listOf(
            "$ok?type=ws&path=%2F" to "ws",
            "$ok?type=tcp&security=tls" to "tls",
            "$ok?plugin=obfs-local%3Bobfs%3Dhttp" to "plugin",
            "ss://2022-blake3-aes-256-gcm:${b64(ByteArray(16))}@h:1" to "32 byte",
            "ss://rc4-md5:abc@h:1" to "chưa hỗ trợ",
            "ss://2022-blake3-chacha20-poly1305:${b64(serverPsk)}:${b64(userPsk)}@h:1" to "nhiều người dùng",
        )) {
            try {
                SsLink.parse(link)
                fail("phải báo lỗi: $link")
            } catch (e: IllegalArgumentException) {
                assertTrue("$link → ${e.message}", e.message!!.contains(word))
            }
        }
    }

    /** Máy chủ thử (viết riêng theo đặc tả, không dùng SsInput / SsOutput): nhận một kết nối, trả lại "echo:" + dữ liệu. */
    private class TestServer(val m: SsMethod, val identity: ByteArray?, val users: List<ByteArray>) {
        val server = ServerSocket(0, 5, InetAddress.getByName("127.0.0.1"))
        val target = CompletableFuture<String>()
        val first = CompletableFuture<String>()

        fun start(): TestServer {
            thread(isDaemon = true) { runCatching { serve(server.accept()) }.onFailure { target.completeExceptionally(it) } }
            return this
        }

        private fun open(key: ByteArray, nonce: Long, c: ByteArray): ByteArray {
            val cipher = if (m.chacha) Cipher.getInstance("ChaCha20-Poly1305") else Cipher.getInstance("AES/GCM/NoPadding")
            val n = ByteArray(12)
            for (i in 0 until 8) n[i] = (nonce ushr (8 * i)).toByte()
            val spec = if (m.chacha) javax.crypto.spec.IvParameterSpec(n) else javax.crypto.spec.GCMParameterSpec(128, n)
            cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, if (m.chacha) "ChaCha20" else "AES"), spec)
            return cipher.doFinal(c)
        }

        private fun seal(key: ByteArray, nonce: Long, p: ByteArray): ByteArray {
            val cipher = if (m.chacha) Cipher.getInstance("ChaCha20-Poly1305") else Cipher.getInstance("AES/GCM/NoPadding")
            val n = ByteArray(12)
            for (i in 0 until 8) n[i] = (nonce ushr (8 * i)).toByte()
            val spec = if (m.chacha) javax.crypto.spec.IvParameterSpec(n) else javax.crypto.spec.GCMParameterSpec(128, n)
            cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, if (m.chacha) "ChaCha20" else "AES"), spec)
            return cipher.doFinal(p)
        }

        private fun subkey(psk: ByteArray, salt: ByteArray) = if (m.v2022) Blake3.deriveKey("shadowsocks 2022 session subkey", psk + salt, m.keySize)
        else Shadowsocks.hkdfSha1(psk, salt, "ss-subkey".toByteArray(), m.keySize)

        private fun serve(s: Socket): Unit = s.use {
            val i = DataInputStream(s.getInputStream())
            val o = s.getOutputStream()
            val salt = ByteArray(m.keySize).also { i.readFully(it) }
            var psk = users[0]
            if (identity != null) {
                val eih = ByteArray(16).also { i.readFully(it) }
                val ecb = Cipher.getInstance("AES/ECB/NoPadding")
                ecb.init(Cipher.DECRYPT_MODE, SecretKeySpec(Blake3.deriveKey("shadowsocks 2022 identity subkey", identity + salt, m.keySize), "AES"))
                val h = ecb.doFinal(eih)
                psk = users.firstOrNull { Blake3.hash(it, 16).contentEquals(h) } ?: return // không nhận ra người dùng: đóng
            }
            val key = subkey(psk, salt)
            var rn = 0L
            val payload = ByteArrayOutputStream()
            val addr: ByteArray
            if (m.v2022) {
                val fixed = open(key, rn++, ByteArray(27).also { i.readFully(it) })
                check(fixed[0].toInt() == 0)
                val ts = java.nio.ByteBuffer.wrap(fixed, 1, 8).long
                check(kotlin.math.abs(System.currentTimeMillis() / 1000 - ts) <= 30)
                val len = ((fixed[9].toInt() and 0xFF) shl 8) or (fixed[10].toInt() and 0xFF)
                val v = open(key, rn++, ByteArray(len + 16).also { i.readFully(it) })
                val alen = addrLen(v)
                addr = v.copyOfRange(0, alen)
                val pad = ((v[alen].toInt() and 0xFF) shl 8) or (v[alen + 1].toInt() and 0xFF)
                check(pad > 0 || v.size > alen + 2) // phải có dữ liệu hoặc đệm
                payload.write(v, alen + 2 + pad, v.size - alen - 2 - pad)
            } else {
                val l = open(key, rn++, ByteArray(18).also { i.readFully(it) })
                val len = ((l[0].toInt() and 0xFF) shl 8) or (l[1].toInt() and 0xFF)
                val v = open(key, rn++, ByteArray(len + 16).also { i.readFully(it) })
                val alen = addrLen(v)
                addr = v.copyOfRange(0, alen)
                payload.write(v, alen, v.size - alen)
            }
            target.complete(describe(addr))
            first.complete(String(payload.toByteArray()))
            // Trả lời: salt mới + (2022: phần đầu kèm độ dài) + "echo:" + dữ liệu đầu, rồi vang lại từng chunk tới khi hết
            val rsalt = Shadowsocks.random(m.keySize)
            val wkey = subkey(psk, rsalt)
            var wn = 0L
            val reply = "echo:".toByteArray() + payload.toByteArray()
            val out = ByteArrayOutputStream()
            out.write(rsalt)
            if (m.v2022) {
                val h = java.nio.ByteBuffer.allocate(1 + 8 + m.keySize + 2).put(1).putLong(System.currentTimeMillis() / 1000).put(salt)
                    .putShort(reply.size.toShort()).array()
                out.write(seal(wkey, wn++, h))
                out.write(seal(wkey, wn++, reply))
            } else {
                out.write(seal(wkey, wn++, byteArrayOf((reply.size shr 8).toByte(), reply.size.toByte())))
                out.write(seal(wkey, wn++, reply))
            }
            o.write(out.toByteArray())
            while (true) {
                val lc = ByteArray(18)
                try {
                    i.readFully(lc)
                } catch (e: EOFException) {
                    break
                }
                val l = open(key, rn++, lc)
                val n = ((l[0].toInt() and 0xFF) shl 8) or (l[1].toInt() and 0xFF)
                val data = open(key, rn++, ByteArray(n + 16).also { i.readFully(it) })
                writeChunk(o, wkey, wn, data)
                wn += 2
            }
        }

        private fun writeChunk(o: OutputStream, key: ByteArray, n: Long, data: ByteArray) {
            o.write(seal(key, n, byteArrayOf((data.size shr 8).toByte(), data.size.toByte())) + seal(key, n + 1, data))
        }

        private fun addrLen(v: ByteArray) = when (v[0].toInt()) {
            1 -> 1 + 4 + 2
            3 -> 1 + 1 + (v[1].toInt() and 0xFF) + 2
            4 -> 1 + 16 + 2
            else -> error("ATYP ${v[0]}")
        }

        private fun describe(a: ByteArray): String {
            val port = ((a[a.size - 2].toInt() and 0xFF) shl 8) or (a[a.size - 1].toInt() and 0xFF)
            val host = when (a[0].toInt()) {
                3 -> String(a, 2, a[1].toInt() and 0xFF)
                else -> InetAddress.getByAddress(a.copyOfRange(1, a.size - 2)).hostAddress
            }
            return "$host:$port"
        }
    }

    private fun roundTrip(m: SsMethod, password: String, server: TestServer, initial: String) {
        val c = SsConfig("127.0.0.1", server.server.localPort, m.id, password)
        val up = Shadowsocks.connect(c, "phim.example", null, 443)
        up.socket.soTimeout = 5000
        up.output.write(initial.toByteArray())
        assertEquals("phim.example:443", server.target.get())
        assertEquals(initial, server.first.get())
        val buf = ByteArray(5 + initial.toByteArray().size)
        DataInputStream(up.input).readFully(buf)
        assertEquals("echo:$initial", String(buf))
        // Chunk lớn (quá 0x3FFF với kiểu cũ → chia nhiều chunk)
        val big = ByteArray(40_000) { (it % 97).toByte() }
        up.output.write(big)
        val back = ByteArray(big.size)
        DataInputStream(up.input).readFully(back)
        assertArrayEquals(big, back)
        up.socket.shutdownOutput()
        assertEquals(-1, up.input.read())
        up.socket.close()
    }

    @Test
    fun ss2022MultiUserRoundTrip() {
        val other = ByteArray(32) { 7 }
        val s = TestServer(SsMethod.B3_AES_256, serverPsk, listOf(other, userPsk)).start()
        roundTrip(SsMethod.B3_AES_256, "${b64(serverPsk)}:${b64(userPsk)}", s, "GET / HTTP/1.1\r\n\r\n")
    }

    @Test
    fun ss2022Aes128SingleUser() {
        val k = ByteArray(16) { (it * 3).toByte() }
        val s = TestServer(SsMethod.B3_AES_128, null, listOf(k)).start()
        roundTrip(SsMethod.B3_AES_128, b64(k), s, "xin chào")
    }

    @Test
    fun ss2022ChachaSingleUser() {
        val s = TestServer(SsMethod.B3_CHACHA, null, listOf(userPsk)).start()
        roundTrip(SsMethod.B3_CHACHA, b64(userPsk), s, "chacha")
    }

    @Test
    fun legacyAeadRoundTrip() {
        for (m in listOf(SsMethod.AES_256, SsMethod.AES_128, SsMethod.CHACHA)) {
            val key = Shadowsocks.evpBytesToKey("mật khẩu".toByteArray(), m.keySize)
            val s = TestServer(m, null, listOf(key)).start()
            roundTrip(m, "mật khẩu", s, "legacy ${m.id}")
        }
    }

    @Test
    fun wrongUserKeyReportsClearError() {
        val s = TestServer(SsMethod.B3_AES_256, serverPsk, listOf(userPsk)).start()
        val c = SsConfig("127.0.0.1", s.server.localPort, SsMethod.B3_AES_256.id, "${b64(serverPsk)}:${b64(ByteArray(32) { 9 })}")
        val up = Shadowsocks.connect(c, "phim.example", null, 443)
        up.socket.soTimeout = 5000
        up.output.write("hello".toByteArray())
        try {
            up.input.read()
            fail("phải báo lỗi")
        } catch (e: SsError) {
            assertTrue(e.message, e.message!!.contains("sai khoá"))
        }
        up.socket.close()
    }

    /** Cổng nội bộ đi qua Shadowsocks: bên gọi nói SOCKS5 với cổng nội bộ, máy chủ thử nhận đúng đích + dữ liệu. */
    @Test
    fun bridgeRelaysThroughShadowsocks() {
        val s = TestServer(SsMethod.B3_AES_256, serverPsk, listOf(userPsk)).start()
        val cfg = SsConfig("127.0.0.1", s.server.localPort, SsMethod.B3_AES_256.id, "${b64(serverPsk)}:${b64(userPsk)}")
        val bridge = Socks5Bridge({ RouteConfig(null, dns = false, split = true, ss = cfg) }, { emptyList() })
        try {
            Socket().use { k ->
                k.connect(InetSocketAddress("127.0.0.1", bridge.port), 3000)
                k.soTimeout = 5000
                val i = DataInputStream(k.getInputStream())
                val o = k.getOutputStream()
                o.write(byteArrayOf(5, 1, 0))
                assertArrayEquals(byteArrayOf(5, 0), ByteArray(2).also { i.readFully(it) })
                val host = "phim.example".toByteArray()
                o.write(byteArrayOf(5, 1, 0, 3, host.size.toByte()) + host + byteArrayOf(1, 0xBB.toByte()))
                assertEquals(0, ByteArray(10).also { i.readFully(it) }[1].toInt())
                // Gói bắt tay TLS giả: qua Shadowsocks không chia nhỏ — tới máy chủ nguyên một mẩu, đúng thứ tự
                val hello = byteArrayOf(0x16, 0x03, 0x01) + "phim.example".toByteArray()
                o.write(hello)
                assertEquals("phim.example:443", s.target.get())
                assertArrayEquals(hello, s.first.get().toByteArray(Charsets.ISO_8859_1))
                val back = ByteArray(5 + hello.size).also { i.readFully(it) }
                assertEquals("echo:", String(back, 0, 5))
            }
        } finally {
            bridge.close()
        }
    }

    /** Bên gọi chưa gửi gì (máy chủ nói trước): sau 0,3 s cổng nội bộ gửi phần đầu (có đệm) để máy chủ mở kết nối. */
    @Test
    fun bridgeSendsHeaderWhenClientIsSilent() {
        val s = TestServer(SsMethod.B3_AES_256, serverPsk, listOf(userPsk)).start()
        val cfg = SsConfig("127.0.0.1", s.server.localPort, SsMethod.B3_AES_256.id, "${b64(serverPsk)}:${b64(userPsk)}")
        val bridge = Socks5Bridge({ RouteConfig(null, dns = false, ss = cfg) }, { emptyList() })
        try {
            Socket().use { k ->
                k.connect(InetSocketAddress("127.0.0.1", bridge.port), 3000)
                k.soTimeout = 5000
                val i = DataInputStream(k.getInputStream())
                val o = k.getOutputStream()
                o.write(byteArrayOf(5, 1, 0))
                i.readFully(ByteArray(2))
                o.write(byteArrayOf(5, 1, 0, 1, 10, 0, 0, 1, 0, 25))
                i.readFully(ByteArray(10))
                assertEquals("10.0.0.1:25", s.target.get())
                assertEquals("", s.first.get())
            }
        } finally {
            bridge.close()
        }
    }
}
