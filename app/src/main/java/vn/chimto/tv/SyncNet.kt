package vn.chimto.tv

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.provider.Settings
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.InetSocketAddress
import java.net.Proxy
import java.net.ServerSocket
import java.net.Socket
import java.security.SecureRandom
import java.util.concurrent.Executors
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/** Máy đã ghép để đồng bộ trong nhà. */
data class SyncPeer(
    val id: String, val name: String, val platform: String, val token: String,
    val host: String? = null, val port: Int = 0, val last: Long = 0, val status: String = "",
)

/** Máy tìm thấy trong mạng (NSD / mDNS), chưa chắc đã ghép. */
data class SyncFound(val id: String, val name: String, val platform: String, val host: String, val port: Int)

/**
 * Đồng bộ trong nhà (bản iPad: SyncService.swift). TV mở cổng 8791 (bản debug 8792) trong mạng nhà và quảng bá qua
 * NSD / mDNS (`_conbangtv._tcp`):
 *   GET  /hello — mã máy, tên, có đang cho ghép không (không cần khoá)
 *   POST /pair  — {device, name, platform, code}: đúng mã ghép đang hiện trên TV → trả khoá chung (token)
 *   POST /sync  — thân = SyncPayload của máy gửi; ký X-Device / X-Time / X-Sig = HMAC-SHA256(token, "time\nthân"). TV gộp
 *                 vào dữ liệu của mình rồi trả lại SyncPayload đã gộp (cũng ký) — máy gửi gộp nốt.
 * Cổng chỉ mở khi đã ghép ít nhất một máy hoặc đang mở hộp thoại Đồng bộ.
 */
object SyncNet {
    private const val TAG = "ChimToSync"
    const val SERVICE_TYPE = "_conbangtv._tcp"
    val port get() = if (BuildConfig.DEBUG) 8792 else 8791
    const val PLATFORM = "android"

    private lateinit var app: Context
    private val pool = Executors.newCachedThreadPool()
    @Volatile private var server: ServerSocket? = null
    private var nsd: NsdManager? = null
    private var registration: NsdManager.RegistrationListener? = null
    private var discovery: NsdManager.DiscoveryListener? = null
    private val resolveQueue = ArrayDeque<NsdServiceInfo>()
    private var resolving = false

    /** Mã ghép đang hiện (null = không cho ghép). */
    @Volatile var pairCode: String? = null
        private set
    private var pairTries = 0

    val found = LinkedHashMap<String, SyncFound>()
    @Volatile var busy = false
        private set

    /** Hộp thoại Đồng bộ đang mở: vẽ lại khi có máy mới / trạng thái đổi (luồng chính). */
    var onChanged: (() -> Unit)? = null

    /** Màn chính: dữ liệu vừa đổi do đồng bộ — vẽ lại Xem tiếp / lịch sử / theo dõi (luồng chính). */
    var onApplied: (() -> Unit)? = null

    fun init(context: Context) {
        app = context.applicationContext
        if (peers.isNotEmpty()) start()
    }

    val deviceName: String
        get() = (if (Build.VERSION.SDK_INT >= 25) Settings.Global.getString(app.contentResolver, "device_name") else null)
            ?.takeIf { it.isNotBlank() } ?: "TV ${Build.MODEL}"

    var auto: Boolean
        get() = Prefs.syncAuto
        set(value) {
            Prefs.syncAuto = value
        }

    // ---- Máy đã ghép ----

    val peers: List<SyncPeer>
        @Synchronized get() = try {
            val a = JSONArray(Prefs.syncPeersJson)
            List(a.length()) { i ->
                val o = a.getJSONObject(i)
                SyncPeer(o.getString("id"), o.optString("name"), o.optString("platform"), o.getString("token"),
                    o.optString("host").ifEmpty { null }, o.optInt("port"), o.optLong("last"), o.optString("status"))
            }
        } catch (e: Exception) {
            emptyList()
        }

    @Synchronized
    private fun savePeers(list: List<SyncPeer>) {
        val a = JSONArray()
        list.forEach {
            a.put(JSONObject().put("id", it.id).put("name", it.name).put("platform", it.platform).put("token", it.token)
                .put("host", it.host ?: "").put("port", it.port).put("last", it.last).put("status", it.status))
        }
        Prefs.syncPeersJson = a.toString()
    }

    @Synchronized
    private fun updatePeer(id: String, change: (SyncPeer) -> SyncPeer) {
        savePeers(peers.map { if (it.id == id) change(it) else it })
        changed()
    }

