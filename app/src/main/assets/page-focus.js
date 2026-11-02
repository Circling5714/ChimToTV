// Chế độ "mở trang tập" (pageMode): tìm trình phát lớn nhất trong trang (iframe hoặc <video>) và phóng nó
// ra toàn màn hình, ẩn phần còn lại của trang. Chạy lặp vì trình phát thường được nạp sau bằng ajax.
(function () {
  if (window.__chimtoFocus) return;
  window.__chimtoFocus = 1;

  var css = document.createElement('style');
  css.textContent =
    'html,body{background:#000!important;overflow:hidden!important}' +
    '.chimto-hide{visibility:hidden!important}' +
    '.chimto-path{visibility:visible!important;transform:none!important;filter:none!important;' +
    'perspective:none!important;contain:none!important;will-change:auto!important;animation:none!important}' +
    '.chimto-focus{position:fixed!important;left:0!important;top:0!important;right:auto!important;bottom:auto!important;' +
    'width:100vw!important;height:100vh!important;max-width:none!important;max-height:none!important;' +
    'margin:0!important;padding:0!important;border:0!important;transform:none!important;' +
    'z-index:2147483647!important;background:#000!important;visibility:visible!important}';
  (document.head || document.documentElement).appendChild(css);

  var current = null;

  // Điểm = diện tích, nhân thêm nếu nằm trong khung tên "player"/"embed" (tránh chọn nhầm iframe quảng cáo)
  function score(el) {
    var r = el.getBoundingClientRect();
    var s = r.width * r.height;
    if (s < 200 * 120) return 0;
    if (el.closest('[class*="player"], [id*="player"], [class*="embed"]')) s *= 4;
    return s;
  }

  function best() {
    var top = null, topScore = 0;
    var els = document.querySelectorAll('iframe, video');
    for (var i = 0; i < els.length; i++) {
      var el = els[i];
      if (el.tagName === 'IFRAME' && !/^https?:/.test(el.src || '')) continue;
      var s = el === current ? Infinity : score(el);
      if (s > topScore) { top = el; topScore = s; }
    }
    return top;
  }

  function clear() {
    var marked = document.querySelectorAll('.chimto-hide, .chimto-path, .chimto-focus');
    for (var i = 0; i < marked.length; i++) marked[i].classList.remove('chimto-hide', 'chimto-path', 'chimto-focus');
  }

  function apply(el) {
    clear();
    for (var n = el; n && n.parentElement && n !== document.body; n = n.parentElement) {
      var p = n.parentElement;
      p.classList.add('chimto-path');
      for (var i = 0; i < p.children.length; i++) if (p.children[i] !== n) p.children[i].classList.add('chimto-hide');
    }
    el.classList.add('chimto-focus');
    window.scrollTo(0, 0);
    current = el;
  }

  function tick() {
    if (current && !current.isConnected) current = null;
    var b = best();
    if (b && b !== current) apply(b);
  }
  tick();
  setInterval(tick, 700);
})();
