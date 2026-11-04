import SwiftUI
import UIKit
import ImageIO

/// Màu của app (giống bản TV: res/values/colors.xml).
enum Theme {
    static let bg = Color(hex: 0x0E1014)
    static let surface = Color(hex: 0x171A21)
    static let surface2 = Color(hex: 0x232833)
    static let text = Color(hex: 0xF2F4F8)
    static let dim = Color(hex: 0x9AA3B2)
    static let focus = Color(hex: 0xFFB020)
    static let brand = Color(hex: 0x2BAF85)
    static let onBrand = Color(hex: 0x07140F)
    static let watched = Color(hex: 0x3A6B5C)
    static let uiBg = UIColor(red: 0x0E / 255, green: 0x10 / 255, blue: 0x14 / 255, alpha: 1)
}

extension Color {
    init(hex: UInt32, alpha: Double = 1) {
        self.init(.sRGB, red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255, opacity: alpha)
    }
}

/// Ảnh bìa: bộ nhớ → đĩa (Caches/img) → mạng (gửi Referer của trang như bản TV). Thu nhỏ khi giải mã cho nhẹ RAM.
final class ImageCache {
    static let shared = ImageCache()

    private let memory = NSCache<NSString, UIImage>()
    private let dir: URL
    private let lock = NSLock()
    private var inflight: [String: Task<UIImage?, Never>] = [:]

    private init() {
        memory.countLimit = 300
        dir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent("img", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    }

    func image(_ url: String, maxPixel: CGFloat = 600) async -> UIImage? {
        let key = "\(url)#\(Int(maxPixel))" as NSString
        if let img = memory.object(forKey: key) { return img }
        lock.lock()
        if let t = inflight[key as String] {
            lock.unlock()
            return await t.value
        }
        let task = Task<UIImage?, Never>.detached(priority: .utility) { [dir] in
            let file = dir.appendingPathComponent(ImageCache.hash(url))
            var data = try? Data(contentsOf: file)
            if data == nil, let r = try? await Http.get(url, headers: ["Referer": Prefs.baseUrl + "/", "Accept": "image/*"]) {
                data = r.data
                try? r.data.write(to: file, options: .atomic)
            }
            guard let d = data else { return nil }
            return ImageCache.decode(d, maxPixel: maxPixel)
        }
        inflight[key as String] = task
        lock.unlock()
        let img = await task.value
        lock.lock()
        inflight[key as String] = nil
        lock.unlock()
        if let img = img { memory.setObject(img, forKey: key) }
        return img
    }

    static func decode(_ data: Data, maxPixel: CGFloat) -> UIImage? {
        guard let src = CGImageSourceCreateWithData(data as CFData, nil) else { return UIImage(data: data) }
        let opts: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixel,
        ]
        guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return UIImage(data: data) }
        return UIImage(cgImage: cg)
    }

    /// Tên tệp ổn định theo địa chỉ ảnh (FNV-1a 64 bit).
    static func hash(_ s: String) -> String {
        var h: UInt64 = 0xcbf29ce484222325
        for b in s.utf8 {
            h ^= UInt64(b)
            h = h &* 0x100000001b3
        }
        return String(h, radix: 16)
    }

    func clear() {
        memory.removeAllObjects()
        try? FileManager.default.removeItem(at: dir)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    }

    /// Dung lượng ảnh đã lưu (byte).
    var diskSize: Int {
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.fileSizeKey])) ?? []
        return files.reduce(0) { $0 + ((try? $1.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0) }
    }
}

struct PosterImage: View {
    let url: String?
    var maxPixel: CGFloat = 600
    @State private var image: UIImage?

    var body: some View {
        ZStack {
            Theme.surface2
            if let img = image {
                Image(uiImage: img).resizable().scaledToFill()
            } else {
                Image(systemName: "film").font(.system(size: 28)).foregroundColor(Theme.dim.opacity(0.5))
            }
        }
        .task(id: url) {
            image = nil
            if let u = url { image = await ImageCache.shared.image(u, maxPixel: maxPixel) }
        }
    }
}

/// Thông báo ngắn ở cuối màn hình (như Toast của Android).
struct ToastView: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.system(size: 16, weight: .medium))
            .foregroundColor(Theme.text)
            .multilineTextAlignment(.center)
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            .background(RoundedRectangle(cornerRadius: 12).fill(Color.black.opacity(0.85)))
            .padding(.horizontal, 40)
            .padding(.bottom, 30)
    }
}
