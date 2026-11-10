import Foundation

/// Miễn trừ trách nhiệm & quyền riêng tư — cùng nội dung với bản TV (Disclaimer.kt), Windows (Disclaimer.cs) và README (mục
/// «Miễn trừ trách nhiệm»). Sửa nội dung đáng kể thì tăng `version`: mọi máy hiện lại màn «Trước khi bắt đầu» một lần.
enum Disclaimer {
    static let version = 1
    static let title = "Miễn trừ trách nhiệm & quyền riêng tư"
    static let date = "11/2026"

    static let sections: [(title: String, body: String)] = [
        ("1. App chỉ là trình xem",
         "Côn Bằng TV là ứng dụng miễn phí giúp xem các trang phim công khai trên Internet bằng remote hoặc màn hình cảm ứng. App không lưu trữ, không tải lên, không sở hữu và không phát tán phim, hình ảnh hay phụ đề nào. Mọi nội dung do các trang web bên thứ ba cung cấp và nằm trên máy chủ của họ."),
        ("2. Bản quyền thuộc chủ sở hữu",
         "Tên phim, hình ảnh, video và nhãn hiệu thuộc về chủ sở hữu hợp pháp. Côn Bằng TV không liên kết với, không được tài trợ hay bảo trợ bởi trang web, hãng phim hay nhà phát hành nào. Chủ sở hữu muốn app ngừng hỗ trợ một trang, xin liên hệ ở mục 9."),
        ("3. Người dùng tự chịu trách nhiệm",
         "Bạn tự chọn trang để xem và tự tuân thủ pháp luật về bản quyền, sử dụng Internet nơi bạn sinh sống. Khi có thể, hãy ủng hộ nhà sản xuất bằng cách xem trên nền tảng chính thức."),
        ("4. Trang bên thứ ba",
         "Nhà phát triển không kiểm soát và không chịu trách nhiệm về nội dung, quảng cáo, đường dẫn hay mức độ an toàn của các trang bên thứ ba. App có lọc quảng cáo nhưng không bảo đảm chặn hết."),
        ("5. Tính năng mạng",
         "Proxy SOCKS5 / Shadowsocks, Vượt chặn nhà mạng và DNS mã hoá là công cụ kết nối nhằm bảo vệ quyền riêng tư và khắc phục lỗi kết nối. App không cung cấp máy chủ proxy. Bạn tự chịu trách nhiệm dùng các tính năng này đúng pháp luật và đúng điều khoản của nhà mạng / nhà cung cấp máy chủ."),
        ("6. Dữ liệu và quyền riêng tư",
         "App không có tài khoản, không thu thập hay bán dữ liệu cá nhân, không gắn công cụ theo dõi. Lịch sử xem, hồ sơ người xem và cài đặt chỉ lưu trên thiết bị; «Đồng bộ trong nhà» truyền thẳng giữa các máy trong mạng nhà. App chỉ kết nối tới: các trang phim bạn chọn; GitHub để kiểm tra bản mới; dịch vụ DNS mã hoá (NextDNS, Cloudflare, Google — tắt được trong Cài đặt); Cloudflare khi bạn bấm «Kiểm tra proxy». DNS mã hoá mặc định dùng hồ sơ NextDNS riêng của app, không lưu nhật ký truy cập."),
        ("7. Không bảo đảm",
         "App được cung cấp miễn phí, «nguyên trạng», không kèm bảo đảm nào. Trang phim có thể đổi, ngừng hoạt động hoặc bị chặn bất cứ lúc nào. Ở mức tối đa pháp luật cho phép, nhà phát triển không chịu trách nhiệm cho thiệt hại phát sinh từ việc dùng app."),
        ("8. Trẻ em",
         "Hồ sơ trẻ em và giới hạn giờ xem chỉ hỗ trợ cha mẹ, không thay thế sự giám sát của người lớn. Nội dung từ trang bên thứ ba có thể không phù hợp với trẻ."),
        ("9. Liên hệ",
         "Góp ý, báo lỗi hoặc yêu cầu về bản quyền: Telegram t.me/tivikit."),
    ]

    static let consent = "Tiếp tục sử dụng app nghĩa là bạn đồng ý với các điều trên."

    /// Chưa đồng ý bản hiện tại (máy mới cài, hoặc cập nhật từ bản chưa có màn này / nội dung vừa đổi).
    static var needed: Bool { Prefs.disclaimerVersion < version }

    static func accept() { Prefs.disclaimerVersion = version }
}
