package vn.chimto.tv

import android.util.Log
import org.json.JSONObject
import java.io.IOException
import java.net.Inet4Address
import java.net.InetAddress
import java.net.Proxy
import java.net.URL
import java.net.URLEncoder
import java.util.concurrent.ConcurrentHashMap
import javax.net.ssl.HttpsURLConnection

/**
 * DNS mã hoá (DNS-over-HTTPS) cho cổng nội bộ: hỏi địa chỉ của tên miền qua HTTPS tới máy chủ DNS công cộng thay cho
 * DNS của nhà mạng — nhà mạng không đọc / sửa được truy vấn DNS (vượt chặn bằng DNS). Nhà mạng vẫn thấy tên trang trong
 * lúc bắt tay TLS (SNI) và IP đích. Máy chủ DoH gọi bằng IP (khỏi phải hỏi DNS để tìm máy chủ DNS), đi thẳng.
 */
object Doh {
    enum class Server(val label: String, val url: String) {
        CLOUDFLARE("Cloudflare 1.1.1.1", "https://1.1.1.1/dns-query"),
        GOOGLE("Google 8.8.8.8", "https://8.8.8.8/resolve"),
    }

    private class Entry(val addr: InetAddress, val until: Long)

    private val cache = ConcurrentHashMap<String, Entry>()
    private val IPV4 = Regex("""^\d{1,3}(\.\d{1,3}){3}$""")

    /** Địa chỉ IPv4 của host (nhớ theo TTL, tối đa 5 phút). Lỗi → SocksError (cổng nội bộ báo lại cho WebView / Http). */
    fun resolve(server: Server, host: String): InetAddress {
        val key = host.lowercase().trimEnd('.')
        if (IPV4.matches(key) || key == "localhost") return InetAddress.getByName(key)
        val now = System.currentTimeMillis()
        cache[key]?.takeIf { it.until > now }?.let { return it.addr }
        val body = try {
            val conn = URL("${server.url}?name=${URLEncoder.encode(key, "UTF-8")}&type=A").openConnection(Proxy.NO_PROXY) as HttpsURLConnection
            try {
                conn.connectTimeout = 6_000
                conn.readTimeout = 6_000
                conn.setRequestProperty("Accept", "application/dns-json")
                conn.inputStream.use { it.readBytes().toString(Charsets.UTF_8) }
            } finally {
                conn.disconnect()
            }
        } catch (e: IOException) {
            throw SocksError(4, "không hỏi được DNS mã hoá ${server.label} (${e.message})")
        }
        val answers = runCatching { JSONObject(body).optJSONArray("Answer") }.getOrNull()
        for (i in 0 until (answers?.length() ?: 0)) {
            val a = answers!!.optJSONObject(i) ?: continue
            if (a.optInt("type") != 1) continue // bỏ CNAME, lấy bản ghi A
            val addr = runCatching { InetAddress.getByName(a.optString("data")) }.getOrNull() as? Inet4Address ?: continue
            val ttl = a.optLong("TTL", 60).coerceIn(10, 300)
            cache[key] = Entry(addr, now + ttl * 1000)
            if (BuildConfig.DEBUG) Log.d("ChimToDoh", "$key → ${addr.hostAddress} (${server.label})")
            return addr
        }
        throw SocksError(4, "DNS mã hoá không tìm thấy $host")
    }

    /** Vượt chặn: hỏi Cloudflare, lỗi thì Google; null nếu cả hai đều lỗi (bên gọi báo lỗi của DNS thiết bị như cũ). */
    fun resolveAny(host: String): InetAddress? {
        for (s in Server.values()) {
            try {
                return resolve(s, host)
            } catch (e: IOException) {
                // thử máy chủ sau
            }
        }
        return null
    }

    fun clearCache() = cache.clear()
}
