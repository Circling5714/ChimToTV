package vn.chimto.tv

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.io.DataInputStream
import java.io.IOException
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Proxy
import java.net.ServerSocket
import java.net.Socket
import java.util.Collections
import kotlin.concurrent.thread

/** Cổng SOCKS5 nội bộ: đi qua một proxy SOCKS5 giả (ghi lại đích được hỏi) tới một máy "echo" trên máy thử. */
class Socks5BridgeTest {
    /** Máy đích: đọc một dòng, trả lại "echo:<dòng>". */
    private val echo = ServerSocket(0, 50, InetAddress.getByName("127.0.0.1"))

    /** Proxy giả: login = (user, pass) bắt buộc hay null; mọi CONNECT đều nối tới máy echo, ghi lại đích được hỏi. */
    private inner class FakeProxy(private val login: Pair<String, String>?) {
        val server = ServerSocket(0, 50, InetAddress.getByName("127.0.0.1"))
        val asked: MutableList<String> = Collections.synchronizedList(ArrayList())

        init {
            thread(isDaemon = true) {
                while (!server.isClosed) {
                    val s = runCatching { server.accept() }.getOrNull() ?: break
                    thread(isDaemon = true) { runCatching { serve(s) }; s.close() }
                }
            }
        }

        private fun serve(s: Socket) {
            val i = DataInputStream(s.getInputStream())
            val o = s.getOutputStream()
            i.readUnsignedByte()
            val methods = ByteArray(i.readUnsignedByte()).also { i.readFully(it) }.map { it.toInt() }
            if (login == null) {
                o.write(byteArrayOf(5, 0))
            } else {
                if (2 !in methods) return o.write(byteArrayOf(5, 0xFF.toByte()))
                o.write(byteArrayOf(5, 2))
                i.readUnsignedByte()
                val u = String(ByteArray(i.readUnsignedByte()).also { i.readFully(it) })
                val p = String(ByteArray(i.readUnsignedByte()).also { i.readFully(it) })
                val ok = u == login.first && p == login.second
                o.write(byteArrayOf(1, if (ok) 0 else 1))
                if (!ok) return
            }
            i.readUnsignedByte(); i.readUnsignedByte(); i.readUnsignedByte()
            val target = when (i.readUnsignedByte()) {
                1 -> "ip:" + InetAddress.getByAddress(ByteArray(4).also { i.readFully(it) }).hostAddress
                3 -> "name:" + String(ByteArray(i.readUnsignedByte()).also { i.readFully(it) })
                else -> "ip6"
            }
            asked += target + ":" + i.readUnsignedShort()
            val up = Socket("127.0.0.1", echo.localPort)
            o.write(byteArrayOf(5, 0, 0, 1, 0, 0, 0, 0, 0, 0))
            thread(isDaemon = true) { runCatching { up.getInputStream().copyTo(o) } }
            runCatching { i.copyTo(up.getOutputStream()) }
            up.close()
        }
    }

    private val closers = ArrayList<() -> Unit>()

    @Before
    fun startEcho() {
        thread(isDaemon = true) {
            while (!echo.isClosed) {
                val s = runCatching { echo.accept() }.getOrNull() ?: break
                thread(isDaemon = true) {
                    runCatching {
                        val line = s.getInputStream().bufferedReader().readLine()
                        s.getOutputStream().write("echo:$line\n".toByteArray())
                    }
                    s.close()
                }
            }
        }
    }

    @After
    fun stop() {
        closers.forEach { runCatching(it) }
        echo.close()
    }

    private fun proxy(login: Pair<String, String>? = null) = FakeProxy(login).also { p -> closers += { p.server.close() } }

    private fun bridge(c: SocksConfig, sites: List<String> = emptyList()) =
        Socks5Bridge({ RouteConfig(c, false) }, { sites }).also { b -> closers += { b.close() } }

    /** Gọi qua cổng nội bộ như Http.kt (Java tự gửi tên miền chưa phân giải), trả lời của máy echo. */
    private fun call(b: Socks5Bridge, host: String): String {
        Socket(Proxy(Proxy.Type.SOCKS, InetSocketAddress("127.0.0.1", b.port))).use { s ->
            s.soTimeout = 5000
            s.connect(InetSocketAddress.createUnresolved(host, echo.localPort), 5000)
            s.getOutputStream().write("hi\n".toByteArray())
            return s.getInputStream().bufferedReader().readLine()
        }
    }

    @Test
    fun loginAndProxyDns() {
        val p = proxy("anh" to "mk123")
        val b = bridge(SocksConfig("127.0.0.1", p.server.localPort, "anh", "mk123", remoteDns = true))
        assertEquals("echo:hi", call(b, "phim.test"))
        // DNS của proxy: proxy nhận tên miền, thiết bị không phân giải
        assertEquals(listOf("name:phim.test:${echo.localPort}"), p.asked)
    }

