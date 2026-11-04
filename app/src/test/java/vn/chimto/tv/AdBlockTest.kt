package vn.chimto.tv

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AdBlockTest {
    @Test
    fun blocksTrackers() {
        assertTrue(AdBlock.isAd("static.cloudflareinsights.com"))
        assertTrue(AdBlock.isAd("prd.jwpltx.com"))
        assertTrue(AdBlock.isAd("www.googletagmanager.com"))
        assertTrue(AdBlock.isAd("ACSCDN.COM"))
        assertTrue(AdBlock.isAd("atroblxac.in"))
    }

    /** Phim của trang khác: tên ngắn; hai trang trùng tên ngắn (hoathinh3d.ee / .pw) thì ghi đủ tên miền. */
    @Test
    fun siteTags() {
        val sites = listOf("https://yanhh3d.ee", "https://hoathinh3d.pw", "https://hoathinh3d.ee", "https://hhkungfu.ee")
        assertTrue(siteTag("hhkungfu.ee", sites) == "hhkungfu")
        assertTrue(siteTag("hoathinh3d.pw", sites) == "hoathinh3d.pw")
        assertTrue(siteTag("hoathinh3d.ee", sites) == "hoathinh3d.ee")
        assertTrue(siteTag("hoathinh3d.ee", listOf("https://hoathinh3d.ee")) == "hoathinh3d")
    }

    /** Thứ trình phát cần thì không được chặn. */
    @Test
    fun keepsPlayers() {
        assertFalse(AdBlock.isAd("ssl.p.jwpcdn.com"))
        assertFalse(AdBlock.isAd("entitlements.jwplayer.com"))
        assertFalse(AdBlock.isAd("p16-ad-sg.ibyteimg.com"))
        assertFalse(AdBlock.isAd("cdnjs.cloudflare.com"))
        assertFalse(AdBlock.isAd("www.facebook.com"))
        assertFalse(AdBlock.isAd(null))
    }
}
