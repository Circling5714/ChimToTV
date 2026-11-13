# Nhật ký phiên bản

Côn Bằng TV — từ 1.0 đến 1.4 tên là ChimTo TV. Tải bản mới nhất ở [GitHub Releases](https://github.com/Circling5714/ChimToTV/releases/latest). Bản mới nhất ở trên cùng.
Mọi bản ký cùng một khoá (chứng chỉ SHA-256 `949ef2b52f3cf895d7fe5ff9db495f273e6fd35d02b23b4a521df43957a0177b`), cài đè
được lên nhau và giữ nguyên lịch sử xem.

Bản iPad / iPhone (iOS / iPadOS 15+, cài bằng TrollStore; có bản iPhone từ 1.3) ghi riêng với tiêu đề "iPad …", tag `ios-v…`.
Bản Windows (Windows 10 / 11, một tệp `.exe`) ghi riêng với tiêu đề "Windows …", tag `win-v…`.
Bản Mac (macOS 12+, từ 13/11/2026, cùng mã nguồn và số bản với bản iPad) ghi riêng với tiêu đề "Mac …", tag `mac-v…`.

## Mac 1.17 — 13/11/2026 (bản Mac đầu tiên)

- **Côn Bằng TV cho máy Mac**: macOS 12 trở lên, máy Intel và máy chip Apple (M1…). Là chính app iPad dựng bằng Mac Catalyst nên
  có đủ tính năng của bản iPad: trang chủ tổng hợp nhiều nguồn, người xem + mã PIN + hồ sơ trẻ em, theo dõi phim, Điểm IMDb, tìm
  cùng lúc mọi trang, đồng bộ trong nhà, proxy SOCKS5 / Shadowsocks, vượt chặn nhà mạng (tự lên Mức 2), DNS mã hoá. Dùng chuột, bàn
  di chuột, bàn phím (phím tắt như bản iPad). Thanh menu tiếng Việt; cửa sổ nhỏ nhất 900 × 600.
- **Tự cập nhật**: app tìm bản mới (release `mac-v…`) trên GitHub; bấm «Cài đặt ngay» thì app tự tải, kiểm, thay bản đang dùng rồi mở
  lại — cần để app trong thư mục Applications.
- **Cài:** tải `ConBangTV-Mac-1.17.zip` (hoặc link luôn là bản mới nhất
  `https://github.com/Circling5714/ChimToTV/releases/download/conbangtv_macos/ConBangTV-Mac.zip`), giải nén, kéo vào Applications.
  App không mua chứng chỉ của Apple nên lần đầu macOS chặn: OK → Cài đặt hệ thống → Quyền riêng tư & Bảo mật → **Vẫn mở** → nhập
  mật khẩu máy → **Mở** (chỉ một lần).
- Khác bản iPad: không có widget; mật khẩu proxy cất trong tệp riêng của tài khoản macOS thay cho Keychain.
- bản 1.17 (CFBundleVersion 23) · mã gói `vn.chimto.tv` · tag `mac-v1.17` (không phải "latest") · commit `c416cd5` (tính năng:
  `24cb97b`, `c416cd5`) · `ConBangTV-Mac-1.17.zip` 7 945 079 byte · SHA-256
  `e7726b03fbae6668d566ef961164ce09c31ddcb2cf5075c38f6e1b107ada7d7f` · Intel + Apple Silicon, ký ad-hoc · dựng trên GitHub (run
  `43382966442`, Xcode 16.4, SDK macOS 15.5). Link cố định `conbangtv_macos` tạo lúc 14:15, cùng tệp.
- 155 test lõi (thêm 3; đạt cả Xcode 15.4 trên Mac mini lẫn Xcode 16.4 trên GitHub). Thử trên Mac mini 2012 (Intel, macOS 14.4):
  bản thử — trang chủ, xem phim (11 server, tua / dừng / phát / đóng), DNS mã hoá, vượt chặn với máy soi Viettel thật (lần đầu 4,6 s
  rồi lên Mức 2, sau đó 0,5 s; HTTP thường 0,3 s), tự cập nhật (tải .zip, thay `.app`, tắt, mở bản mới), cất mật khẩu; bản chính —
  mở lần đầu đúng các bước Gatekeeper ở trên (tệp mang cờ «Safari đã tải về»), màn miễn trừ, trang chủ, không văng. Sau khi đăng: bản
  thử (giả đang 1.16) tự thấy «Có bản mới 1.17» từ GitHub thật; `releases/latest` vẫn `v2.17`; tệp tải từ link công khai và link
  cố định khớp SHA-256. **Chưa thử trên máy chip Apple**; tự cập nhật thật từ GitHub sẽ thử ở bản Mac sau (bản 1.17 để sẵn trong
  Applications của Mac mini).
- Máy thử 2012 chạy macOS nhờ OCLP nên video rớt khung hình (WindowServer ~480% CPU, app ~6%) — giới hạn của máy, không phải app.

## Tệp APK bản TV đổi sang tên mới — 13/11/2026

- Từ bản 2.17 tệp APK mang tên mới: **`ConBangTV-<phiên bản>.apk`**, link cố định mới
  `https://github.com/Circling5714/ChimToTV/releases/latest/download/ConBangTV.apk` (trang giới thiệu đã dùng link này).
- Tệp tên cũ **`ChimToTV.apk` vẫn có trong mọi bản TV** (bản sao giống hệt): mã Downloader **9192365** trỏ vào link tên cũ, và màn
  «Cài lên tivi» của các bản đã cài (TV 2.16–2.17, iPad 1.16–1.17, Windows 1.14–1.15) in link đó — cả hai link tải cùng một tệp.
- Release `v2.17` đã sửa sau khi đăng (10:33): thêm `ConBangTV-2.17.apk` + `ConBangTV.apk`, bỏ `ChimToTV-2.17.apk`, giữ
  `ChimToTV.apk` — ba tệp cùng SHA-256 `663dc083…1206`. Tự cập nhật không bị ảnh hưởng: mọi bản từ 1.3 lấy tệp `.apk` đầu tiên của
  bản mới nhất, không xét tên. Đã kiểm: hai link cố định tải về đúng bản 2.17, trang chuyển hướng của mã 9192365 vẫn trỏ link tên
  cũ.
- Trong app (có hiệu lực từ bản sau 2.17 / iPad 1.17 / Windows 1.15): màn «Cài lên tivi / mời bạn bè» in link tên mới.
  `build.ps1` dựng ra `dist\ConBangTV-<phiên bản>.apk`; `release.ps1 tv` đính kèm ba tệp và kiểm cả hai link.

## iPad 1.17 — 13/11/2026

- **Vượt chặn nhà mạng tự lên Mức 2:** đang ở Mức 1 mà một trang vẫn bị chặn (gửi gói bắt tay đã chia nhỏ mà trang im 4 giây
  hoặc cắt kết nối), app tự kết nối lại theo Mức 2 cho riêng trang đó và nhớ 7 ngày — không phải vào Cài đặt đổi mức. Chỉ lần đầu
  chậm khoảng 4 giây.
- **Vượt chặn chia nhỏ cả yêu cầu HTTP thường** (dòng `Host:`), không chỉ HTTPS.
- **DNS:** nhận ra thêm kiểu chặn bằng địa chỉ trang cảnh báo của nhà mạng (vd DNS Viettel trả 127.0.0.1; cùng một địa chỉ trả
  cho hai trang khác nhau) và tự hỏi lại qua DNS mã hoá. Chi tiết: README mục «Vượt chặn nhà mạng» và «DNS mã hoá».
- bản 1.17 (CFBundleVersion 23) · mã gói `vn.chimto.tv` · tag `ios-v1.17` (không phải "latest") · commit `c52e32f` (tính năng:
  `c474288`) · `ConBangTV-iPad-1.17.ipa` 4 501 255 byte · SHA-256
  `5ca703428bd7003d68be236438dcc2195f9b3f72fe2bc89264eab8ed7572746d`
- 153 test lõi trên macOS (thêm 5). IPA: Info.plist đúng (mã gói, tên, bản 1.17 / 23), không có cổng gỡ lỗi, danh sách thư viện
  nạp giống hệt 1.16.1. iPhone thật iOS 15.8.3, bản thử 139 (cùng mã nguồn): máy soi Viettel thật, giả «Mức 1 không lọt» —
  lần đầu 4,6 s rồi lọt ở Mức 2, lần sau 0,57 s; HTTP thường tới trang bị chặn 0,32 s; trình phát mở bình thường (đo trên bản thử
  137 cùng mã). Bản chính thức cài đè 1.16.1: mở được, không có báo cáo văng. Link cố định `conbangtv_ios` đã chép sang 1.17, tệp
  tải từ link công khai và link cố định khớp SHA-256; `releases/latest` vẫn là bản TV. Chưa thử trên iPad (máy thử không bật).

## Windows 1.15 — 13/11/2026

- **Vượt chặn nhà mạng tự lên Mức 2:** như bản TV 2.17 — Mức 1 mà trang im / cắt kết nối thì tự thử lại theo Mức 2 cho riêng
  trang đó và nhớ 7 ngày; áp dụng cho cả trang phim (WebView2) lẫn kết nối riêng của app (đọc danh sách phim, trình phát).
- **Vượt chặn chia nhỏ cả yêu cầu HTTP thường**, không chỉ HTTPS.
- **DNS:** nhận ra địa chỉ trang cảnh báo của nhà mạng, tự hỏi lại qua DNS mã hoá.
- bản 1.15.0 · tag `win-v1.15` (không phải "latest") · commit `c52e32f` (tính năng: `c474288`) · `ConBangTV-Windows-1.15.exe`
  72 930 621 byte · SHA-256 `3de526fb96dabe13e74f01fbc1c5e908e3041270a736d2df045966db4b1233c1`
- 135 test lõi (thêm 4). Lỗi tìm ra khi thử bản gỡ lỗi và đã sửa trước phát hành: SslStream của .NET 8 đọc 0 byte trước để chờ
  dữ liệu — bị coi là «trang im» nên kết nối nào cũng lên Mức 2; có test. Máy soi Viettel thật (bản gỡ lỗi): cổng nội bộ 4,3 s →
  0,3 s, HttpClient 4,2 s. Tệp đóng gói: mở bản 1.15, trang chủ đủ 5 nguồn, không có cổng gỡ lỗi. **Tự cập nhật thật 1.14 → 1.15**
  (bản chép riêng) lúc 10:21:56 → mở lại bản 1.15 lúc 10:22:09; hộp cập nhật hiện mục «Có gì mới». Link cố định
  `conbangtv_windows` chép đè sang 1.15 (lần chép đè thật đầu tiên — chỉ còn một tệp, khớp SHA-256); `releases/latest` vẫn là
  bản TV.

## 2.17 — 13/11/2026

- **Vượt chặn nhà mạng tự lên Mức 2:** đang ở Mức 1 mà một trang vẫn bị chặn (gửi gói bắt tay đã chia nhỏ mà trang im 4 giây
  hoặc cắt kết nối — máy soi đã ghép được các mẩu), app tự kết nối lại theo Mức 2 cho riêng trang đó và nhớ 7 ngày. Không phải
  vào Cài đặt đổi mức; chỉ lần đầu chậm khoảng 4 giây.
- **Vượt chặn chia nhỏ cả yêu cầu HTTP thường** (dòng `Host:`), không chỉ HTTPS — trang mở bằng `http://` cũng không còn treo.
- **DNS:** nhận ra thêm kiểu chặn bằng địa chỉ trang cảnh báo của nhà mạng (DNS Viettel trả 127.0.0.1 cho trang bị chặn; cùng một
  địa chỉ trả cho hai trang khác nhau) và tự hỏi lại qua DNS mã hoá — dùng khi bật Vượt chặn mà tắt DNS mã hoá.
- versionCode 41 · tag `v2.17` · commit `c52e32f` (tính năng: `c474288`) · `ChimToTV-2.17.apk` 1 250 817 byte · SHA-256
  `663dc0830a0550c8cb5cf1a75d839b78a47b5bd990bd0ed6b8100b7b06f21206` · kèm `ChimToTV.apk` (bản sao giống hệt, cho link cố định).
- 163 test (thêm 7). Máy soi Viettel thật, bản gỡ lỗi trên TV box (giả «Mức 1 không lọt»): lần đầu 4,4 s rồi lọt ở Mức 2, sau đó
  0,3 s; HTTP thường tới 3 trang bị chặn 0,3–0,6 s (gửi thẳng từ máy tính: treo 15 s); giả DNS trả địa chỉ cảnh báo cho 2 trang →
  đi qua DNS mã hoá; trình phát vẫn phát bình thường. Bản chính thức trên TV box: trang bị chặn (HTTPS và HTTP) qua cổng nội bộ
  0,3–0,7 s, trang chủ tải đủ, phát được phim. Đăng bằng `release.ps1`: Latest = `v2.17`, hai tệp `.apk` cùng SHA-256, link cố
  định của Downloader tải đúng bản 2.17. **Cập nhật tự động thật 2.16.1 → 2.17** trên TV box lúc 10:20.

## Link tải cố định cho bản Windows và iPad / iPhone — 13/11/2026

Như link APK của bản TV, hai link này **luôn tải bản mới nhất** và không bao giờ đổi (trang giới thiệu
<https://conbangtv.github.io> đã dùng):

- Windows: `https://github.com/Circling5714/ChimToTV/releases/download/conbangtv_windows/ConBangTV-Windows.exe`
- iPad / iPhone: `https://github.com/Circling5714/ChimToTV/releases/download/conbangtv_ios/ConBangTV-iPad.ipa`

Mỗi nền tảng có một release riêng với tag không đổi (`conbangtv_windows`, `conbangtv_ios`); mỗi lần phát hành `release.ps1` chép
đè tệp mới vào đó và ghi ở ghi chú của release đang là bản nào. Lúc tạo (07:47 và 07:50): Windows = bản 1.14, iPad / iPhone = bản
1.16.1; tệp tải qua hai link khớp SHA-256 với tệp của `win-v1.14` / `ios-v1.16.1`. Ba bộ tự cập nhật không bị ảnh hưởng: bản
Windows 1.13 chạy thử vẫn được mời lên đúng 1.14, bản iPhone 1.16.1 không bị mời cài gì, `releases/latest` vẫn là bản TV.

## iPad 1.16.1 — 13/11/2026

- **Sửa nhãn IMDb đè lên nhãn tập** trên thẻ phim (có từ 1.15): nhãn tập ở góc dưới trái và điểm IMDb ở góc dưới phải trước đây
  là hai lớp riêng, nên thẻ hẹp (iPhone cầm dọc, dải vuốt ngang) hoặc nhãn tập dài («Tập 189 mới») thì chồng lên nhau — rõ nhất ở
  trang «Điểm IMDb cao» và hàng «Có tập mới». Giờ đủ chỗ thì vẫn chung một hàng, không đủ thì nhãn IMDb lên hàng trên.
- **Màn «Cài lên tivi / mời bạn bè» trên iPhone cầm dọc** (lỗi của 1.16): nội dung không còn tràn sát hai mép màn hình — nút
  «Gửi hướng dẫn…» một hàng riêng, «Chép» / «Đóng» chia đôi hàng dưới; màn ngang và iPad như cũ.
- bản 1.16.1 (CFBundleVersion 22) · mã gói `vn.chimto.tv` · tag `ios-v1.16.1` (không phải "latest") · commit `3a3e862` (sửa:
  `9a972eb`, `9e706e7`) · `ConBangTV-iPad-1.16.1.ipa` 4 489 492 byte · SHA-256
  `fb6986e9e859eb5cd4e59a9f708f1c653b1ac155b1d0dc8f0bd1f40ae768eab4`
- 148 test lõi trên macOS (thêm 1). iPhone thật iOS 15.8.3 (414 pt), bản thử cùng mã nguồn: trang chủ (thẻ «Già Thiên» trước bị
  đè → nhãn IMDb nằm trên nhãn tập), «Điểm IMDb cao» (một hàng, vừa khít), Phim bộ xếp theo IMDb (hai hàng), màn «Cài lên tivi»
  dọc / ngang. Máy ảo iPhone SE 375 pt trên CI: các trang trên đúng, không đè, có lề. Bản chính thức cài lên iPhone (đè 1.16): mở
  được, không có báo cáo văng. Tệp tải từ link công khai và từ link cố định khớp SHA-256; `releases/latest` vẫn là bản TV. Chưa thử
  trên iPad (máy thử không bật).

## 2.16.1 — 13/11/2026

- **Sửa nhãn IMDb đè lên nhãn tập** trên thẻ phim (có từ 2.15): khi không đủ chỗ chung một hàng — điện thoại cầm dọc, thẻ hẹp ở
  dải vuốt ngang, nhãn tập dài như «Tập 108 mới» — nhãn IMDb nằm ở hàng trên, sát mép phải; nhãn tập quá dài thì cắt bằng dấu ….
  Đủ chỗ thì vẫn một hàng như cũ (nhãn tập trái, IMDb phải). Thấy rõ nhất ở trang «Điểm IMDb cao» và hàng «Có tập mới».
- versionCode 40 · tag `v2.16.1` · commit `3a3e862` (sửa: `9a972eb`) · `ChimToTV-2.16.1.apk` 1 248 537 byte · SHA-256
  `5eaa6c99e24f90d4191dcd0f0883cd6ca189aa98e59f15028b88f3d80166a82c` · kèm `ChimToTV.apk` (bản sao giống hệt, cho link cố định).
- 156 test (thêm 1). Thử trên TV box: bản gỡ lỗi ở chế độ TV («Điểm IMDb cao», Phim bộ xếp theo IMDb — vẫn một hàng) và giả điện
  thoại cầm dọc 3 cột (các trang trên — nhãn IMDb ở hàng trên, không đè); bản chính thức: «Điểm IMDb cao», trang chủ (thẻ có
  «Tập 108 mới» + IMDb xếp hai hàng). Đăng bằng `release.ps1`: Latest = `v2.16.1`, hai tệp `.apk` cùng SHA-256, link cố định của
  Downloader tải đúng bản 2.16.1, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.16 → 2.16.1** trên TV box lúc
  07:47 (hộp cập nhật chỉ hiện mục «Có gì mới»). Chưa thử trên điện thoại Android thật. Bản Windows không dính lỗi này (nhãn tập ở
  góc trên trái) nên không có bản mới.

## iPad 1.16 — 13/11/2026

- **Cài lên tivi / mời bạn bè** — mục mới ở Cài đặt → Liên hệ («Cài lên tivi / mời bạn bè — mã Downloader 9192365»): màn hiện
  **mã Downloader `9192365`** thật to cùng ba bước cài bản TV lên tivi / box Android, không cần USB hay máy tính. Nút **«Gửi
  hướng dẫn…»** mở bảng chia sẻ của máy (Zalo, Tin nhắn, AirDrop…) với sẵn tin nhắn hướng dẫn — cùng nội dung với bản TV và
  Windows; **«Chép»** chép tin nhắn đó; «Đóng». Dùng được cả dọc lẫn ngang, iPad và iPhone. Không có mã QR.
- bản 1.16 (CFBundleVersion 21) · mã gói `vn.chimto.tv` · tag `ios-v1.16` (không phải "latest") · commit `c4472af` (tính năng:
  `2d96a65`) · `ConBangTV-iPad-1.16.ipa` 4 481 661 byte · SHA-256
  `22cc5039b80199b9e98c7475916bf5ea6f6ee7445c53293520e3d175579e9295`
- 147 test lõi trên macOS (thêm 2). Bản thử 117 (cùng mã nguồn app) trên iPhone iOS 15.8.3: Cài đặt → cuộn tới «Liên hệ» → bấm
  mục mới, màn hiện đúng khi cầm dọc và ngang; «Chép» đổi thành «Đã chép»; «Gửi hướng dẫn…» mở bảng chia sẻ có đúng tin nhắn;
  «Đóng» về lại Cài đặt. Bản chính thức cài lên iPhone (đè 1.15): mở được, không có báo cáo văng (dừng ở màn «Miễn trừ trách
  nhiệm» — bản chính thức trên máy thử chưa bấm «Đồng ý», nên chưa xem được màn mới trong chính bản này). Bản thử thấy bản mới
  trên GitHub và hiện hộp «Có bản mới 1.16» lúc 05:20:50. **Bản iPad đầu tiên đăng bằng `release.ps1`** (tự đặt
  `--latest=false`): `releases/latest` vẫn `v2.16`, tệp tải từ link công khai khớp SHA-256. Chưa thử trên iPad (máy thử không
  bật).
- Máy ảo iPhone trên CI lúc phát hành không mở được app: bộ máy ảo iOS 18.5 của máy chạy GitHub không có
  `/usr/lib/swift/libswiftWebKit.dylib` mà tệp chạy đặt mức tối thiểu iOS 15 lại cần. Chỉ máy ảo bị — tệp chạy trong IPA 1.16 nạp
  cùng danh sách thư viện, cùng SDK 18.5 với IPA 1.15, và chạy trên iPhone thật. (Sau phát hành đã sửa job chụp màn hình máy ảo:
  dựng riêng bản máy ảo với mức tối thiểu = đời SDK, qua màn miễn trừ lần đầu mở, dừng với lỗi khi app không trả lời.)
- **Lỗi đã biết (tìm ra sau phát hành, nhờ ảnh máy ảo iPhone SE):** trên iPhone cầm dọc màn rộng 375 pt (SE / 6s / 7 / 8 / X…),
  màn «Cài lên tivi» tràn sát hai mép màn hình vì ba nút xếp một hàng không đủ chỗ — vẫn bấm được; màn 320 pt hoặc cỡ chữ lớn thì
  bị cắt hai bên. iPhone 6s Plus (414 pt) dùng để thử không lộ ra. Đã sửa trong mã nguồn (nút «Gửi hướng dẫn…» một hàng riêng,
  «Chép» / «Đóng» chia đôi hàng dưới; thử trên máy ảo 375 pt và iPhone thật) — **đã sửa ở bản 1.16.1**.

## Windows 1.14 — 13/11/2026

- **Cài lên tivi / mời bạn bè** — mục mới ở Cài đặt → Giới thiệu («Cài lên tivi / mời bạn bè — mã Downloader 9192365»): trang
  hiện **mã Downloader `9192365`** thật to cùng ba bước cài bản TV lên tivi / box Android (cài app Downloader → nhập mã → Install),
  không cần USB. Ba nút: **«Chép hướng dẫn để gửi bạn bè»** (chép sẵn tin nhắn để dán vào Zalo, tin nhắn — cùng nội dung với nút
  «Gửi hướng dẫn…» của bản TV), **«Chép mã»**, **«Mở trang hướng dẫn»**. Không có mã QR (máy tính thì gửi thẳng tin nhắn).
- Ghi chú bản này trên trang Releases có thêm link trang giới thiệu của app: <https://conbangtv.github.io>.
- bản 1.14.0 · tag `win-v1.14` (không phải "latest") · commit `c4472af` (tính năng: `a8bd585`) · `ConBangTV-Windows-1.14.exe`
  72 924 136 byte · SHA-256 `9fdd8581683f6745cb048abb112c11e47f15d6400d5a06bd138fa504106e63e8`
- 131 test lõi (thêm 2: mã, link và tin nhắn gửi người quen đúng từng chữ như bản TV). Thử tệp đóng gói (điều khiển bằng UI
  Automation): Cài đặt → mục mới mở đúng trang, đủ chữ; «Chép mã» đưa `9192365` vào bộ nhớ tạm, «Chép hướng dẫn…» đưa đúng tin
  nhắn; không có cổng gỡ lỗi. **Bản Windows đầu tiên đăng bằng `release.ps1`** (tự đặt `--latest=false`): `releases/latest` vẫn
  `v2.16`, tệp tải từ link công khai khớp SHA-256. **Tự cập nhật thật 1.13 → 1.14** (bản chép riêng) lúc 04:40:31 — hộp «Có bản
  mới» hiện đúng mục «Có gì mới».

## 2.16 — 12/11/2026

- **Mời bạn bè cài app** — mục mới trong menu bên trái (ngay trên «Cài đặt») và trong Cài đặt → Liên hệ: màn hiện **mã Downloader
  `9192365`** thật to, ba bước cài (cài app Downloader trên tivi → nhập mã → Install) và mã QR mở trang hướng dẫn tiếng Việt, để
  người quen tự cài Côn Bằng TV lên tivi của họ mà không cần USB hay máy tính. Mã do aftv.news cấp, trỏ vào link cố định
  `…/releases/latest/download/ChimToTV.apk` (luôn là bản TV mới nhất).
- Dùng app trên điện thoại / máy tính bảng: màn này xếp dọc, có nút **«Gửi hướng dẫn…»** gửi cách cài qua Zalo, tin nhắn.
- Trang Releases: ghi chú mỗi bản TV từ nay có thêm mục «Cài mới trên Android TV» ghi mã Downloader (hộp cập nhật trên TV không
  đổi — vẫn chỉ hiện «Có gì mới»).
- versionCode 39 · tag `v2.16` · commit `2892584` (tính năng: `28abd2a`, `5249319`) · `ChimToTV-2.16.apk` 1 248 194 byte (lớn hơn
  2.15 4 KB — mã QR vẽ sẵn, không thêm thư viện) · SHA-256 `16a6c2f338a1ae5383f26f628a4bac94b0b33697dd166f4fe727c76be7974e79` ·
  kèm `ChimToTV.apk` (bản sao giống hệt, cho link cố định).
- 154 test (thêm 3: mã QR trong mã nguồn được dựng lại và đọc lại bằng ZXing — chỉ trong test). Thử trên TV box, bản gỡ lỗi và
  bản chính thức: vào màn từ menu bằng remote và từ Cài đặt, OK / Back đóng màn, mã QR trên ảnh chụp màn hình đọc ra đúng link
  hướng dẫn; giả điện thoại cầm dọc: bố cục dọc, nút «Gửi hướng dẫn…» mở bảng chọn app. **Bản đầu tiên đăng bằng `release.ps1`:**
  Latest = `v2.16`, hai tệp `.apk` cùng SHA-256, link cố định tải về đúng bản 2.16; API `releases/latest` trả `v2.16` + `.apk`, tệp
  tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.15 → 2.16** trên TV box lúc 21:07:50 (hộp cập nhật chỉ hiện mục
  «Có gì mới»). Chưa thử: nhập mã bằng app Downloader thật trên một tivi (TV box thử không cài Downloader), bấm «Gửi hướng dẫn…»
  trên điện thoại thật.

## Windows 1.13 — 12/11/2026

- **Sửa «Vượt chặn nhà mạng» Mức 2 trên Windows:** trước đây chọn Mức 2 thì trang bị nhà mạng chặn lại không vào được, trong khi
  Mức 1 vẫn vào được. Windows gửi lại mọi byte chưa được xác nhận trong một gói, nên gói gửi lại chứa nguyên gói bắt tay (có tên
  trang) và máy soi của nhà mạng đọc được. Bản này gửi theo nhịp riêng cho Windows: byte đầu (TTL 1), ba byte kế mỗi byte một gói,
  chờ trang xác nhận rồi mới gửi phần còn lại như Mức 1 — không gói nào còn chứa tên trang nguyên vẹn. Mỗi kết nối mới ở Mức 2
  chậm thêm khoảng 0,1 giây.
- Mức 1 (mặc định) và DNS mã hoá không đổi. Bản TV không bị lỗi này (Android gửi lại đúng 1 byte — bắt gói trên TV box 12/11);
  iPad / iPhone chưa đo lại.
- bản 1.13.0 · tag `win-v1.13` (không phải "latest") · commit `3250e9b` (sửa: `821dcdf`) · `ConBangTV-Windows-1.13.exe`
  72 922 753 byte · SHA-256 `862b248946b67962678ccd6cfb45a8656000e1fa2ad5f07760693b9a713fc3c8`
- 129 test lõi (thêm 2). Thử với trang đang bị chặn theo tên trang: mã cũ ở Mức 2 bị chặn 2/2 lần; mã mới qua 4/4 lần, bắt gói ở
  đầu nhận thấy gói gửi lại chỉ có 4 byte đầu. Bản gỡ lỗi ở Mức 2: trang chủ của trang bị chặn tải được, trình phát (WebView2) tải
  được ảnh từ trang bị chặn, phim vẫn phát; đối chứng qua cổng nội bộ: Mức 1 qua, tắt thì treo rồi lỗi, Mức 2 qua. Tệp đóng gói
  chạy ở Mức 2 tải được trang bị chặn. Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.15`. **Tự cập nhật thật
  1.12 → 1.13** (bản chép riêng) lúc 20:01:49.

## iPad 1.15 — 10/11/2026

- **Điểm IMDb** như TV 2.15: nhãn «IMDb 8.5» trên thẻ phim, dòng IMDb ở trang phim, mục menu «Điểm IMDb cao», nút «Mới cập nhật» /
  «Điểm IMDb cao» ở danh mục KKPhim.
- bản 1.15 (CFBundleVersion 20) · mã gói `vn.chimto.tv` · tag `ios-v1.15` (không phải "latest") · commit `8425261` ·
  `ConBangTV-iPad-1.15.ipa` 4 453 281 byte · SHA-256 `484d5ff393285221074af8d580744964ce7ba8124b6b37355f497b751e00bc7a`
- 145 test lõi trên macOS (thêm 5). Bản thử 109 trên iPhone: trang «Điểm IMDb cao», nút xếp trong «Phim bộ» KKPhim. Bản chính
  thức cài lên iPhone (đè 1.14): chạy. Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.15`.

## Windows 1.12 — 10/11/2026

- **Điểm IMDb** như TV 2.15: nhãn «IMDb 8.5» trên thẻ phim, dòng IMDb ở trang phim, mục menu «Điểm IMDb cao», nút «Mới cập nhật» /
  «Điểm IMDb cao» ở danh mục KKPhim.
- bản 1.12.0 · tag `win-v1.12` (không phải "latest") · commit `8425261` · `ConBangTV-Windows-1.12.exe` 72 922 413 byte · SHA-256
  `2ec0af487d33914b343eb008ebf28aa55f10e364ed2bd8ef3f38a240cd3d6b5b`
- 127 test lõi (thêm 4). Thử bản gỡ lỗi: trang «Điểm IMDb cao», nút xếp trong «Phim bộ» KKPhim. Tệp tải từ link công khai khớp
  SHA-256. **Tự cập nhật thật 1.11 → 1.12** (bản chép riêng) lúc 19:25:08.

## 2.15 — 10/11/2026

- **Điểm IMDb** (lấy từ dữ liệu KKPhim — phimapi.com có sẵn điểm + số lượt chấm IMDb; không gọi IMDb):
  - Thẻ phim có nhãn vàng «IMDb 8.5» ở góc dưới bên phải (phim nguồn KKPhim, từ 100 lượt chấm); trang phim KKPhim ghi
    «IMDb 8.7 (448 nghìn lượt chấm)».
  - **Mục menu «Điểm IMDb cao»** (sau «Phim theo dõi»): hàng Phim lẻ, Phim bộ, Phim Hàn Quốc, Phim Âu Mỹ, Phim Trung Quốc, Hoạt
    hình, mỗi hàng 60 phim — xếp theo điểm có trọng số (như Top 250 của IMDb: phim ít lượt chấm bị kéo về điểm trung bình) trong
    vài trăm phim mới cập nhật mỗi nhóm (API KKPhim không sắp theo điểm, kho ≈ 26 nghìn phim). Hồ sơ trẻ em chỉ hàng Hoạt hình.
    Lần đầu mở ~20 giây trên TV box, sau đó lưu 12 giờ.
  - **Danh mục KKPhim**: nút «Mới cập nhật» / «Điểm IMDb cao» ở đầu mục — xếp ≈ 320 phim mới cập nhật của mục theo điểm IMDb.
- Màn miễn trừ trách nhiệm: bấm OK khi đang ở vùng chữ cũng là «Đồng ý» (trước đây mở app ở chế độ chạm thì phải ▼ tới nút).
- versionCode 38 · tag `v2.15` · commit `8425261` (tính năng: `9445879`, `f2539cd`) · `ChimToTV-2.15.apk` 1 244 099 byte · SHA-256
  `beaa11b18e345916467216f8ce778b4df0f6e212c29aede87a6071633cf1e96d`
- 151 test (thêm 5). Thử bản gỡ lỗi trên TV box: trang «Điểm IMDb cao», nút xếp trong «Phim bộ», dòng IMDb ở trang phim. API
  `releases/latest` trả `v2.15` + `.apk`, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.14 → 2.15** trên TV box
  lúc 19:22:43, bản chính thức mở «Điểm IMDb cao» được.
- **Bổ sung 12/11/2026 — link tải cố định cho app Downloader:** release `v2.15` có thêm tệp `ChimToTV.apk`, bản sao giống hệt
  `ChimToTV-2.15.apk` (cùng 1 244 099 byte, cùng SHA-256), để
  `https://github.com/Circling5714/ChimToTV/releases/latest/download/ChimToTV.apk` luôn tải thẳng bản TV mới nhất. App không đổi.
  Từ nay mọi bản TV đăng bằng `release.ps1` đều kèm hai tệp. Thử lại cập nhật tự động với release hai tệp: TV box hạ về 2.14 →
  2.15 lúc 20:22:45.

## iPad 1.14 — 10/11/2026

- **Miễn trừ trách nhiệm & quyền riêng tư** như TV 2.14: lần đầu mở app (kể cả máy cập nhật từ bản cũ) hiện màn «Trước khi bắt
  đầu» phủ toàn màn — «Đồng ý» mới vào app, «Thoát» đưa app về màn hình chính (lần mở sau vẫn hỏi). Xem lại: Cài đặt → Liên hệ →
  «Miễn trừ trách nhiệm & quyền riêng tư».
- bản 1.14 (CFBundleVersion 19) · mã gói `vn.chimto.tv` · tag `ios-v1.14` (không phải "latest") · commit `f179d63` ·
  `ConBangTV-iPad-1.14.ipa` 4 418 891 byte · SHA-256 `5de3efdbc1693811c12aa64a2d81912657dcf0eafffc637dbc50e235cf7ab801`
- 140 test lõi trên macOS. Bản thử 105 trên iPhone: hiện màn «Trước khi bắt đầu», «Đồng ý» → trang chủ. Bản chính thức cài lên
  iPhone (đè 1.13): chạy. Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.14`.

## Windows 1.11 — 10/11/2026

- **Miễn trừ trách nhiệm & quyền riêng tư** như TV 2.14: lần đầu mở app (kể cả máy cập nhật từ bản cũ) hiện hộp «Trước khi bắt
  đầu» — «Đồng ý» mới vào app, «Thoát» / Esc là đóng app; hỏi cập nhật và «Ai đang xem?» chờ sau khi đồng ý. Xem lại: Cài đặt →
  «Miễn trừ trách nhiệm & quyền riêng tư».
- bản 1.11.0 · tag `win-v1.11` (không phải "latest") · commit `f179d63` · `ConBangTV-Windows-1.11.exe` 72 915 844 byte · SHA-256
  `f4023adc65894cfa28ea7a0319c89a749babcb7abedaa64311345278bce5ab5f`
- 123 test lõi. Tệp tải từ link công khai khớp SHA-256. **Tự cập nhật thật 1.10 → 1.11** (bản chép riêng) lúc 16:55:50 — mở lại
  là hiện hộp «Trước khi bắt đầu».

## 2.14 — 10/11/2026

- **Miễn trừ trách nhiệm & quyền riêng tư:** lần đầu mở app (kể cả TV cập nhật từ bản cũ) hiện màn «Trước khi bắt đầu» với 9 mục
  (app chỉ là trình xem, bản quyền, trách nhiệm người dùng, trang bên thứ ba, tính năng mạng, dữ liệu và quyền riêng tư — DNS mã hoá
  không lưu nhật ký truy cập —, không bảo đảm, trẻ em, liên hệ). Chọn «Đồng ý» mới vào app; «Thoát» / Back là đóng app. Remote: ▲
  lên vùng chữ, ▲▼ cuộn, hết chữ ▼ về nút. Hỏi «Ai đang xem?» chờ sau khi đồng ý; đã đồng ý thì không hỏi lại (mỗi máy riêng).
- Xem lại: Cài đặt → Liên hệ → «Miễn trừ trách nhiệm & quyền riêng tư». README có mục cùng nội dung.
- versionCode 37 · tag `v2.14` · commit `f179d63` (tính năng: `a418a6c`) · `ChimToTV-2.14.apk` 1 239 190 byte · SHA-256
  `950a36576c4bfed95c11d63fecd12fb5b8d71a184af2b3cb14ff2f20c634d6d3`
- 146 test. Thử bản gỡ lỗi trên TV box (hiện trước «Ai đang xem?», cuộn bằng remote, mở lại không hỏi). API `releases/latest` trả
  `v2.14` + `.apk`, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.13 → 2.14** trên TV box lúc 16:43:55 — mở
  lại là hiện màn «Trước khi bắt đầu», đồng ý xong vào app.
- Biết trước: mở app ở chế độ chạm (chuột bay / màn cảm ứng) thì lần bấm phím đầu chọn vùng chữ, OK ở đó chưa tính là «Đồng ý» —
  bấm ▼ tới nút. Đã sửa cho bản sau.

## iPad 1.13 — 10/11/2026

- **Proxy kiểu Shadowsocks (mã hoá)** như TV 2.13: Cài đặt → Proxy → «Kiểu: Shadowsocks (mã hoá)»; nút «Dán link ss:// từ bộ
  nhớ tạm» (hoặc dán vào ô máy chủ) tự điền máy chủ, cổng, kiểu mã hoá, khoá. Khoá lưu trong Keychain. «Kiểm tra proxy» báo IP ra
  ngoài; SS 2022 không trả lời thì xem giờ trên máy có lệch quá 30 giây không.
- bản 1.13 (CFBundleVersion 18) · mã gói `vn.chimto.tv` · tag `ios-v1.13` (không phải "latest") · commit `693ae51` ·
  `ConBangTV-iPad-1.13.ipa` 4 396 000 byte · SHA-256 `84a4f7265e9cc370c77e8a04c1640038b71db94ad5a7c31084b78b8054af78e2`
- 140 test lõi trên macOS (thêm 12). Bản thử trên iPhone: «Kiểm tra proxy» qua máy chủ 3X-UI thật ✓, phát phim KKPhim qua
  Shadowsocks. Bản chính thức cài lên iPhone (đè 1.12.1): chạy. Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.13`.

## Windows 1.10 — 10/11/2026

- **Proxy kiểu Shadowsocks (mã hoá)** như TV 2.13: Cài đặt → Proxy → «Kiểu proxy: Shadowsocks (mã hoá)», ô dán link `ss://` tự
  điền, chọn kiểu mã hoá, khoá mã hoá bằng tài khoản Windows (DPAPI); «Thử kết nối» báo IP ra ngoài và độ trễ. Trang, ảnh và trình
  phát đều đi qua máy chủ Shadowsocks. Kiểu ChaCha20 chỉ có khi Windows hỗ trợ (Windows 10 22H2 không có — nút ghi «máy này không
  có»); kiểu AES (mặc định của 3X-UI) chạy mọi máy.
- bản 1.10.0 · tag `win-v1.10` (không phải "latest") · commit `693ae51` · `ConBangTV-Windows-1.10.exe` 72 913 038 byte · SHA-256
  `206226cfc63cb30c9a12de29b2d83e739016c5f2c5d4fbcf7ddd447e5ac9ff04`
- 123 test lõi (thêm 14). Thử bản gỡ lỗi qua máy chủ 3X-UI thật: «Thử kết nối» ✓, phim VSMOV (server HLS) phát đều 90 giây. Tệp tải từ link công khai khớp SHA-256. **Tự cập nhật thật 1.9 → 1.10** (bản chép riêng) lúc 15:00:04.

## 2.13 — 10/11/2026

- **Proxy kiểu Shadowsocks (mã hoá):** Cài đặt → Proxy → nút «Kiểu: SOCKS5 / Shadowsocks». Dán link `ss://` chép từ 3X-UI (nút
  Copy / QR của người dùng) vào ô máy chủ là app tự điền máy chủ, cổng, kiểu mã hoá, khoá. Nhà mạng chỉ thấy dữ liệu đã mã hoá —
  không đọc được tên trang hay mật khẩu như với SOCKS5.
  - Kiểu mã hoá: `2022-blake3-aes-256-gcm` / `2022-blake3-aes-128-gcm` (mặc định của 3X-UI, máy chủ nhiều người dùng),
    `2022-blake3-chacha20-poly1305`, `aes-256-gcm`, `aes-128-gcm`, `chacha20-ietf-poly1305` (ChaCha20 cần Android 9+). Qua TCP
    thường (3X-UI: Transmission = TCP, Security = None); link dùng ws / grpc / tls / plugin thì app báo chưa hỗ trợ.
  - «Kiểm tra proxy» báo IP ra ngoài và độ trễ; sai khoá / sai kiểu mã hoá, giờ trên TV lệch quá 30 giây (SS 2022 cần giờ đúng)
    thì báo rõ. Khoá lưu mã hoá bằng Android Keystore. SOCKS5 dùng như cũ — mỗi kiểu giữ máy chủ riêng; DNS của proxy / của thiết
    bị và «chỉ trang phim» dùng chung.
  - Lưu ý: TV box yếu xem «Trình phát VSMOV» qua Shadowsocks có thể giật (TV box thử rớt ~30% khung hình cả khi đi thẳng) — chọn
    server «HLS» (app tự gợi ý khi thấy giật).
- versionCode 36 · tag `v2.13` · commit `693ae51` (tính năng: `9307a07`, `cd04e6f`, `f206ece`, `0863a56`) · `ChimToTV-2.13.apk`
  1 234 912 byte · SHA-256 `b994f314e39b98abcd5e6d033071afcc1aa49b9ba2116d557761ca6f78234977`
- 146 test (thêm 14: BLAKE3 theo bộ số chuẩn, link 3X-UI / SIP002, máy chủ thử viết theo đặc tả). Thử với Xray chính thức (cả 6
  kiểu mã hoá) và máy chủ 3X-UI thật: TV box dán link → «Kiểm tra proxy» ✓ 0,3 s, trang chủ + phim KKPhim (HLS) phát mượt qua
  Shadowsocks. Bản chính thức thử trên TV box trước khi đăng, hạ về 2.12, **cập nhật tự động thật 2.12 → 2.13 lúc 14:53:54**.
  API `releases/latest` trả `v2.13` + `.apk`.

## iPad 1.12.1 — 10/11/2026

- **Sửa:** chạm / giữ một phim (nhất là phim VSMOV) có khi trúng phim bên cạnh. Ảnh bìa phóng cho đầy khung 2:3 (ảnh ngang của
  VSMOV rộng gấp mấy lần thẻ); phần thừa bị cắt khi vẽ nhưng vẫn là vùng chạm / vùng giữ, phủ lên thẻ kế bên. Giờ ảnh bìa nằm gọn
  trong khung, không nhận chạm; vùng chạm của thẻ đúng bằng khung thẻ. TV / Windows không bị lỗi này.
- bản 1.12.1 (CFBundleVersion 17) · mã gói `vn.chimto.tv` · tag `ios-v1.12.1` (không phải "latest") · commit `423e9f2` (sửa:
  `eae166c`) · `ConBangTV-iPad-1.12.1.ipa` 4 345 736 byte · SHA-256 `09f7a3abc398a188c9b898aff73de26b503d23f46341a8a6cb5169a04a3f7351`
- 128 test lõi trên macOS. Anh thử bằng tay trên iPhone (bản thử 96, danh sách VSMOV → Phim bộ): chạm / giữ đúng thẻ. Bản chính thức
  cài lên iPhone (đè 1.12): chạy, không văng. Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.12`.

## iPad 1.12 — 10/11/2026

- **Phim liên quan ở trang phim** như TV 2.12: hàng «Cùng bộ» và «Cùng thể loại» (thẻ phim như trang chủ); phim lẻ ngay dưới nút
  «Xem phim», phim bộ ở cuối danh sách tập + nút «Phim liên quan ▼» cuộn thẳng xuống.
- bản 1.12 (CFBundleVersion 16) · mã gói `vn.chimto.tv` · tag `ios-v1.12` (không phải "latest") · commit `f4df900` ·
  `ConBangTV-iPad-1.12.ipa` 4 344 479 byte · SHA-256 `b0eba931829776a0e65719b1fd0688bcbc6e0e452552aa43802a3a03593373c5`
- 128 test lõi trên macOS (thêm 5). Bản thử 93 thử thật trên iPhone: phim bộ «Đấu Phá Thương Khung (Phần 5)» → nút «Phim liên
  quan ▼» → hàng «Cùng bộ «Đấu Phá Thương Khung»» và «Cùng thể loại · Hoạt hình Trung Quốc · Phiêu lưu»; phim lẻ «… Thức Tỉnh» hàng
  ngay dưới nút Xem. Bản chính thức đã cài lên iPhone (gói app ghi 1.12; máy đang khoá màn hình nên chưa mở lên được). Tệp tải từ
  link công khai khớp SHA-256; `releases/latest` vẫn `v2.12`.

## Windows 1.9 — 10/11/2026

- **Phim liên quan ở trang phim** như TV 2.12: hàng «Cùng bộ» (các phần / mùa khác, phim ngoại truyện) và «Cùng thể loại»; phim
  lẻ ngay dưới thông tin phim, phim bộ ở cuối danh sách tập + nút «▼ Phim liên quan» (chọn sẵn phim đầu).
- bản 1.9.0 · tag `win-v1.9` (không phải "latest") · commit `f4df900` · `ConBangTV-Windows-1.9.exe` 72 893 026 byte · SHA-256
  `858c4e7ba911c84c72ee67951f293373ca39facd90373134cd6b7f67de3d3ebf`
- 109 test lõi (thêm 5). Thử bản gỡ lỗi: trang «Đấu Phá Thương Khung (Phần 5)» → nút «▼ Phim liên quan» cuộn xuống hàng «Cùng bộ»,
  chọn phim đầu. Tệp tải từ link công khai khớp SHA-256. **Tự cập nhật thật 1.8.1 → 1.9** (bản chép riêng).

## 2.12 — 10/11/2026

- **Phim liên quan ở trang phim:**
  - **Cùng bộ** — các phần / mùa khác và phim ngoại truyện của cùng bộ, xếp theo số phần (tìm theo tên gốc trên trang của phim +
    KKPhim; bỏ phim trùng ở nguồn khác).
  - **Cùng thể loại** — KKPhim / VSMOV theo loại + nước + thể loại đặc trưng nhất; bỏ phim đang xem và các phần cùng bộ, phim
    chưa xem lên trước; hồ sơ trẻ em chỉ hoạt hình.
  - Phim lẻ: hàng nằm ngay dưới nút «Xem phim» (bỏ ô tập «Full» thừa). Phim bộ: ở cuối danh sách tập + nút «Liên quan ▼» nhảy
    xuống. Đang chọn trong hàng liên quan thì dòng phía trên lưới ghi tên hàng (màn TV thấp, tên hàng hay bị cuộn khuất). Tải sau
    khi trang phim đã hiện.
- Chưa có «cùng đạo diễn / diễn viên» — KKPhim, VSMOV, NguonC, ophim chỉ tìm được theo tên phim.
- versionCode 35 · tag `v2.12` · commit `f4df900` · `ChimToTV-2.12.apk` 1 222 867 byte · SHA-256
  `ae65639e09a94ff68917fda84178100602c3df6e4cbc1787eb04582d1138d3b5`
- 132 test (thêm 5). Thử bản gỡ lỗi trên TV box: phim bộ (nút «Liên quan ▼» → hàng «Cùng bộ», ▼ → «Cùng thể loại», ▲ về tập, nhãn
  đổi theo hàng), phim lẻ (hàng ngay dưới nút Xem). Bản chính thức thử trên TV box trước khi đăng (phim yanhh3d có nút + hàng «Cùng
  bộ»). API `releases/latest` trả `v2.12` + `.apk`, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.11.1 → 2.12**
  trên TV box.

## iPad 1.11.1 — 09/11/2026

- **DNS mã hoá nhanh hơn** như TV 2.11.1: NextDNS dùng máy chủ gần nhất (Hà Nội) thay cho máy chủ Hồng Kông, giữ kết nối tới
  máy chủ DNS để dùng lại (Network.framework, tối đa 90 giây).
- bản 1.11.1 (CFBundleVersion 15) · mã gói `vn.chimto.tv` · tag `ios-v1.11.1` (không phải "latest") · commit `a775794` ·
  `ConBangTV-iPad-1.11.1.ipa` 4 316 750 byte · SHA-256 `64547dce3321e7b7ff75728e101efc9b698302a48bb7d7d380a46b7175f14147`
- 123 test lõi trên macOS (thêm 2). Bản thử 90 thử thật trên iPhone: «NextDNS gần nhất: 38.54.31.178, 171.244.57.202»; 9/9 tập
  phát (NguonC, VSMOV, yanhh3d, mephim, hoathinh3d, KKPhim); 28 lần hỏi DNS trung vị 108 ms, dùng lại kết nối 19–22 ms, không lần
  nào > 300 ms, không lỗi. Bản chính thức cài lên iPhone (đè 1.11): chạy, dữ liệu giữ nguyên. Chưa cài lên iPad (anh đang dùng).
  Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.11.1`.

## Windows 1.8.1 — 09/11/2026

- **DNS mã hoá nhanh hơn** như TV 2.11.1: NextDNS dùng máy chủ gần nhất (Hà Nội) thay cho máy chủ Hồng Kông, giữ kết nối tới
  máy chủ DNS để dùng lại (tối đa 90 giây).
- bản 1.8.1 · tag `win-v1.8.1` (không phải "latest") · commit `a775794` · `ConBangTV-Windows-1.8.1.exe` 72 890 281 byte ·
  SHA-256 `e4d15cdd92c3b11299c87c9ac85a3099b447fb8fe1ea7bfd78cc997822b4fdab`
- 104 test lõi (thêm: mọi bản ghi A, đọc ba trả lời keep-alive liên tiếp, chỉ NextDNS lấy máy chủ gần nhất). Thử bản gỡ lỗi
  trên máy Windows: «NextDNS gần nhất: 38.54.31.178, 171.244.57.202» (Hà Nội), hỏi dùng lại kết nối 70–260 ms lúc mở app. Tệp tải
  từ link công khai khớp SHA-256. **Tự cập nhật thật 1.8 → 1.8.1** (bản chép riêng): mở lại 1.8.1, tìm được NextDNS Hà Nội ngay.

## 2.11.1 — 09/11/2026

- **DNS mã hoá nhanh hơn:** NextDNS dùng máy chủ gần mạng nhà nhất thay cho IP anycast (từ mạng Viettel bị đưa sang Hồng Kông):
  lần đầu app hỏi chính NextDNS tên `dns.nextdns.io` (đã mã hoá; NextDNS trả máy chủ gần IP của mạng này), không được thì DNS của
  máy; nhớ 6 giờ; anycast vẫn đứng sau để dự phòng. Giữ kết nối TLS tới máy chủ DNS để dùng lại (DoT; DoH bằng HTTP/1.1
  keep-alive), tối đa 4 kết nối nghỉ mỗi máy chủ, bỏ sau 90 giây; máy chủ đóng kết nối thì mở mới ngay (không tính là máy chủ lỗi).
- Đo từ mạng nhà (máy tính cùng mạng TV): NextDNS Hà Nội hỏi «lạnh» ~140–160 ms / «ấm» ~18 ms, anycast Hồng Kông ~210 / ~40 ms,
  Cloudflare ~200 / ~35–41 ms, Google ~174 / ~30–37 ms; mở kết nối TLS mới chiếm phần lớn thời gian.
- versionCode 34 · tag `v2.11.1` · commit `a775794` · `ChimToTV-2.11.1.apk` 1 216 343 byte · SHA-256
  `0ba4163740303b5e6ec2a83a7578f7195139c8b12447a481a8cb1e065ef95f24`
- 127 test (thêm 2). Thử bản gỡ lỗi trên TV box: «NextDNS gần nhất: 38.54.31.178, 171.244.57.202»; 6 tập (yanhh3d, VSMOV, mephim,
  hoathinh3d, KKPhim, NguonC) đều phát, 23 lần hỏi DNS trung vị 109 ms (dùng lại kết nối 39–140 ms; >300 ms chỉ lúc mở app — TV box
  bắt tay TLS ~400–750 ms), không lỗi. Bản chính thức thử trên TV box trước khi đăng (phát yanhh3d). API `releases/latest` trả
  `v2.11.1` + `.apk`, tệp tải từ link công khai khớp SHA-256. **Cập nhật tự động thật 2.11 → 2.11.1** trên TV box.
- Sau 2.11 (không phải lỗi app): hồ sơ NextDNS của app đang chặn một số tên miền app cần (yanhh3d.ee, máy chủ video KKPhim
  `*.phim1280.tv` / `kvp726.com` / `kkphimplayer1–7.com`, ảnh `clbphimxua.com` / `img.ophim.live`, `imasdk.googleapis.com`) →
  video lỗi; đã thêm vào danh sách cho phép của hồ sơ, kiểm lại 290 tên miền không còn bị chặn.

## iPad 1.11 — 09/11/2026

- **DNS mã hoá** (mới trên iPad / iPhone, bật sẵn; Cài đặt → mục «DNS mã hoá», ngay trên «Vượt chặn nhà mạng»): như TV 2.11 —
  DoH rồi DoT, lần lượt NextDNS → Cloudflare → Google; bị chặn hết thì dùng DNS của máy — app không hỏng. Kết nối tới máy chủ DNS
  bằng Network.framework (IP cố định, kiểm chứng chỉ theo tên máy chủ), đi thẳng không qua cổng nội bộ.
- **Vượt chặn nhà mạng mức 1 bật sẵn.** Máy đang dùng cũng tự bật cả hai một lần khi cập nhật (đang mức 2 thì giữ), sau đó ai
  tắt thì giữ tắt. Như trước: trên iOS 15–16, lúc bật app không giữ cookie của trang web sau khi tắt app.
- **Sửa:** ảnh bìa CLB Phim Xưa không hiện khi DNS trong nhà chặn tên miền ảnh (`clbphimxua.com` — DNS trả 0.0.0.0).
- bản 1.11 (CFBundleVersion 14) · mã gói `vn.chimto.tv` · tag `ios-v1.11` (không phải "latest") · commit `4908aa4` ·
  `ConBangTV-iPad-1.11.ipa` 4 311 267 byte · SHA-256 `4ee6ec823ae9ce77f3aafcf44f5ac6a8f580913e8b8402c63e3ca1451aaf7f28`
- 121 test lõi trên macOS (thêm: gói DNS, trả lời HTTP, thứ tự máy chủ, mặc định mới, máy chủ chết → DNS máy). Bản thử 86–87 thử
  thật trên iPad và iPhone (mạng có bộ lọc DNS chặn thật): tắt Vượt chặn thì ảnh CLB Phim Xưa trống, bật thì hiện; mọi tên miền
  qua DoH NextDNS; giả chặn 3 DoH → DoT NextDNS (ảnh vẫn hiện); giả chặn cả 6 → DNS của máy (danh sách vẫn mở); iPad đang mức 2
  giữ mức 2, iPhone đang tắt tự bật mức 1. Bản chính thức cài lên iPad và iPhone (đè 1.10): chạy, dữ liệu đã ghi mặc định mới
  (`doh` = AUTO, `net_defaults_v1`). Tệp tải từ link công khai khớp SHA-256; `releases/latest` vẫn `v2.11`.

## Windows 1.8 — 09/11/2026

- **DNS mã hoá bật sẵn** như TV 2.11 (Cài đặt → «DNS mã hoá (DoH / DoT)»: Bật / Tắt): DoH rồi DoT, lần lượt NextDNS → Cloudflare →
  Google; bị chặn hết thì dùng DNS của máy — app không hỏng. **Vượt chặn nhà mạng mức 1 bật sẵn**; máy đang dùng cũng tự bật cả
  hai một lần khi cập nhật (đang mức 2 thì giữ), sau đó ai tắt thì giữ tắt.
- **Sửa:** trang chủ báo «An item with the same key has already been added» (cả trang thành «Không tải được») khi hai hàng cùng
  một danh sách — vd hàng tự thêm «Hoạt hình Trung Quốc» và hàng «Hoạt hình Trung Quốc mới cập nhật · mọi nguồn». Ảnh bìa CLB
  Phim Xưa không hiện khi DNS trong nhà chặn tên miền ảnh (`clbphimxua.com` — DNS trả 0.0.0.0).
- bản 1.8.0 · tag `win-v1.8` (không phải "latest") · commit `4908aa4` · `ConBangTV-Windows-1.8.exe` 72 885 597 byte · SHA-256
  `9a226978cb863679883bdff8db35a5b487b49e6bc838d391dc38643ca562d2b3`
- 102 test lõi (thêm 11: gói DNS, trả lời HTTP, thứ tự máy chủ, mặc định mới, hai hàng cùng danh sách…). Thử trên máy Windows 10
  có bộ lọc DNS chặn thật: ảnh CLB Phim Xưa hiện qua DoH NextDNS; giả chặn 4 máy chủ đầu → DoT Cloudflare; giả chặn cả 6 → DNS
  của máy (danh sách vẫn mở). Tệp tải từ link công khai khớp SHA-256. **Tự cập nhật thật 1.7 → 1.8** từ GitHub (bản chép riêng,
  dữ liệu mới): hộp thoại «Có bản mới: Côn Bằng TV 1.8» → Cập nhật ngay → mở lại 1.8, Vượt chặn mức 1 + DNS mã hoá đã bật.

## 2.11 — 09/11/2026

- **DNS mã hoá bật sẵn:** app hỏi địa chỉ trang qua DoH (RFC 8484) rồi DoT (RFC 7858), lần lượt NextDNS (hồ sơ riêng của app) →
  Cloudflare → Google; bị chặn hết thì dùng DNS của máy — app không bao giờ hỏng vì DNS mã hoá bị chặn. Máy chủ bị chặn nghỉ 2
  phút; nhiều kết nối cùng hỏi một tên miền thì chỉ hỏi một lần; tên miền không có (hồ sơ DNS chặn) thì không hỏi máy chủ khác.
  Cài đặt → «DNS mã hoá: Bật (DoH / DoT: NextDNS → Cloudflare → Google)» / Tắt (trước: chọn Cloudflare / Google / Tắt).
- **Vượt chặn nhà mạng mức 1 bật sẵn.** Máy đang dùng cũng tự bật cả hai một lần khi cập nhật (đang mức 2 thì giữ mức 2); sau đó
  ai tắt thì giữ tắt.
- **Sửa:** ảnh bìa CLB Phim Xưa không hiện khi DNS trong nhà chặn tên miền ảnh (`clbphimxua.com` — DNS trả 0.0.0.0): tắt DNS mã
  hoá mà vẫn bật Vượt chặn thì DNS của máy trả 0.0.0.0 / :: là app tự hỏi lại qua DNS mã hoá.
- Bỏ chữ «· DoH» cạnh tên trang (nay là mặc định).
- versionCode 33 · tag `v2.11` · commit `4908aa4` · `ChimToTV-2.11.apk` 1 215 137 byte · SHA-256
  `d6d2218d3fe067f3df6b237d9c7f9508b61a6facc6fdf9e20708e5fd5f766ea7`
- 125 test (thêm 9). Thử bản gỡ lỗi trên TV box: mọi tên miền qua DoH NextDNS; giả chặn 3 DoH → DoT NextDNS (phim vẫn phát);
  giả chặn cả 6 → DNS của máy (trang phim mở đủ ảnh); giả DNS máy trả 0.0.0.0 → ảnh CLB Phim Xưa vẫn hiện. Bản chính thức thử trên
  TV box trước khi đăng (Cài đặt hiện NextDNS, Vượt chặn mức 1). API `releases/latest` trả `v2.11` + `.apk`, tệp tải từ link công
  khai khớp SHA-256. **Cập nhật tự động thật 2.10 → 2.11** trên TV box.

## iPad 1.10 — 09/11/2026

- **Hàng vuốt ngang cả trên iPad và iPhone xoay ngang** (trước chỉ iPhone cầm dọc): «Xem thêm ›» ở tên hàng + ô hẹp cuối dải.
- **Sửa nhanh ngay trên trang chủ:** giữ ngón trên một phim, tên hàng hoặc ô «Xem thêm» (chuột: nhấn phải) → ẩn hàng, chuyển
  hàng lên / xuống (qua hàng đang hiện kề nó), «Các hàng trên trang chủ…», «Nguồn cho trang chủ…» (bảng riêng mở từ trang chủ,
  thanh tiêu đề tự vẽ; đóng bảng là trang chủ xếp lại / tải lại). Hồ sơ trẻ em có mã PIN phụ huynh: chỉ mở bảng các hàng (hỏi mã).
- **Nguồn cho trang chủ** (Cài đặt → Trang chủ, riêng từng người xem): bật / tắt KKPhim, NguonC, VSMOV, CLB Phim Xưa, trang đang
  dùng + trang xem gần đây; còn một nguồn thì không tắt được; tắt hết (nguồn còn bật bị xoá khỏi danh sách trang) thì coi như
  bật hết. Tắt KKPhim thì hàng lọc theo thể loại / nước lấy từ VSMOV.
- bản 1.10 (CFBundleVersion 13) · mã gói `vn.chimto.tv` · tag `ios-v1.10` (không phải "latest") · commit `947c004` ·
  `ConBangTV-iPad-1.10.ipa` 4 284 165 byte · SHA-256 `38735a306dc8decf5860684846ca5d836af0ff3476d7df533864224b737026d8`
- 112 test lõi trên macOS (thêm 4 test: chuyển qua hàng đang hiện, ẩn hàng, loại hàng bản cũ, nguồn tắt). Bản thử 83 thử thật:
  iPad — dải, ẩn / chuyển hàng, bảng «Nguồn» mở từ trang chủ (tắt VSMOV → đóng bảng → trang chủ tải lại, «2 nguồn» thành
  «kkphim»), không còn lỗi bảng mở / đóng liên tục; iPhone — dải dọc / ngang, bảng «Các hàng» trên màn hẹp, bật lại hàng → đóng
  bảng tải lại. Bản chính thức cài lên iPad và iPhone (đè 1.9): chạy, tải trang chủ, không văng. Tệp tải từ link công khai khớp
  SHA-256; `releases/latest` vẫn `v2.10`.

## Windows 1.7 — 09/11/2026

- **Hàng vuốt ngang** như TV 2.10: tới 12 phim một dải, ô «Xem thêm» hẹp cuối dải, «Xem thêm ›» ở tên hàng (chuột), nút ‹ ›
  khi rê chuột, Shift + lăn chuột cuộn dải (lăn chuột thường vẫn cuộn trang). Phím: ◄ ► chỉ đi trong dải (cuối dải đứng yên, ◄
  ở phim đầu ra menu bên trái), ▲ ▼ sang phim đang thấy của dải kia.
- **Sửa nhanh ngay trên trang chủ:** nhấn phải / phím Menu / Shift + F10 / M trên phim của một hàng hoặc tên hàng → «Hàng
  «…»»: ẩn hàng này, ▲ / ▼ chuyển (giữ ô đang chọn), «Các hàng…», «Nguồn…». Hồ sơ trẻ em: mã PIN phụ huynh.
- **Nguồn cho trang chủ** (Cài đặt → «Nguồn cho trang chủ: n / m…», riêng từng người xem) như bản TV.
- **Sửa:** mở lại cửa sổ luôn kéo về trong màn hình (trước chỉ kiểm chiều ngang — lưu sát mép dưới / màn hình phụ đã rút thì lần
  sau cửa sổ gần như nằm ngoài màn hình, trông như trắng một nửa). Biến môi trường `CONBANG_SOFTWARE_RENDER=1`: vẽ bằng CPU.
- bản 1.7.0 · tag `win-v1.7` (không phải "latest") · commit `947c004` · `ConBangTV-Windows-1.7.exe` 72 876 194 byte · SHA-256
  `fa5db5b359bbb8877b57bf42cb76160c1b49f90798c6af27fee6d44feb4771ef`
- 91 test lõi (thêm 4). Thử trên máy Windows 10 (bản gỡ lỗi): dải + phím mũi tên, phím M / Menu → bảng, chuyển xuống (giữ ô),
  ẩn hàng (chọn sang phim đầu hàng kế), trang «Nguồn» (tắt / bật NguonC). Tệp tải từ link công khai khớp SHA-256. **Tự cập nhật
  thật 1.6 → 1.7** từ GitHub (bản chép riêng, dữ liệu mới): hộp thoại «Có bản mới: Côn Bằng TV 1.7» → Cập nhật ngay → mở lại 1.7
  với trang chủ dải vuốt ngang.

## 2.10 — 09/11/2026

- **Trang chủ dạng hàng vuốt ngang trên TV / màn ngang** (như điện thoại cầm dọc từ 2.8.1): thấy 5 phim và một phần phim kế, ◄ ►
  cuộn trong hàng (tới 12 phim; ◄ ở phim đầu ra menu bên trái), ô «Xem thêm» hẹp ở cuối hàng; Back từ «Xem thêm» về đúng ô.
- **Sửa nhanh ngay trên trang chủ:** nhấn Menu hoặc giữ OK trên phim của một hàng / ô «Xem thêm» (cảm ứng: giữ tên hàng) →
  «Hàng «…»»: ẩn hàng này, ▲ chuyển lên / ▼ chuyển xuống (qua hàng đang hiện kề nó), «Các hàng trên trang chủ…», «Nguồn cho
  trang chủ…». Hộp xoá phim ở «Xem tiếp» / bỏ theo dõi ở «Có tập mới» có nút «Ẩn hàng này». Hồ sơ trẻ em: mã PIN phụ huynh.
- **Nguồn cho trang chủ** (Cài đặt → Trang chủ → «Nguồn cho trang chủ: n / m…», riêng từng người xem): bật / tắt KKPhim, NguonC,
  VSMOV, CLB Phim Xưa, trang đang dùng + trang xem gần đây; còn một nguồn thì không tắt được; tắt hết thì coi như bật hết. Tắt
  KKPhim thì hàng lọc theo thể loại / nước lấy từ VSMOV. Đổi nguồn là tải lại trang chủ.
- TV box: dải không vẽ vệt cuộn quá đầu / cuối; vẽ lại khung lưới sau hiệu ứng phóng to ô (trước còn vệt viền cam ở mép lưới).
- versionCode 32 · tag `v2.10` · commit `947c004` · `ChimToTV-2.10.apk` 1 212 683 byte · SHA-256
  `b777935cd2928bdd66fa0838b12daed5a11eb0e547eacbaf58ed73fed5086411`
- 116 test (thêm 4). Thử bản gỡ lỗi trên TV box bằng remote: Menu → bảng, ▲ chuyển (giữ ô), ẩn hàng (chọn sang hàng kế), tắt
  KKPhim (hàng lọc lấy VSMOV, không còn nhãn «kkphim»), dải: ► tới ô «Xem thêm», ◄ ra menu, ▼ sang dải dưới, Back từ «Xem thêm» về
  đúng ô. Bản chính thức thử trên TV box trước khi đăng (dải, bảng sửa nhanh). API `releases/latest` trả `v2.10` + `.apk`, tệp tải từ
  link công khai khớp SHA-256. **Cập nhật tự động thật 2.9 → 2.10** trên TV box.

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
  các hàng hiện đúng. Bản chính thức cài lên iPad (đè 1.8.2): mở được, không văng; cài lên iPhone (đè 1.8.2): mở được, tải
  xong trang chủ (ảnh poster mới ghi), không văng. `releases/latest` vẫn `v2.9`; tệp tải từ link công khai khớp SHA-256.

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
