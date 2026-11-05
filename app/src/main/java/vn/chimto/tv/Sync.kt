package vn.chimto.tv

import org.json.JSONArray
import org.json.JSONObject

// Đồng bộ trong nhà (TV ↔ iPad / iPhone): dữ liệu trao đổi và cách gộp — giống hệt bản iPad (SyncData.swift), cùng định
// dạng JSON. Mỗi máy gửi toàn bộ dữ liệu của mình, máy kia gộp rồi trả lại bản đã gộp; gộp xong hai máy như nhau. Xoá
// cũng được đồng bộ nhờ "bia mộ" (thời điểm xoá): phim xoá khỏi lịch sử / bỏ theo dõi / trang bỏ khỏi danh sách ở máy
// này không bị máy kia trả về, trừ khi bên kia xem / theo dõi / thêm lại sau thời điểm đó.

data class SyncHistory(val film: String, val title: String, val poster: String, val ep: String, val epUrl: String, val time: Long)

/** Chỗ đang xem một tập (giây; 0 = đã xem hết), t = lúc lưu (0 = lưu từ bản cũ, không rõ lúc nào). */
data class SyncPos(val ep: String, val sec: Int, val film: String, val t: Long)

data class SyncFollow(
    val film: String, val title: String, val poster: String, val count: Int, val latest: String, val seen: Int, val t: Long,
)

/** Dữ liệu riêng một người xem. */
data class SyncProfile(
    val id: String, val name: String,
    val history: List<SyncHistory> = emptyList(),
    val histDel: Map<String, Long> = emptyMap(),
    val pos: List<SyncPos> = emptyList(),
    val follows: List<SyncFollow> = emptyList(),
    val followDel: Map<String, Long> = emptyMap(),
)

/** Danh sách trang phim (dùng chung cả máy): thứ tự, lúc thêm, lúc bỏ. */
data class SyncSites(val list: List<String>, val add: Map<String, Long> = emptyMap(), val del: Map<String, Long> = emptyMap())

data class SyncPayload(
    val device: String, val name: String, val platform: String, val profiles: List<SyncProfile>, val sites: SyncSites,
) {
    fun toJson(): String {
        fun map(m: Map<String, Long>) = JSONObject().apply { m.forEach { (k, v) -> put(k, v) } }
        val ps = JSONArray()
        for (p in profiles) {
            val h = JSONArray()
            p.history.forEach {
                h.put(JSONObject().put("film", it.film).put("title", it.title).put("poster", it.poster).put("ep", it.ep)
                    .put("epUrl", it.epUrl).put("time", it.time))
            }
            val pos = JSONArray()
            p.pos.forEach { pos.put(JSONObject().put("ep", it.ep).put("sec", it.sec).put("film", it.film).put("t", it.t)) }
            val f = JSONArray()
            p.follows.forEach {
                f.put(JSONObject().put("film", it.film).put("title", it.title).put("poster", it.poster).put("count", it.count)
                    .put("latest", it.latest).put("seen", it.seen).put("t", it.t))
            }
            ps.put(JSONObject().put("id", p.id).put("name", p.name).put("history", h).put("histDel", map(p.histDel))
                .put("pos", pos).put("follows", f).put("followDel", map(p.followDel)))
        }
        val sites = JSONObject().put("list", JSONArray(this.sites.list)).put("add", map(this.sites.add))
            .put("del", map(this.sites.del))
        return JSONObject().put("v", 1).put("device", device).put("name", name).put("platform", platform)
            .put("profiles", ps).put("sites", sites).toString()
    }

    companion object {
        fun parse(json: String): SyncPayload {
            val o = JSONObject(json)
            fun map(m: JSONObject?): Map<String, Long> =
                m?.let { j -> j.keys().asSequence().associateWith { j.optLong(it) } }.orEmpty()
            fun <T> list(a: JSONArray?, f: (JSONObject) -> T): List<T> =
                a?.let { arr -> List(arr.length()) { f(arr.getJSONObject(it)) } }.orEmpty()
            val ps = list(o.optJSONArray("profiles")) { p ->
                SyncProfile(
                    id = p.optString("id"), name = p.optString("name"),
                    history = list(p.optJSONArray("history")) {
                        SyncHistory(it.getString("film"), it.optString("title"), it.optString("poster"), it.optString("ep"),
                            it.optString("epUrl"), it.optLong("time"))
                    },
                    histDel = map(p.optJSONObject("histDel")),
                    pos = list(p.optJSONArray("pos")) {
                        SyncPos(it.getString("ep"), it.optInt("sec"), it.optString("film"), it.optLong("t"))
                    },
                    follows = list(p.optJSONArray("follows")) {
                        SyncFollow(it.getString("film"), it.optString("title"), it.optString("poster"), it.optInt("count"),
                            it.optString("latest"), it.optInt("seen"), it.optLong("t"))
                    },
                    followDel = map(p.optJSONObject("followDel")),
                )
            }
            val s = o.optJSONObject("sites")
            val sl = s?.optJSONArray("list")
            val sites = SyncSites(
                sl?.let { a -> List(a.length()) { a.getString(it) } }.orEmpty(),
                map(s?.optJSONObject("add")), map(s?.optJSONObject("del")),
            )
            return SyncPayload(o.optString("device"), o.optString("name"), o.optString("platform"), ps, sites)
        }
    }
}

