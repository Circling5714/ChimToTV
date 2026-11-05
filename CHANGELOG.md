# Nhật ký phiên bản

Côn Bằng TV — từ 1.0 đến 1.4 tên là ChimTo TV. Tải bản mới nhất ở [GitHub Releases](https://github.com/Circling5714/ChimToTV/releases/latest). Bản mới nhất ở trên cùng.
Mọi bản ký cùng một khoá (chứng chỉ SHA-256 `949ef2b52f3cf895d7fe5ff9db495f273e6fd35d02b23b4a521df43957a0177b`), cài đè
được lên nhau và giữ nguyên lịch sử xem.

Bản iPad / iPhone (iOS / iPadOS 15+, cài bằng TrollStore; có bản iPhone từ 1.3) ghi riêng với tiêu đề "iPad …", tag `ios-v…`.

## 2.4.1 — 05/11/2026

Sửa lỗi sau khi thử 2.4 trên điện thoại thật (vivo X200 Pro, Android 15):

- Cầm dọc: nút «‹ Quay lại» (Cài đặt, chi tiết phim) có dải riêng ở trên cùng, nội dung cuộn không còn chui xuống dưới nút.
- Menu ☰: mở ra vẫn thấy vài mục trước mục đang chọn (trước đây «Người xem», «Trang chủ» bị khuất phía trên).
- Android 15: chỗ thanh điều hướng cùng màu nền app (hết dải xám nhạt hệ thống tự phủ).
- Tìm kiếm trên điện thoại: Back tới thẳng bước «Nhấn Back lần nữa để thoát», không còn một lần bấm thừa.
- versionCode 23 · tag `v2.4.1` · commit `69ad195` · SHA-256 APK
  `b5f7437cc702d3ff4d03645318569b94a81e3dfb4ba28fd64ef5aa4b6ea99a0d`
- Đã thử trên vivo X200 Pro qua cổng gỡ lỗi của bản debug: màn chính, menu ☰, chi tiết phim, Cài đặt, Tìm kiếm, Duyệt web
  (cầm dọc), trình phát (ngang), xoay ngang. Cập nhật tự động 2.4 → 2.4.1 đã thử trên TV box.

## 2.4 — 05/11/2026

- **Dùng tốt trên điện thoại Android:** xoay dọc được (trước đây luôn nằm ngang). Cầm dọc thì màn chính giống bản iPhone:
  thanh trên cùng ☰ · tên mục · tên trang, bấm ☰ để mở menu trượt ra từ bên trái; lưới 3 cột. Chi tiết phim: poster nhỏ,
  hàng nút xuống dưới rộng cả màn. Trình phát luôn ngang, ẩn thanh điều hướng. Xoay lúc đang dùng thì tự đổi bố cục.
- Cài đặt: hàng nút tự xuống dòng khi không đủ chỗ (cả trên TV — trước đây nút cuối của hàng dài bị tràn ra ngoài),
  nút chữ dài không bị cắt; cầm dọc thì các ô proxy xếp dọc.
- «Ai đang xem?»: các ô tự xuống dòng, căn giữa; sửa lỗi có lúc phím ◀ ▶ của remote không chọn được người xem.
- Điện thoại Android 15: nội dung không bị tai thỏ và thanh điều hướng che; Duyệt web không bị bàn phím ảo che.
- versionCode 22 · tag `v2.4` · commit `9f78cef` · SHA-256 APK
  `9501df8fb614d9d3b51dafa7790fd56b03e59141a47c9b9b39d2ee511ab8b77f`
- Cập nhật tự động 2.3 → 2.4 đã thử trên TV box. Giao diện cầm dọc thử trên TV box giả lập màn điện thoại 360dp (sau đó thử
  trên điện thoại thật — xem 2.4.1).

## iPad 1.4 — 05/11/2026

- **Vượt chặn nhà mạng mức 2** (Cài đặt → Vượt chặn nhà mạng → Mức 1 / Mức 2), như TV 2.3: mẩu đầu của gói mở kết nối HTTPS
  gửi với TTL 1 (chết ở router trong nhà), phần sau đi trước, hệ thống tự gửi lại mẩu đầu — nhà mạng thấy gói thiếu đầu, sai
  thứ tự. Dành cho khi mức 1 không còn vào được trang bị chặn; qua proxy thì dùng cách mức 1.
- bản 1.4 (CFBundleVersion 5) · mã gói `vn.chimto.tv` · tag `ios-v1.4` · commit `4e9b37b` · SHA-256 IPA
  `47f4fe7f0a96d21a99f940b7ca56aaeba131ae20ce5baafe8608d6d88e905c74`

## 2.3 — 05/11/2026

- **Vượt chặn nhà mạng mức 2:** nút Cài đặt → Vượt chặn nhà mạng đổi vòng Tắt → Mức 1 (chia nhỏ) → Mức 2 (đảo thứ tự). Mức 2:
  mẩu đầu của gói mở kết nối HTTPS gửi với TTL 1 nên chết ở router trong nhà, phần sau đi trước, hệ thống tự gửi lại mẩu đầu —
  máy soi của nhà mạng thấy gói thiếu đầu, sai thứ tự. Chậm thêm vài chục mili giây; không cần VPN / root; qua proxy thì dùng
  cách mức 1. Thử trên TV box: phimmoir.fun (nhà mạng chặn theo tên) tắt vượt chặn thì lỗi, mức 2 vào được.
- versionCode 21 · tag `v2.3` · commit `4e9b37b` · SHA-256 APK
  `564656d089739e2d3fa47350bc68c67a29014f3e15a10cc200f760c73373dacf`
- Cập nhật tự động 2.2 → 2.3 đã thử trên TV box (Cài đặt → "Kiểm tra cập nhật" → "Cài đặt ngay").

## iPad 1.3 — 05/11/2026

- **Có bản cho iPhone:** cùng một tệp `ConBangTV-iPad-1.3.ipa` cài được cả iPhone và iPad. Lưới phim, trang phim, «Ai đang
  xem?», bàn phím mã PIN, màn «Hết giờ xem» co theo màn hẹp (dọc / ngang); trên iPhone trình phát luôn xem ngang, thanh nút
  gọn lại.
- **Widget «Xem tiếp»** trên màn hình chính (nhỏ 1 phim / vừa 3 phim / lớn 6 phim): phim đang xem dở của người xem đang dùng,
  chạm là mở app phát tiếp đúng chỗ.
- **Đồng bộ trong nhà** với TV (Côn Bằng TV 2.2+) hoặc iPad / iPhone khác (Cài đặt → Đồng bộ trong nhà): lịch sử xem, chỗ
  đang xem, tập đã xem, phim theo dõi, danh sách trang phim; ghép bằng mã 4 số, tự tìm máy cùng Wi‑Fi hoặc gõ IP; tự đồng
  bộ khi mở app / xem xong.
- **Cỡ chữ** (Cài đặt → Cỡ chữ: Nhỏ – Cực lớn).
- **Bàn phím rời / chuột:** mũi tên chọn phim / chọn tập, Enter mở, Delete xoá khỏi lịch sử, ⌘F / ⌘1–3 / ⌘R / ⌘O / ⌘E /
  ⌘, / Esc; trình phát thêm F / S / W; gõ mã PIN bằng bàn phím; rê chuột hiện thanh nút, cuộn để tua / chỉnh âm lượng.
- hoathinh3d.pw: các mục Phim Lẻ, Đang Chiếu, Lịch Chiếu, Hoàn Thành, Top 10, Đánh Giá Cao hiện phim trở lại (như TV 2.2).
- Cài đè giữ lịch sử xem và mật khẩu proxy đã lưu (IPA ký kèm đúng nhóm Keychain như bản cũ).
- bản 1.3 (CFBundleVersion 4) · mã gói `vn.chimto.tv` (widget `vn.chimto.tv.widget`) · tag `ios-v1.3` · commit `a4c405e` ·
  SHA-256 IPA `bdb0da1b50b381628050d95b9e4f2a75b569e10000967b0e0c8243328380eb54`

## 2.2 — 05/11/2026

- **Đồng bộ với iPad / iPhone** (Cài đặt → «Đồng bộ với iPad / iPhone…»): xem dở trên TV thì mở iPad / iPhone xem tiếp đúng
  chỗ, và ngược lại. Đồng bộ lịch sử xem, Xem tiếp, chỗ đang xem, tập đã xem, phim theo dõi, danh sách trang phim; người xem
  đầu tiên ghép với nhau, người khác ghép theo tên. Ghép bằng mã 4 số (chỉ dùng được lúc hộp thoại mở), mỗi lần đồng bộ ký
  bằng khoá chung; tự tìm máy cùng Wi‑Fi, không thấy thì nhập IP. Tự đồng bộ khi mở app / xem xong (tắt được), có «Đồng bộ
  ngay»; xoá / bỏ theo dõi ở máy này thì máy kia cũng xoá.
- **hoathinh3d.pw:** Phim Lẻ, Đang Chiếu, Lịch Chiếu, Hoàn Thành, Top 10 HH3D, Đánh Giá Cao trống vì trang đổi sang nạp phim
  bằng JavaScript — app đọc thẳng dữ liệu của trang (có trang sau; Lịch Chiếu chia theo ngày từ hôm nay, phim chiếu sớm ghi
  giờ; Top 10 ghi thứ hạng).
- versionCode 20 · tag `v2.2` · commit `a4c405e` · SHA-256 APK
  `54b448b7b168708737bbc6944dd783923afe1dac481cec81ea11da99168097e4`
- Cập nhật tự động 2.1 → 2.2 đã thử trên TV box (Cài đặt → "Kiểm tra cập nhật" → "Cài đặt ngay").

## iPad 1.2 — 05/11/2026

- **Proxy SOCKS5** (Cài đặt → Proxy SOCKS5), như bản TV: máy chủ / cổng / tên đăng nhập / mật khẩu (dán được
  `socks5://tên:mật-khẩu@máy:cổng`), DNS của proxy hoặc của iPad, dùng cho toàn bộ app hoặc chỉ trang phim, nút "Kiểm tra
  proxy" (báo IP ra ngoài). Mật khẩu lưu trong Keychain; ô mật khẩu không tự viết hoa, có nút con mắt để xem lại. Proxy
  báo "không cho phép" thì app nói rõ "thường do sai tên đăng nhập / mật khẩu".
- **Vượt chặn nhà mạng** (Cài đặt): vào được trang phim bị nhà mạng chặn theo tên trang mà không cần VPN; trang không vào
  được thì app gợi ý bật. Lúc bật proxy / vượt chặn, trên iPadOS 15–16 app không giữ cookie trang web sau khi tắt app.
- **phimmoir.fun đổi tên thành phimmois.fun:** danh sách trang, lịch sử, phim theo dõi, chỗ đang xem… tự đổi sang tên mới;
  trang tự chuyển tên miền thì app thay tên cũ trong danh sách (không thêm trùng).
- **Cử chỉ lúc xem:** vuốt ngang để tua (hiện giờ tới), vuốt dọc nửa trái chỉnh độ sáng / nửa phải chỉnh âm lượng, giữ tay
  xem nhanh 2×, chụm / mở hai ngón: vừa / lấp đầy màn hình. Vuốt bắt đầu sát mép trên / dưới vẫn để cho iPad.
- **Màn hình khoá, Trung tâm điều khiển, AirPods:** tên phim, tập, ảnh phim; phát / dừng, tua, kéo thanh thời gian, tập
  trước / sau (AirPods chạm hai lần: tập sau).
- **Giới hạn giờ xem** cho từng người xem (Cài đặt → Người xem → "Giới hạn giờ xem…"), như bản TV 2.1.
- **Chọn trang phim:** thêm lại trang mẫu đã xoá, gõ tên miền để thêm trang khác, vuốt trái một trang để xoá.
- **Tìm kiếm trên mọi trang:** trước đây chỉ ra kết quả của một trang và chậm — nay kết quả hiện dần theo từng trang,
  tối đa 3 trang cùng lúc, mỗi trang chờ tối đa 12 giây.
- **Hình trong hình:** vuốt lên từ mép dưới lúc phim đang phát thì chắc chắn thu nhỏ phim (lần vuốt đầu dành cho app);
  phim dừng / chưa chạy thì vuốt lên về thẳng màn hình chính. Tên người xem luôn hiện ở đầu menu.
- bản 1.2 (CFBundleVersion 3) · mã gói `vn.chimto.tv` · tag `ios-v1.2` · commit `52a15e9` · SHA-256 IPA
  `fd412842d36c0f51b1242d5b49d43cf204b60b999177551d3e383dab804e2cf8`

## 2.1 — 05/11/2026

- **Vượt chặn nhà mạng** (Cài đặt → Vượt chặn nhà mạng): một số nhà mạng chặn trang phim bằng cách đọc tên trang lúc mở
  kết nối HTTPS (trang mở mãi không được, dùng VPN thì vào được). Bật mục này: app chia nhỏ gói mở kết nối nên nhà mạng
  không đọc được tên trang — không cần VPN hay proxy; dùng được cùng DNS mã hoá và proxy. Trang không vào được (quá giờ /
  lỗi SSL) thì app gợi ý bật mục này.
- **phimmoir.fun đổi tên thành phimmois.fun:** trang mẫu đổi theo; danh sách trang, trang đang dùng của từng người xem,
  lịch sử, Xem tiếp, phim theo dõi, chỗ đang xem, server ưu tiên, điểm server tự đổi sang tên mới khi mở app. Trang tự
  chuyển tên miền thì app thay tên cũ trong danh sách (trước đây thêm tên mới bên cạnh).
- **Giới hạn giờ xem** cho từng người xem (Cài đặt → Người xem → "Giới hạn giờ xem…"): đặt **mã PIN phụ huynh**, rồi chọn
  số phút được xem mỗi ngày (30 phút – 3 giờ) và giờ đi ngủ (không xem được tới 6 giờ sáng). Chỉ đếm lúc phim thật sự chạy;
  còn 5 phút thì nhắc; hết giờ thì phim dừng, màn "Hết giờ xem" cho người lớn nhập mã để cho xem thêm 15 / 30 / 60 phút
  hoặc không giới hạn tới 6 giờ sáng. Đã có giới hạn thì thêm / xoá người xem cũng cần mã phụ huynh.
- **Đổi trang phim:** thêm "＋ Thêm lại: …" (trang mẫu đã xoá) và "＋ Thêm trang khác…" (gõ tên miền); Cài đặt có nút
  thêm lại trang mẫu.
- **Proxy:** proxy trả "không cho phép" khi có đăng nhập thì app báo "thường do sai tên đăng nhập / mật khẩu" (nhiều proxy
  nhận mọi mật khẩu lúc đăng nhập rồi mới từ chối).
- **Tìm kiếm trên mọi trang:** kết quả hiện dần theo từng trang (ghi rõ trang nào đang tìm / không có phim / không trả
  lời); hỏi cùng lúc tối đa 3 trang, mỗi trang chờ tối đa 12 giây — một trang chậm hay bị chặn không làm chậm cả lượt tìm.
- Mục **Người xem** luôn hiện ở đầu menu, kể cả khi mới có một người xem.
- versionCode 19 · tag `v2.1` · commit `52a15e9` · SHA-256 APK
  `0b7c1aad0b849a4270eebbbdb6e2fa95a0f3988a98cd4b8fe5ea2bf0c53f5d2d`
- Cập nhật tự động 2.0 → 2.1 đã thử trên TV box (Cài đặt → "Kiểm tra cập nhật" → "Cài đặt ngay").

## iPad 1.1 — 05/11/2026

- **Người xem cho cả nhà** (như bản TV 2.0): mỗi người một hồ sơ (tên + màu) với lịch sử xem, Xem tiếp, chỗ đang xem, phim
  theo dõi, trang phim, server ưu tiên, Xem riêng, cách xem riêng; lịch sử cũ thành của người đầu tiên "Cả nhà". Mở app
  (và quay lại sau 30 phút) hỏi **"Ai đang xem?"** khi có từ 2 người; đổi người bằng tên người xem ở đầu menu; thêm / sửa /
  xoá ở Cài đặt → Người xem. **Mã PIN** 4 số tuỳ từng người, gõ trên bàn phím số ngay trên màn hình.
- **Hình trong hình:** nút thu nhỏ cạnh nút Menu của trình phát — phim chạy tiếp ở cửa sổ nổi của iPad, vẫn chọn phim khác
  / dùng app khác được; nút phóng to của cửa sổ mở lại màn xem phim, nút đóng lưu chỗ đang xem. Vuốt về màn hình chính
  lúc đang xem là tự thu nhỏ (tắt được: Cài đặt → Phát video).
- bản 1.1 (CFBundleVersion 2) · mã gói `vn.chimto.tv` · tag `ios-v1.1` · commit `0edf596` · SHA-256 IPA
  `c2ce6ebe992451ad80b3f53927d8ccd782fd156ba11a669fc8719fe7bb7239d1`

## 2.0 — 05/11/2026

- **Người xem cho cả nhà:** TV dùng chung thì mỗi người một hồ sơ (tên + màu, tối đa 8) với lịch sử xem, hàng Xem tiếp,
  chỗ đang xem dở, phim theo dõi, trang phim đang dùng, server ưu tiên, Xem riêng, bước tua, tự chuyển tập, khung hình
  riêng. Danh sách trang phim, proxy, DNS, dữ liệu duyệt web, điểm server, mốc nhạc mở đầu dùng chung.
  - Lịch sử có từ trước thành của người xem đầu tiên "Cả nhà" — cài đè không mất gì.
  - Nhà có từ 2 người: mở app (và quay lại app sau 30 phút không dùng) thì hỏi **"Ai đang xem?"** — tắt được trong Cài
    đặt. Đổi người: mục "Người xem: …" đầu menu. Thêm / đổi tên / đổi màu / xoá: Cài đặt → Người xem.
  - **Mã PIN** 4 số bật / tắt cho từng người: người có mã phải nhập đúng mới mở / sửa / xoá được hồ sơ.
- **Thu nhỏ phim vào góc:** lúc xem nhấn ▼ → "Thu nhỏ vào góc" (hoặc phím Window) — phim chạy tiếp ở góc, vẫn chọn phim
  khác được; đầu menu có "Phóng to" / "Tắt phim thu nhỏ", phím phát / dừng / chuyển tập của remote điều khiển phim nhỏ.
  - TV / máy có hình trong hình của Android: cửa sổ nhỏ của hệ thống (nổi cả trên app khác); bấm Home lúc đang xem là tự
    thu nhỏ (tắt được trong Cài đặt).
  - TV không có (TV box ít RAM như TX3, TV Skyworth…): cửa sổ phim nhỏ ngay trong app — rời app thì phim dừng, thoát app
    thì phim tắt.
- Chọn phim khác lúc đang thu nhỏ thì phim đó mở ngay trong trình phát đang chạy (lưu chỗ xem phim cũ).
- versionCode 18 · tag `v2.0` · commit `0edf596` · SHA-256 APK
  `bf86165ba925f40d959f8df9cc0db359d00ea66f9657c6eb75a6ba65f6078c33`

## iPad 1.0 — 04/11/2026

- **Bản đầu tiên cho iPad** (iPadOS 15 trở lên, cài bằng TrollStore — tải `ConBangTV-iPad-1.0.ipa`, mở bằng TrollStore):
  cùng 5 trang phim và các tính năng chính của bản TV, điều khiển bằng chạm và bàn phím rời — trang chủ, thể loại, lịch
  chiếu, tìm trên mọi trang, trang phim (Việt Sub / Thuyết Minh, đến tập), Xem tiếp, phim theo dõi + Có tập mới, Đổi
  trang phim (đo tốc độ), Duyệt web, Xem riêng, tự xoá dữ liệu web.
- Trình phát: chạm để hiện thanh nút (tập trước / sau, lùi / tới 10 giây, dừng / phát, Menu, Thoát), kéo thanh thời gian,
  chạm đúp hai bên để tua; menu đổi server (kèm điểm), đổi bản, nhạc mở đầu, tốc độ, khung hình, hẹn giờ tắt; xem tiếp
  đúng chỗ, tự sang tập, gợi ý đổi server khi giật, chặn quảng cáo; trình phát phủ kín màn iPad.
- Tự kiểm tra bản iPad mới trên GitHub và mở TrollStore để cài đè.
- Chưa có: proxy SOCKS5, DNS mã hoá (iOS 15 không cho app đặt riêng cho trình duyệt trong app).
- bản 1.0 (CFBundleVersion 1) · mã gói `vn.chimto.tv` · tag `ios-v1.0` · commit `2ac27fc` · SHA-256 IPA
  `649f3d49c33959ec71428bb299757ebf02a64597b29d6541c8da9a5cff6384d1`

## 1.9.4 — 04/11/2026

- **Hàng Xem tiếp / Lịch sử xem:** phim của hoathinh3d.ee và hoathinh3d.pw trước đây cùng ghi "· hoathinh3d" — nay ghi
  đủ tên miền (vd "Linh Cảnh Hành Giả · hoathinh3d.pw"); phim của các trang khác vẫn ghi tên ngắn.
- Chặn thêm quảng cáo trong trang tập của hoathinh3d.pw (`atroblxac.in`, gặp khi thử bản iPad).
- versionCode 17 · tag `v1.9.4` · commit `2ac27fc` · SHA-256 APK
  `fbb10fedfaa0effe4764e1165e8cedc36736bf2c12de9d55b030bdd9eb0d95b6`

## 1.9.3 — 04/11/2026

- **Điều khiển bằng màn hình cảm ứng** (TV box nối màn cảm ứng, điện thoại) — trước đây màn xem phim không điều khiển
  được bằng chạm và cả app không có nút quay lại trên màn hình:
  - Màn xem phim: chạm vào phim để hiện / ẩn thanh nút to — Tập trước, lùi 10 giây, Dừng / Phát, tới 10 giây, Tập sau,
    Menu (menu ▼: server, bản phim, tốc độ, nhạc mở đầu…), Thoát — và thanh thời gian kéo để tua; chạm đúp nửa trái /
    phải: lùi / tới 10 giây. Thanh tự ẩn sau 5 giây, phim đang dừng thì ở lại. Chạm không còn lọt vào trang web / trình
    phát của trang (khỏi bấm nhầm quảng cáo).
  - Các màn khác: có chạm thì hiện nút **"‹ Quay lại"** ở góc trên (trang chủ: **"Thoát"**; Duyệt web: góc dưới trái,
    khỏi che thanh đầu của trang web); bấm remote / bàn phím là nút tự ẩn. Duyệt web ẩn con trỏ của remote khi đang chạm.
  - Remote và bàn phím dùng như cũ; nút trên màn hình đổi màu khi đang nhấn.
- versionCode 16 · tag `v1.9.3` · commit `d0f531c` · SHA-256 APK
  `05e4ddaa3d1c898595bce4595b4c7f33a4e4ae9eba91330169860f75518699bf`

## 1.9.2 — 04/11/2026

- **Sửa lỗi phím ESC của bàn phím rời** (điện thoại / TV box cắm bàn phím): ESC giờ luôn là Back ở mọi màn hình —
  thoát được Cài đặt và màn xem phim (anh báo trên Samsung S10e gắn bàn phím). Trước đây app trông vào việc Android tự
  đổi ESC thành Back, mà Android chỉ đổi khi không ai nhận ESC trước: trong màn xem phim, WebView chuyển ESC cho trang
  web / trình phát; trong Cài đặt, ô nhập (bàn phím ảo) có thể nhận. Giờ app tự xử lý ESC; trong màn xem phim, ESC khi
  đang mở menu ▼ thì đóng menu, đang hiện gợi ý đổi server thì bỏ gợi ý. Phím Back của remote không đổi.
- versionCode 15 · tag `v1.9.2` · commit `5d2a70b` · SHA-256 APK
  `90c18b47f147f7362df35d684262665ea47da9b24e739612b1e5cc6fc0bb095b`

## 1.9.1 — 04/11/2026

- **Tìm trên mọi trang:** mục Tìm kiếm tìm cùng lúc trên mọi trang trong danh sách, mỗi trang một nhóm ("yanhh3d.ee —
  18 phim"), trang đang dùng đứng đầu; trang lỗi / quá 20 giây thì bỏ qua.
- **Menu ▼ trong trình phát:** **Tốc độ** 1× → 1,25× → 1,5× → 2× → 0,75× (cho cả lần xem này); **Khung hình**: Vừa màn
  hình / Lấp đầy (phim màn ảnh rộng 21:9 phóng cho hết viền đen, cắt bớt hai bên; app nhớ lựa chọn); **Hẹn giờ tắt**:
  30 / 60 / 90 phút hoặc hết tập này — hết giờ thì dừng phim, 60 giây không ai bấm phím thì ra màn hình chính của TV.
- **phimmoir.fun:** menu có thêm **Thể loại…** (đủ 36 thể loại) và **Quốc gia…** (mọi nước, bỏ mục rác như năm / mã).
- **Xem tiếp nhanh hơn:** app báo trước chỗ đang xem cho trình phát (trình phát streamfree của yanhh3d / hoathinh3d.ee /
  hhkungfu và server KK Phim) → tải thẳng đoạn video ở chỗ đó thay vì tải đoạn đầu tập rồi mới tua (mỗi đoạn trên CDN
  có khi mất 6–7 giây trên TV box); phim đã đánh dấu nhạc mở đầu cũng bắt đầu thẳng ở chỗ hết nhạc. NguonC vẫn tua như
  cũ.
- **Riêng tư:** **DNS mã hoá (DoH)** trong Cài đặt (Cloudflare 1.1.1.1 / Google 8.8.8.8, qua HTTPS, cả khi không bật
  proxy; lỗi thì báo, không quay về DNS nhà mạng); **mật khẩu proxy mã hoá** bằng khoá Android Keystore (Android 6+, tự
  chuyển từ bản lưu chữ thường); chặn thêm trình theo dõi / quảng cáo đo được trên 5 trang (thống kê Cloudflare, JW
  Player, Ahrefs, quảng cáo Adcash…).
- versionCode 14 · tag `v1.9.1` · commit `c3e3da7` · SHA-256 APK
  `48f6375241806c3023d411c2bff64a3dc7526485962657e7dffdce00b35011e8`

## 1.9 — 04/11/2026

- **Phim theo dõi + báo tập mới:** trang phim có nút "＋ Theo dõi"; menu có mục **Phim theo dõi** (ghi "Theo dõi (2 mới)"
  khi có phim ra tập). App hỏi lại trang các phim đang theo dõi khi mở trang chủ (tối đa 30 phút một lần); phim có tập
  mới hiện ở hàng **"Có tập mới"** đầu trang chủ (vd "Tập 166 mới", "+2 · Tập 168") kèm thông báo; mở phim là hết "mới".
  Giữ OK / nhấn Menu trên phim theo dõi để bỏ theo dõi.
- **Bỏ qua nhạc mở đầu:** lúc xem nhấn ▼ → hàng "Nhạc mở đầu": "Bắt đầu ở …" (nếu nhạc không nằm ngay đầu tập) và
  "Hết nhạc ở …". Mốc lưu cho cả phim; các tập sau vừa chạy tới mốc đầu là tự tua qua (◀ để xem lại). Xem tiếp từ giữa
  tập thì không tua; "Bỏ mốc" để tắt.
- **Sửa lỗi trên TV chậm:** xem tiếp một tập xem dở mà thoát / chuyển tập trước khi trình phát kịp tua tới chỗ cũ thì
  chỗ xem dở bị ghi đè bằng giây đầu tập; chuyển tập có thể ghi giờ của tập cũ sang tập mới (trang tập cũ còn trả lời
  vài giây). Cả hai đã sửa.
- versionCode 13 · tag `v1.9` · commit `5c5bf8c` · SHA-256 APK
  `b221ffc045d4c9161f60f64211b3588bdc9fade237d6030247441bca2d18be7a`

## 1.8.2 — 04/11/2026

- **Hàng "Xem tiếp"** ở đầu trang chủ (5 phim xem gần nhất, mọi trang): bấm là phát tiếp đúng chỗ, tập đã xem hết thì
  sang tập kế; xem xong quay ra hàng tự cập nhật. Phim của trang khác ghi thêm tên trang (vd "Tiên Nghịch · yanhh3d").
- **Xoá từng phim khỏi lịch sử:** giữ OK (hoặc nhấn Menu) trên phim ở Xem tiếp / Lịch sử xem — xoá cả chỗ đang xem
  các tập của phim đó.
- **Đến tập theo số:** trang phim có nút "Đến tập…" (phim trên 20 tập) và nhận phím số trên remote (gõ 1 2 0 → Tập 120).
- **Cài đặt → Dữ liệu:** "Xem riêng" (không lưu lịch sử / chỗ đang xem) và "Tự xoá dữ liệu web khi rời app" (mặc định
  tắt).
- **Riêng tư:** không sao lưu dữ liệu app lên đám mây / sang máy mới / qua `adb backup`; WebView không gửi số liệu sử
  dụng của app về Google.
- Sửa lỗi văng trên TV Android 5 (màu chữ / ảnh bìa dùng hàm chỉ có từ Android 6).
- versionCode 12 · tag `v1.8.2` · commit `99513d3` · SHA-256 APK
  `71b83c314b6bb3e414415b6da48c531daef6d194fffb9d15ee03c2520068efbb`

## 1.8.1 — 04/11/2026

- **Xoá dữ liệu web nhanh:** mục mới **Xoá dữ liệu web** ở menu chính (ngay trên Cài đặt), hỏi xác nhận một lần rồi
  xoá cookie, dữ liệu các trang đã lưu và bộ nhớ đệm của trình duyệt trong app. Lịch sử xem, chỗ đang xem dở, cài đặt
  và proxy giữ nguyên.
- Nút trong Cài đặt đổi thành **"Xoá ảnh & dữ liệu duyệt web"**: trước chỉ xoá ảnh và cookie, nay xoá cả dữ liệu
  trang và bộ nhớ đệm.
- **Riêng tư hơn:** cả app dùng chung một User-Agent rút gọn, không còn gửi tên mẫu TV hay số bản build Android;
  client hints ghi "Chromium" thay vì "Android WebView"; khi app tự tải trang hộ trình phát thì không gửi tên gói app.
  (Yêu cầu do WebView tự gửi vẫn kèm tên gói trên WebView 138 — giới hạn của WebView.)
- versionCode 11 · tag `v1.8.1` · commit `5700ff5` · SHA-256 APK
  `8941f6eb1c200608252592a00bdf42d6c72c82b324b1ea0aabfae5f09730e1b1`

## 1.8.0 — 03/11/2026

- **Proxy SOCKS5** (Cài đặt → Proxy SOCKS5, mặc định tắt): bật / tắt; proxy có hoặc không tên đăng nhập / mật khẩu
  (dán được cả `socks5://user:pass@máy-chủ:cổng`); **DNS của proxy** (proxy tự tìm địa chỉ trang, vượt chặn tên miền
  của nhà mạng) hoặc **DNS của thiết bị**; **dùng cho toàn bộ app** hoặc **chỉ trang phim** (video, ảnh CDN, cập nhật
  app đi thẳng mạng nhà). Nút "Kiểm tra proxy" báo IP ra ngoài, nơi đặt máy và thời gian.
- Proxy lỗi (chết, sai mật khẩu) thì app báo rõ, không tự đi thẳng; màn hình chính ghi "· proxy" khi đang bật.
- Máy chủ video có thể chặn IP của proxy (vd CDN của KK Phim trên phimmoir.fun): app gợi ý chọn "chỉ trang phim";
  điểm server khi xem qua proxy được chấm riêng, không làm app né server đó lúc xem thẳng.
- versionCode 10 · tag `v1.8.0` · commit `e0f1662` · SHA-256 APK
  `fda0f275eeb8c889a9af6e73bc502391f4722a329f74c3eb55104b0c6939af16`

## 1.7.1 — 03/11/2026

Sửa lỗi của bản 1.7 (rà soát code sau phát hành):

- Server không phát được thì thông báo lỗi không còn bị lời nhắc "Video chưa chạy?" đè mất sau 25 giây; lời nhắc
  cũng không bật ngay sau khi trình phát vừa chạy xong quảng cáo dài.
- Mạng chập chờn lúc mở tập không còn bị coi là server hỏng (app tự thử tải lại vài lần trước khi báo lỗi); đang
  xem phim dài mà mạng rớt nhiều lần cách xa nhau thì vẫn tự phát tiếp.
- Thoát / đổi server lúc trình phát đang chạy quảng cáo không còn làm server bị chấm "hay lỗi".
- hoathinh3d.pw: video nạp xong mà không tự phát thì app nhắc nhấn OK, không còn kẹt ở màn "Đang mở trang tập phim…".
- phimmoir.fun: không mất nhóm Thuyết Minh / Lồng Tiếng của phim mà bản đó không bắt đầu từ tập 1; một ảnh bìa lỗi
  không làm hỏng cả danh sách; khối quảng cáo nằm cuối phim không làm hỏng thanh thời gian / tự sang tập.
- Trang tải lỗi hẳn (tên miền chết…) không còn bắt chờ gấp đôi; Cài đặt ghi thêm trang kiểu PhimMoi.
- versionCode 9 · tag `v1.7.1` · commit `a929b1d` · SHA-256 APK
  `f8beb32160be9fc7e1a5b7e2ff7fb4e37f8ccb8b0e0824ad85787d41ef7db78e`

## 1.7 — 03/11/2026

- **Trang phim mới phimmoir.fun** (chọn ở **Đổi trang phim**; TV đã cài tự có trang này trong danh sách sau khi cập
  nhật): hoạt hình 3D Trung Quốc — khối "Hoạt hình Trung Quốc mới cập nhật" ở đầu trang chủ — và cả phim bộ / phim
  lẻ Hàn, Trung, Âu Mỹ, Nhật, Thái. Menu có Hoạt hình, Phim bộ mới, Phim lẻ mới, Phim Hàn Quốc / Trung Quốc / Âu
  Mỹ / Nhật Bản / Thái Lan và 10 thể loại; tìm kiếm theo tên. Có Vietsub, Thuyết Minh, Lồng Tiếng và Song Ngữ (có
  phụ đề tiếng Việt).
- **Video mở nhanh, không quảng cáo cờ bạc:** server KK Phim mở khoảng 5 giây trên TV box (trang yanhh3d ~17 giây);
  app tự bỏ đoạn quảng cáo 30 giây chèn giữa phim. Server NguonC: app tự bỏ qua quảng cáo trước phim (~7 giây, trước
  phải xem hết hơn 2 phút) và xem tiếp đúng chỗ.
- Xem tiếp đúng chỗ cả khi trình phát chạy quảng cáo trước; server không phát được (vd OPhim đang hỏng) thì app báo
  ngay, mở menu đổi server, và lần sau tự mở server khác cùng ngôn ngữ.
- Đổi Vietsub ↔ Thuyết Minh mà bản kia chưa có tập đang xem thì app báo rõ, mở tập gần nhất của bản đó (không mang
  giờ đang xem sang tập khác).
- Lưu ý: bản Vietsub KK của một số phim hoạt hình là video rộng 2542 điểm — TV box yếu (TX3) rớt nhiều khung hình,
  hơi khựng; chọn Thuyết Minh hoặc NguonC nếu bị. TV Skyworth (đã chạy tốt 4K) không ảnh hưởng.
- versionCode 8 · tag `v1.7` · commit `b103bbf` · SHA-256 APK
  `5166cd79293ebad818ad9497541eda98c76a90261ac152637d588690477e920a`

## 1.6 — 03/11/2026

- **Video 1080 mượt hơn trên yanhh3d.ee, hoathinh3d.ee, hhkungfu.ee** (3 trang dùng chung trình phát streamfree): trình
  phát này trước chỉ tải trước đúng 1 đoạn phim (1–13 giây), mạng TV hụt một nhịp là đứng hình; nay tải trước tới
  2 phút. Đo trên TV box TX3, cùng tập, cùng server Thuyết minh 1080, 3 phút: trước giật 12 lần (18 giây), nay 0 lần.
- Thời gian mở phim không đổi; hoathinh3d.pw (trình phát riêng) không bị ảnh hưởng.
- versionCode 7 · tag `v1.6` · commit `6dc1522` · SHA-256 APK
  `774a9395501e5d623be6614020c890ffd75c5039f5c5f32a977c8a8faaee661d`

## 1.5 — 02/11/2026

- **ChimTo TV đổi tên thành Côn Bằng TV** (鯤鵬 — chim thần khổng lồ trong truyện tiên hiệp). Vẫn là app cũ: cài đè, giữ
  nguyên lịch sử xem, cài đặt và điểm server; sau khi cập nhật, tên và banner trên màn hình chính của TV đổi theo.
- **Logo mới:** ấn triện đỏ chữ 鵬 trong vòng bút mực vàng; banner mới "Côn Bằng TV".
- **Liên hệ:** Cài đặt → Liên hệ → nhóm Telegram [t.me/tivikit](https://t.me/tivikit) (mở ngay trong app).
- Mã gói, khoá ký và tên tệp APK (`ChimToTV-1.5.apk`) giữ nguyên, nên bản 1.3 / 1.4 tự cập nhật lên như thường.
- versionCode 6 · tag `v1.5` · commit `83bc2a5` · SHA-256 APK
  `2908a6c60c7f72058c83237934044f1fd7a3e6166f5552c6778629456c8e4497`

## 1.4 — 02/11/2026

- **Biết trang nào nhanh hơn:** mở **Đổi trang phim** là app đo cùng lúc mọi trang: tải trang chủ mất bao lâu, đọc được
  bao nhiêu phim (0 phim = có thể đang bị nhà mạng chặn), trang nhanh nhất có dấu ★, kèm điểm video các lần xem trước.
- **Chấm điểm server video:** app tự ghi lại mỗi server mở mất bao lâu, giật mấy lần, đứng hình bao lâu (8 lần xem gần
  nhất, riêng từng TV). Trong lúc xem nhấn ▼ để thấy điểm từng server (vd "mượt, mở 4 s") và tình hình lần xem này; TV
  rớt nhiều khung hình thì app báo "TV xử lý không kịp, đổi server không đỡ".
- **Gợi ý đổi server khi giật nhiều** (từ 3 lần trong 3 phút, hoặc đứng hình liền 10 giây): **OK** để chuyển sang server
  tốt hơn và xem tiếp đúng chỗ, **Back** để bỏ qua. Chỉ gợi ý server cùng ngôn ngữ (không tự đổi Vietsub ↔ Thuyết minh)
  và không gợi ý server nét hơn (4K) khi chưa thử.
- Mở tập mới: app vẫn dùng server đã chọn, chỉ đổi khi server đó hay giật mà có server khác đã chấm tốt (có báo lý do).
- Bản 1.3 tự cập nhật lên (hoặc Cài đặt → "Kiểm tra cập nhật"); bản 1.2 trở về trước cài tay một lần.
- versionCode 5 · tag `v1.4` · commit `db9e94d` · SHA-256 APK
  `27b3529ba5b0546d4d099357fee3adc510a6e12e26a5a81861f4c641c95e4523`

## 1.3 — 02/11/2026

- **Tự động cập nhật (OTA):** app tự tìm bản mới trên GitHub khi mở (tối đa 2 giờ một lần), tải sẵn, kiểm tra cùng khoá ký
  rồi hỏi "Cài đặt ngay". Lần đầu Android hỏi cho phép "cài ứng dụng không rõ nguồn"; mỗi lần cập nhật cần bấm xác nhận
  (Android 12+ thì từ lần thứ hai tự động hẳn). Cài đặt có nút "Kiểm tra cập nhật" và công tắc tự động cập nhật.
- **Lưới phim mượt hơn trên TV yếu:** khung hình giật khi di chuyển giảm từ 3,6–7,3% xuống 0–1,3%, khung chậm nhất từ
  48 ms xuống 16 ms (đo trên TV Skyworth SWTV-22AE).
- **Lưu ý:** TV đang dùng bản 1.2 trở về trước phải cài tay 1.3 một lần; từ đó các bản sau tự cập nhật.
- versionCode 4 · tag `v1.3` · commit `b194ecb` · SHA-256 APK
  `eac3d276d7b26f94e54aa13a6bf62eb87b9da7dde73ca33b072d5c7c50fdc755`

## 1.2 — 02/11/2026

- **Nhiều trang phim:** yanhh3d.ee, hoathinh3d.ee, hhkungfu.ee, hoathinh3d.pw — chọn ở mục **Đổi trang phim**; thêm trang
  mới cùng kiểu (yanhh3d / theme HaLim) trong Cài đặt.
- Trang chia tập theo bản: chọn **Việt Sub / Thuyết Minh** ở trang phim hoặc trong menu ▼ khi đang xem.
- hoathinh3d.pw phát bằng cách mở thẳng trang tập rồi phóng trình phát ra toàn màn hình (trình phát có khoá xác thực riêng).
- Server ưu tiên nhớ theo từng trang; tự gỡ kẹt khi video treo sau lúc tua.
- versionCode 3 · tag `v1.2` · commit `8cb96ef` · SHA-256 APK
  `ce71d9b99264a4cb3c9104d1be96e405e7356cc5f31906d8352f71003a991e98`

## 1.1 — 02/11/2026

- **Logo mới** phong cách hoạt hình 3D tiên hiệp (ấn triện chữ 仙 trong vòng bút mực vàng): banner TV đủ cỡ 720p → 4K,
  icon thích ứng Android 8+, logo ở đầu menu.
- **Poster nét hơn trên TV 4K:** tải ảnh gốc thay cho ảnh thu nhỏ 300×450.
- versionCode 2 · tag `v1.1` · commit `319113b` · SHA-256 APK
  `370b783d56efd68f2063da98b7356336724ec286f4788df5bb380e5306e9da38`

## 1.0 — 02/11/2026

- Bản đầu tiên: giao diện TV (menu thể loại, lưới poster, tìm kiếm, lịch chiếu theo ngày, lịch sử xem), trang phim với
  "Xem tiếp", trình phát điều khiển bằng remote / bàn phím (dừng–phát, tua, đổi server, nhớ vị trí, tự chuyển tập),
  chế độ Duyệt web bằng con trỏ ảo, Cài đặt đổi link trang phim.
- versionCode 1 · tag `v1.0` · commit `5d9b87b` · SHA-256 APK
  `94975cd3220a96b31e24d5b41af0a7892d33c3a91dcc9c25343c8fd58f997e24`
