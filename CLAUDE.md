# Côn Bằng TV — hướng dẫn cho Claude

App Android TV (Kotlin, WebView) xem phim hoạt hình 3D từ các trang web, điều khiển bằng remote / bàn phím.
Trước đây tên là ChimTo TV: chỉ đổi tên hiển thị / logo — mã gói `vn.chimto.tv`, repo `ChimToTV`, tên tệp APK giữ nguyên.
Đọc `README.md` để biết cấu trúc và cách phát video; `CHANGELOG.md` để biết các bản đã phát hành.

## Cách làm việc với anh

- Trả lời bằng tiếng Việt, xưng "anh".
- **Chỉ đẩy GitHub / phát hành khi anh bảo** (vd "Ok e phát hành 1.4"). Commit trong máy thì được.
- Thử trên tivi thật qua adb trước khi báo xong (thiết bị thử: xem bộ nhớ và file `TIEN-DO-*.md` gần nhất).

## Phát hành một bản

1. Tăng `versionCode` / `versionName` trong `app/build.gradle.kts`.
2. `.\build.ps1` → `dist\ChimToTV-<bản>.apk`, ký bằng `keys\` (không có trong git — mất là app trên mọi TV không nhận
   bản cập nhật OTA nữa).
3. Kiểm APK: `apksigner verify --print-certs` (chứng chỉ SHA-256 `949ef2b5…177b`), `aapt2 dump badging` (đúng bản).
4. `git push`, rồi `gh release create v<bản> dist\ChimToTV-<bản>.apk --title "Côn Bằng TV <bản>" --notes-file … --latest`.
   Ghi chú phải có mục `## Có gì mới` — app hiện mục này trong hộp thoại cập nhật trên TV.
5. Kiểm sau phát hành: API `releases/latest` trả đúng tag + có tệp `.apk`; tải từ link công khai, SHA-256 khớp.

## Phát hành bản iPad (`ios/`, cài bằng TrollStore)

1. Tăng `MARKETING_VERSION` trong `ios/project.yml`; gộp nhánh `ios` vào `main` (nhánh `ios` em được tự đẩy để dựng
   bản thử; `main` + phát hành chờ anh bảo).
2. `gh workflow run ios.yml --ref main -f release=true` → artifact `ipa` có `ConBangTV-iPad-<bản>.ipa` (mã gói
   `vn.chimto.tv`, không có cổng gỡ lỗi). Kiểm Info.plist (mã gói, tên, bản), cài thử lên iPad thật.
3. `gh release create ios-v<bản> ConBangTV-iPad-<bản>.ipa --title "Côn Bằng TV cho iPad <bản>" --notes-file … --latest=false`
   — **luôn `--latest=false`**: app TV đọc `releases/latest` + tệp `.apk`. Ghi chú cũng phải có `## Có gì mới`.
4. Kiểm: `releases/latest` vẫn là bản Android; release iPad có `.ipa`; tải từ link công khai, SHA-256 khớp.

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
