#!/bin/bash
# Đóng gói bản Mac (Mac Catalyst): ký ad-hoc (máy Mac chip Apple bắt buộc có chữ ký; không cần tài khoản Apple) rồi nén .zip.
#   ios/build/Build/Products/<Beta|Release>-maccatalyst/ConBangTV.app → ios/dist/ConBangTV-Mac-<bản>[-thu-<build>].zip
# Bản Mac tự cập nhật bằng tệp .zip của release mac-v… (MacUpdate.swift) — tên tệp phải kết thúc bằng .zip.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
found=0
for cfg in Beta Release; do
  app="build/Build/Products/${cfg}-maccatalyst/ConBangTV.app"
  [ -d "$app" ] || continue
  found=1
  plist="$app/Contents/Info.plist"
  version=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$plist")
  build=$(/usr/libexec/PlistBuddy -c "Print CFBundleVersion" "$plist")
  # Không cần quyền nào (app không chạy trong sandbox, không App Group, không widget)
  codesign --force --deep --sign - --timestamp=none "$app"
  codesign --verify --deep --strict "$app"
  name="ConBangTV-Mac-${version}"
  [ "$cfg" = Beta ] && name="${name}-thu-${build}"
  rm -f "dist/${name}.zip"
  # ditto giữ nguyên chữ ký, quyền chạy và liên kết bên trong gói .app (zip thường làm hỏng)
  ditto -c -k --sequesterRsrc --keepParent "$app" "dist/${name}.zip"
  echo "Mac: dist/${name}.zip ($(stat -f%z "dist/${name}.zip") byte, SHA-256 $(shasum -a 256 "dist/${name}.zip" | cut -d' ' -f1)," \
    "kiến trúc: $(lipo -archs "$app/Contents/MacOS/ConBangTV"), mã gói $(/usr/libexec/PlistBuddy -c "Print CFBundleIdentifier" "$plist"))"
done
[ "$found" = 1 ] || { echo "Không thấy bản Mac nào đã dựng"; exit 1; }
