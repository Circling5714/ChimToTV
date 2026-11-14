package vn.chimto.tv

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Cùng ca với test của lõi Rust (web_fetch.rs). */
class WebFetchTest {
    @Test
    fun detectsCloudflareChallenge() {
        assertTrue(WebFetch.isChallenge(403, "challenge", null))
        assertTrue(WebFetch.isChallenge(200, " Challenge ", null)) // đầu thư là đủ
        val page = "<!DOCTYPE html><html lang=\"en-US\"><head><title>Just a moment...</title>".toByteArray()
        assertTrue(WebFetch.isChallenge(403, null, page))
        assertTrue(WebFetch.isChallenge(503, null, "<script>window._cf_chl_opt={cvId:'3'}</script>".toByteArray()))
        // Lỗi thật của trang / trang thường có mã JS của Cloudflare (jsd) thì không phải lớp chặn
        assertFalse(WebFetch.isChallenge(403, null, "<html><title>403 Forbidden</title></html>".toByteArray()))
        assertFalse(WebFetch.isChallenge(404, null, page))
        assertFalse(WebFetch.isChallenge(200, null, "<script src=\"/cdn-cgi/challenge-platform/scripts/jsd/main.js\"></script>".toByteArray()))
        assertFalse(WebFetch.isChallenge(403, null, null))
    }

    @Test
    fun detectsCloudflareBlockPage() {
        val block = "<!DOCTYPE html><head><title>Attention Required! | Cloudflare</title></head><div id=\"cf-error-details\">".toByteArray()
        assertTrue(WebFetch.isBlockPage(403, block))
        assertFalse(WebFetch.isBlockPage(200, block))
        assertFalse(WebFetch.isBlockPage(403, "<title>Just a moment...</title>".toByteArray()))
        assertFalse(WebFetch.isBlockPage(403, null))
    }

    @Test
    fun remembersBlockedHostsFor12Hours() {
        val now = 1_800_000_000_000L
        WebFetch.remember("HoatHinh3D.pw", now)
        assertTrue(WebFetch.needs("https://hoathinh3d.pw/phim/abc", now + 1000))
        assertTrue(WebFetch.needs("https://hoathinh3d.pw/", now + WebFetch.KEEP_MS - 1))
        assertFalse(WebFetch.needs("https://hoathinh3d.pw/", now + WebFetch.KEEP_MS))
        assertFalse(WebFetch.needs("https://hoathinh3d.ee/", now + 1000))
        assertFalse(WebFetch.needs("không phải địa chỉ", now))
        assertEquals("hoathinh3d.pw", WebFetch.hostOf("https://HoatHinh3D.pw/xem-phim/a?b=1"))
        WebFetch.forgetAll()
    }

    @Test
    fun readsHeadOfErrorStream() {
        val big = ByteArray(20_000) { 'a'.code.toByte() }
        assertEquals(8192, WebFetch.head(big.inputStream())!!.size)
        assertEquals(3, WebFetch.head("abc".byteInputStream())!!.size)
        assertEquals(null, WebFetch.head(null))
    }
}
