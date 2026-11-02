package vn.chimto.tv

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import android.widget.ImageView
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.Executors

/** Tải poster: bộ nhớ (LRU) → đĩa (cacheDir/img) → mạng. Ảnh được thu nhỏ khi giải mã cho nhẹ RAM của TV box. */
object ImageLoader {
    private const val DISK_LIMIT = 80L * 1024 * 1024

    private val pool = Executors.newFixedThreadPool(4)
    private val mem = object : LruCache<String, Bitmap>((Runtime.getRuntime().maxMemory() / 1024 / 8).toInt()) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount / 1024
    }
    private lateinit var dir: File

    fun init(context: Context) {
        dir = File(context.cacheDir, "img").apply { mkdirs() }
        pool.execute { trimDisk() }
    }

    fun load(view: ImageView, url: String?, maxWidth: Int = 320) {
        view.setTag(R.id.image_url, url)
        if (url.isNullOrBlank()) {
            view.setImageDrawable(null)
            return
        }
        val key = "$maxWidth|$url"
        mem.get(key)?.let {
            view.setImageBitmap(it)
            return
        }
        view.setImageDrawable(null)
        pool.execute {
            val bmp = try {
                decode(bytes(url), maxWidth)
            } catch (e: Exception) {
                null
            }
            if (bmp != null) mem.put(key, bmp)
            view.post { if (bmp != null && view.getTag(R.id.image_url) == url) view.setImageBitmap(bmp) }
        }
    }

    private fun bytes(url: String): ByteArray {
        val f = File(dir, md5(url))
        if (f.exists()) return f.readBytes().also { f.setLastModified(System.currentTimeMillis()) }
        val c = Http.open(url, mapOf("Referer" to Prefs.baseUrl + "/"))
        val data = try {
            c.inputStream.use { it.readBytes() }
        } finally {
            c.disconnect()
        }
        f.writeBytes(data)
        return data
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
