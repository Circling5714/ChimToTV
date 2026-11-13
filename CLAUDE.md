# Côn Bằng TV — hướng dẫn cho Claude

App Android TV (Kotlin, WebView) xem phim hoạt hình 3D từ các trang web, điều khiển bằng remote / bàn phím.
Trước đây tên là ChimTo TV: chỉ đổi tên hiển thị / logo — mã gói `vn.chimto.tv`, repo `ChimToTV` giữ nguyên. Tên tệp APK đổi
sang `ConBangTV…apk` từ 13/11/2026 (anh bảo), nhưng release TV vẫn kèm bản sao `ChimToTV.apk` (xem dưới).
Đọc `README.md` để biết cấu trúc và cách phát video; `CHANGELOG.md` để biết các bản đã phát hành.

## Cách làm việc với anh

- Trả lời bằng tiếng Việt, xưng "anh".
- **Chỉ đẩy GitHub / phát hành khi anh bảo** (vd "Ok e phát hành 1.4"). Commit trong máy thì được.
- Thử trên tivi thật qua adb trước khi báo xong (thiết bị thử: xem bộ nhớ và file `TIEN-DO-*.md` gần nhất).

## Phát hành một bản

1. Tăng `versionCode` / `versionName` trong `app/build.gradle.kts`.
2. `.\build.ps1` → `dist\ConBangTV-<bản>.apk`, ký bằng `keys\` (không có trong git — mất là app trên mọi TV không nhận
   bản cập nhật OTA nữa).
3. Kiểm APK: `apksigner verify --print-certs` (chứng chỉ SHA-256 `949ef2b5…177b`), `aapt2 dump badging` (đúng bản).
4. `git push`, rồi `.\release.ps1 tv -Notes <tệp ghi chú>` — **không gọi `gh release create` tay**. Script đăng `v<bản>`, luôn
   đánh dấu Latest, kèm **ba tệp giống hệt nhau**: `ConBangTV-<bản>.apk`, `ConBangTV.apk` (link cố định chính
   `…/releases/latest/download/ConBangTV.apk`) và `ChimToTV.apk` (tên cũ — **không bỏ**: mã Downloader 9192365 và màn «Cài
   lên tivi» của các app đã cài trỏ vào `…/latest/download/ChimToTV.apk`), rồi tự kiểm (Latest đúng tag, ba tệp cùng SHA-256
   với tệp trong máy, hai link cố định tải về đúng tệp). Ghi chú phải có mục `## Có gì mới` — app hiện mục này trong hộp thoại cập
   nhật trên TV. `-DryRun` để xem lệnh trước.
5. Kiểm sau phát hành: API `releases/latest` trả đúng tag + có tệp `.apk`; tải từ link công khai, SHA-256 khớp
   (`.\release.ps1 tv -Verify` chạy lại các bước kiểm này bất cứ lúc nào).

## Phát hành bản iPad (`ios/`, cài bằng TrollStore)

1. Tăng `MARKETING_VERSION` trong `ios/project.yml`; gộp nhánh `ios` vào `main` (nhánh `ios` em được tự đẩy để dựng
   bản thử; `main` + phát hành chờ anh bảo).
2. `gh workflow run ios.yml --ref main -f release=true` → artifact `ipa` có `ConBangTV-iPad-<bản>.ipa` (mã gói
   `vn.chimto.tv`, không có cổng gỡ lỗi). Kiểm Info.plist (mã gói, tên, bản), cài thử lên iPad thật.
3. `.\release.ps1 ipad -Notes <tệp ghi chú> -File ConBangTV-iPad-<bản>.ipa` — script **luôn đặt `--latest=false`**: app TV đọc
   `releases/latest` + tệp `.apk`, link Downloader cũng đi theo Latest. Ghi chú cũng phải có `## Có gì mới`. Script tự chép tệp
   vào link cố định (xem dưới).
4. Kiểm: `releases/latest` vẫn là bản Android (script tự kiểm, lệch thì trả về bản TV mới nhất); release iPad có `.ipa`; tải từ
   link công khai và từ link cố định, SHA-256 khớp.

## Phát hành bản Windows (`windows/`)

1. Tăng `<Version>` trong `windows/ConBangTV/ConBangTV.csproj`.
2. `.\windows\build.ps1` → chạy test rồi đóng gói `dist\ConBangTV-Windows-<bản>.exe`. Chạy thử tệp đóng gói (thư mục dữ liệu riêng:
   `CONBANG_DATA_DIR`).
3. `git push`, rồi `.\release.ps1 windows -Notes <tệp ghi chú>` — tag `win-v<bản>`, script **luôn đặt `--latest=false`**. Ghi chú
   cũng phải có `## Có gì mới`. Script tự chép tệp vào link cố định (xem dưới).
4. Kiểm: `releases/latest` vẫn là bản Android; release có `.exe`; tải từ link công khai và từ link cố định, SHA-256 khớp; tự cập
   nhật thật từ bản trước (bản chép riêng).

## Link tải cố định của bản Windows / iPad (từ 13/11/2026) — không đổi tag, không xoá

- `…/releases/download/conbangtv_windows/ConBangTV-Windows.exe` và `…/releases/download/conbangtv_ios/ConBangTV-iPad.ipa` luôn là
  bản mới nhất (anh đặt hai tên tag này; link đã gửi cho người dùng và nằm trên trang giới thiệu). Hai release đó là release
  «cuốn chiếu»: `release.ps1` chép đè tệp mỗi lần phát hành — **đừng đổi tag / tên tệp, đừng xoá, đừng để thành Latest**.
- Link lệch (script `-Verify` báo) thì sửa bằng `.\release.ps1 <windows|ipad> -FixedOnly`.

## Luật lâu dài: phát hành thành công xong thì luôn cập nhật markdown

Sau **mỗi** lần phát hành thành công (đã qua bước 5 ở trên), làm đủ các việc sau rồi mới báo xong:

1. **`CHANGELOG.md`** — thêm mục bản mới lên đầu: ngày, các thay đổi chính (viết cho người dùng), versionCode, tag,
   commit, SHA-256 của APK, lưu ý cài đặt nếu có.
2. **`README.md`** — sửa nếu tính năng, cách dùng, danh sách trang hỗ trợ, phím điều khiển hay quy trình đã đổi.
3. **`TIEN-DO-<YYYY-MM-DD>.md`** (ngày làm việc, để ở gốc dự án, không đưa lên git) — tạo hoặc cập nhật theo kiểu
   TiviKit: tình trạng lúc dừng (nhánh, commit, đã đẩy chưa), bản phát hành mới nhất (versionCode, tag, commit,
   giờ phát hành, SHA-256, đã kiểm những gì), các commit từ bản trước, tivi thử đang chạy bản nào, việc còn dở.
4. **Bộ nhớ** của Claude cho dự án — cập nhật điều gì mới quan trọng cho các phiên sau.
5. Commit tài liệu (`Tài liệu: …`) và đẩy lên GitHub, rồi **báo anh danh sách file đã sửa**.
