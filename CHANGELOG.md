# Nhật ký phiên bản

Côn Bằng TV — từ 1.0 đến 1.4 tên là ChimTo TV. Tải bản mới nhất ở [GitHub Releases](https://github.com/Circling5714/ChimToTV/releases/latest). Bản mới nhất ở trên cùng.
Mọi bản ký cùng một khoá (chứng chỉ SHA-256 `949ef2b52f3cf895d7fe5ff9db495f273e6fd35d02b23b4a521df43957a0177b`), cài đè
được lên nhau và giữ nguyên lịch sử xem.

Bản iPad / iPhone (iOS / iPadOS 15+, cài bằng TrollStore; có bản iPhone từ 1.3) ghi riêng với tiêu đề "iPad …", tag `ios-v…`.
Bản Windows (Windows 10 / 11, một tệp `.exe`) ghi riêng với tiêu đề "Windows …", tag `win-v…`.

## iPad 1.9 — 09/11/2026

- **Các hàng trên trang chủ** như TV 2.9: Cài đặt → Trang chủ → «Các hàng trên trang chủ» (riêng từng người xem) — bật / tắt
  từng hàng (hàng tắt thì không tải), ▲ ▼ đổi thứ tự, «Thêm hàng…» (chủ đề có sẵn, hoặc loại phim + nước + thể loại + ngôn ngữ,
  tối đa 6 hàng), xoá hàng tự thêm, «Khôi phục mặc định». «Có tập mới» / «Xem tiếp» chỉ bật / tắt, luôn ở đầu trang. Chủ đề
  người xem mới chọn giờ là hàng tự thêm. Hồ sơ trẻ em: chỉ các hàng hoạt hình, sửa cần mã PIN phụ huynh (hỏi ngay trong trang).
  Đóng Cài đặt là trang chủ xếp lại, tải hàng mới.
- bản 1.9 (CFBundleVersion 12) · mã gói `vn.chimto.tv` · tag `ios-v1.9` (không phải "latest") · commit `e139253` ·
  `ConBangTV-iPad-1.9.ipa` 4 251 574 byte · SHA-256 `48bb2034f25feb39c5527529caa349fa4cc6fc8c51872cc9f3e56992e2dd7cfb`
- 108 test lõi trên macOS (9 test các hàng). Bản thử 78 thử thật trên iPad: bật / tắt, đổi thứ tự, thêm «Phim Hàn» → đóng Cài
  đặt → trang chủ xếp đúng (sửa lỗi bảng Cài đặt tự mở / đóng liên tục khi đóng lúc đang ở trang các hàng — iOS 15); iPhone: trang
  các hàng hiện đúng. Bản chính thức cài lên iPad (đè 1.8.2): mở được, không văng; cài lên iPhone (đè 1.8.2) xong nhưng máy đang
  khoá nên chưa mở. `releases/latest` vẫn `v2.9`; tệp tải từ link công khai khớp SHA-256.

## Windows 1.6 — 09/11/2026

- **Các hàng trên trang chủ** như TV 2.9: Cài đặt → «☰ Các hàng trên trang chủ…» (riêng từng người xem) — bật / tắt từng hàng
  (hàng tắt thì không tải), ▲ ▼ đổi thứ tự (đi bằng phím, giữ nguyên ô đang chọn), «＋ Thêm hàng…» (chủ đề có sẵn, hoặc loại phim
  / nước / thể loại / ngôn ngữ bằng hộp chọn của app, tối đa 6 hàng), ✕ xoá hàng tự thêm, «Khôi phục mặc định». «Có tập mới» /
  «Xem tiếp» chỉ bật / tắt, luôn ở đầu trang. Hồ sơ trẻ em: chỉ các hàng hoạt hình, sửa cần mã PIN phụ huynh.
- bản 1.6.0 · tag `win-v1.6` (không phải "latest") · commit `e139253` · `ConBangTV-Windows-1.6.exe` 72 869 613 byte ·
  SHA-256 `5c558c9c8b9cca70731f04131ccddc380520d2690123be0c242a641fc12b005b`
- 87 test lõi (8 test các hàng). Thử trên máy Windows 10 (bản gỡ lỗi): bật / tắt, ▲ ▼ bằng phím, thêm «Phim Hàn» → trang chủ
  hiện ngay sau «Xem tiếp». Tệp tải từ link công khai khớp SHA-256. **Tự cập nhật thật 1.5 → 1.6** từ GitHub: hộp thoại «Có bản
  mới: Côn Bằng TV 1.6» → Cập nhật ngay → mở lại 1.6.

## 2.9 — 09/11/2026

- **Các hàng trên trang chủ** (Cài đặt → Trang chủ → «Các hàng trên trang chủ…», riêng từng người xem): bật / tắt từng hàng —
  hàng tắt thì không tải; ▲ ▼ đổi thứ tự; «Mặc định» trả thứ tự cũ và bật lại mọi hàng (giữ hàng tự thêm). «Có tập mới» và «Xem
  tiếp» chỉ bật / tắt, luôn ở đầu trang.
- **Hàng tự thêm** («＋ Thêm hàng…», tối đa 6): chủ đề có sẵn (Phim Hàn, Anime Nhật, Kiếm hiệp…) hoặc tự chọn loại phim + nước +
  thể loại + ngôn ngữ (vd «Hoạt hình Nhật Bản · viễn tưởng»). Phim lấy từ KKPhim (không có thì VSMOV), phim cùng tên ở trang khác
  tự gộp vào. Hàng mới đứng sau «Xem tiếp», đổi chỗ được như các hàng khác, ✕ để xoá. Chủ đề người xem mới chọn lúc chưa có lịch
  sử giờ là hàng tự thêm — vẫn còn khi đã xem phim.
- Hồ sơ trẻ em: chỉ các hàng hoạt hình (chủ đề hợp trẻ em, loại phim hoạt hình, bỏ thể loại người lớn); sửa cần mã PIN phụ huynh.
- versionCode 31 · tag `v2.9` · commit `e139253` · `ChimToTV-2.9.apk` 1 207 471 byte · SHA-256
  `471808e304abc2b289dcad514e9da2d6ceda14b5b423460c8e2fc8a22eaccfaf`
- 112 test (8 test các hàng). Thử bản gỡ lỗi trên TV box bằng remote: bật / tắt, ▲ ▼, «Mặc định», tắt «Xem tiếp», thêm «Phim
  Hàn» và «Hoạt hình Nhật Bản · viễn tưởng» — tải đúng phim. Bản chính thức thử trên TV box trước khi đăng (trang chủ, hộp các
  hàng). API `releases/latest` trả `v2.9` + `.apk`, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.8.1 → 2.9**
  trên TV box.

## iPad 1.8.2 — 09/11/2026

- **Cập nhật trên iOS / iPadOS 15:** bấm «Kiểm tra cập nhật» trong Cài đặt giờ hiện hộp thoại «Có bản mới … — Cài đặt ngay»
  → TrollStore tự tải và hỏi Install (trước đây trên iOS 15 hộp thoại SwiftUI bị bảng Cài đặt che, chỉ thấy dòng «Có bản
  mới …» nên phải tải tay; đóng bảng thì app văng). Hộp thoại giờ hiện trong cửa sổ riêng, đè lên mọi màn. Bản 1.8.1 trở về
  trước trên iOS 15 cần tải tay một lần lên 1.8.2.
- **Vượt chặn nhà mạng / proxy:** app nằm nền lâu thì iOS thu hồi cổng nội bộ — vào lại mọi trang báo mất kết nối tới khi tắt
  app. Giờ quay lại app là tự mở lại cổng (đúng số cổng cũ); vòng nhận kết nối thấy cổng hỏng thì dừng, không quay vòng ăn CPU.
- **iPad không còn văng app khi đóng Cài đặt** (lỗi bố cục SwiftUI trên iPadOS 15 ở nút «Xong» của thanh tiêu đề): thanh tiêu
  đề Cài đặt tự vẽ (Xong / Esc); cập nhật màn chính sau khi bảng đóng hẳn.
- **Trình phát → Menu → «Danh sách tập»** mở trang phim, cuộn tới tập đang xem (mở từ hàng Xem tiếp trước đây chỉ về trang chủ).
- **Ô phim:** nhãn tập ở góc dưới bên trái như bản TV — trên iPhone dọc không còn đè nhãn «N nguồn»; hàng Xem tiếp có nút ▶
  giữa poster.
- bản 1.8.2 (CFBundleVersion 11) · mã gói `vn.chimto.tv` · tag `ios-v1.8.2` (không phải "latest") · commit `d6010ea` ·
  `ConBangTV-iPad-1.8.2.ipa` 4 186 872 byte · SHA-256 `9b583a21a9b1f3957e9cc305ceaf682c86e0b3f11ec99a68595f973f275ef36f`
- 99 test lõi trên macOS. Bản thử thử thật: iPad (iPadOS 15.4.1) — hộp thoại cập nhật hiện đè bảng Cài đặt → «Cài đặt ngay» →
  TrollStore tự tải đủ 1.8.1; «Để sau» rồi đóng Cài đặt 3 lần không văng; «Danh sách tập» mở đúng trang phim. iPhone 6s Plus
  (iOS 15.8.3) — gọi TrollStore bằng `apple-magnifier://install` mở được; giả cổng nội bộ hỏng → ra nền → vào lại: «mở lại
  cổng nội bộ: 54733 → 54733», trang tải qua cổng bình thường. Bản chính thức cài lên iPad (đè 1.8) và iPhone (đè 1.8.1), mở
  được, không văng. Tệp tải từ link công khai khớp SHA-256.

## iPad 1.8.1 — 09/11/2026

- **iPhone cầm dọc:** mỗi hàng của trang chủ tổng hợp là một **dải vuốt ngang** — thấy trọn 3 phim và một phần phim kế, tới 12
  phim một dải. «Xem thêm ›» nằm bên phải tên hàng (trước đây ô «Xem thêm» chiếm một trong 3 ô của hàng, chỉ còn 2 phim); cuối
  dải có ô «Xem thêm» hẹp. «Có tập mới» / «Xem tiếp» cũng là dải. Bấm «Xem thêm» rồi quay lại (‹) thì cuộn về đúng hàng đó.
  Áp dụng khi lưới hẹp hơn 500 pt (iPhone dọc, iPad Slide Over / chia màn hẹp); iPad và iPhone xoay ngang giữ lưới như cũ, xoay
  qua lại thì tự đổi. Bàn phím: mỗi dải là một dòng (◄ / ► đi trong dải, ▲ / ▼ sang dải khác giữ thứ tự ô).
- bản 1.8.1 (CFBundleVersion 10) · mã gói `vn.chimto.tv` · tag `ios-v1.8.1` (không phải "latest") · commit `7204ee0` ·
  `ConBangTV-iPad-1.8.1.ipa` 4 177 680 byte · SHA-256 `df2abda78b92b50a0387d3ace5348538e5880dcb5d086ca0cf930c434f5497f0`
- 99 test lõi trên macOS (thêm test phím trên dải). Bản thử 64 thử thật trên iPhone 6s Plus: dải, ô «Xem thêm» cuối dải, «Xem thêm»
  ở tiêu đề và ở cuối dải + quay lại về đúng hàng, xoay ngang ra lưới 5 cột rồi xoay dọc lại. Bản chính thức cài lên iPhone (đè
  1.7), mở được, tải xong trang chủ. iPad khoá màn hình nên chưa cài. Tệp tải từ link công khai khớp SHA-256.

## 2.8.1 — 09/11/2026

- **Điện thoại cầm dọc:** trang chủ tổng hợp xếp mỗi hàng thành **dải vuốt ngang** như bản iPhone — thấy trọn 3 phim và một phần
  phim kế, tới 12 phim một dải; «Xem thêm ›» bên phải tên hàng (chạm) + ô «Xem thêm» hẹp cuối dải (remote / bàn phím). «Có tập
  mới» / «Xem tiếp» cũng là dải. Dải nhớ chỗ đã vuốt tới; ◄ / ► ở đầu / cuối dải đứng yên; quay lại từ «Xem thêm» thì về đúng
  hàng (dùng remote: chọn lại đúng ô). Xoay máy thì xếp lại trang chủ từ lần tải gần nhất. TV và điện thoại xoay ngang giữ lưới
  như cũ.
- versionCode 30 · tag `v2.8.1` · commit `7204ee0` · `ChimToTV-2.8.1.apk` 1 197 959 byte · SHA-256
  `b50e819ff4130e6092ed2dfcabf823b8f55a73304faa799d39e2623902c42ba9`
- 104 test. Thử bản gỡ lỗi trên TV box giả điện thoại dọc (720×1280): vuốt dải, ô cuối dải, «Xem thêm» ở tiêu đề + quay lại,
  remote ▲ ▼ ◄ ► trong / giữa các dải, OK ở ô cuối dải + Back về đúng ô, xoay ngang ↔ dọc; trên TV lưới vẫn như cũ. Bản chính
  thức thử trên TV box trước khi đăng. API `releases/latest` trả `v2.8.1` + `.apk`, tệp tải từ link công khai khớp SHA-256.
  **Cập nhật tự động thật 2.8 → 2.8.1** trên TV box.

## Windows 1.5 — 09/11/2026

- **Trang chủ tổng hợp** như TV 2.8 (`Core/Feed.cs`, `Core/HomeFeed.cs` — cùng tên hàm với bản TV): phim mới từ mọi trang trong
  danh sách, gộp phim trùng, hàng theo lịch sử từng người, «Có tập mới» cả phim đang xem dở ở trang khác, hồ sơ trẻ em, hàng
  thuyết minh, chọn chủ đề cho người chưa có lịch sử. Mỗi hàng một dòng + «Xem thêm ›» (Esc về đúng ô); nhãn «N nguồn» dưới tên
  phim; trang phim có nút «⇄ Nguồn: … ▾» (hộp chọn của app, đi bằng phím). Cài đặt → Trang chủ: Tổng hợp / Theo trang đang
  dùng, Hồ sơ trẻ em, «↻ Thử lại ngay» khi có trang đang tạm nghỉ.
- **Phím:** ▲ / ▼ trong trang không còn nhảy sang menu bên trái khi hàng dưới cách xa (có tiêu đề hàng); ◄ / ► ưu tiên mục cùng
  hàng (hàng nút xuống dòng ở trang phim). Đổi nguồn rồi quay lại vẫn về đúng ô ở trang chủ.
- bản 1.5.0 · tag `win-v1.5` (không phải "latest") · commit `d1e0b8d` · `ConBangTV-Windows-1.5.exe` 72 861 002 byte ·
  SHA-256 `0781cb21f7c05b7dd905ae30e33081b04640830fb038232adf9dd22230e6daee`
- 79 test lõi (8 test trang chủ). Thử trên máy Windows 10 (bản gỡ lỗi): các hàng, «Xem thêm» + Esc, đổi nguồn hoathinh3d → kkphim,
  hồ sơ trẻ em bật / tắt, kiểu theo trang, giả KKPhim lỗi → «lỗi» → 3 lượt → «tạm nghỉ» → «Thử lại ngay». Tệp tải từ link
  công khai khớp SHA-256. **Tự cập nhật thật 1.4 → 1.5** từ GitHub: hộp thoại «Có bản mới: Côn Bằng TV 1.5» → Cập nhật ngay →
  mở lại 1.5 với trang chủ tổng hợp.

## iPad 1.8 — 09/11/2026

- **Trang chủ tổng hợp** như TV 2.8 (`Feed.swift`, `HomeFeed.swift`): các hàng theo lịch sử từng người, «Có tập mới» cả phim
  đang xem dở ở trang khác, hồ sơ trẻ em, hàng thuyết minh, chọn chủ đề; mỗi hàng một dòng + ô «Xem thêm» (nút ‹ / Esc quay lại);
  nhãn «N nguồn» ở góc ô phim; trang phim có nút «Nguồn: … ▾». Cài đặt → Trang chủ: tổng hợp / theo trang, hồ sơ trẻ em,
  «Thử lại ngay».
- bản 1.8 (CFBundleVersion 9) · mã gói `vn.chimto.tv` · tag `ios-v1.8` (không phải "latest") · commit `d1e0b8d` ·
  `ConBangTV-iPad-1.8.ipa` 4 150 419 byte · SHA-256 `706ed1641b966ff75764aec04b9075bdbbf26267a060978acbee2b264b3b68bf`
- 98 test lõi trên macOS. Bản thử 59 đã thử thật trên iPad Pro: các hàng, nhãn nguồn, «Xem thêm» + quay lại, đổi nguồn phim 3
  nguồn (hoathinh3d.pw / yanhh3d / kkphim), hồ sơ trẻ em bật / tắt, kiểu theo trang. Bản chính thức đã cài lên iPad (1.8); chưa
  mở được vì iPad khoá màn hình ngay sau đó. iPhone mới cài bản thử 59, chưa thử. Tệp tải từ link công khai khớp SHA-256.

## 2.8 — 09/11/2026

- **Trang chủ tổng hợp** (`Feed.kt`, `HomeFeed.kt`) — mặc định cho mọi người xem:
  - Lấy phim mới từ mọi trang trong danh sách (KKPhim, NguonC, VSMOV, CLB Phim Xưa + trang đang dùng / xem gần đây), gộp phim
    trùng giữa các trang (tên đã chuẩn hoá, năm lệch ≤ 1).
  - Hàng theo lịch sử xem của từng người: **Có tập mới** (cả phim đang xem dở có tập mới ở trang khác — «có ở nguonc»), Xem tiếp,
    Giống «phim vừa xem», Vì … hay xem <thể loại>, <loại phim hay xem> mới cập nhật · mọi nguồn, **Thuyết minh mới** (khi từ nửa
    số lần mở tập gần đây là bản thuyết minh / lồng tiếng), Mới cập nhật · mọi nguồn, Phim lẻ mới. Người chưa có lịch sử chọn
    chủ đề.
  - Mỗi hàng một dòng + ô «Xem thêm»; ô phim ghi «N nguồn»; trang phim có nút «Nguồn: … ▾» để mở ở trang khác.
  - Trang lỗi / chậm: mỗi trang hạn 6 giây, dùng bản lưu (dưới 24 giờ), về muộn thì trang chủ tự cập nhật (giữ ô đang chọn), lỗi
    3 lượt liền thì tạm nghỉ 30 phút / 2 giờ / 6 giờ; dòng trạng thái bên phải tiêu đề; Cài đặt → «Thử lại ngay».
  - Mở app hiện ngay trang chủ lần trước của người xem (~0,8 s trên TV box), rồi làm mới (~5–9 s).
- **Hồ sơ trẻ em** (Cài đặt → Trang chủ, riêng từng người; tắt cần mã PIN phụ huynh nếu đã đặt): trang chủ chỉ có hoạt hình hợp
  trẻ em — thiếu nhi, gia đình, lồng tiếng; bỏ phim 18+ / kinh dị / hình sự / chiến tranh và tên phim ma. Chỉ lọc trang chủ.
- Cài đặt → Trang chủ: «Tổng hợp mọi nguồn» hoặc «Theo trang đang dùng» (như trước 2.8).
- versionCode 29 · tag `v2.8` · commit `d1e0b8d` · `ChimToTV-2.8.apk` 1 195 968 byte · SHA-256
  `2f13f8b9573a49cba6e44bb2786e1c8b48207d1b3a9a4ef037992bac688b5cf7`
- 104 test (17 test trang chủ). Thử bản gỡ lỗi trên TV box: gộp nguồn (Đấu Phá Thương Khung Phần 5 = yanhh3d + hoathinh3d.pw +
  KKPhim), đổi nguồn, «Xem thêm», giả KKPhim lỗi → bản lưu / «lần kiểm trước» / tạm nghỉ / «Thử lại ngay», kiểu cũ, người xem
  mới chọn chủ đề, hàng thuyết minh, hồ sơ trẻ em. Bản chính thức thử trên TV box (lịch sử cũ → «Có tập mới» 4 phim). API
  `releases/latest` trả `v2.8` + `.apk`, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.7 → 2.8** trên TV box.

## Windows 1.4 — 08/11/2026

- **Thêm 3 trang** như TV 2.7 (`NguonC.cs`, `Stremio.cs`, VSMOV trong `ApiSite.cs`): NguonC, VSMOV, CLB Phim Xưa (qua add-on k-20);
  có sẵn trong Đổi trang phim (người đã cài được thêm một lần). **Server dự phòng NguonC** cạnh KKPhim; Cài đặt: «Server dự phòng
  NguonC».
- **Thoát được toàn màn hình** (anh báo lỗi, có từ 1.0): «Mở app ở chế độ toàn màn hình: Tắt» nay thoát luôn; khi toàn màn hình
  (cửa sổ không còn thanh tiêu đề) menu có «⤡ Thoát toàn màn hình (F11)»; Esc ở menu cũng thoát.
- Cập nhật app: link tải github.com lỗi thì tải cùng tệp qua API GitHub, thử lại cả hai đường sau 5 s / 15 s.
- bản 1.4.0 · tag `win-v1.4` (không phải "latest") · commit `7c879e4` · `ConBangTV-Windows-1.4.exe` 72 813 124 byte ·
  SHA-256 `de0435cf258bac80b7743fa9df260602e79ffd5eda00e8a2675116ff508b5401`
- 71 test lõi. Thử trên máy Windows 10 (bản gỡ lỗi): danh sách trang được thêm 3 trang đúng một lần; trang chủ NguonC / VSMOV /
  CLB Phim Xưa; phát VSMOV (HLS), NguonC, CLB Phim Xưa (MP4); NguonC dự phòng trên yanhh3d; thoát toàn màn hình bằng menu / F11
  / Esc; đường tải dự phòng (máy chủ giả trả 504 → tải qua API → bước kiểm tệp vẫn chạy). Tệp tải từ link công khai khớp SHA-256.
  **Tự cập nhật thật 1.3 → 1.4** từ GitHub: hộp thoại «Có bản mới: Côn Bằng TV 1.4» → Cập nhật ngay → mở lại 1.4.

## iPad 1.7 — 08/11/2026

- **Thêm 3 trang** như TV 2.7 (`NguonC.swift`, `Stremio.swift`, VSMOV trong `ApiSite.swift`) và **server dự phòng NguonC** cạnh
  KKPhim (Cài đặt → Phát video: «Server dự phòng NguonC»). VSMOV phát HLS được cả trên iPhone (trình phát của iOS tự bỏ phần ảnh
  PNG đứng trước dữ liệu video).
- bản 1.7 (CFBundleVersion 8) · mã gói `vn.chimto.tv` · tag `ios-v1.7` (không phải "latest") · commit `7c879e4` ·
  `ConBangTV-iPad-1.7.ipa` 3 912 873 byte · SHA-256 `76d585319ba2638d8011087bf0321e922216fa41657a6b3cb50de619a5a028c6`
- 90 test lõi trên macOS. Bản thử 54 đã thử thật trên iPad Pro và iPhone 6s Plus: trang chủ 3 trang, phát VSMOV (HLS), NguonC,
  CLB Phim Xưa (MP4 mượt), NguonC dự phòng trên yanhh3d (iPad). **Bản chính thức đã cài thật**: iPad 1.6 → 1.7 (danh sách trang
  được thêm 3 trang đúng một lần), iPhone 1.7; app mở và chạy. Tệp tải từ link công khai khớp SHA-256.

## 2.7 — 08/11/2026

- **Thêm 3 trang**, có sẵn trong ☰ → Đổi trang phim (người đã cài được thêm một lần):
  - **NguonC** (phim.nguonc.com, `NguonC.kt`): API riêng — trang chủ Hoạt hình mới cập nhật / Phim mới / Phim bộ, lẻ mới /
    Thuyết minh; menu theo loại phim, ngôn ngữ, nước; «Thể loại…», «Quốc gia…», tìm kiếm. Phát bằng trình phát của NguonC
    (iframe, app tự bỏ quảng cáo trước phim).
  - **VSMOV** (vsmov.com, trong `ApiSite.kt`): API kiểu KKPhim khác đường dẫn; app đọc luồng HLS (link có chữ ký) trong trang
    trình phát và tự phát — `hls.html` cắt phần ảnh PNG ngụy trang trước dữ liệu TS; còn «Trình phát VSMOV» dự phòng.
  - **CLB Phim Xưa** (add-on Stremio k-20, `Stremio.kt`): phim Hồng Kông / TVB / kiếm hiệp cũ; mỗi chất lượng một server MP4 (phát
    thẳng bằng `<video>`). Qua máy chủ trung gian nên trên TV mở chậm (~25–35 s), thỉnh thoảng khựng, tua xa có khi không được.
- **Server dự phòng NguonC** cạnh KKPhim cho yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw (NguonC hay có tập mới sớm, vd Mục
  Thần Ký tập 108 khi KKPhim chưa có). Hai nguồn dự phòng tìm song song, nguồn chậm không làm mất nguồn nhanh. Cài đặt → Phát video:
  «Server dự phòng NguonC».
- **Cập nhật app**: link tải github.com lỗi thì tải cùng tệp qua API GitHub (8/11 GitHub vùng Đông Nam Á trả 504 / 500 xen kẽ
  nhiều giờ); thử lại cả hai đường sau 5 s / 15 s; tệp vẫn qua bước kiểm gói + chữ ký + số hiệu.
- Tên tập VSMOV ghi số trần → «Tập N»; tên bản có xuống dòng được gộp lại; nhãn trang «nguonc», «CLB Phim Xưa».
- versionCode 28 · tag `v2.7` · commit `7c879e4` · `ChimToTV-2.7.apk` 1 160 565 byte · SHA-256
  `a879c8aef5e6ec847f3b003bd53b929746577daffd71f3c001767e7a642f5ee0`
- 87 test. Thử bản gỡ lỗi trên TV box và Skyworth: trang chủ / chi tiết 3 trang, đo tốc độ, phát VSMOV (HLS, server nhúng),
  NguonC, CLB Phim Xưa, NguonC dự phòng trên yanhh3d; đường tải dự phòng (máy chủ giả trả 504 → tải qua API). Bản chính thức thử
  trên TV box (Tiên Nghịch tập 165 có «KKPhim Vietsub» và «NguonC Vietsub»). API `releases/latest` trả `v2.7` + `.apk`, tệp tải
  từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.6 → 2.7** trên TV box (cũng đã thử 2.5 → 2.6 cho bản 2.6). Skyworth
  (bản 1.8.1) tải được bản mới nhưng bước cài bị khoá trẻ em TiviKit chặn (cần mã bố mẹ).

## Windows 1.3 — 08/11/2026

- **Server dự phòng KKPhim** như TV 2.6 (`KkPhim.cs`): yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw có thêm «KKPhim Vietsub /
  Thuyết Minh» trong bảng ▼ (HLS, bỏ quảng cáo; đúng ngôn ngữ của bản đang xem; «Trình phát của trang» cho hoathinh3d.pw);
  trang lỗi mà KKPhim có tập thì vẫn xem được. Cài đặt: «Server dự phòng KKPhim» (mặc định bật).
- **Thêm trang KKPhim** (www.kkphim.com, gọi thẳng API phimapi.com), có sẵn trong Đổi trang phim (người đã cài được thêm
  một lần). Gõ «kkphim.com» khi thêm trang là trang KKPhim có sẵn (tên miền trần bị nhà mạng chặn).
- Sửa lỗi có từ 1.0: trang «Đổi trang phim» bấm Đo / Xoá / Thêm / + trang mẫu một lần là mất ô thêm trang.
- Sửa: mở phim mới mà trang phim tải lỗi thì bảng ▼ không còn giữ server / bản / tập / tiêu đề của phim trước (bấm vào có thể
  phát nhầm phim); «Tải lại» lúc đó tải lại thông tin phim.
- bản 1.3.0 · tag `win-v1.3` (không phải "latest") · commit `b2a92de` · `ConBangTV-Windows-1.3.exe` 72 796 716 byte ·
  SHA-256 `98950fc9d9c37e69e58a3afc86d87356c03d6f728b923f3b0bfa2a844cbb349d`
- Đã thử trên máy Windows 10 (bản gỡ lỗi): 63 test lõi; danh sách trang của người dùng 1.2 được thêm KKPhim đúng một lần;
  Đo tốc độ; trang KKPhim (trang chủ, menu, chi tiết, phát HLS 1080p); server dự phòng không trùng; đổi qua lại trình phát
  của hoathinh3d.pw ↔ KKPhim giữ chỗ xem; tắt / bật trong Cài đặt. Tệp tải từ link công khai khớp SHA-256.

## iPad 1.6 — 08/11/2026

- **Server dự phòng KKPhim** như TV 2.6 (`KkPhim.swift`): «KKPhim Vietsub / Thuyết Minh» trong Menu của trình phát cho
  yanhh3d / hoathinh3d / hhkungfu / hoathinh3d.pw; Cài đặt → Phát video: «Server dự phòng KKPhim».
- **Thêm trang KKPhim** (www.kkphim.com, gọi thẳng phimapi.com), có sẵn trong Đổi trang phim (thêm một lần cho người đã
  cài); gõ «kkphim.com» khi thêm trang là trang KKPhim có sẵn; gợi ý thêm trang có nhắc KKPhim.
- bản 1.6 (CFBundleVersion 7) · mã gói `vn.chimto.tv` · tag `ios-v1.6` (không phải "latest") · commit `b2a92de` ·
  `ConBangTV-iPad-1.6.ipa` 3 853 799 byte · SHA-256 `8d7aaba7fdcdfc2b0e6b11eb1cfc7304a47439a974cb0607f9f3445bfd46c1ac`
- 81 test lõi trên macOS. Bản thử 50 đã thử trên iPad (server dự phòng, trang KKPhim). **Bản chính thức đã cài thật**: iPad
  Pro (1.5 → 1.6, danh sách trang được thêm www.kkphim.com đúng một lần) và iPhone 6s Plus (cài mới), app mở và chạy.

## 2.6 — 08/11/2026

- **Server dự phòng KKPhim** (`KkPhim.kt`, API công khai phimapi.com) cho yanhh3d, hoathinh3d.ee, hhkungfu (nhúng chung
  streamfree.vip — hỏng là hỏng cả ba) và hoathinh3d.pw: mở tập là app tìm cùng phim (slug của trang, không khớp thì theo
  tên / tên gốc), lấy tập cùng số rồi thêm «KKPhim Vietsub / Thuyết Minh» vào menu ▼ — luồng HLS, app tự phát, bỏ quảng cáo.
  Trang chia bản thì chỉ thêm đúng ngôn ngữ đang xem; hoathinh3d.pw có thêm «Trình phát của trang». Tải song song với trang
  tập, KKPhim chậm / lỗi thì bỏ qua; trang lỗi mà KKPhim có tập thì vẫn xem được. Cài đặt → Phát video: «Server dự phòng
  KKPhim» (mặc định bật).
- **Thêm trang KKPhim** (www.kkphim.com; mọi dữ liệu gọi thẳng phimapi.com): trang chủ Hoạt hình Trung Quốc mới cập nhật /
  thuyết minh, Phim mới, Phim bộ / lẻ mới; menu thêm «Hoạt hình Trung Quốc thuyết minh», «Phim chiếu rạp»; Thể loại… /
  Quốc gia…, tìm kiếm, bản Vietsub / Thuyết Minh, server HLS (bỏ quảng cáo) + trình phát KKPhim. Có sẵn trong ☰ → Đổi
  trang phim (người đã cài được thêm một lần). Gõ «kkphim.com» khi thêm trang là trang KKPhim có sẵn (tên miền trần bị nhà
  mạng chặn, trước đây thành dòng trùng).
- Lưu ý: bản trên KKPhim có khi dài ngắn khác bản của trang (Tiên Nghịch tập 165: yanhh3d 23:41, KKPhim 18:49) — đổi server
  giữa chừng giữ số giây nên có thể lệch cảnh.
- versionCode 27 · tag `v2.6` · commit `b2a92de` · `ChimToTV-2.6.apk` 1 149 262 byte · SHA-256
  `15041c01e996dea66ccb358131d759412aeea26e178b787a579cecc76318928c`
- Thử bản gỡ lỗi trên Skyworth và TV box (server dự phòng cả 4 trang, Thuyết Minh / Vietsub đúng bản, trang lỗi vẫn phát,
  trang KKPhim, tắt / bật trong Cài đặt, gõ «kkphim.com»); bản chính thức thử trên TV box trước khi phát hành (yanhh3d phát
  được, có «KKPhim Vietsub» trong menu ▼). API `releases/latest` trả `v2.6` + tệp `.apk`; tệp tải từ link công khai khớp SHA-256.

## Windows 1.2 — 06/11/2026

- **Người xem**: mỗi người một lịch sử, Xem tiếp, chỗ đang xem, phim theo dõi, trang phim / server / cách xem riêng; khoá bằng mã
  PIN; hỏi «Ai đang xem?» khi mở app và khi rời app hơn 30 phút (lịch sử của bản 1.0 thuộc «Cả nhà»).
- **Giới hạn giờ xem** theo từng người (mỗi ngày 30 phút – 3 giờ, giờ đi ngủ tới 6 giờ sáng) + mã PIN phụ huynh; còn 5 phút thì
  nhắc; hết giờ phim dừng, cho xem thêm 15 / 30 / 60 phút hoặc tới 6 giờ sáng.
- **Cửa sổ phim nhỏ**: «▣ Thu nhỏ» / phím W / thu nhỏ cửa sổ app lúc xem → phim chạy tiếp ở cửa sổ nhỏ luôn nổi trên cùng.
- **Đồng bộ trong nhà** với TV 2.2+ và iPad / iPhone 1.3+ (cùng giao thức, theo từng người xem cùng tên).
- **Proxy SOCKS5** (đăng nhập, DNS của proxy / máy, toàn app / chỉ trang phim); proxy và vượt chặn đổi là có hiệu lực ngay.
- bản 1.2.0 (không có bản 1.1) · tag `win-v1.2` (không phải "latest") · commit `7209da7` · `ConBangTV-Windows-1.2.exe`
  72 792 073 byte · SHA-256 `f17b404f5b44b04acf29ba0db8c3c91a21578562cfd588197f7744ab4260966c`
- Đã thử trên máy Windows 10: 53 test lõi; cửa sổ phim nhỏ (thu nhỏ / phóng to / thu nhỏ cửa sổ app — phim chạy liền, không
  tải lại; tắt từ menu lưu đúng chỗ xem); thêm người xem, khoá PIN, mở lại app hiện «Ai đang xem?» nền kín; giới hạn 30 phút →
  phim dừng đúng lúc hết giờ, sai mã phụ huynh bị từ chối, đúng mã + «Thêm 15 phút» mở lại tập; nhắc «Còn 4 phút»; proxy SOCKS5
  giả trên máy (sai mật khẩu / đúng / sai cổng → báo lỗi, không đi thẳng; video cũng đi qua proxy); vượt chặn mức 1 / 2 với
  phimmois.fun; đồng bộ thật với TV box (TV 2.5): ghép bằng IP, hai chiều dữ liệu, xoá lan sang TV (rồi bỏ ghép).
  **Tự cập nhật thật 1.0 → 1.2 từ GitHub**: hộp thoại «Có bản mới 1.2» → Cập nhật ngay → mở lại 1.2, lịch sử còn nguyên.
  Chưa thử: TV / iPad gọi sang máy tính (tường lửa), iPad / iPhone, remote thật.

## Windows 1.0 — 06/11/2026

- **Bản đầu tiên cho máy Windows 10 / 11** (máy tính hoặc mini PC cắm tivi), dùng chuột hoặc bàn phím / remote: đủ 6 trang
  phim như bản TV (yanhh3d, hoathinh3d.pw / .ee, hhkungfu, phimmois → phimmoiw, mephim), trang chủ, menu mục của trang,
  Thể loại… / Quốc gia…, tìm trên mọi trang, chi tiết phim (bản Vietsub / Thuyết Minh, tập đã xem), Xem tiếp / Lịch sử,
  Theo dõi (báo tập mới), Đổi trang phim (đo nhanh, thêm / xoá trang), trình phát (server nhớ theo trang, xem tiếp đúng
  chỗ, tự chuyển tập, tốc độ, khung hình, nhạc mở đầu tự bỏ qua, HLS bỏ quảng cáo, chặn quảng cáo), toàn màn hình,
  cỡ giao diện 100–175%, **vượt chặn nhà mạng mức 1 / 2**, tự cập nhật từ GitHub.
- Chưa có: người xem / mã PIN, giới hạn giờ xem, cửa sổ phim nhỏ, đồng bộ trong nhà, proxy SOCKS5.
- bản 1.0.0 · tag `win-v1.0` (không phải "latest") · commit `9624e00` · tệp `ConBangTV-Windows-1.0.exe` 72 736 562 byte ·
  SHA-256 `bd2600678c1825ac4c788c6ac00f12519aa1daf46635db05886ccf4d34bd2938`
- Cài: tải tệp .exe, mở; Windows báo «Windows protected your PC» thì **More info → Run anyway** (app chưa có chữ ký số).
- Đã thử trên máy Windows 10 thật: 28 test lõi; phát phim ở yanhh3d (iframe), hoathinh3d.pw (mở thẳng trang tập), mephim
  (HLS), phimmoiw (HLS qua vượt chặn mức 1 và mức 2 — phimmois.fun bị nhà mạng chặn, tắt vượt chặn thì hết giờ chờ); phím
  mũi tên / Enter / Esc, chuột thật (bấm phát / dừng, bấm đúp toàn màn hình, bấm thanh tua); tự cập nhật trọn vẹn
  1.0 → 1.0.1 qua release giả; tệp tải từ link công khai khớp SHA-256, tự kiểm tra bản mới với GitHub thật. Chưa thử với
  remote thật, máy chưa có WebView2.

## iPad 1.5 — 05/11/2026

- **Thêm trang mephim.ddns.net** như TV 2.5 (`ApiSite.swift`): có sẵn trong Đổi trang phim, trang chủ / menu / Thể loại… /
  Quốc gia…, tìm kiếm, bản Vietsub / Thuyết Minh, server HLS (bỏ quảng cáo) + trình phát KKPhim, dự phòng phimapi.com.
- bản 1.5 (CFBundleVersion 6) · mã gói `vn.chimto.tv` · tag `ios-v1.5` · commit `e1294b0` · SHA-256 IPA
  `4e5d3f7fb9975d7a6dd52b167cea3290e3672a0f6a139433ffe42e3a9c063045`
- Test lõi trên macOS qua (thêm `ApiSiteTests`). **Chưa cài thử trên iPad / iPhone thật** (iPhone không nối SSH được lúc
  phát hành; anh tự thử sau).

## 2.5 — 05/11/2026

- **Thêm trang mephim.ddns.net** (ứng dụng React, API JSON `/api/movies/…`, nguồn KKPhim): có sẵn trong ☰ → Đổi trang phim
  (người đã cài được thêm một lần). Trang chủ: Hoạt hình Trung Quốc mới cập nhật, Phim mới cập nhật, Thịnh hành, Song ngữ /
  Thuyết minh; menu Hoạt hình Trung Quốc, Hoạt hình, Phim bộ, Phim lẻ, TV Shows, Thuyết minh, Lồng tiếng, 5 nước;
  «Thể loại…» / «Quốc gia…»; tìm kiếm mọi trang. Bản Vietsub / Thuyết Minh riêng; mỗi tập 2 server: **HLS (bỏ quảng cáo)**
  và **Trình phát KKPhim** (dự phòng, có quảng cáo chèn). Máy chủ của trang lỗi (hay báo 500) thì app gọi thẳng phimapi.com.
- versionCode 26 · tag `v2.5` · commit `e1294b0` · SHA-256 APK
  `26e879b19b76ad053011ddb0f2eebba373da2c96dbc685f034a627344ddec98c`
- Thử trên TV box (bản gỡ lỗi + bản chính): trang chủ, phân trang, Thể loại, tìm kiếm, Thuyết Minh, Xem tiếp, cả hai server;
  anh thử trên vivo X200 Pro. Cập nhật tự động 2.4.3 → 2.5 đã thử trên TV box.

## 2.4.3 — 05/11/2026

- **Cử chỉ chạm khi xem phim** (điện thoại, màn cảm ứng), như bản iPhone: **giữ tay** xem nhanh 2× tới khi nhấc tay;
  **vuốt ngang** để tua (giữa màn hiện giờ sẽ tới, hết bề ngang = 2 phút); **vuốt dọc nửa trái** chỉnh độ sáng (chỉ trong
  lúc xem), **nửa phải** chỉnh âm lượng; **chụm / mở hai ngón** vừa màn / lấp đầy. Vuốt từ sát mép trên / dưới để dành cho
  điện thoại. Remote trên TV không đổi.
- versionCode 25 · tag `v2.4.3` · commit `ae34126` · SHA-256 APK
  `8ae31e339430e2c08efe429a5663f4456ff2a33ba79d3dba398ec2de3e8d0e61`
- Thử trên TV box qua cổng gỡ lỗi (giữ tay → tốc độ 2 rồi về 1, vuốt ngang tua ~1 phút, độ sáng); anh thử trên vivo X200 Pro
  (bản debug5). Cập nhật tự động 2.4.2 → 2.4.3 đã thử trên TV box.

## 2.4.2 — 05/11/2026

- **Điện thoại màn dài (vd 20:9): xem phim không còn bị lẹm mất phần dưới hình.** Một số trang phát đặt khung video theo
  bề ngang (cao = ngang × 9/16) — trên màn dài hơn 16:9 khung cao hơn màn, phần dưới (phụ đề, chữ) bị cắt. Nay video tràn
  khung thì app ghim vừa màn, giữ tỉ lệ (viền đen hai bên); «Khung hình: Lấp đầy» vẫn như cũ. TV 16:9 không đổi.
- versionCode 24 · tag `v2.4.2` · commit `49b43b7` · SHA-256 APK
  `ec6f6290cd4e8fa18c2ad33fa3157ae33bbbf339cdd0c106c950683d3a7d2724`
- Anh thử trên vivo X200 Pro (bản debug) thấy hết lẹm. Cập nhật tự động 2.4.1 → 2.4.2 đã thử trên TV box.

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
