package vn.chimto.tv

import org.junit.Assume.assumeTrue
import org.junit.Test

/**
 * Thử với máy chủ Shadowsocks thật (3X-UI / Xray…): chỉ chạy khi có biến môi trường SS_TEST_LINK (link ss://, có thể
 * nhiều link cách nhau bằng dấu cách). Không đưa link thật vào code.
 */
class SsInteropTest {
    @Test
    fun checkRealServer() {
        val links = System.getenv("SS_TEST_LINK")?.split(' ')?.filter { it.isNotBlank() }.orEmpty()
        assumeTrue("không có SS_TEST_LINK", links.isNotEmpty())
        for (l in links) {
            val c = SsLink.parse(l)
            val t = System.currentTimeMillis()
            val ip = Shadowsocks.check(c)
            println("SS ${c.method} → IP ra ngoài $ip (${System.currentTimeMillis() - t} ms)")
        }
    }
}