    @Test
    fun deviceDns() {
        val p = proxy()
        val b = bridge(SocksConfig("127.0.0.1", p.server.localPort, remoteDns = false))
        assertEquals("echo:hi", call(b, "localhost"))
        // DNS của thiết bị: tự phân giải, proxy chỉ nhận IP
        assertEquals(listOf("ip:127.0.0.1:${echo.localPort}"), p.asked)
    }

    @Test
    fun wrongPasswordIsReported() {
        val p = proxy("anh" to "mk123")
        val b = bridge(SocksConfig("127.0.0.1", p.server.localPort, "anh", "sai"))
        try {
            call(b, "phim.test")
            fail("phải lỗi")
        } catch (e: IOException) {
            assertTrue(b.lastError!!, b.lastError!!.contains("sai tên đăng nhập"))
        }
    }

    @Test
    fun proxyNeedsLogin() {
        val p = proxy("anh" to "mk123")
        val b = bridge(SocksConfig("127.0.0.1", p.server.localPort))
        runCatching { call(b, "phim.test") }
        assertTrue(b.lastError!!, b.lastError!!.contains("cần tên đăng nhập"))
    }

    @Test
    fun deadProxyIsReportedNotBypassed() {
        val dead = ServerSocket(0).let { val port = it.localPort; it.close(); port }
        val b = bridge(SocksConfig("127.0.0.1", dead))
        assertTrue(runCatching { call(b, "localhost") }.isFailure) // không lén đi thẳng
        assertTrue(b.lastError!!, b.lastError!!.contains("không kết nối được proxy"))
    }

    @Test
    fun sitesOnlyGoesDirectForOtherHosts() {
        val p = proxy()
        val b = bridge(SocksConfig("127.0.0.1", p.server.localPort, sitesOnly = true), sites = listOf("phim.test"))
        // Trang phim (kể cả tên miền con) qua proxy; nơi khác đi thẳng
        assertEquals("echo:hi", call(b, "www.phim.test"))
        assertEquals("echo:hi", call(b, "localhost"))
        assertEquals(listOf("name:www.phim.test:${echo.localPort}"), p.asked)
    }

    @Test
    fun blockedDnsAnswers() {
        // DNS chặn trả địa chỉ của chính máy → Vượt chặn hỏi lại qua DNS mã hoá
        for (a in listOf("0.0.0.0", "::", "127.0.0.1", "::1")) assertTrue(a, Socks5.blockedAnswer(InetAddress.getByName(a)))
        for (a in listOf("104.21.35.220", "2606:4700:3035::6815:23dc", "10.0.0.1")) assertFalse(a, Socks5.blockedAnswer(InetAddress.getByName(a)))
    }

    @Test
    fun blockedWhenAnyAddressIsBlocked() {
        // Câu trả lời lẫn địa chỉ chặn với địa chỉ thật: cả câu trả lời bị coi là chặn
        val mixed = listOf("203.113.131.1", "127.0.0.1").map { InetAddress.getByName(it) }
        assertTrue(Socks5.blockedAnswer(mixed))
        assertFalse(Socks5.blockedAnswer(listOf("104.21.35.220", "172.67.180.59").map { InetAddress.getByName(it) }))
    }

    @Test
    fun sameAnswerForDifferentSitesIsSuspicious() {
        Socks5.forgetAnswers()
        val warn = listOf(InetAddress.getByName("203.0.113.9"))
        assertFalse(Socks5.sharedAnswer("phimmoi.test", warn))
        assertFalse(Socks5.sharedAnswer("img.phimmoi.test", warn)) // cùng tên miền gốc
        assertTrue(Socks5.sharedAnswer("www.kkphim.test", warn)) // trang khác, cùng địa chỉ → kiểu trang cảnh báo
        assertTrue(Socks5.sharedAnswer("phimmoi.test", warn)) // từ giờ trang đầu tiên cũng hỏi lại
        assertFalse(Socks5.sharedAnswer("khac.test", listOf(InetAddress.getByName("198.51.100.7"))))
        Socks5.forgetAnswers()
    }

    @Test
    fun baseDomains() {
        assertEquals("phimmoi.net", baseDomain("img.phimmoi.net"))
        assertEquals("abc.com.vn", baseDomain("www.abc.com.vn"))
        assertEquals("kkphim.com", baseDomain("www.kkphim.com."))
        assertEquals("1.2.3.4", baseDomain("1.2.3.4"))
    }

    @Test
    fun level2MemoKeepsSevenDays() {
        val now = 1_000_000_000_000L
        Level2Memo.load(mapOf("phimmoi.test" to now - Level2Memo.KEEP_MS - 1, "kkphim.test" to now - 1000), now)
        assertFalse(Level2Memo.needs("phimmoi.test", now)) // quá 7 ngày: về Mức 1
        assertTrue(Level2Memo.needs("www.kkphim.test", now)) // theo tên miền gốc
        Level2Memo.clear()
    }

