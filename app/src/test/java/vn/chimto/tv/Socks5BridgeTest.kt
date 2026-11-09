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
        Socks5Bridge({ RouteConfig(c, null) }, { sites }).also { b -> closers += { b.close() } }

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
    fun unblockKeepsLocalhostDirect() {
        // Vượt chặn bật, không proxy: localhost (127.0.0.1) không bị coi là DNS chặn, không hỏi DNS mã hoá
        val b = Socks5Bridge({ RouteConfig(null, null, split = true) }, { emptyList() }).also { b -> closers += { b.close() } }
        assertEquals("echo:hi", call(b, "localhost"))
    }
}
