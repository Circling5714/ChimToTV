import SwiftUI
import UIKit
import WidgetKit

/// Widget «Xem tiếp» của Côn Bằng TV: các phim đang xem dở của người xem đang dùng (app ghi sẵn trong App Group —
/// WidgetFeedData). Chạm một phim: mở app phát tiếp đúng chỗ. Cỡ nhỏ: 1 phim; vừa: 3 phim; lớn: 6 phim.
@main
struct ConBangWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "XemTiep", provider: FeedProvider()) { entry in
            FeedView(entry: entry)
        }
        .configurationDisplayName("Xem tiếp")
        .description("Phim đang xem dở — chạm để xem tiếp đúng chỗ.")
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct FeedEntry: TimelineEntry {
    let date: Date
    let feed: WidgetFeedData?
    /// Ảnh bìa đã đọc sẵn (theo tên tệp).
    let images: [String: UIImage]
}

struct FeedProvider: TimelineProvider {
    func placeholder(in context: Context) -> FeedEntry { FeedEntry(date: Date(), feed: Self.sample, images: [:]) }

    func getSnapshot(in context: Context, completion: @escaping (FeedEntry) -> Void) {
        let e = Self.entry()
        // Thư viện widget: chưa có phim nào thì cho xem mẫu
        completion(context.isPreview && (e.feed?.items.isEmpty ?? true) ? placeholder(in: context) : e)
    }

    /// App tự báo vẽ lại khi lịch sử đổi (WidgetCenter.reloadAllTimelines) — không cần hẹn giờ.
    func getTimeline(in context: Context, completion: @escaping (Timeline<FeedEntry>) -> Void) {
        completion(Timeline(entries: [Self.entry()], policy: .never))
    }

    static func entry() -> FeedEntry {
        let feed = WidgetFeedData.load()
        var images: [String: UIImage] = [:]
        if let dir = WidgetFeedData.posterFolder {
            for name in (feed?.items ?? []).compactMap({ $0.poster }) {
                if let img = UIImage(contentsOfFile: dir.appendingPathComponent(name).path) { images[name] = img }
            }
        }
        return FeedEntry(date: Date(), feed: feed, images: images)
    }

    static let sample = WidgetFeedData(
        profileId: "", profileName: "Cả nhà",
        items: [
            .init(title: "Đấu Phá Thương Khung", episode: "Tập 120", site: "yanhh3d", filmUrl: "", epUrl: "", poster: nil),
            .init(title: "Tiên Nghịch", episode: "Tập 64", site: "hoathinh3d", filmUrl: "", epUrl: "", poster: nil),
            .init(title: "Thế Giới Hoàn Mỹ", episode: "Tập 188", site: "yanhh3d", filmUrl: "", epUrl: "", poster: nil),
        ],
        note: nil, updated: 0)
}

// MARK: - Giao diện

private enum C {
    static let bg = Color(red: 0x0E / 255, green: 0x10 / 255, blue: 0x14 / 255)
    static let surface = Color(red: 0x23 / 255, green: 0x28 / 255, blue: 0x33 / 255)
    static let text = Color(red: 0xF2 / 255, green: 0xF4 / 255, blue: 0xF8 / 255)
    static let dim = Color(red: 0x9A / 255, green: 0xA3 / 255, blue: 0xB2 / 255)
    static let brand = Color(red: 0x2B / 255, green: 0xAF / 255, blue: 0x85 / 255)
}

struct FeedView: View {
    let entry: FeedEntry
    @Environment(\.widgetFamily) private var family

    private var items: [WidgetFeedData.Item] { entry.feed?.items ?? [] }

    var body: some View {
        ZStack {
            C.bg
            if items.isEmpty {
                empty
            } else {
                switch family {
                case .systemSmall: small(items[0])
                case .systemLarge: grid(Array(items.prefix(6)), rows: 2)
                default: grid(Array(items.prefix(3)), rows: 1)
                }
            }
        }
    }

    private var empty: some View {
        VStack(spacing: 8) {
            Image(systemName: "play.rectangle.on.rectangle").font(.system(size: 26)).foregroundColor(C.brand)
            Text(entry.feed?.note ?? "Mở Côn Bằng TV để xem phim.")
                .font(.system(size: 13, weight: .medium))
                .foregroundColor(C.text)
                .multilineTextAlignment(.center)
        }
        .padding(12)
        .widgetURL(WidgetFeedData.openURL)
    }

    private func poster(_ item: WidgetFeedData.Item) -> some View {
        ZStack {
            C.surface
            if let name = item.poster, let img = entry.images[name] {
                Image(uiImage: img).resizable().scaledToFill()
            } else {
                Image(systemName: "film").font(.system(size: 22)).foregroundColor(C.dim.opacity(0.6))
            }
        }
    }

    /// Cỡ nhỏ: ảnh bìa phủ kín, tên phim + tập ở dưới.
    private func small(_ item: WidgetFeedData.Item) -> some View {
        GeometryReader { geo in
            ZStack(alignment: .bottomLeading) {
                poster(item).frame(width: geo.size.width, height: geo.size.height).clipped()
                LinearGradient(colors: [.clear, .black.opacity(0.85)], startPoint: .center, endPoint: .bottom)
                VStack(alignment: .leading, spacing: 2) {
                    Label("Xem tiếp", systemImage: "play.fill")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundColor(C.brand)
                    Text(item.title).font(.system(size: 14, weight: .semibold)).foregroundColor(.white).lineLimit(2)
                    Text(item.episode).font(.system(size: 12)).foregroundColor(C.text.opacity(0.85)).lineLimit(1)
                }
                .padding(10)
            }
        }
        .widgetURL(url(item))
    }

    /// Cỡ vừa / lớn: hàng ảnh bìa, mỗi phim một link.
    private func grid(_ list: [WidgetFeedData.Item], rows: Int) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: "play.circle.fill").foregroundColor(C.brand)
                Text("Xem tiếp").font(.system(size: 13, weight: .bold)).foregroundColor(C.text)
                Spacer(minLength: 0)
                Text(entry.feed?.profileName ?? "").font(.system(size: 11)).foregroundColor(C.dim).lineLimit(1)
            }
            ForEach(0..<rows, id: \.self) { r in
                HStack(alignment: .top, spacing: 8) {
                    ForEach(0..<3, id: \.self) { c in
                        let i = r * 3 + c
                        if i < list.count {
                            cell(list[i])
                        } else {
                            Color.clear.frame(maxWidth: .infinity)
                        }
                    }
                }
            }
        }
        .padding(12)
    }

    private func cell(_ item: WidgetFeedData.Item) -> some View {
        Link(destination: url(item) ?? WidgetFeedData.openURL!) {
            VStack(alignment: .leading, spacing: 3) {
                GeometryReader { geo in
                    poster(item).frame(width: geo.size.width, height: geo.size.height).clipped()
                }
                .clipShape(RoundedRectangle(cornerRadius: 8))
                Text(item.title).font(.system(size: 11, weight: .semibold)).foregroundColor(C.text).lineLimit(1)
                Text(item.episode).font(.system(size: 10)).foregroundColor(C.dim).lineLimit(1)
            }
            .frame(maxWidth: .infinity)
        }
    }

    private func url(_ item: WidgetFeedData.Item) -> URL? {
        item.filmUrl.isEmpty ? WidgetFeedData.openURL : WidgetFeedData.playURL(item, profile: entry.feed?.profileId ?? "")
    }
}
