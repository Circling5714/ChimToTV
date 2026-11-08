import Foundation

// Dữ liệu đọc từ trang phim — giống hệt bản Android (Site.kt), để sửa bộ đọc trang hai bên song song.

struct Film: Hashable, Identifiable {
    let url: String
    let title: String
    let poster: String?
    let badge: String?
    var id: String { url }
}

struct FilmSection: Hashable, Identifiable {
    let title: String
    let films: [Film]
    var id: String { title }
}

struct Category: Hashable, Identifiable {
    let title: String
    let url: String
    var id: String { url }
}

/// Đo nhanh một trang: thời gian tải trang chủ (ms) và số phim đọc được (0 = có thể là trang chặn của nhà mạng).
struct SiteCheck {
    let ms: Int
    let films: Int
}

/// movedTo: trang đã chuyển sang tên miền khác (theo chuyển hướng) → gốc mới.
struct Home {
    var sections: [FilmSection]
    var categories: [Category]
    var movedTo: String?
}

/// sections: trang chia nhóm (vd lịch chiếu theo ngày) — khi có thì hiển thị thay cho films.
struct FilmPage {
    var films: [Film]
    var nextUrl: String?
    var sections: [FilmSection] = []
}

struct Episode: Hashable, Identifiable {
    let title: String
    let url: String
    var id: String { url }
}

/// Một danh sách tập, vd "Việt Sub" / "Thuyết Minh" (trang HaLim chia tập theo server).
struct EpisodeGroup: Hashable, Identifiable {
    let name: String
    let episodes: [Episode]
    var id: String { name }
}

struct FilmDetail {
    let url: String
    let title: String
    let otherTitle: String?
    let poster: String?
    let description: String?
    let info: String?
    var groups: [EpisodeGroup]

    var episodeCount: Int { groups.map { $0.episodes.count }.max() ?? 0 }

    /// Tập mới nhất của bản nhiều tập nhất (bản Vietsub thường ra trước Thuyết Minh).
    var latestEpisode: Episode? { groups.max { $0.episodes.count < $1.episodes.count }?.episodes.last }

    func groupOf(_ epUrl: String) -> EpisodeGroup? { groups.first { g in g.episodes.contains { $0.url == epUrl } } }
}

struct Server: Hashable, Identifiable {
    let label: String
    let type: String
    var id: String { type }
}

/// Link phát của một server: iframe trình phát / trang tập, hoặc luồng .m3u8 (kèm phụ đề rời nếu có).
/// file: tệp video phát thẳng bằng <video> (MP4 của CLB Phim Xưa), không phải trang trình phát.
struct Video {
    let url: String
    var subtitle: String?
    var file: Bool = false
}

/// Cách phát một tập (xem Site.kt): player.php → iframe; pageMode → mở thẳng trang tập; next → trang Next.js.
struct EpisodeSource {
    let epUrl: String
    let servers: [Server]
    let postId: String
    let ep: String
    let sv: String
    let playerUrl: String?
    let directEmbed: String?
    let pageMode: Bool
    var links: [String: Video] = [:]
    var next: Bool = false

    /// Loại server "Trình phát của trang" (mở thẳng trang tập) khi trang pageMode có thêm server dự phòng KKPhim.
    static let pageServer = "page"

    /// Chỉ có trình phát của trang tập (không có server nào khác để chọn, kể cả server dự phòng KKPhim).
    var pageOnly: Bool { pageMode && !servers.contains { $0.type == EpisodeSource.pageServer } }
}

struct HistoryItem: Codable, Hashable {
    let filmUrl: String
    let title: String
    let poster: String?
    let epTitle: String
    let epUrl: String
    /// ms kể từ 1970
    let time: Int64
}

/// Phim theo dõi: count = số tập lần kiểm gần nhất, seen = số tập lúc người xem mở phim lần cuối.
struct Follow: Codable, Hashable {
    let filmUrl: String
    let title: String
    let poster: String?
    var count: Int
    var latest: String
    var seen: Int
    /// Lúc bấm theo dõi (ms; nil = bản cũ) — đồng bộ trong nhà.
    var t: Int64? = nil

    var newCount: Int { max(0, count - seen) }
}

/// Lỗi có câu báo tiếng Việt cho người xem.
struct AppError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}