    /** Ghép các đoạn của splitPlan thành chữ, ngăn bằng «|». */
    private fun plan(text: String, host: String): String {
        val b = text.toByteArray(Charsets.ISO_8859_1)
        return splitPlan(b, b.size, host).joinToString("|") { (s, n) -> String(b, s, n, Charsets.ISO_8859_1) }
    }

    @Test
    fun httpRequestIsSplitAroundHost() {
        val req = "GET /phim HTTP/1.1\r\nHost: phim.test\r\nAccept: */*\r\n\r\n"
        assertEquals("G|ET /phim HTTP/1.1\r\nHo|st: |ph|im|.t|es|t|\r\nAccept: */*\r\n\r\n", plan(req, "phim.test"))
        // Gửi đủ, đúng thứ tự
        assertEquals(req, plan(req, "phim.test").replace("|", ""))
        // Không phải HTTP / TLS: gửi nguyên
        assertEquals("SSH-2.0-x\r\n", plan("SSH-2.0-x\r\n", "phim.test"))
    }

    @Test
    fun tlsHelloIsSplitAroundName() {
        val hello = ByteArray(80).also { it[0] = 0x16; it[1] = 0x03; it[2] = 0x01; it[4] = 75 }
        "phim.test".toByteArray().copyInto(hello, 40)
        val p = splitPlan(hello, hello.size, "phim.test")
        assertEquals(0 to 1, p.first())
        assertEquals(listOf(40 to 2, 42 to 2, 44 to 2, 46 to 2, 48 to 1), p.filter { it.first in 40..48 })
        assertEquals(hello.size, p.sumOf { it.second })
    }

    /**
     * Máy đích giả nhà mạng chặn ở Mức 1: kết nối đầu nhận gói bắt tay nhưng không trả lời (treo); kết nối sau trả lời
     * ngay. Cổng nội bộ phải tự mở lại và gửi lại gói đó — bên gọi nhận được trả lời như không có gì.
     */
    @Test
    fun hangingAtLevel1RetriesAtLevel2() {
        val seen = Collections.synchronizedList(ArrayList<Int>())
        val target = ServerSocket(0, 50, InetAddress.getByName("127.0.0.1"))
        closers += { target.close() }
        thread(isDaemon = true) {
            var k = 0
            while (!target.isClosed) {
                val s = runCatching { target.accept() }.getOrNull() ?: break
                val first = k++ == 0
                thread(isDaemon = true) {
                    runCatching {
                        val buf = ByteArray(4096)
                        var n = 0
                        while (n < 80) n += s.getInputStream().read(buf, n, buf.size - n).also { if (it < 0) return@runCatching }
                        seen += n
                        if (first) Thread.sleep(3000) else s.getOutputStream().write("hello-back".toByteArray())
                    }
                    if (!first) Thread.sleep(200)
                    s.close()
                }
            }
        }
        Level2Memo.clear()
        var ttlCalls = 0
        val b = Socks5Bridge({ RouteConfig(null, false, split = true) }, { emptyList() }).also { b -> closers += { b.close() } }
        b.firstReplyMs = 500
        b.setTtl = { _, _ -> ttlCalls++; true } // giả đặt được TTL (trên máy tính không đặt được)
        val hello = ByteArray(80).also { it[0] = 0x16; it[1] = 0x03; it[2] = 0x01; it[4] = 75 }
        "localhost".toByteArray().copyInto(hello, 40)
        Socket(Proxy(Proxy.Type.SOCKS, InetSocketAddress("127.0.0.1", b.port))).use { s ->
            s.soTimeout = 5000
            s.connect(InetSocketAddress.createUnresolved("localhost", target.localPort), 5000)
            s.getOutputStream().write(hello)
            val got = ByteArray(10)
            DataInputStream(s.getInputStream()).readFully(got)
            assertEquals("hello-back", String(got))
        }
        assertEquals(listOf(80, 80), seen) // gửi lại đủ gói bắt tay ở kết nối thứ hai
        assertEquals(1, b.autoLevel2)
        assertEquals(2, ttlCalls) // Mức 2: byte đầu TTL 1 rồi trả TTL mặc định
        assertTrue(Level2Memo.needs("localhost"))
        Level2Memo.clear()
    }

    @Test
    fun unblockKeepsLocalhostDirect() {
        // Vượt chặn bật, không proxy: localhost (127.0.0.1) không bị coi là DNS chặn, không hỏi DNS mã hoá
        val b = Socks5Bridge({ RouteConfig(null, false, split = true) }, { emptyList() }).also { b -> closers += { b.close() } }
        assertEquals("echo:hi", call(b, "localhost"))
    }
}
