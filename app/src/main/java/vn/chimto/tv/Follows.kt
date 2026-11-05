package vn.chimto.tv

import android.app.Activity

/** Phim theo dõi: hỏi lại trang từng phim (tối đa 30 phút một lần) để biết phim nào có tập mới. */
object Follows {
    private const val INTERVAL = 30 * 60_000L

    /** Người xem đang được kiểm (mỗi người một danh sách theo dõi). */
    private val running = HashSet<String>()

    /**
     * Kiểm nếu đã tới hạn (force: kiểm ngay). done chạy ở luồng chính với các phim vừa có thêm tập so với lần kiểm
     * trước (để báo một lần); chưa tới hạn / đang kiểm / không theo dõi phim nào thì không gọi. Đổi người xem trong lúc
     * kiểm thì bỏ kết quả (danh sách theo dõi là của người cũ).
     */
    fun check(activity: Activity, force: Boolean = false, done: (List<Follow>) -> Unit) {
        val owner = Prefs.profileId
        val list = Prefs.follows
        val now = System.currentTimeMillis()
        if (list.isEmpty() || owner in running || (!force && now - Prefs.lastFollowCheck < INTERVAL)) return
        running += owner
        Prefs.lastFollowCheck = now
        // Lần lượt từng phim trên một luồng (không chiếm hết luồng tải trang chủ / ảnh)
        inBackground({ list.map { f -> f to runCatching { Site.detail(f.filmUrl) }.getOrNull() } }) { r ->
            running -= owner
            if (Prefs.profileId != owner) return@inBackground
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
