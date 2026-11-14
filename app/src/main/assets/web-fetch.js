// Côn Bằng TV — đọc hộ trang bật lớp chống bot của Cloudflare (lõi: web_fetch). App mở một WebView ẩn ở trang đó (trình
// duyệt thật tự qua lớp chặn), rồi gọi window.__cbFetch(id, url, method, body, headers) ngay trong trang cùng gốc: fetch()
// dùng cookie của trình duyệt. Kết quả {id, code, url, type, cf, b64} (hoặc {id, err}) — nội dung luôn là base64 (cả ảnh) —
// trả về Promise và gửi qua window.__cbReply nếu bản app đặt hàm đó (WebView2: chrome.webview.postMessage; Android: giao
// diện JS; WebKit: messageHandlers). Dùng chung mọi bản — sửa một chỗ.
(function () {
  if (window.__cbFetch) return;
  function b64(buf) {
    var b = new Uint8Array(buf), s = '', i;
    for (i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
    return btoa(s);
  }
  window.__cbFetch = function (id, url, method, body, headers) {
    var opt = { method: method || 'GET', credentials: 'include', headers: {}, redirect: 'follow', cache: 'no-store' };
    var h = headers || {};
    Object.keys(h).forEach(function (k) {
      var lk = k.toLowerCase();
      // Referer đặt bằng referrer; các đầu thư trình duyệt tự lo thì bỏ
      if (lk === 'referer') { opt.referrer = h[k]; return; }
      if (lk === 'user-agent' || lk === 'cookie' || lk === 'origin' || lk === 'host' || lk === 'accept-encoding') return;
      opt.headers[k] = h[k];
    });
    if (body !== null && body !== undefined) opt.body = body;
    return fetch(url, opt).then(function (r) {
      return r.arrayBuffer().then(function (buf) {
        return { id: id, code: r.status, url: r.url, type: r.headers.get('content-type'), cf: r.headers.get('cf-mitigated'), b64: b64(buf) };
      });
    }).catch(function (e) {
      return { id: id, err: String((e && e.message) || e) };
    }).then(function (msg) {
      if (window.__cbReply) window.__cbReply(msg);
      return msg;
    });
  };
})();
