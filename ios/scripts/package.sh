#!/bin/bash
# Đóng gói .app đã dựng thành .ipa cho TrollStore: ký giả bằng ldid (không cần chứng chỉ Apple) rồi nén Payload/.
#   ios/build/Build/Products/<Beta|Release>-iphoneos/ConBangTV.app → ios/dist/ConBangTV-iPad-<bản>[-thu].ipa
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
found=0
for cfg in Beta Release; do
  app="build/Build/Products/${cfg}-iphoneos/ConBangTV.app"
  [ -d "$app" ] || continue
  found=1
  version=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$app/Info.plist")
  build=$(/usr/libexec/PlistBuddy -c "Print CFBundleVersion" "$app/Info.plist")
  # Quyền App Group (app ghi dữ liệu cho widget «Xem tiếp») — ký widget trước, rồi tới app
  group=$(/usr/libexec/PlistBuddy -c "Print ConBangAppGroup" "$app/Info.plist")
  ent=$(mktemp)
  cat > "$ent" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>com.apple.security.application-groups</key><array><string>${group}</string></array>
</dict></plist>
EOF
  for ext in "$app"/PlugIns/*.appex; do
    [ -d "$ext" ] || continue
    ldid -S"$ent" "$ext/$(/usr/libexec/PlistBuddy -c "Print CFBundleExecutable" "$ext/Info.plist")"
  done
  ldid -S"$ent" "$app/ConBangTV"
  rm -f "$ent"
  # Tên tệp giữ "iPad" (bản 1.2 trở về trước tìm tệp .ipa bất kỳ trong release ios-v*); dùng cho cả iPhone
  name="ConBangTV-iPad-${version}"
  [ "$cfg" = Beta ] && name="${name}-thu-${build}"
  tmp=$(mktemp -d)
  mkdir "$tmp/Payload"
  cp -R "$app" "$tmp/Payload/"
  (cd "$tmp" && zip -qry "$OLDPWD/dist/${name}.ipa" Payload)
  rm -rf "$tmp"
  echo "IPA: dist/${name}.ipa ($(stat -f%z "dist/${name}.ipa") byte, SHA-256 $(shasum -a 256 "dist/${name}.ipa" | cut -d' ' -f1))"
done
[ "$found" = 1 ] || { echo "Không thấy .app nào đã dựng"; exit 1; }
