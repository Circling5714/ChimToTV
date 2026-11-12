#!/bin/bash
# Chụp màn hình bản thử trên máy ảo iPhone (GitHub Actions, macOS): dọc / ngang, trang chủ, trang phim, Cài đặt, trình
# phát, cỡ chữ lớn. Điều khiển app qua cổng gỡ lỗi (localhost:8790 — máy ảo dùng chung mạng với máy chạy).
#   bash ios/scripts/sim-shots.sh "<tên máy ảo>"  → ios/shots/*.png
set -uo pipefail
cd "$(dirname "$0")/.."
DEVICE="${1:-iPhone SE (3rd generation)}"
APP="build/Build/Products/Beta-iphonesimulator/ConBangTV.app"
OUT=shots
mkdir -p "$OUT"
D="http://127.0.0.1:8790"

# Chọn máy ảo đúng đời iOS của SDK đã dựng app (máy chạy của GitHub cài nhiều đời: 11/2026 có 18.5, 18.6, 26.x).
# Bản cho máy ảo phải dựng với mức iOS tối thiểu = đời SDK (ios.yml đặt IPHONEOS_DEPLOYMENT_TARGET): dựng với mức 15.0 thì
# tệp chạy đòi /usr/lib/swift/libswiftWebKit.dylib, mà bộ máy ảo iOS 18.5 không có tệp này → app văng ngay lúc mở (dyld
# «Library missing», cổng gỡ lỗi không trả lời). Chỉ máy ảo bị: máy thật có thư viện đó — IPA mức 15.0 chạy trên iOS 15.8.
SDK=$(/usr/libexec/PlistBuddy -c 'Print :DTPlatformVersion' "$APP/Info.plist" 2>/dev/null || true)
read -r udid runtime < <(xcrun simctl list devices available -j | python3 -c "
import json,re,sys
want,sdk=sys.argv[1],sys.argv[2]
def ver(rt):
    m=re.search(r'iOS-(\d+)-(\d+)',rt)
    return (int(m.group(1)),int(m.group(2))) if m else None
rts=sorted(((ver(rt),l) for rt,l in json.load(sys.stdin)['devices'].items() if ver(rt) and l),key=lambda r:r[0])
print('đời máy ảo có sẵn:',', '.join('%d.%d'%v for v,_ in rts),'| SDK dựng app:',sdk or '?',file=sys.stderr)
sdkv=tuple(int(x) for x in sdk.split('.')[:2]) if sdk else None
# Ưu tiên: đúng đời SDK → đời mới nhất không vượt SDK → (không còn cách nào) đời cũ nhất
ok=[r for r in rts if sdkv and r[0]<=sdkv][::-1] or rts
for v,l in ok:
    pick=[d for d in l if d['name']==want] or [d for d in l if d['name'].startswith('iPhone')]
    if pick:
        print(pick[0]['udid'],'%d.%d'%v); break
" "$DEVICE" "$SDK")
if [ -z "${udid:-}" ]; then echo "::error::Không tìm thấy máy ảo iPhone nào"; exit 1; fi
echo "Máy ảo: $DEVICE ($udid), iOS $runtime"
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b
xcrun simctl install "$udid" "$APP"
xcrun simctl launch "$udid" vn.chimto.tv.debug

alive() { curl -sf -m 5 "$D/ping" > /dev/null; }
# App không còn trả lời cổng gỡ lỗi (văng, hoặc bị đưa ra nền): in nhật ký của app + tóm tắt báo cáo văng để biết vì sao, rồi
# dừng với lỗi. Chụp tiếp chỉ ra màn hình chính của máy ảo mà job vẫn báo thành công (đã gặp 13/11/2026).
dead() {
  echo "::error::App không trả lời cổng gỡ lỗi trên máy ảo ($1)"
  xcrun simctl spawn "$udid" log show --last 4m --style compact --predicate 'process == "ConBangTV"' 2>/dev/null | tail -60
  python3 scripts/crash-brief.py "$HOME"/Library/Logs/DiagnosticReports/ConBangTV* 2>&1
  exit 1
}

for i in $(seq 1 60); do alive && break; sleep 1; done
curl -s "$D/ping"; echo
alive || dead "lúc mở"

shot() { sleep "${2:-4}"; alive || dead "trước ảnh $1"; xcrun simctl io "$udid" screenshot "$OUT/$1.png" > /dev/null 2>&1 && echo "chụp $1"; }
orient() { curl -s -X POST "$D/orient?to=$1" > /dev/null; sleep 2; }
q() { python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1], safe=''))" "$1"; }