object SyncMerge {
    const val MAX_HISTORY = 60
    const val MAX_POSITIONS = 400

    /** Giữ tối đa bấy nhiêu "bia mộ" mỗi loại (mới nhất). */
    const val MAX_TOMBSTONES = 300

    /** Ghép người xem hai máy: người đầu tiên (id "") với nhau, còn lại theo tên (không phân biệt hoa / thường, khoảng trắng). */
    fun match(local: List<SyncProfile>, remote: List<SyncProfile>): List<Pair<Int, Int>> {
        fun key(s: String) = s.trim().lowercase()
        val out = ArrayList<Pair<Int, Int>>()
        val used = HashSet<Int>()
        val l0 = local.indexOfFirst { it.id.isEmpty() }
        val r0 = remote.indexOfFirst { it.id.isEmpty() }
        if (l0 >= 0 && r0 >= 0) {
            out += l0 to r0
            used += r0
        }
        local.forEachIndexed { i, p ->
            if (p.id.isEmpty()) return@forEachIndexed
            val j = remote.indices.firstOrNull { it !in used && remote[it].id.isNotEmpty() && key(remote[it].name) == key(p.name) }
            if (j != null) {
                out += i to j
                used += j
            }
        }
        return out
    }

    fun tombstones(a: Map<String, Long>, b: Map<String, Long>): Map<String, Long> {
        val m = LinkedHashMap(a)
        for ((k, v) in b) if (v > (m[k] ?: Long.MIN_VALUE)) m[k] = v
        if (m.size <= MAX_TOMBSTONES) return m
        return m.entries.sortedWith(compareByDescending<Map.Entry<String, Long>> { it.value }.thenBy { it.key })
            .take(MAX_TOMBSTONES).associateTo(LinkedHashMap()) { it.key to it.value }
    }

    /** Gộp dữ liệu một người xem (local: máy này, remote: máy kia); trùng thời điểm thì lấy local. */
    fun profile(local: SyncProfile, remote: SyncProfile): SyncProfile {
        val histDel = tombstones(local.histDel, remote.histDel)

        // Lịch sử: mỗi phim một mục, lấy lần xem mới hơn; bỏ phim đã xoá sau lần xem đó
        val hist = LinkedHashMap<String, SyncHistory>()
        for (h in local.history + remote.history) {
            val old = hist[h.film]
            if (old != null && old.time >= h.time) continue
            hist[h.film] = h
        }
        val history = hist.values.filter { h -> histDel[h.film]?.let { it < h.time } ?: true }
            .sortedWith(compareByDescending<SyncHistory> { it.time }.thenBy { it.film })
            .take(MAX_HISTORY)

        // Chỗ đang xem: mỗi tập lấy lần lưu mới hơn; phim đã xoá khỏi lịch sử sau lần lưu đó thì bỏ
        val pos = LinkedHashMap<String, SyncPos>()
        for (p in local.pos + remote.pos) {
            val old = pos[p.ep]
            if (old == null || p.t > old.t) pos[p.ep] = p
        }
        // Thứ tự lưu: cũ nhất trước (bản cũ — t = 0 — đứng đầu, giữ thứ tự cũ); sortedBy giữ thứ tự khi bằng nhau
        val positions = pos.values.filter { p -> histDel[p.film]?.let { it < p.t } ?: true }
            .sortedBy { it.t }
            .takeLast(MAX_POSITIONS)

        // Phim theo dõi: hợp hai bên; số tập / số tập đã thấy lấy số lớn hơn; bỏ phim đã bỏ theo dõi sau lần theo dõi đó
        val followDel = tombstones(local.followDel, remote.followDel)
        val fol = LinkedHashMap<String, SyncFollow>()
        for (f in local.follows + remote.follows) {
            val old = fol[f.film]
            if (old == null) {
                fol[f.film] = f
                continue
            }
            var m = old.copy(
                count = maxOf(old.count, f.count), seen = maxOf(old.seen, f.seen),
                latest = if (f.count > old.count) f.latest else old.latest,
            )
            if (f.t > old.t) m = m.copy(t = f.t, title = f.title, poster = f.poster.ifEmpty { old.poster })
            fol[f.film] = m
        }
        val follows = fol.values.filter { f -> followDel[f.film]?.let { it < f.t } ?: true }.sortedByDescending { it.t }

        return local.copy(history = history, histDel = histDel, pos = positions, follows = follows, followDel = followDel)
    }

    /**
     * Gộp danh sách trang: trang có mặt nếu lần thêm mới hơn lần bỏ (trang có sẵn từ bản cũ: lúc thêm = 0). Thứ tự: trang
     * của máy này trước, trang mới của máy kia sau.
     */
    fun sites(local: SyncSites, remote: SyncSites): SyncSites {
        val add = tombstones(local.add, remote.add)
        val del = tombstones(local.del, remote.del)
        val list = (local.list + remote.list).distinct().filter { s -> del[s]?.let { it < (add[s] ?: 0) } ?: true }
        return SyncSites(list.ifEmpty { local.list }, add, del)
    }
}
