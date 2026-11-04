# Côn Bằng TV

![Côn Bằng TV](design/banner-1280.png)

App Android TV xem phim hoạt hình 3D (và phim bộ / phim lẻ ở phimmoir.fun) từ trang web bằng remote hoặc bàn phím
(trước đây tên là ChimTo TV — vẫn cùng một app, cài đè và tự cập nhật bình thường).
Dữ liệu (danh sách phim, tập, server) đọc trực tiếp từ trang, nên trang đổi tên miền chỉ cần sửa link trong **Cài đặt**.

**Bản mới nhất:** [tải APK trên GitHub Releases](https://github.com/Circling5714/ChimToTV/releases/latest) ·
[nhật ký phiên bản](CHANGELOG.md). Từ bản 1.3 app tự cập nhật.

**Liên hệ** (góp ý, báo lỗi, hỏi đáp): nhóm Telegram [t.me/tivikit](https://t.me/tivikit) — trong app: **Cài đặt** →
**Liên hệ**.

## Trang phim hỗ trợ

Có sẵn trong danh sách (menu **Đổi trang phim**). Mở menu này là app đo ngay từng trang: tải trang chủ mất bao lâu,
đọc được bao nhiêu phim, trang nhanh nhất có dấu ★, kèm điểm video các lần xem trước (vd "video mượt").

| Trang | Kiểu giao diện | Cách phát |
| --- | --- | --- |
| yanhh3d.ee | yanhh3d (`flw-item`) | `player.php` → iframe |
| hoathinh3d.ee | HaLim, chia Việt Sub / Thuyết Minh | `player.php` → iframe |
| hhkungfu.ee | HaLim, chia Vietsub / Thuyết Minh | `player.php` → iframe |
| hoathinh3d.pw | HaLimMovies | mở thẳng trang tập (trình phát có khoá xác thực riêng) |
| phimmoir.fun | Next.js kiểu PhimMoi, chia Vietsub / Thuyết Minh / Lồng Tiếng / Song Ngữ | link `.m3u8` → app tự phát (KK Phim, HD); NguonC → iframe |

App không viết riêng cho từng trang mà nhận dạng theo mẫu giao diện, nên trang khác cùng kiểu (yanhh3d / theme HaLim /
Next.js kiểu PhimMoi) thường cũng chạy: nhập tên miền trong **Cài đặt** → **Lưu** → **Kiểm tra kết nối**. Trang nào
không có `player.php` thì app tự mở trang tập và phóng trình phát ra toàn màn hình.

phimmoir.fun là trang tổng hợp: ngoài hoạt hình 3D (khối "Hoạt hình Trung Quốc mới cập nhật" đầu trang chủ) còn phim
bộ / phim lẻ Hàn, Trung, Âu Mỹ, Nhật, Thái. Menu có Hoạt hình, Phim bộ mới, Phim lẻ mới, 5 mục theo nước và 10 thể
loại trên menu của trang; thể loại / nước khác thì dùng **Tìm kiếm**. Server: **KK Phim** (nhanh nhất), **NguonC**
(app tự bỏ qua quảng cáo trước phim), **HD** (bản Song Ngữ, có phụ đề tiếng Việt), OPhim (đang hỏng).

## Cài lên tivi

- Tải `ChimToTV-<phiên bản>.apk` ở [Releases](https://github.com/Circling5714/ChimToTV/releases/latest), chép vào USB /
  dùng app "Send Files to TV" rồi mở bằng trình quản lý file trên tivi.
- Qua mạng (tivi/box bật gỡ lỗi ADB): `.\build.ps1 -Install <IP-tivi>` (dựng từ mã nguồn rồi cài).
- Đã có bản 1.3 trở lên thì không cần cài tay nữa: app tự tìm bản mới và hỏi cài.

## Phím điều khiển

| Màn hình | Phím |
| --- | --- |
| Trang chủ / danh sách | Mũi tên: di chuyển • OK/Enter: mở phim (hàng **Xem tiếp**: phát tiếp luôn) • Giữ OK / Menu trên phim ở Xem tiếp, Lịch sử xem: xoá khỏi lịch sử • Back: về menu bên trái, Back lần nữa để thoát |
| Chi tiết phim | OK: xem tiếp / xem tập mới nhất • ▼: vào danh sách tập (ô xanh = đã xem) • Số 0–9 hoặc nút **Đến tập…**: nhảy tới tập theo số |
| Đang xem phim | OK/Space: dừng–phát • ◀ ▶: tua (giữ để tua nhanh) • ▲: xem thời gian • ▼/Menu: đổi server (kèm điểm từng server), đổi tập • Số 0–9: nhảy tới 0–90% • CH+/CH−, N/P: tập sau/trước • Back: thoát |
| Gợi ý đổi server (hiện khi video giật nhiều) | OK: chuyển sang server được gợi ý, xem tiếp đúng chỗ • Back: bỏ qua |
| Duyệt web | Mũi tên: di chuyển con trỏ (chạm mép thì cuộn) • OK: bấm • CH+/CH−: cuộn nhanh • Menu: tải lại • Back: lùi trang |

## Khi trang thay đổi

- **Đổi tên miền**: Cài đặt → nhập tên miền mới → Lưu (nút "Kiểm tra kết nối" cho biết đọc được bao nhiêu phim). Nếu tên miền cũ tự chuyển hướng, app tự cập nhật.
- **Video không chạy / hay giật**: trong lúc xem nhấn ▼ → chọn server khác. App nhớ server chọn lần cuối và chấm điểm
  từng server theo những lần xem trên chính TV đó (mở mất bao lâu, giật nhiều hay ít). Server đang giật nhiều thì app
  gợi ý server khác cùng ngôn ngữ, nhấn OK để chuyển. Báo "TV rớt …% khung hình" nghĩa là TV xử lý không kịp, đổi
  server không đỡ.
- **Trang chậm / bị chặn**: mở **Đổi trang phim** để xem trang nào nhanh. "✗ không vào được" = không kết nối được;
  "không đọc được phim" = vào được nhưng không có danh sách phim (có thể là trang chặn của nhà mạng).
- **Trang đổi giao diện** (app không đọc được danh sách): dùng mục **Duyệt web** để xem bằng con trỏ.

## Proxy SOCKS5

**Cài đặt → Proxy SOCKS5** (từ 1.8.0, mặc định tắt):

- Nhập máy chủ, cổng, tên đăng nhập / mật khẩu (bỏ trống nếu proxy không cần), hoặc dán nguyên
  `socks5://user:pass@máy-chủ:cổng`. **Kiểm tra proxy** thử ngay (chưa cần lưu): báo IP ra ngoài, nơi đặt máy, thời
  gian. **Proxy: Bật / Tắt** — tắt thì app đi mạng mặc định của TV.
- **DNS: của proxy** (proxy tự tìm địa chỉ trang — vượt được chặn tên miền của nhà mạng) / **của thiết bị** (TV tự tìm
  rồi chỉ gửi IP cho proxy).
- **Dùng cho: toàn bộ app** (trang, ảnh, video, kiểm tra cập nhật) / **chỉ trang phim** (chỉ các trang trong danh sách;
  video, ảnh từ CDN, GitHub đi thẳng mạng nhà — đỡ tốn proxy).
- Proxy lỗi (chết, sai mật khẩu) thì app báo rõ, **không tự đi thẳng**; màn hình chính ghi "· proxy" khi đang bật.
  Máy chủ video có thể chặn IP của proxy (đã gặp với CDN của KK Phim trên phimmoir.fun): video lỗi khi bật proxy thì
  chọn "chỉ trang phim".
- Mật khẩu proxy lưu trong bộ nhớ riêng của app (app khác không đọc được), không mã hoá.

## Riêng tư và dữ liệu duyệt web

- **Xoá dữ liệu web** (menu chính, ngay trên Cài đặt — từ 1.8.1): xoá cookie, dữ liệu các trang đã lưu (localStorage,
  IndexedDB…), bộ nhớ đệm và mật khẩu HTTP đã nhớ của trình duyệt trong app. Lịch sử xem, chỗ đang xem dở, điểm
  server, cài đặt và proxy giữ nguyên. **Cài đặt → Dữ liệu → "Xoá ảnh & dữ liệu duyệt web"** làm việc tương tự và xoá
  thêm ảnh bìa đã lưu.
- **Cài đặt → Dữ liệu** (từ 1.8.2): **Xem riêng** — không ghi phim đã xem và chỗ đang xem (chỗ xem chỉ giữ trong bộ nhớ
  tới khi tắt app; màn hình chính ghi "· xem riêng"); **Tự xoá dữ liệu web khi rời app** — xoá như mục trên mỗi khi rời
  app (Home, Back thoát), trừ lúc trình phát còn mở giữa chừng; app bị tắt trước khi kịp xoá thì lần mở sau tự xoá.
- **Xoá một phim khỏi lịch sử:** giữ OK (hoặc nhấn Menu) trên phim ở hàng Xem tiếp / mục Lịch sử xem — xoá cả chỗ
  đang xem các tập của phim đó.
- Từ 1.8.2 app không cho sao lưu dữ liệu (lịch sử xem, mật khẩu proxy, cookie) lên đám mây, không chuyển sang máy mới
  và không lấy ra được bằng `adb backup`; WebView không gửi số liệu sử dụng / chẩn đoán của app về Google.
- Trang phim thấy app như một trình duyệt Chromium trên Android: app gửi chung một User-Agent rút gọn
  (`Android 10; K … Chrome/<bản chính>.0.0.0`), không có tên mẫu TV hay số bản build Android; client hints ghi
  "Chromium" thay vì "Android WebView". Khi app tự tải trang hộ trình phát thì không gửi tên gói app. Riêng các yêu cầu
  do WebView tự gửi, WebView 138 trên TV thử vẫn kèm `X-Requested-With: vn.chimto.tv` (app đã tắt tính năng này nhưng
  bản WebView đó chưa làm theo).

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
Lần đầu Android hỏi cho phép Côn Bằng TV "cài ứng dụng không rõ nguồn" (app tự mở đúng trang cài đặt).
Android luôn yêu cầu bấm xác nhận cài; riêng Android 12+ thì từ lần thứ hai trở đi cập nhật hoàn toàn tự động.

1. Tăng `versionCode` / `versionName` trong `app/build.gradle.kts`.
2. `.\build.ps1` → `dist\ChimToTV-<phiên bản>.apk` (ký bằng `keys\` — phải cùng khoá thì app mới nhận).
3. Tạo release tag `v<phiên bản>` và đính kèm APK, ví dụ:
   `gh release create v1.8.0 dist\ChimToTV-1.8.0.apk --title "Côn Bằng TV 1.8.0" --notes-file ghi-chu.md`
   Mục `## Có gì mới` trong ghi chú được hiện trong hộp thoại cập nhật trên TV.

Thử OTA không cần phát hành thật: bản debug hỏi `http://127.0.0.1:8765/latest.json` (cùng định dạng GitHub API).
Dựng bản giả số hiệu cao hơn bằng `gradlew assembleDebug "-Pvc=5" "-Pvn=1.4"`, phục vụ JSON + APK từ máy tính
rồi `adb reverse tcp:8765 tcp:8765`. Tệp APK lớn nên phục vụ ngay trên TV (`toybox nc`) vì `adb reverse` qua mạng
hay cắt giữa chừng.

## Logo

Logo và banner TV vẽ bằng SVG trong `design/logo.html` (phương án C — ấn triện chữ 鵬 của chim Côn Bằng trong vòng bút
mực vàng; font Spectral SC cho tên có dấu tiếng Việt, Yuji Boku cho chữ bút lông 鵬).
Sửa xong thì xuất lại toàn bộ icon / icon thích ứng / banner (xhdpi → xxxhdpi) bằng Chrome headless:

```powershell
node design/render.mjs preview xem-truoc.png   # xem cả 3 phương án
node design/render.mjs export c                 # ghi PNG vào app/src/main/res
```

Đổi tên app chỉ là đổi **tên hiển thị** (`app_name`), logo và banner. Mã gói `vn.chimto.tv`, khoá ký và repo
`Circling5714/ChimToTV` phải giữ nguyên — đổi một trong ba thì các TV đã cài không nhận bản cập nhật nữa.

## Cách phát video (cho người sửa code)

Trình phát nhúng của trang chỉ chạy khi nằm trong iframe của chính trang phim. `PlayerActivity` dựng trang bọc
`assets/player.html` (origin = địa chỉ tập phim), đặt trình phát vào iframe; khi WebView tải tài liệu của iframe,
app tự tải hộ và chèn `assets/player-agent.js` vào đầu. Agent nhận lệnh phát / dừng / tua qua `postMessage`
và báo trạng thái lên trang bọc; app đọc trạng thái mỗi giây để vẽ thanh thời gian, nhớ vị trí và tự chuyển tập.

Trang không có `player.php` (chế độ trang tập): WebView mở thẳng trang tập, `assets/page-focus.js` phóng iframe /
video lớn nhất ra toàn màn hình, agent chạy cả ở khung trên cùng lẫn trong iframe.

Bộ đệm (từ 1.6): yanhh3d.ee, hoathinh3d.ee, hhkungfu.ee nhúng chung trình phát streamfree.vip (JW Player 8.24 + hls.js,
một chất lượng, đoạn 10–19 s ngụy trang ảnh PNG trên CDN). JW để preload "metadata" nên ép hls.js
`maxMaxBufferLength = 1` (chỉ tải trước một đoạn). `player-agent.js` bọc `jwplayer(...).setup`, chạy `onHlsInit` của
trang rồi đặt `maxBufferLength` 120 s, bỏ qua lệnh ép < 10 s; agent báo `ahead` (giây đã tải trước) để đo.

Trang Next.js (từ 1.7, phimmoir.fun — `NextSite.kt`): không đọc HTML mà đọc dữ liệu trong các đoạn
`self.__next_f.push([1,"…"])` của trang (phim, đủ danh sách tập theo từng ngôn ngữ, server, link video). Server cho link
`.m3u8` (KK Phim, server HD của trang) → `PlayerActivity` dựng `assets/hls.html` (origin = địa chỉ tập) với hls.js
1.5.20 light đóng kèm (`assets/hls.light.min.js`, trả ở đường dẫn `/__chimto/hls.js`), agent nằm ngay trong trang,
bộ đệm 120 s. hls.js bỏ khối quảng cáo ~30 s chèn giữa phim (khối DISCONTINUITY mà mọi đoạn đều dạng
`/vN/<32 hex>/segment_NNNN.ts`; khối `convertvN/` là phim có chữ quảng cáo đè lên — giữ). Phụ đề rời của server HD
(WebVTT ngụy trang ảnh PNG) được bóc phần ảnh rồi gắn `<track>`. NguonC chỉ có trình phát nhúng (có Cloudflare
Turnstile — cố ý không gỡ để lấy link) → chạy qua iframe như các trang khác; agent tự bỏ qua quảng cáo JW
(`.jw-flag-ads`, nút bỏ qua) và chọn "Xem lại từ đầu" ở hộp thoại xem tiếp của trình phát (app tự tua tới chỗ cũ).
Test: `.\gradlew.bat :app:testDebugUnitTest --tests vn.chimto.tv.NextSiteTest`.

Proxy (từ 1.8.0 — `AppProxy.kt`, `Socks5.kt`): WebView (Chromium) không đăng nhập được SOCKS5 có mật khẩu và luôn
để proxy phân giải tên miền, nên app mở cổng SOCKS5 nội bộ `Socks5Bridge` (chỉ nghe 127.0.0.1, không mật khẩu). WebView
(androidx.webkit `ProxyController`, bản 1.14 — bản cuối còn chạy Android 5) và `Http.kt` đều đi qua cổng này; cổng nội
bộ đăng nhập proxy thật (RFC 1929), gửi tên miền (DNS của proxy) hoặc tự phân giải rồi gửi IP (DNS của thiết bị), ở chế
độ "chỉ trang phim" cho đích khác đi thẳng. Đổi cài đặt → mở cổng mới, đóng kết nối cũ (kết nối keep-alive không giữ
cài đặt cũ). Điểm server khi video đi qua proxy chấm riêng (`<trang>·proxy`). Test:
`.\gradlew.bat :app:testDebugUnitTest --tests vn.chimto.tv.Socks5BridgeTest` (proxy SOCKS5 giả trong test).

Riêng tư (từ 1.8.1 — `Privacy.kt`, `BrowsingData.kt`): User-Agent rút gọn lấy số bản chính Chrome từ UA của WebView
(lưu ở `Prefs.userAgent` để `Http.kt` dùng ngay khi mở app), áp cho mọi WebView kèm `UserAgentMetadata` (brand
"Chromium", model rỗng) và danh sách origin trống cho `X-Requested-With`; `PlayerActivity.fetchWithAgent` bỏ header đó
khi giá trị là tên gói. `BrowsingData.clear` gom `WebStorage.deleteAllData`, xoá cache qua một WebView tạm,
`WebViewDatabase.clearHttpAuthUsernamePassword` và `CookieManager.removeAllCookies` (`Http.kt` không có kho cookie
riêng). Test: `.\gradlew.bat :app:testDebugUnitTest --tests vn.chimto.tv.PrivacyTest`.

Từ 1.8.2: `AutoClear.kt` đếm Activity đang hiện (ActivityLifecycleCallbacks) — về 0 là rời app → xoá nếu không còn
`PlayerActivity` / `WebActivity` sống và `Prefs.webDirty` (WebView đã chạy từ lần xoá trước); tiến trình mới thì thử
xoá sau 1,5 s. Chỗ đang xem lưu kèm phim của tập (`pos_film`) để xoá một phim khỏi lịch sử xoá được mọi tập của nó
(vị trí lưu trước 1.8.2 chỉ xoá được tập cuối). Sao lưu: `allowBackup="false"` + `res/xml/data_extraction_rules.xml`
(Android 12+); `android.webkit.WebView.MetricsOptOut` trong manifest.

Chấm điểm server (`PlayStats.kt`): agent báo thêm số khung hình rớt; `PlaySession` đếm từ trạng thái mỗi giây (đang
phát mà trình phát chờ tải = giật; chờ tải ngay sau lệnh tua thì không tính), lưu 8 lần xem gần nhất theo trang + server
trong `Prefs.playStats`. Cách chọn / gợi ý server (cùng ngôn ngữ, không nét hơn khi chưa thử; server quen mà "hay
lỗi" thì lúc mở tập sang server cùng ngôn ngữ còn phát được) có test riêng:
`.\gradlew.bat :app:testDebugUnitTest --tests vn.chimto.tv.PlayStatsTest`.

Bộ đọc trang nằm ở `Site.kt`. Kiểm tra nó trên HTML thật đã tải về (không cần TV):

```powershell
$env:CHIMTO_FIXTURES = "<thư mục chứa fixtures.txt và các tệp .html>"
.\gradlew.bat :app:testReleaseUnitTest --tests vn.chimto.tv.SiteProbeTest --rerun
```
