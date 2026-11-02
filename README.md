# ChimTo TV

![ChimTo TV](design/banner-1280.png)

App Android TV xem phim từ trang web (mặc định `https://yanhh3d.ee`) bằng remote hoặc bàn phím.
Dữ liệu (danh sách phim, tập, server) đọc trực tiếp từ trang, nên trang đổi tên miền chỉ cần sửa link trong **Cài đặt**.

## Cài lên tivi

- File cài: `dist\ChimToTV-<phiên bản>.apk`
- Qua mạng (tivi/box bật gỡ lỗi ADB): `.\build.ps1 -Install 10.10.17.104`
- Hoặc chép APK vào USB / dùng app "Send Files to TV" rồi mở bằng trình quản lý file trên tivi.

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
.\build.ps1 -Install 10.10.17.104 # dựng + cài + mở trên tivi
```

Cần JDK 17 (`%USERPROFILE%\.jdks\jdk-17.0.20.1+1`) và Android SDK (`%LOCALAPPDATA%\Android\Sdk`).
Thư mục `keys\` chứa khoá ký app — **giữ lại**, mất khoá thì bản mới không cài đè lên bản cũ được.

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