    @Synchronized
    private fun addPeer(p: SyncPeer) = savePeers(peers.filter { it.id != p.id } + p)

    fun unpair(p: SyncPeer) {
        savePeers(peers.filter { it.id != p.id })
        if (peers.isEmpty() && pairCode == null) stop()
        changed()
    }

    private fun changed() = mainHandler.post { onChanged?.invoke() }

    // ---- Mở cổng / tìm máy ----

    fun start() {
        startServer()
        startNsd()
    }

    fun stop() {
        try {
            server?.close()
        } catch (_: Exception) {
        }
        server = null
        nsd?.let { m ->
            registration?.let { runCatching { m.unregisterService(it) } }
            discovery?.let { runCatching { m.stopServiceDiscovery(it) } }
        }
        registration = null
        discovery = null
        synchronized(found) { found.clear() }
    }

    /** Hộp thoại Đồng bộ mở: hiện mã ghép 4 số (đổi mỗi lần mở), mở cổng + tìm máy. */
    fun openPairing() {
        pairCode = "%04d".format(SecureRandom().nextInt(10000))
        pairTries = 0
        start()
    }

    fun closePairing() {
        pairCode = null
        if (peers.isEmpty()) stop()
    }

    private fun startServer() {
        if (server != null) return
        val s = try {
            ServerSocket().apply {
                reuseAddress = true
                bind(InetSocketAddress(port))
            }
        } catch (e: Exception) {
            Log.w(TAG, "không mở được cổng $port: $e")
            return
        }
        server = s
        pool.execute {
            while (!s.isClosed) {
                val c = try {
                    s.accept()
                } catch (e: Exception) {
                    break
                }
                pool.execute { serve(c) }
            }
        }
    }

