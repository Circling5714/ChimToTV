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
  ldid -S "$app/ConBangTV"
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
