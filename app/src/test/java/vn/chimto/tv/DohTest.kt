package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** DNS mã hoá (11/2026): gói DNS, trả lời HTTP, thứ tự máy chủ, mặc định mới — cùng ca với DohTests.cs / Socks5Tests.swift. */
class DohTest {
    private fun hex(s: String) = s.replace(" ", "").chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    @Test
    fun queryForA() {
        // id 0, RD, 1 câu hỏi: "a.bc" kiểu A lớp IN
        assertEquals("0000 0100 0001 0000 0000 0000 01 61 02 6263 00 0001 0001".replace(" ", ""),
            DnsWire.query("a.bc").joinToString("") { "%02x".format(it) })
    }

    @Test
    fun answerWithCnameAndCompression() {
        // Câu hỏi www.x.vn; trả lời: CNAME (nén, trỏ về câu hỏi) rồi 2 bản ghi A — lấy A đầu, TTL nhỏ nhất
        val reply = hex(
            "0000 8180 0001 0003 0000 0000" +
                "03 777777 01 78 02 766e 00 0001 0001" +
                "c00c 0005 0001 00000e10 0004 c010 c010" + // CNAME → con trỏ (độ dài 4 cho gọn)
                "c010 0001 0001 0000012c 0004 68 15 23 dc" + // 104.21.35.220, TTL 300
                "c010 0001 0001 0000003c 0004 ac 43 b4 3b", // 172.67.180.59, TTL 60
        )
        val a = DnsWire.answer(reply, 0) as Doh.Answer.Found
        assertEquals("104.21.35.220", a.addr.hostAddress)
        assertEquals(60, a.ttl)
        assertEquals(listOf("104.21.35.220", "172.67.180.59"), a.all.map { it.hostAddress }) // mọi bản ghi A (máy chủ NextDNS gần nhất)
    }

    @Test
    fun noDomainNoIpv4AndWrongId() {
        val nx = hex("0000 8183 0001 0000 0000 0000 01 78 00 0001 0001")
        assertTrue(DnsWire.answer(nx, 0) is Doh.Answer.NoDomain)
        val empty = hex("0000 8180 0001 0000 0000 0000 01 78 00 0001 0001")
        assertTrue(DnsWire.answer(empty, 0) is Doh.Answer.NoIpv4)
        assertTrue(DnsWire.answer(empty, 0x1234) is Doh.Answer.Failed) // DoT: id khác → bỏ
        val servfail = hex("0000 8182 0001 0000 0000 0000 01 78 00 0001 0001")
        assertTrue(DnsWire.answer(servfail, 0) is Doh.Answer.Failed)
    }

    @Test
    fun httpBodyPlainAndChunked() {
        val plain = "HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: 3\r\n\r\nabc".toByteArray()
        assertEquals("abc", String(DohHttp.body(plain)!!))
        val chunked = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n3;x=1\r\ncde\r\n0\r\n\r\n".toByteArray()
        assertEquals("abcde", String(DohHttp.body(chunked)!!))
        assertNull(DohHttp.body("HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n".toByteArray()))
    }

    @Test
    fun chainOrder() {
        // DoH NextDNS → Cloudflare → Google, rồi DoT cũng vậy; không có mã NextDNS thì bỏ NextDNS
        assertEquals(listOf("DoH NextDNS", "DoH Cloudflare", "DoH Google", "DoT NextDNS", "DoT Cloudflare", "DoT Google"),
            Doh.chain("abc123").map { it.label })
        assertEquals("/abc123", Doh.chain("abc123")[0].path)
        assertEquals("abc123.dns.nextdns.io", Doh.chain("abc123")[3].host)
        assertEquals(listOf("DoH Cloudflare", "DoH Google", "DoT Cloudflare", "DoT Google"), Doh.chain("").map { it.label })
    }

    @Test
    fun deadServersFallBackToDevice() {
        // Mọi máy chủ không tới được (cổng đóng trên máy) → null: bên gọi dùng DNS của thiết bị, không ném lỗi
        val dead = listOf(Doh.Server("Chết", Doh.Kind.DOT, "dns.test", "", listOf("127.0.0.1")))
        assertNull(Doh.resolve("phim.test", dead))
        assertEquals("127.0.0.1", Doh.resolve("localhost", dead)!!.hostAddress) // không hỏi DNS cho localhost / IP
    }

    @Test
    fun newNetDefaults() {
        // Máy cài mới / chưa chỉnh: bật Vượt chặn mức 1 + DNS mã hoá
        Prefs.init(MemoryPrefs())
        Prefs.applyNetDefaults()
        assertTrue(Prefs.unblock && Prefs.unblockLevel == 1 && Prefs.secureDns)
        // Máy đang dùng: tắt Vượt chặn → bản mới bật một lần; đang mức 2 thì giữ; sau đó tắt thì giữ tắt
        val sp = MemoryPrefs().apply { map["unblock"] = false }
        Prefs.init(sp)
        Prefs.applyNetDefaults()
        assertTrue(Prefs.unblock && Prefs.secureDns)
        Prefs.unblock = false
        Prefs.secureDns = false
        Prefs.applyNetDefaults()
        assertFalse(Prefs.unblock || Prefs.secureDns)
        val lv2 = MemoryPrefs().apply { map["unblock"] = true; map["unblock_level"] = 2; map["doh"] = "GOOGLE" }
        Prefs.init(lv2)
        Prefs.applyNetDefaults()
        assertTrue(Prefs.unblockLevel == 2 && Prefs.secureDns)
    }

    @Test
    fun keepAliveReadsOneReplyAtATime() {
        // Hai trả lời liền nhau trên một kết nối (keep-alive): đọc đúng từng cái, kết nối còn dùng lại được
        val two = ("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc" +
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nde\r\n0\r\n\r\n" +
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\nf").toByteArray()
        val i = java.io.BufferedInputStream(java.io.ByteArrayInputStream(two))
        val r1 = DohHttp.read(i)
        assertEquals("abc", String(r1.body))
        assertTrue(r1.keepAlive)
        val r2 = DohHttp.read(i)
        assertEquals("de", String(r2.body))
        assertTrue(r2.keepAlive)
        val r3 = DohHttp.read(i)
        assertEquals("f", String(r3.body))
        assertFalse(r3.keepAlive) // máy chủ báo đóng → không dùng lại
    }

    @Test
    fun onlyNextDnsIsSteered() {
        // Chỉ NextDNS lấy IP theo máy chủ gần nhất; Cloudflare / Google dùng IP cố định
        assertEquals(listOf(true, false, false, true, false, false), Doh.chain("abc123").map { it.steer })
        assertEquals(listOf("1.1.1.1", "1.0.0.1"), Doh.ipsFor(Doh.chain("abc123")[4]))
    }
}