    private fun startNsd() {
        val m = nsd ?: (app.getSystemService(Context.NSD_SERVICE) as? NsdManager)?.also { nsd = it } ?: return
        if (registration == null) {
            val info = NsdServiceInfo().apply {
                serviceName = deviceName
                serviceType = SERVICE_TYPE
                port = this@SyncNet.port
                setAttribute("id", Prefs.deviceId)
                setAttribute("p", PLATFORM)
            }
            val l = object : NsdManager.RegistrationListener {
                override fun onServiceRegistered(info: NsdServiceInfo) = Unit
                override fun onRegistrationFailed(info: NsdServiceInfo, code: Int) {
                    Log.w(TAG, "không quảng bá được ($code)")
                    registration = null
                }
                override fun onServiceUnregistered(info: NsdServiceInfo) = Unit
                override fun onUnregistrationFailed(info: NsdServiceInfo, code: Int) = Unit
            }
            registration = l
            runCatching { m.registerService(info, NsdManager.PROTOCOL_DNS_SD, l) }
        }
        if (discovery == null) {
            val l = object : NsdManager.DiscoveryListener {
                override fun onDiscoveryStarted(type: String) = Unit
                override fun onDiscoveryStopped(type: String) = Unit
                override fun onStartDiscoveryFailed(type: String, code: Int) {
                    discovery = null
                }
                override fun onStopDiscoveryFailed(type: String, code: Int) = Unit
                override fun onServiceFound(info: NsdServiceInfo) {
                    synchronized(resolveQueue) { resolveQueue.addLast(info) }
                    resolveNext()
                }
                override fun onServiceLost(info: NsdServiceInfo) {
                    synchronized(found) { found.values.removeAll { it.name == info.serviceName } }
                    changed()
                }
            }
            discovery = l
            runCatching { m.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, l) }
        }
    }

    /** Android cũ chỉ cho phân giải từng dịch vụ một (đang phân giải mà gọi tiếp thì lỗi FAILURE_ALREADY_ACTIVE). */
    private fun resolveNext() {
        val m = nsd ?: return
        val info = synchronized(resolveQueue) {
            if (resolving) return
            resolveQueue.removeFirstOrNull()?.also { resolving = true }
        } ?: return
        @Suppress("DEPRECATION")
        m.resolveService(info, object : NsdManager.ResolveListener {
            override fun onResolveFailed(info: NsdServiceInfo, code: Int) = done()
            override fun onServiceResolved(info: NsdServiceInfo) {
                val id = info.attributes["id"]?.let { String(it) }
                val host = info.host?.hostAddress
                if (id != null && id != Prefs.deviceId && host != null) {
                    val p = info.attributes["p"]?.let { String(it) }.orEmpty()
                    synchronized(found) { found[id] = SyncFound(id, info.serviceName, p, host, info.port) }
                    changed()
                }
                done()
            }

            fun done() {
                synchronized(resolveQueue) { resolving = false }
                resolveNext()
            }
        })
    }

    // ---- Máy chủ (máy kia gọi tới) ----

    private fun serve(c: Socket) {
        c.use { sock ->
            try {
                sock.soTimeout = 10_000
                val req = Http1.read(sock.getInputStream(), request = true) ?: return
                // Máy gọi tới báo cổng của nó (X-Port) → nhớ địa chỉ để TV tự gọi lại (khác lớp mạng thì NSD không thấy)
                val port = req.headers["x-port"]?.toIntOrNull()?.takeIf { it > 0 }
                val back = port?.let { p -> sock.inetAddress?.hostAddress?.let { it to p } }
                val resp = handle(req, back)
                sock.getOutputStream().apply {
                    write(resp)
                    flush()
                }
            } catch (e: Exception) {
                Log.w(TAG, "lỗi phục vụ: $e")
            }
        }
    }

    private fun handle(r: Http1.Message, back: Pair<String, Int>?): ByteArray = when {
        r.method == "GET" && r.path == "/hello" -> Http1.response(200, JSONObject()
            .put("app", "conbangtv").put("device", Prefs.deviceId).put("name", deviceName).put("platform", PLATFORM)
            .put("version", BuildConfig.VERSION_NAME).put("pairing", pairCode != null).toString().toByteArray())
        r.method == "POST" && r.path == "/pair" -> pairRequest(r.body, back)
        r.method == "POST" && r.path == "/sync" -> {
            val peer = peers.firstOrNull { it.id == r.headers["x-device"] }
            when {
                peer == null -> Http1.json(403, "unpaired")
                !Http1.verify(r, peer.token) -> Http1.json(401, "sig")
                else -> try {
                    val remote = SyncPayload.parse(String(r.body))
                    val changed = absorb(remote)
                    updatePeer(peer.id) {
                        it.copy(name = remote.name, last = System.currentTimeMillis(),
                            status = if (changed) "Đã nhận dữ liệu mới" else "Đã khớp",
                            host = back?.first ?: it.host, port = back?.second ?: it.port)
                    }
                    if (changed) mainHandler.post { onApplied?.invoke() }
                    Http1.response(200, payload().toJson().toByteArray(), peer.token)
                } catch (e: Exception) {
                    Http1.json(400, "json")
                }
            }
        }
        else -> Http1.json(404, "not found")
    }

    @Synchronized
    private fun pairRequest(body: ByteArray, back: Pair<String, Int>?): ByteArray {
        val code = pairCode ?: return Http1.json(403, "closed")
        val o = try {
            JSONObject(String(body))
        } catch (e: Exception) {
            return Http1.json(400, "json")
        }
        val device = o.optString("device")
        if (device.isEmpty()) return Http1.json(400, "json")
        if (o.optString("code") != code) {
            // Đoán sai nhiều lần: đổi mã
            if (++pairTries >= 5) {
                openPairing()
                changed()
            }
            return Http1.json(403, "code")
        }
        val token = Http1.randomToken()
        val name = o.optString("name").ifEmpty { "Máy khác" }
        addPeer(SyncPeer(device, name, o.optString("platform"), token, back?.first, back?.second ?: 0, status = "Vừa ghép"))
        pairCode = "%04d".format(SecureRandom().nextInt(10000)) // mã dùng một lần
        pairTries = 0
        changed()
        mainHandler.post { android.widget.Toast.makeText(app, "Đã ghép với $name.", android.widget.Toast.LENGTH_LONG).show() }
        return Http1.response(200, JSONObject().put("device", Prefs.deviceId).put("name", deviceName)
            .put("platform", PLATFORM).put("token", token).toString().toByteArray())
    }

    // ---- Dữ liệu ----

    fun payload() = SyncPayload(Prefs.deviceId, deviceName, PLATFORM, Prefs.profiles.map { Prefs.syncProfile(it) }, Prefs.syncSites)

    /** Gộp dữ liệu máy kia vào TV (người xem ghép theo tên). true = TV có thay đổi. */
    @Synchronized
    fun absorb(remote: SyncPayload): Boolean {
        val local = Prefs.profiles.map { Prefs.syncProfile(it) }
        var changed = false
        for ((l, r) in SyncMerge.match(local, remote.profiles)) {
            if (Prefs.applySync(SyncMerge.profile(local[l], remote.profiles[r]))) changed = true
        }
        if (Prefs.applySites(SyncMerge.sites(Prefs.syncSites, remote.sites))) changed = true
        return changed
    }

    // ---- Máy khách (TV gọi tới máy kia) ----

    private fun address(p: SyncPeer): Pair<String, Int>? {
        synchronized(found) { found[p.id] }?.let { return it.host to it.port }
        return p.host?.let { it to (if (p.port > 0) p.port else 8791) }
    }

    /** Ghép với máy kia bằng mã 4 số đang hiện trên máy đó (chạy ở luồng nền). null = được, không thì lời báo lỗi. */
    fun pair(host: String, port: Int, code: String): String? {
        val body = JSONObject().put("device", Prefs.deviceId).put("name", deviceName).put("platform", PLATFORM)
            .put("code", code).toString().toByteArray()
        return try {
            val r = Http1.send(host, port, "POST", "/pair", body)
            when (r.status) {
                200 -> Unit
                403 -> return if (String(r.body).contains("closed")) "Máy kia chưa mở màn Đồng bộ (mở để hiện mã ghép)."
                else "Sai mã ghép."
                else -> return "Máy kia trả lỗi ${r.status}."
            }
            val o = JSONObject(String(r.body))
            addPeer(SyncPeer(o.getString("device"), o.optString("name").ifEmpty { "Máy khác" }, o.optString("platform"),
                o.getString("token"), host, port, status = "Vừa ghép"))
            start()
            syncAll(manual = true)
            null
        } catch (e: Exception) {
            "Không kết nối được: ${e.message ?: e.javaClass.simpleName}"
        }
    }

    private var soon: Runnable? = null

    /** Đồng bộ sau một lúc (gộp các lần gọi sát nhau: quay ra màn chính, xem xong một tập…). */
    fun syncSoon(delayMs: Long = 3000) {
        if (!auto || peers.isEmpty()) return
        start()
        soon?.let { mainHandler.removeCallbacks(it) }
        val r = Runnable { pool.execute { syncAll(manual = false) } }
        soon = r
        mainHandler.postDelayed(r, delayMs)
    }

    /** Đồng bộ với mọi máy đã ghép (luồng nền). Trả về lời báo ngắn. */
    fun syncAll(manual: Boolean): String {
        synchronized(this) {
            if (busy) return "Đang đồng bộ…"
            busy = true
        }
        changed()
        try {
            if (synchronized(found) { found.isEmpty() }) Thread.sleep(1500) // NSD cần một lúc để thấy máy kia
            var any = false
            var ok = 0
            for (p in peers) {
                when (sync(p)) {
                    true -> {
                        any = true
                        ok++
                    }
                    false -> ok++
                    null -> Unit
                }
            }
            if (any) mainHandler.post { onApplied?.invoke() }
            return when {
                peers.isEmpty() -> "Chưa ghép máy nào."
                ok == 0 -> "Không kết nối được máy nào (máy kia cần mở Côn Bằng TV)."
                any -> "Đã đồng bộ — có dữ liệu mới."
                else -> "Đã đồng bộ."
            }.also { if (manual) Log.i(TAG, it) }
        } finally {
            busy = false
            changed()
        }
    }

    /** Đồng bộ với một máy: true = TV có thay đổi, false = đã khớp, null = không được. */
    private fun sync(p: SyncPeer): Boolean? {
        val (host, port) = address(p) ?: run {
            updatePeer(p.id) { it.copy(status = "Không thấy máy trong mạng") }
            return null
        }
        return try {
            val r = Http1.send(host, port, "POST", "/sync", payload().toJson().toByteArray(), p.token)
            if (r.status == 403) {
                updatePeer(p.id) { it.copy(status = "Máy kia đã bỏ ghép — ghép lại") }
                return null
            }
            if (r.status != 200 || !Http1.verify(r, p.token)) {
                updatePeer(p.id) { it.copy(status = "Lỗi ${r.status}") }
                return null
            }
            val remote = SyncPayload.parse(String(r.body))
            val changed = absorb(remote)
            updatePeer(p.id) {
                it.copy(name = remote.name, host = host, port = port, last = System.currentTimeMillis(),
                    status = if (changed) "Đã nhận dữ liệu mới" else "Đã khớp")
            }
            Log.i(TAG, "đồng bộ với ${remote.name}: ${if (changed) "có thay đổi" else "đã khớp"}")
            changed
        } catch (e: Exception) {
            updatePeer(p.id) { it.copy(status = "Không kết nối được") }
            Log.w(TAG, "không đồng bộ được với ${p.name}: $e")
            null
        }
    }
}

