package vn.chimto.tv

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import android.widget.ImageView
import java.io.File
import java.io.IOException
import java.security.MessageDigest
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger

/**
 * Tải poster: bộ nhớ (LRU) → đĩa (cacheDir/img) → mạng. Ảnh được thu nhỏ khi giải mã cho nhẹ RAM của TV box.
 * Màn mật độ cao (TV 4K): ảnh thu nhỏ "-300x450" của WordPress bị phóng to thấy mờ, nên thử tải ảnh gốc trước.
 */
object ImageLoader {
    private const val DISK_LIMIT = 80L * 1024 * 1024
    private const val MAX_ORIGINAL_MISSES = 8

    private val pool = Executors.newFixedThreadPool(4)
    private val mem = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 1024 / 8).toInt()) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount / 1024
    }
    private lateinit var dir: File
    // "-300x450.webp" hoặc "-300x450.jpg?w=300" (ảnh qua CDN có tham số)
    private val thumbSuffix = Regex("""-\d+x\d+\.\w+(\?.*)?$""")
    private var hiRes = false

    // Trang không giữ ảnh gốc (thử mãi không thấy) → thôi không thử nữa trong phiên này
    private val originalMisses = AtomicInteger()

    fun init(context: Context) {
        dir = File(context.cacheDir, "img").apply { mkdirs() }
        hiRes = context.resources.displayMetrics.density >= 3f
        pool.execute { trimDisk() }
    }

    fun load(view: ImageView, url: String?, maxWidth: Int = 320) {
        view.setTag(R.id.image_url, url)
        if (url.isNullOrBlank()) {
            view.setImageDrawable(null)
            return
        }
        val width = if (hiRes) maxWidth * 2 else maxWidth
        val key = "$width|$url"
        mem.get(key)?.let {
            view.setImageBitmap(it)
            return
        }
        view.setImageDrawable(null)
        pool.execute {
            val bmp = try {
                decode(bytes(url), width)
            } catch (e: Exception) {
                null
            }
            if (bmp != null) mem.put(key, bmp)
            view.post { if (bmp != null && view.getTag(R.id.image_url) == url) view.setImageBitmap(bmp) }
        }
    }

    private fun bytes(url: String): ByteArray {
        val f = File(dir, md5(if (hiRes) "hi|$url" else url))
        if (f.exists()) return f.readBytes().also { f.setLastModified(System.currentTimeMillis()) }
        val data = (if (hiRes) originalOf(url) else null) ?: fetch(url) ?: throw IOException("Không tải được ảnh")
        f.writeBytes(data)
        return data
    }

    /** "…/Ten-Phim-300x450.webp" → thử "…/Ten-Phim.jpg" rồi ".png" (ảnh gốc WordPress tải lên). */
    private fun originalOf(url: String): ByteArray? {
        if (originalMisses.get() >= MAX_ORIGINAL_MISSES) return null
        val m = thumbSuffix.find(url) ?: return null
        val base = url.substring(0, m.range.first)
        for (ext in listOf(".jpg", ".png")) {
            fetch(base + ext)?.let {
                originalMisses.set(0)
                return it
            }
        }
        originalMisses.incrementAndGet()
        return null
    }

    /** null nếu không phải ảnh (vd trang 404) — không đọc phần thân để đỡ tốn mạng. */
    private fun fetch(url: String): ByteArray? {
        val c = Http.open(url, mapOf("Referer" to Prefs.baseUrl + "/"))
        try {
            if (c.responseCode != 200 || c.contentType?.startsWith("image/") != true) return null
            return c.inputStream.use { it.readBytes() }
        } finally {
            c.disconnect()
        }
    }

    private fun decode(data: ByteArray, maxWidth: Int): Bitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(data, 0, data.size, bounds)
        var sample = 1
        while (bounds.outWidth / (sample * 2) >= maxWidth) sample *= 2
        val opts = BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = Bitmap.Config.RGB_565
        }
        return BitmapFactory.decodeByteArray(data, 0, data.size, opts)
    }

    fun clearDisk() {
        pool.execute { dir.listFiles()?.forEach { it.delete() } }
        mem.evictAll()
    }

    private fun trimDisk() {
        val files = dir.listFiles()?.sortedBy { it.lastModified() } ?: return
        var total = files.sumOf { it.length() }
        for (f in files) {
            if (total <= DISK_LIMIT) break
            total -= f.length()
            f.delete()
        }
    }

    private fun md5(s: String): String =
        MessageDigest.getInstance("MD5").digest(s.toByteArray()).joinToString("") { "%02x".format(it) }
}
