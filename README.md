# ChimTo TV

![ChimTo TV](design/banner-1280.png)

App Android TV xem phim hoạt hình 3D từ trang web bằng remote hoặc bàn phím.
Dữ liệu (danh sách phim, tập, server) đọc trực tiếp từ trang, nên trang đổi tên miền chỉ cần sửa link trong **Cài đặt**.

**Bản mới nhất:** [tải APK trên GitHub Releases](https://github.com/Circling5714/ChimToTV/releases/latest) ·
[nhật ký phiên bản](CHANGELOG.md). Từ bản 1.3 app tự cập nhật.

## Trang phim hỗ trợ

Có sẵn trong danh sách (menu **Đổi trang phim**):

| Trang | Kiểu giao diện | Cách phát |
| --- | --- | --- |
| yanhh3d.ee | yanhh3d (`flw-item`) | `player.php` → iframe |
| hoathinh3d.ee | HaLim, chia Việt Sub / Thuyết Minh | `player.php` → iframe |
| hhkungfu.ee | HaLim, chia Vietsub / Thuyết Minh | `player.php` → iframe |
| hoathinh3d.pw | HaLimMovies | mở thẳng trang tập (trình phát có khoá xác thực riêng) |

App không viết riêng cho từng trang mà nhận dạng theo mẫu giao diện, nên trang khác cùng kiểu (yanhh3d / theme HaLim)
thường cũng chạy: nhập tên miền trong **Cài đặt** → **Lưu** → **Kiểm tra kết nối**. Trang nào không có `player.php`
thì app tự mở trang tập và phóng trình phát ra toàn màn hình.

## Cài lên tivi

- Tải `ChimToTV-<phiên bản>.apk` ở [Releases](https://github.com/Circling5714/ChimToTV/releases/latest), chép vào USB /
  dùng app "Send Files to TV" rồi mở bằng trình quản lý file trên tivi.
- Qua mạng (tivi/box bật gỡ lỗi ADB): `.\build.ps1 -Install <IP-tivi>` (dựng từ mã nguồn rồi cài).
- Đã có bản 1.3 trở lên thì không cần cài tay nữa: app tự tìm bản mới và hỏi cài.

## Phím điều khiển

| Màn hình | Phím |
| --- | --- |
| Trang chủ / danh sách | Mũi tên: di chuyển • OK/Enter: mở phim • Back: về menu bên trái, Back lần nữa để thoát |
| Chi tiết phim | OK: xem tiếp / xem tập mới nhất • ▼: vào danh sách tập (ô xanh = đã xem) |
| Đang xem phim | OK/Space: dừng–phát • ◀ ▶: tua (giữ để tua nhanh) • ▲: xem thời gian • ▼/Menu: đổi server, đổi tập • Số 0–9: nhảy tới 0–90% • CH+/CH−, N/P: tập sau/trước • Back: thoát |
| Duyệt web | Mũi tên: di chuyển con trỏ (chạm mép thì cuộn) • OK: bấm • CH+/CH−: cuộn nhanh • Menu: tải lại • Back: lùi trang |

## Khi trang thay đổi

- **Đổi tên miền**: Cài đặt → nhập tên miền mới → Lưu (nút "Kiểm tra kết nối" cho biết đọc được bao nhiêu phim). Nếu tên miền cũ tự chuyển hướng, app tự cập nhật.
- **Video không chạy**: trong lúc xem nhấn ▼ → chọn server khác. App nhớ server chọn lần cuối.
- **Trang đổi giao diện** (app không đọc được danh sách): dùng mục **Duyệt web** để xem bằng con trỏ.

## Dựng lại

```powershell
.\build.ps1                      # chỉ dựng APK
.\build.ps1 -Install <IP-tivi>   # dựng + cài + mở trên tivi
```

Cần JDK 17 (`%USERPROFILE%\.jdks\jdk-17.0.20.1+1`) và Android SDK (`%LOCALAPPDATA%\Android\Sdk`).
Thư mục `keys\` chứa khoá ký app — **giữ lại**, mất khoá thì bản mới không cài đè lên bản cũ được.

## Phát hành bản mới (app tự cập nhật)

App (từ bản 1.3) tự hỏi GitHub Releases khi mở / quay lại màn hình chính (tối đa 2 giờ một lần). Có bản mới thì
tải APK ngầm, kiểm tra cùng gói + cùng chữ ký + số hiệu lớn hơn, rồi hỏi **Cài đặt ngay**.
Lần đầu Android hỏi cho phép ChimTo TV "cài ứng dụng không rõ nguồn" (app tự mở đúng trang cài đặt).
Android luôn yêu cầu bấm xác nhận cài; riêng Android 12+ thì từ lần thứ hai trở đi cập nhật hoàn toàn tự động.

1. Tăng `versionCode` / `versionName` trong `app/build.gradle.kts`.
2. `.\build.ps1` → `dist\ChimToTV-<phiên bản>.apk` (ký bằng `keys\` — phải cùng khoá thì app mới nhận).
3. Tạo release tag `v<phiên bản>` và đính kèm APK, ví dụ:
   `gh release create v1.4 dist\ChimToTV-1.4.apk --title "ChimTo TV 1.4" --notes-file ghi-chu.md`
   Mục `## Có gì mới` trong ghi chú được hiện trong hộp thoại cập nhật trên TV.

Thử OTA không cần phát hành thật: bản debug hỏi `http://127.0.0.1:8765/latest.json` (cùng định dạng GitHub API).
Dựng bản giả số hiệu cao hơn bằng `gradlew assembleDebug "-Pvc=5" "-Pvn=1.4"`, phục vụ JSON + APK từ máy tính
rồi `adb reverse tcp:8765 tcp:8765`. Tệp APK lớn nên phục vụ ngay trên TV (`toybox nc`) vì `adb reverse` qua mạng
hay cắt giữa chừng.

## Logo

Logo và banner TV vẽ bằng SVG trong `design/logo.html` (phương án C — ấn triện chữ 仙 trong vòng bút mực vàng).
Sửa xong thì xuất lại toàn bộ icon / icon thích ứng / banner (xhdpi → xxxhdpi) bằng Chrome headless:

```powershell
node design/render.mjs preview xem-truoc.png   # xem cả 3 phương án
node design/render.mjs export c                 # ghi PNG vào app/src/main/res
```

## Cách phát video (cho người sửa code)

Trình phát nhúng của trang chỉ chạy khi nằm trong iframe của chính trang phim. `PlayerActivity` dựng trang bọc
`assets/player.html` (origin = địa chỉ tập phim), đặt trình phát vào iframe; khi WebView tải tài liệu của iframe,
app tự tải hộ và chèn `assets/player-agent.js` vào đầu. Agent nhận lệnh phát / dừng / tua qua `postMessage`
và báo trạng thái lên trang bọc; app đọc trạng thái mỗi giây để vẽ thanh thời gian, nhớ vị trí và tự chuyển tập.

Trang không có `player.php` (chế độ trang tập): WebView mở thẳng trang tập, `assets/page-focus.js` phóng iframe /
video lớn nhất ra toàn màn hình, agent chạy cả ở khung trên cùng lẫn trong iframe.

Bộ đọc trang nằm ở `Site.kt`. Kiểm tra nó trên HTML thật đã tải về (không cần TV):

```powershell
$env:CHIMTO_FIXTURES = "<thư mục chứa fixtures.txt và các tệp .html>"
.\gradlew.bat :app:testReleaseUnitTest --tests vn.chimto.tv.SiteProbeTest --rerun
```
