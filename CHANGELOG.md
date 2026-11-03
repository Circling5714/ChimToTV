# Nhật ký phiên bản

Côn Bằng TV — từ 1.0 đến 1.4 tên là ChimTo TV. Tải bản mới nhất ở [GitHub Releases](https://github.com/Circling5714/ChimToTV/releases/latest). Bản mới nhất ở trên cùng.
Mọi bản ký cùng một khoá (chứng chỉ SHA-256 `949ef2b52f3cf895d7fe5ff9db495f273e6fd35d02b23b4a521df43957a0177b`), cài đè
được lên nhau và giữ nguyên lịch sử xem.

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
