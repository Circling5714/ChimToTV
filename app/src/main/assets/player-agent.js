// Chèn vào đầu mọi tài liệu iframe của trình phát (xem PlayerActivity.shouldInterceptRequest),
// vào khung trên cùng khi app mở thẳng trang tập phim (chế độ pageMode), và vào trang phát HLS của app (hls.html).
// Nhận lệnh {chimto:'cmd'} qua postMessage, điều khiển <video>, báo trạng thái {chimto:'state'} lên khung cha.
// Khung không có <video> thì chuyển lệnh xuống iframe con, và chuyển trạng thái của con lên trên.
// Ở khung trên cùng: lưu trạng thái vào window.__chimtoState và cung cấp window.chimtoCmd() cho app gọi.
(function () {
  if (window.__chimtoAgent) return;
  window.__chimtoAgent = 1;
  var isTop = window.top === window;
  var userActed = false, restarted = false;

  // Ẩn thanh điều khiển của jwplayer — app đã có thanh thời gian riêng điều khiển bằng remote.
  try {
    var st = document.createElement('style');
    st.textContent = '.jw-controlbar,.jw-title{display:none!important}';
    (document.head || document.documentElement).appendChild(st);
  } catch (e) {}

  // Bộ đệm: JW Player (trình phát streamfree của yanhh3d / hoathinh3d.ee / hhkungfu) để preload "metadata" nên ép
  // hls.js maxMaxBufferLength = 1 → chỉ tải trước đúng một đoạn (đoạn dài 10–19 s) → mạng TV hụt một nhịp là đứng
  // hình. JW chỉ cho qua hlsjsConfig.onHlsInit (không cho maxBufferLength) và trang có onHlsInit riêng, nên bọc
  // jwplayer(...).setup: chạy onHlsInit của trang trước rồi nâng bộ đệm lên 120 s, bỏ qua lệnh ép về mức quá nhỏ
  // (vẫn để hls.js tự hạ khi TV hết bộ nhớ video), chỉ giữ 30 s đã xem cho đỡ tốn RAM.
  var BUFFER = 120, BUFFER_FLOOR = 10;
  function bigBuffer(hls) {
    var c = hls.config, max = 600;
    c.maxBufferLength = BUFFER;
    c.backBufferLength = 30;
    try {
      Object.defineProperty(c, 'maxMaxBufferLength', {
        configurable: true, enumerable: true,
        get: function () { return max; },
        set: function (v) { if (v >= BUFFER_FLOOR) max = v; }
      });
    } catch (e) { c.maxMaxBufferLength = max; }
    window.__chimtoBuffer = BUFFER;
  }
  // Chỗ bắt đầu app báo trước (xem tiếp / đã đánh dấu nhạc mở đầu): trình phát tải thẳng đoạn ở chỗ đó, khỏi tải đoạn
  // đầu tập (mỗi đoạn 10–19 s, CDN chậm có khi mất 6–7 giây) rồi mới tua.
  var START = window.__chimtoStart || 0;
  // JW tự gọi hls.startLoad(0) (bỏ qua starttime) → lần tải đầu tiên không có vị trí thì bắt đầu ở START
  function startAt(hls) {
    hls.config.startPosition = START;
    var orig = hls.startLoad, first = true;
    hls.startLoad = function (pos) {
      if (first) { first = false; if (!(pos > 0)) pos = START; }
      return orig.call(this, pos);
    };
  }
  function patchSetup(cfg) {
    if (!cfg || typeof cfg !== 'object') return cfg;
    if (START > 0) {
      cfg.starttime = START;
      if (Array.isArray(cfg.playlist)) cfg.playlist.forEach(function (p) { if (p && typeof p === 'object') p.starttime = START; });
    }
    var h = cfg.hlsjsConfig || (cfg.hlsjsConfig = {});
    var theirs = h.onHlsInit;
    h.onHlsInit = function (hls) {
      if (typeof theirs === 'function') try { theirs(hls); } catch (e) {}
      try { bigBuffer(hls); } catch (e) {}
      if (START > 0) try { startAt(hls); } catch (e) {}
    };
    return cfg;
  }
  function wrapJw(jw) {
    if (typeof jw !== 'function' || jw.__chimto || typeof Proxy !== 'function') return jw;
    jw.__chimto = 1;
    return new Proxy(jw, {
      apply: function (target, self, args) {
        var api = Reflect.apply(target, self, args);
        if (api && typeof api.setup === 'function' && !api.__chimtoSetup) {
          var setup = api.setup;
          api.setup = function (cfg) { return setup.call(this, patchSetup(cfg)); };
          api.__chimtoSetup = 1;
        }
        return api;
      }
    });
  }
  if (!window.jwplayer) {
    try {
      var jwReal;
      Object.defineProperty(window, 'jwplayer', {
        configurable: true, enumerable: true,
        get: function () { return jwReal; },
        set: function (v) { jwReal = wrapJw(v); }
      });
    } catch (e) {}
  }

  function video() {
    // Khung trên cùng của trang tập: nếu trình phát được chọn là iframe thì để iframe đó xử lý
    // (tránh nhầm sang video quảng cáo / xem trước nằm trong trang).
    var focus = document.querySelector('.chimto-focus');
    if (focus) return focus.tagName === 'VIDEO' ? focus : focus.querySelector('video');
    return document.querySelector('video');
  }
  // Quảng cáo của trình phát JW (vd NguonC chạy quảng cáo cá cược ~2 phút trước phim): báo app là đang quảng cáo
  // (app không coi là phim: không tua, không nhớ vị trí) và bỏ qua ngay khi trình phát cho phép.
  function adPlaying() { return !!document.querySelector('.jwplayer.jw-flag-ads'); }
  function skipAd() {
    var b = document.querySelector('.jw-skip.jw-skippable');
    if (b) { b.click(); return; }
    try { var p = window.jwplayer && window.jwplayer(); if (p && p.skipAd) p.skipAd(); } catch (e) {}
  }

  function up(msg) {
    if (isTop) { window.__chimtoState = msg; return; }
    try { window.parent.postMessage(msg, '*'); } catch (e) {}
  }
  function down(msg) {
    var fs = document.getElementsByTagName('iframe');
    for (var i = 0; i < fs.length; i++) try { fs[i].contentWindow.postMessage(msg, '*'); } catch (e) {}
  }
  function state() {
    if (adPlaying()) return { chimto: 'state', ad: true };
    var v = video();
    if (!v) return null;
    // Khung hình rớt (TV giải mã / vẽ không kịp) — để app phân biệt "TV yếu" với "mạng chậm"
    var q = v.getVideoPlaybackQuality ? v.getVideoPlaybackQuality() : null;
    var t = v.currentTime || 0, ahead = 0;
    for (var i = 0; i < v.buffered.length; i++) {
      if (v.buffered.start(i) <= t + 0.5 && t < v.buffered.end(i)) ahead = v.buffered.end(i) - t;
    }
    return {
      chimto: 'state', ready: v.readyState > 0, t: t,
      d: isFinite(v.duration) ? v.duration : 0, paused: v.paused, ended: v.ended,
      waiting: !v.paused && v.readyState < 3,
      df: q ? q.droppedVideoFrames : (v.webkitDroppedFrameCount || 0),
      tf: q ? q.totalVideoFrames : (v.webkitDecodedFrameCount || 0),
      ahead: Math.round(ahead), buf: window.__chimtoBuffer || 0,
      err: window.__chimtoErr || '', // trang phát HLS của app (hls.html) báo lỗi hẳn
      rate: v.playbackRate,
      // Khung phim (cửa sổ hình trong hình theo đúng tỉ lệ) + đang ở cửa sổ hình trong hình của Safari (bản iPad)
      vw: v.videoWidth || 0, vh: v.videoHeight || 0,
      pip: v.webkitPresentationMode === 'picture-in-picture' || document.pictureInPictureElement === v
    };
  }
  // Hình trong hình của trình duyệt (bản iPad: WebKit; Android dùng hình trong hình của cả màn hình app)
  function pip(v, on) {
    try {
      if (v.webkitSetPresentationMode) { v.webkitSetPresentationMode(on ? 'picture-in-picture' : 'inline'); return; }
      if (on && v.requestPictureInPicture) { var p = v.requestPictureInPicture(); if (p && p.catch) p.catch(function () {}); }
      else if (!on && document.exitPictureInPicture && document.pictureInPictureElement) document.exitPictureInPicture();
    } catch (e) {}
  }
  function play(v) {
    // JW Player còn ở màn "bấm để phát" (trình phát không tự phát, vd NguonC) → nhờ JW phát để nó tắt màn đó
    try {
      var jw = window.jwplayer && window.jwplayer();
      if (jw && jw.getState && jw.getState() === 'idle') { jw.play(); return; }
    } catch (e) {}
    var p = v.play(); if (p && p.catch) p.catch(function () {});
  }

  function handle(m) {
    var v = video();
    if (!v) { down(m); return; }
    // Chỉ phím phát / dừng mới tắt tự phát (lệnh tua có thể là app tua tới chỗ xem dở ngay lúc mở)
    if (m.cmd === 'toggle' || m.cmd === 'play' || m.cmd === 'pause') userActed = true;
    switch (m.cmd) {
      case 'toggle': if (v.paused) play(v); else v.pause(); break;
      case 'play': play(v); break;
      case 'pause': v.pause(); break;
      case 'seek':
        var max = isFinite(v.duration) ? v.duration - 1 : 1e9;
        v.currentTime = Math.max(0, Math.min(max, v.currentTime + m.arg));
        break;
      case 'seekTo': v.currentTime = m.arg; break;
      case 'rate': want.rate = m.arg; applyView(v); break;
      case 'fit': want.fit = m.arg; applyView(v); break;
      case 'pip': pip(v, !!m.arg); break;
    }
    var s = state(); if (s) up(s);
  }

  window.addEventListener('message', function (e) {
    var m = e.data;
    if (!m || typeof m !== 'object') return;
    if (m.chimto === 'state') { if (e.source !== window) up(m); return; }
    if (m.chimto === 'cmd') handle(m);
  });
  if (isTop) window.chimtoCmd = function (c, a) { handle({ chimto: 'cmd', cmd: c, arg: a }); return true; };
  // Bản iPad gọi thẳng trong khung có <video> (hình trong hình cần "người dùng vừa bấm", qua postMessage thì mất):
  // trả true nếu khung này có video
  window.__chimtoHere = function (c, a) { if (!video()) return false; handle({ chimto: 'cmd', cmd: c, arg: a }); return true; };

  // Gỡ kẹt: một số trình phát (MSE) treo ở trạng thái "đang tua" dù dữ liệu phía trước đã tải xong
  // (hay gặp khi lệnh xem tiếp trùng lúc trình phát tự bỏ qua giới thiệu). Đứng yên ≥ 4 giây → đẩy nhẹ 0,5 giây.
  // Tốc độ / khung hình người xem chọn trong app (null = để trình phát tự lo). Đặt lại mỗi giây: trình phát như JW
  // đặt lại tốc độ / kiểu co giãn khi đổi nguồn hoặc tua.
  var want = { rate: null, fit: null };
  function applyView(v) {
    if (!v) return;
    if (want.rate && Math.abs(v.playbackRate - want.rate) > 0.01) v.playbackRate = want.rate;
    if (want.fit === null) return;
    // Lấp đầy: video rộng (21:9) phóng cho hết viền đen trên / dưới, cắt bớt hai bên
    var fit = want.fit ? 'cover' : 'contain';
    if (v.style.getPropertyValue('object-fit') !== fit) v.style.setProperty('object-fit', fit, 'important');
  }

  var lastT = -1, still = 0;
  function buffered(v, t) {
    for (var i = 0; i < v.buffered.length; i++) if (v.buffered.start(i) <= t && t < v.buffered.end(i)) return true;
    return false;
  }
  function unstick(v) {
    if (!v || v.paused || v.ended) { still = 0; return; }
    still = Math.abs(v.currentTime - lastT) < 0.01 ? still + 1 : 0;
    lastT = v.currentTime;
    if (still >= 4 && buffered(v, v.currentTime + 0.5)) { v.currentTime = v.currentTime + 0.5; still = 0; }
  }

  setInterval(function () {
    if (adPlaying()) skipAd(); else { unstick(video()); applyView(video()); }
    // NguonC tự hỏi "Tiếp tục xem?" và chờ bấm (remote không bấm được trong iframe): app đã tự nhớ chỗ xem dở
    // và tua tới đó khi phim chạy → chọn "Xem lại từ đầu"
    var restart = !restarted && document.querySelector('.stream-resume #restartBtn');
    if (restart && restart.getBoundingClientRect().width > 0) { restarted = true; restart.click(); }
    var s = state(); if (s) up(s);
  }, 1000);

  // Tự phát khi trình phát sẵn sàng; dừng thử khi đã chạy hoặc người xem đã bấm phím.
  var tries = 0;
  var auto = setInterval(function () {
    var v = video();
    if (userActed || ++tries > 90 || (v && v.currentTime > 0.5 && !v.paused)) { clearInterval(auto); return; }
    if (v && v.readyState > 0 && v.paused && !v.ended) play(v);
  }, 1000);
})();
