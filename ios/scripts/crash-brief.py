#!/usr/bin/env python3
"""Tóm tắt báo cáo văng (.ips) của app trên máy ảo: lý do, thông báo của Swift / dyld, các hàm ở luồng bị văng.
Dùng: python3 crash-brief.py <tệp .ips> [...]  (sim-shots.sh gọi khi app không còn trả lời cổng gỡ lỗi)."""
import json
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")


def frames(images, fr):
    for f in fr[:30]:
        i = f.get("imageIndex", -1)
        image = images[i].get("name", "?") if 0 <= i < len(images) else "?"
        print("   ", image, f.get("symbol") or hex(f.get("imageOffset", 0)), "+", f.get("symbolLocation", ""))


for path in sys.argv[1:]:
    print("---", path)
    try:
        # Dòng đầu là phần tóm tắt (JSON), phần còn lại là báo cáo (JSON)
        _, _, body = open(path, encoding="utf-8", errors="replace").read().partition("\n")
        r = json.loads(body)
    except (OSError, ValueError) as e:
        print("không đọc được:", e)
        continue
    for k in ("exception", "termination", "asi"):
        if k in r:
            print(k + ":", json.dumps(r[k], ensure_ascii=False)[:1500])
    images = r.get("usedImages", [])
    if r.get("lastExceptionBacktrace"):
        print("lastExceptionBacktrace:")
        frames(images, r["lastExceptionBacktrace"])
    for t in r.get("threads", []):
        if t.get("triggered"):
            print("luồng bị văng:", t.get("name") or t.get("queue") or "")
            frames(images, t.get("frames", []))
