#!/bin/bash
# Dựng bản phát hành trong Ubuntu 22.04 (podman) rồi đóng gói .deb — chạy trên máy dựng Linux, trong thư mục mã nguồn đã gửi
# sang (…/src). In dòng DEB=<tệp .deb> khi xong.
#   bash linux/packaging/release-build.sh <thư mục ra>
set -euo pipefail
OUT=${1:?thư mục ra}
SRC=$(cd "$(dirname "$0")/../.." && pwd)
CACHE=${CONBANG_BUILD_CACHE:-$HOME/TestLab/ConBangTV/release-cache}
mkdir -p "$CACHE/target" "$CACHE/cargo-registry" "$OUT"
if ! podman image exists conbangtv-build; then
  podman build -q -t conbangtv-build -f "$SRC/linux/packaging/Containerfile" "$SRC/linux/packaging" >/dev/null
fi
podman run --rm \
  -v "$SRC:/src:Z" \
  -v "$CACHE/target:/target:Z" \
  -v "$CACHE/cargo-registry:/root/.cargo/registry:Z" \
  -e CARGO_TARGET_DIR=/target \
  conbangtv-build \
  bash -c 'cargo build --release --color never 2>&1 | grep -vE "^\s+(Compiling|Downloaded|Downloading)" | tail -20; test -x /target/release/conbangtv'
echo "glibc cần: $(objdump -T "$CACHE/target/release/conbangtv" | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1)"
bash "$SRC/linux/packaging/make-deb.sh" "$CACHE/target/release/conbangtv" "$OUT"
