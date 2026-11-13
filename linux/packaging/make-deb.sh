#!/bin/bash
# Đóng gói Côn Bằng TV cho Linux thành tệp .deb (Ubuntu 22.04+ / Linux Mint 21+ / Debian 12+, máy 64-bit x86).
#   bash packaging/make-deb.sh <tệp chạy conbangtv> <thư mục ra> [thu]
# «thu» = bản thử: tên tệp thêm -thu-<ngày giờ>. In dòng DEB=<đường dẫn tệp .deb> khi xong.
# Thư viện (GTK3, WebKitGTK 4.1, GStreamer) lấy của hệ thống — tệp .deb chỉ vài MB; apt tự cài các gói còn thiếu.
set -euo pipefail
BIN=${1:?tệp chạy}
OUT=${2:?thư mục ra}
KIND=${3:-}
HERE=$(cd "$(dirname "$0")" && pwd)
VER=$(grep -m1 '^version' "$HERE/../Cargo.toml" | sed -E 's/.*"(.*)".*/\1/')
ARCH=$(dpkg --print-architecture)
NAME="ConBangTV-Linux-$VER"
[ "$KIND" = "thu" ] && NAME="$NAME-thu-$(date +%m%d.%H%M)"
ROOT=$(mktemp -d)
trap 'rm -rf "$ROOT"' EXIT

install -Dm755 "$BIN" "$ROOT/usr/bin/conbangtv"
strip "$ROOT/usr/bin/conbangtv" 2>/dev/null || true
install -Dm644 "$HERE/vn.chimto.tv.desktop" "$ROOT/usr/share/applications/vn.chimto.tv.desktop"
for s in 32 48 64 128 256 512; do
  install -Dm644 "$HERE/icons/$s.png" "$ROOT/usr/share/icons/hicolor/${s}x${s}/apps/vn.chimto.tv.png"
done
install -Dm644 "$HERE/vn.chimto.tv.metainfo.xml" "$ROOT/usr/share/metainfo/vn.chimto.tv.metainfo.xml"

SIZE=$(du -sk "$ROOT/usr" | cut -f1)
mkdir -p "$ROOT/DEBIAN"
cat > "$ROOT/DEBIAN/control" <<EOF
Package: conbangtv
Version: $VER
Architecture: $ARCH
Maintainer: Côn Bằng TV <https://t.me/tivikit>
Installed-Size: $SIZE
Depends: libc6 (>= 2.35), libgtk-3-0t64 | libgtk-3-0 (>= 3.24), libwebkit2gtk-4.1-0 (>= 2.36), libgstreamer1.0-0, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-libav, ca-certificates
Recommends: gstreamer1.0-plugins-bad, gstreamer1.0-vaapi | gstreamer1.0-va
Section: video
Priority: optional
Homepage: https://conbangtv.github.io
Description: Côn Bằng TV - xem phim bằng bàn phím / remote / chuột
 Xem phim hoạt hình 3D (và phim bộ / phim lẻ) từ các trang phim công khai:
 trang chủ tổng hợp mọi nguồn, tìm kiếm, xem tiếp, theo dõi tập mới, người xem
 (mã PIN, giới hạn giờ xem), đồng bộ trong nhà với TV / iPad / Windows, DNS mã
 hoá, vượt chặn nhà mạng, proxy SOCKS5 / Shadowsocks. App không lưu trữ phim.
EOF
cat > "$ROOT/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
exit 0
EOF
chmod 755 "$ROOT/DEBIAN/postinst"
mkdir -p "$OUT"
DEB="$OUT/$NAME.deb"
dpkg-deb --root-owner-group -Zxz --build "$ROOT" "$DEB" >/dev/null
echo "Gói: $(dpkg-deb -f "$DEB" Package) $(dpkg-deb -f "$DEB" Version) ($(dpkg-deb -f "$DEB" Architecture)), $(du -h "$DEB" | cut -f1)"
echo "DEB=$DEB"
