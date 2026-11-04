package vn.chimto.tv

import android.app.Activity

/** Phim theo dõi: hỏi lại trang từng phim (tối đa 30 phút một lần) để biết phim nào có tập mới. */
object Follows {
    private const val INTERVAL = 30 * 60_000L
    private var running = false

    /**
     * Kiểm nếu đã tới hạn (force: kiểm ngay). done chạy ở luồng chính với các phim vừa có thêm tập so với lần kiểm
     * trước (để báo một lần); chưa tới hạn / đang kiểm / không theo dõi phim nào thì không gọi.
     */
    fun check(activity: Activity, force: Boolean = false, done: (List<Follow>) -> Unit) {
        val list = Prefs.follows
        val now = System.currentTimeMillis()
        if (list.isEmpty() || running || (!force && now - Prefs.lastFollowCheck < INTERVAL)) return
        running = true
        Prefs.lastFollowCheck = now
        // Lần lượt từng phim trên một luồng (không chiếm hết luồng tải trang chủ / ảnh)
        inBackground({ list.map { f -> f to runCatching { Site.detail(f.filmUrl) }.getOrNull() } }) { r ->
            running = false
            val fresh = HashSet<String>()
            r.getOrNull()?.forEach { (f, d) ->
                val ep = d?.latestEpisode ?: return@forEach
                if (d.episodeCount > f.count) fresh += f.filmUrl
                Prefs.updateFollow(f.filmUrl, d.episodeCount, ep.title, seen = false)
            }
            if (!activity.isFinishing && !activity.isDestroyed) done(Prefs.follows.filter { it.filmUrl in fresh && it.newCount > 0 })
        }
    }
}
