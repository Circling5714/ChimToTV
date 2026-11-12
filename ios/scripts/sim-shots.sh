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

udid=$(xcrun simctl list devices available -j | python3 -c "
import json,sys
want=sys.argv[1]
devs=[d for rt,l in json.load(sys.stdin)['devices'].items() if 'iOS' in rt for d in l]
pick=[d for d in devs if d['name']==want] or [d for d in devs if d['name'].startswith('iPhone')]
print(pick[0]['udid'])" "$DEVICE")
echo "Máy ảo: $DEVICE ($udid)"
xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b
xcrun simctl install "$udid" "$APP"
xcrun simctl launch "$udid" vn.chimto.tv.debug

for i in $(seq 1 60); do curl -sf -m 3 "$D/ping" > /dev/null && break; sleep 1; done
curl -s "$D/ping"; echo
# App không trả lời cổng gỡ lỗi (văng lúc mở?): in nhật ký của app + báo cáo văng để biết vì sao, rồi dừng — chụp tiếp chỉ ra
# màn hình chính của máy ảo
if ! curl -sf -m 3 "$D/ping" > /dev/null; then
  echo "::error::App không trả lời cổng gỡ lỗi trên máy ảo"
  xcrun simctl spawn "$udid" log show --last 3m --style compact --predicate 'process == "ConBangTV"' 2>/dev/null | tail -80
  for f in "$HOME"/Library/Logs/DiagnosticReports/ConBangTV*; do
    [ -f "$f" ] && { echo "--- $f"; head -120 "$f"; }
  done
  exit 1
fi

shot() { sleep "${2:-4}"; xcrun simctl io "$udid" screenshot "$OUT/$1.png" > /dev/null 2>&1 && echo "chụp $1"; }
orient() { curl -s -X POST "$D/orient?to=$1" > /dev/null; sleep 2; }
q() { python3 -c "import urllib.parse,sys; print(urllib.parse.quote(sys.argv[1], safe=''))" "$1"; }

# Phim đầu tiên trên trang chủ + tập đầu
FILM=$(curl -s -m 60 "$D/probe?fn=home" | python3 -c "import json,sys; o=json.load(sys.stdin); print(o['sections'][0]['films'][0]['url'])" 2>/dev/null)
EP=$(curl -s -m 60 "$D/probe?fn=detail&url=$(q "$FILM")" | python3 -c "import json,sys; o=json.load(sys.stdin); print(o['groups'][0]['first'][0][1])" 2>/dev/null)
echo "phim: $FILM  tập: $EP"

for o in portrait landscape; do
  orient "$o"
  curl -s -X POST "$D/go?to=home" > /dev/null; shot "$o-1-trang-chu" 8
  curl -s -X POST "$D/go?to=settings" > /dev/null; shot "$o-2-cai-dat"
  curl -s -X POST "$D/go?to=back" > /dev/null; sleep 2
  # Cài đặt → «Cài lên tivi / mời bạn bè» (mã Downloader)
  curl -s -X POST "$D/go?to=invite" > /dev/null; shot "$o-5-cai-len-tivi" 6
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
