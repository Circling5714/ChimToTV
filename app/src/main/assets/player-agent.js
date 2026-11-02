// Chèn vào đầu mọi tài liệu iframe của trình phát (xem PlayerActivity.shouldInterceptRequest).
// Nhận lệnh {chimto:'cmd'} qua postMessage, điều khiển <video>, báo trạng thái {chimto:'state'} lên khung cha.
// Khung không có <video> thì chuyển lệnh xuống iframe con, và chuyển trạng thái của con lên trên.
(function () {
  if (window.__chimtoAgent) return;
  window.__chimtoAgent = 1;
  var userActed = false;

  // Ẩn thanh điều khiển của jwplayer — app đã có thanh thời gian riêng điều khiển bằng remote.
  try {
    var st = document.createElement('style');
    st.textContent = '.jw-controlbar,.jw-title{display:none!important}';
    (document.head || document.documentElement).appendChild(st);
  } catch (e) {}

  function video() { return document.querySelector('video'); }
  function up(msg) { try { if (window.parent !== window) window.parent.postMessage(msg, '*'); } catch (e) {} }
  function down(msg) {
    var fs = document.getElementsByTagName('iframe');
    for (var i = 0; i < fs.length; i++) try { fs[i].contentWindow.postMessage(msg, '*'); } catch (e) {}
  }
  function state() {
    var v = video();
    if (!v) return null;
    return {
      chimto: 'state', ready: v.readyState > 0, t: v.currentTime || 0,
      d: isFinite(v.duration) ? v.duration : 0, paused: v.paused, ended: v.ended,
      waiting: !v.paused && v.readyState < 3
    };
  }
  function play(v) { var p = v.play(); if (p && p.catch) p.catch(function () {}); }

  window.addEventListener('message', function (e) {
    var m = e.data;
    if (!m || typeof m !== 'object') return;
    if (m.chimto === 'state') { if (e.source !== window) up(m); return; }
    if (m.chimto !== 'cmd') return;
    var v = video();
    if (!v) { down(m); return; }
    userActed = true;
    switch (m.cmd) {
      case 'toggle': if (v.paused) play(v); else v.pause(); break;
      case 'play': play(v); break;
      case 'pause': v.pause(); break;
      case 'seek':
        var max = isFinite(v.duration) ? v.duration - 1 : 1e9;
        v.currentTime = Math.max(0, Math.min(max, v.currentTime + m.arg));
        break;
      case 'seekTo': v.currentTime = m.arg; break;
    }
    var s = state(); if (s) up(s);
  });

  setInterval(function () { var s = state(); if (s) up(s); }, 1000);

  // Tự phát khi trình phát sẵn sàng; dừng thử khi đã chạy hoặc người xem đã bấm phím.
  var tries = 0;
  var auto = setInterval(function () {
    var v = video();
    if (userActed || ++tries > 90 || (v && v.currentTime > 0.5 && !v.paused)) { clearInterval(auto); return; }
    if (v && v.readyState > 0 && v.paused && !v.ended) play(v);
  }, 1000);
})();