/** HTTP/1.1 tối giản trên socket (đi thẳng trong mạng nhà, không qua proxy của app). */
object Http1 {
    class Message(
        val method: String = "", val path: String = "", val status: Int = 0,
        val headers: Map<String, String> = emptyMap(), val body: ByteArray = ByteArray(0),
    )

    /** Đọc một yêu cầu / trả lời (đầu + thân theo Content-Length, tối đa 8 MB). */
    fun read(input: InputStream, request: Boolean): Message? {
        val head = ByteArrayOutputStream()
        var last4 = 0
        while (true) {
            val b = input.read()
            if (b < 0) return null
            head.write(b)
            last4 = (last4 shl 8) or b
            if (last4 == 0x0D0A0D0A) break
            if (head.size() > 64 * 1024) return null
        }
        val lines = String(head.toByteArray(), Charsets.UTF_8).split("\r\n").filter { it.isNotEmpty() }
        val first = lines.firstOrNull()?.split(" ") ?: return null
        if (first.size < 2) return null
        val headers = lines.drop(1).mapNotNull { l ->
            val i = l.indexOf(':')
            if (i <= 0) null else l.substring(0, i).trim().lowercase() to l.substring(i + 1).trim()
        }.toMap()
        val len = headers["content-length"]?.toIntOrNull() ?: 0
        if (len < 0 || len > 8 shl 20) return null
        val body = ByteArray(len)
        var off = 0
        while (off < len) {
            val n = input.read(body, off, len - off)
            if (n < 0) return null
            off += n
        }
        return if (request) {
            Message(method = first[0], path = first[1].substringBefore('?'), headers = headers, body = body)
        } else {
            Message(status = first[1].toIntOrNull() ?: 0, headers = headers, body = body)
        }
    }

