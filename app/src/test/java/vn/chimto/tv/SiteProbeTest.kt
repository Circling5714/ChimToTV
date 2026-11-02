package vn.chimto.tv

import org.jsoup.Jsoup
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File

/**
 * Chạy bộ đọc trang trên HTML đã tải về từ các trang thật và in kết quả để soát.
 * Không chạy mặc định; cần thư mục mẫu có fixtures.txt (mỗi dòng: loại|tệp|địa chỉ gốc):
 *   CHIMTO_FIXTURES=<thư mục> gradlew :app:testReleaseUnitTest --tests vn.chimto.tv.SiteProbeTest -i
 */
class SiteProbeTest {
    @Test
    fun probe() {
        val dir = System.getenv("CHIMTO_FIXTURES")?.let { File(it) }
        assumeTrue("Đặt CHIMTO_FIXTURES để chạy", dir != null && File(dir, "fixtures.txt").exists())
        for (line in File(dir, "fixtures.txt").readLines().filter { it.isNotBlank() }) {
            val (kind, file, url) = line.split('|')
            val doc = Jsoup.parse(File(dir, file).readText(), url)
            println("\n===== $kind  $file")
            when (kind) {
                "home" -> {
                    val h = Site.parseHome(doc)
                    h.sections.forEach { s -> println("  [${s.title}] ${s.films.size}: " + s.films.take(3).joinToString { "${it.title} (${it.badge})" }) }
                    h.sections.firstOrNull()?.films?.firstOrNull()?.let { println("  vd: ${it.url} | ${it.poster}") }
                    println("  thể loại ${h.categories.size}: " + h.categories.joinToString { it.title })
                }
                "list" -> {
                    val p = Site.parseList(doc, url)
                    println("  ${p.films.size} phim, trang sau: ${p.nextUrl}")
                    println("  " + p.films.take(4).joinToString { "${it.title} (${it.badge})" })
                }
                "detail" -> {
                    val d = Site.parseDetail(doc, url)
                    println("  ${d.title} / ${d.otherTitle} | poster ${d.poster}")
                    println("  info: ${d.info}")
                    println("  mô tả: ${d.description?.take(90)}")
                    d.groups.forEach { g ->
                        println("  nhóm [${g.name}] ${g.episodes.size} tập: ${g.episodes.first().title} … ${g.episodes.last().title} | ${g.episodes.last().url}")
                    }
                }
                "source" -> {
                    val s = Site.parseSource(doc, url)
                    println("  pageMode=${s.pageMode} postId=${s.postId} ep=${s.ep} sv=${s.sv} player=${s.playerUrl} direct=${s.directEmbed}")
                    println("  servers: " + s.servers.joinToString { "${it.label}=${it.type}" })
                }
            }
        }
    }
}
