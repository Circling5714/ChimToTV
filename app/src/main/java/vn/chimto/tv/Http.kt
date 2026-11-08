package vn.chimto.tv

import java.io.EOFException
import java.io.IOException
import java.io.InputStream
import java.net.ConnectException
import java.net.HttpURLConnection
import java.net.SocketException
import java.net.URL
import java.net.URLEncoder
import java.nio.charset.Charset

object Http {
    /** Cùng User-Agent rút gọn với WebView của app (xem Privacy). */
    val UA: String get() = Privacy.userAgent

    class Response(val url: String, val code: Int, val body: String)

    /** Trang trả lời bằng mã lỗi (4xx / 5xx): thử lại cũng vậy. */
    class HttpError(val code: Int) : IOException("Trang trả về lỗi HTTP $code")

    /**
     * Mở kết nối, tự đi theo chuyển hướng (kể cả http → https, đổi tên miền).
     * headers ghi đè header mặc định (vd User-Agent của WebView).
     */
    fun open(url: String, headers: Map<String, String> = emptyMap()): HttpURLConnection {
        var current = url
        repeat(8) {
            val c = connection(current)
            c.instanceFollowRedirects = false
            c.connectTimeout = 15_000
            c.readTimeout = 20_000
            c.setRequestProperty("User-Agent", UA)
            c.setRequestProperty("Accept-Language", "vi-VN,vi;q=0.9,en;q=0.8")
            for ((k, v) in headers) c.setRequestProperty(k, v)
            val code = c.responseCode
            if (code !in 300..399) return c
            val location = c.getHeaderField("Location")
            c.disconnect()
            if (location.isNullOrBlank()) throw IOException("HTTP $code không có Location")
            current = URL(URL(current), location).toString()
        }
        throw IOException("Chuyển hướng quá nhiều lần: $url")
    }

    /** Bật proxy thì đi qua cổng SOCKS5 nội bộ của AppProxy, không thì theo mạng mặc định của máy. */
    private fun connection(url: String): HttpURLConnection {
        val proxy = AppProxy.javaProxy()
        return (if (proxy != null) URL(url).openConnection(proxy) else URL(url).openConnection()) as HttpURLConnection
    }

    /**
     * Kết nối giữ lại từ lần tải trước có thể đã bị mạng / máy chủ ngắt ngầm (TX3 gặp với phimmoir.fun:
     * "unexpected end of stream") → thử lại một lần bằng kết nối mới. Lỗi chắc chắn lặp lại (mã lỗi HTTP, tên miền
     * không phân giải, chứng chỉ, chuyển hướng vòng…) hay quá thời gian chờ thì thôi (trang chết hẳn không bắt chờ gấp đôi).
     */
    fun get(url: String, headers: Map<String, String> = emptyMap()): Response = try {
        getOnce(url, headers)
    } catch (e: IOException) {
        if (!dropped(e)) throw e
        getOnce(url, headers + ("Connection" to "close"))
    }

    /** Lỗi do proxy (cổng nội bộ báo "SOCKS: …") thì không thử lại — proxy hỏng, thử lại cũng vậy. */
    private fun dropped(e: IOException) = !e.message.orEmpty().contains("SOCKS") &&
        (e is EOFException || (e is SocketException && e !is ConnectException) ||
            e.message.orEmpty().let { it.contains("unexpected end of stream") || it.contains("connection reset", true) })

    private fun getOnce(url: String, headers: Map<String, String>): Response {
        val c = open(url, headers)
        try {
            val code = c.responseCode
            if (code >= 400) throw HttpError(code)
            val body = c.inputStream.use { readText(it, c.contentType) }
            return Response(c.url.toString(), code, body)
        } finally {
            c.disconnect()
        }
    }

    /** POST dạng form (dùng cho admin-ajax.php của WordPress). */
    fun post(url: String, form: Map<String, String>, headers: Map<String, String> = emptyMap()): Response {
        val c = connection(url)
        try {
            c.requestMethod = "POST"
            c.doOutput = true
            c.connectTimeout = 15_000
            c.readTimeout = 20_000
            c.setRequestProperty("User-Agent", UA)
            c.setRequestProperty("Content-Type", "application/x-www-form-urlencoded; charset=UTF-8")
            for ((k, v) in headers) c.setRequestProperty(k, v)
            val body = form.entries.joinToString("&") { (k, v) ->
                URLEncoder.encode(k, "UTF-8") + "=" + URLEncoder.encode(v, "UTF-8")
            }
            c.outputStream.use { it.write(body.toByteArray()) }
            val code = c.responseCode
            if (code >= 400) throw IOException("Trang trả về lỗi HTTP $code")
            return Response(c.url.toString(), code, c.inputStream.use { readText(it, c.contentType) })
        } finally {
            c.disconnect()
        }
    }

    fun charsetOf(contentType: String?): Charset {
        val name = contentType?.substringAfter("charset=", "")?.trim()?.trim('"', '\'')?.substringBefore(';')
        return try {
            if (name.isNullOrEmpty()) Charsets.UTF_8 else Charset.forName(name)
        } catch (e: Exception) {
            Charsets.UTF_8
        }
    }

    fun readText(input: InputStream, contentType: String?): String =
        input.readBytes().toString(charsetOf(contentType))
}