    fun sign(token: String, time: String, body: ByteArray): String {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(token.toByteArray(), "HmacSHA256"))
        mac.update("$time\n".toByteArray())
        return mac.doFinal(body).joinToString("") { "%02x".format(it) }
    }

    /** Chữ ký đúng và thời điểm lệch không quá 10 phút. */
    fun verify(m: Message, token: String): Boolean {
        val t = m.headers["x-time"] ?: return false
        val sig = m.headers["x-sig"] ?: return false
        val ms = t.toLongOrNull() ?: return false
        if (kotlin.math.abs(System.currentTimeMillis() - ms) > 10 * 60_000) return false
        return sign(token, t, m.body).equals(sig, ignoreCase = true)
    }

    fun randomToken(): String = ByteArray(32).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) }

    fun json(status: Int, error: String) = response(status, JSONObject().put("error", error).toString().toByteArray())

    fun response(status: Int, body: ByteArray, token: String? = null): ByteArray {
        val sb = StringBuilder("HTTP/1.1 $status ${if (status == 200) "OK" else "Error"}\r\n")
            .append("Content-Type: application/json; charset=utf-8\r\n")
            .append("Content-Length: ${body.size}\r\nConnection: close\r\n")
        if (token != null) {
            val time = System.currentTimeMillis().toString()
            sb.append("X-Time: $time\r\nX-Sig: ${sign(token, time, body)}\r\n")
        }
        sb.append("\r\n")
        return sb.toString().toByteArray() + body
    }

    /** Gửi một yêu cầu tới máy kia, chờ tối đa 8 giây. */
    fun send(host: String, port: Int, method: String, path: String, body: ByteArray, token: String? = null): Message {
        Socket(Proxy.NO_PROXY).use { s ->
            s.connect(InetSocketAddress(host, port), 5000)
            s.soTimeout = 8000
            // X-Port: cổng của TV — máy kia nhớ địa chỉ + cổng để gọi lại
            val sb = StringBuilder("$method $path HTTP/1.1\r\nHost: conbangtv\r\nContent-Type: application/json\r\n")
                .append("Content-Length: ${body.size}\r\nConnection: close\r\nX-Port: ${SyncNet.port}\r\n")
            if (token != null) {
                val time = System.currentTimeMillis().toString()
                sb.append("X-Device: ${Prefs.deviceId}\r\nX-Time: $time\r\nX-Sig: ${sign(token, time, body)}\r\n")
            }
            sb.append("\r\n")
            s.getOutputStream().apply {
                write(sb.toString().toByteArray() + body)
                flush()
            }
            return read(s.getInputStream(), request = false) ?: throw java.io.IOException("máy kia đóng kết nối")
        }
    }
}