# Máy ảo mới cài: lần đầu mở app hiện màn «Miễn trừ trách nhiệm» (từ 1.14), chưa bấm «Đồng ý» thì app chưa bắt đầu — menu
# trống, /go?to=home không có gì để mở. Chụp màn đó, bấm «Đồng ý», rồi chờ tới khi về được trang chủ.
shot "0-mien-tru" 3
curl -s -X POST "$D/press?label=$(q "Đồng ý")"; echo
for i in $(seq 1 40); do
  curl -s -X POST "$D/go?to=home" | grep -q '"ok" *: *true' && break
  sleep 1
done
curl -s -X POST "$D/go?to=home"; echo

# Phim đầu tiên trên trang chủ + tập đầu
FILM=$(curl -s -m 60 "$D/probe?fn=home" | python3 -c "import json,sys; o=json.load(sys.stdin); print(o['sections'][0]['films'][0]['url'])" 2>/dev/null)
EP=$(curl -s -m 60 "$D/probe?fn=detail&url=$(q "$FILM")" | python3 -c "import json,sys; o=json.load(sys.stdin); print(o['groups'][0]['first'][0][1])" 2>/dev/null)
echo "phim: $FILM  tập: $EP"

for o in portrait landscape; do
  orient "$o"
  curl -s -X POST "$D/go?to=home" > /dev/null; shot "$o-1-trang-chu" 8
  curl -s -X POST "$D/go?to=settings" > /dev/null; shot "$o-2-cai-dat"
  # Cài đặt → «Cài lên tivi / mời bạn bè» (mã Downloader): cuộn tới cuối rồi bấm đúng dòng đó như người dùng. Lối tắt
  # /go?to=invite không dùng được trên iPhone — dòng nằm dưới mép màn hình chưa được dựng nên không mở được trang con.
  curl -s -X POST "$D/scroll?dir=bottom" > /dev/null; sleep 2
  curl -s -X POST "$D/press?partial=1&label=$(q "Cài lên tivi")"; echo
  shot "$o-5-cai-len-tivi" 4
  curl -s -X POST "$D/go?to=back" > /dev/null; sleep 2
  if [ -n "$FILM" ]; then
    curl -s -X POST "$D/go?to=film&url=$(q "$FILM")" > /dev/null; shot "$o-3-trang-phim" 8
    curl -s -X POST "$D/go?to=back" > /dev/null; sleep 2
  fi
done

# Cỡ chữ lớn (dọc)
orient portrait
curl -s -X POST "$D/textscale?v=1.3" > /dev/null
curl -s -X POST "$D/go?to=home" > /dev/null; shot "portrait-4-chu-rat-lon" 6
curl -s -X POST "$D/textscale?v=1" > /dev/null

# Trình phát (iPhone: tự xoay ngang dù máy đang dọc)
if [ -n "$EP" ]; then
  curl -s -X POST "$D/go?to=play&film=$(q "$FILM")&ep=$(q "$EP")" > /dev/null; shot "player-1-mo" 12
  curl -s -X POST "$D/player?do=bar" > /dev/null; shot "player-2-thanh-nut" 1
  curl -s -X POST "$D/player?do=menu" > /dev/null; shot "player-3-menu" 2
  curl -s -X POST "$D/player?do=close" > /dev/null; shot "player-4-dong-xong" 4
fi
curl -s "$D/state" > "$OUT/state.json"
ls -la "$OUT"
