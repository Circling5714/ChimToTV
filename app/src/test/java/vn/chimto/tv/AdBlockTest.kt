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
