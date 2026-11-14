//! Cổng gỡ lỗi (chỉ khi chạy với biến CONBANG_DEBUG — bản thử; như DebugServer của bản Android / Windows, cổng 8790, chỉ
//! 127.0.0.1): xem trạng thái, đọc nhật ký, chạy JS trong trình phát, mở phim / trang, bấm nút theo chữ, đặt khoá cài đặt.
//! Chụp màn hình / bấm phím thật thì dùng bộ lệnh ubuntu.ps1 (Ub-Shot, Ub-Key) trên máy thử.

use crate::app::app;
use crate::keynav;
use gtk::prelude::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

struct Req {
    path: String,
    q: HashMap<String, String>,
    body: String,
    reply: std::sync::mpsc::Sender<String>,
}

pub fn start() {
    if std::env::var_os("CONBANG_DEBUG").is_none() {
        return;
    }
    let (tx, mut rx) = async_channel_like();
    std::thread::spawn(move || {
        let l = match std::net::TcpListener::bind("127.0.0.1:8790") {
            Ok(l) => l,
            Err(e) => {
                cbtv::log::d("debug", &format!("Không mở được cổng 8790: {e}"));
                return;
            }
        };
        cbtv::log::d("debug", "Cổng gỡ lỗi: http://127.0.0.1:8790/");
        for s in l.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let _ = serve(s, tx);
            });
        }
    });
    glib::MainContext::default().spawn_local(async move {
        while let Ok(r) = rx.recv().await {
            handle(r);
        }
    });
}

fn async_channel_like() -> (futures::channel::mpsc::UnboundedSender<Req>, Receiver) {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    (tx, Receiver(rx))
}

struct Receiver(futures::channel::mpsc::UnboundedReceiver<Req>);

impl Receiver {
    async fn recv(&mut self) -> Result<Req, ()> {
        use futures::StreamExt;
        self.0.next().await.ok_or(())
    }
}

fn serve(mut s: std::net::TcpStream, tx: futures::channel::mpsc::UnboundedSender<Req>) -> std::io::Result<()> {
    let mut r = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    r.read_line(&mut line)?;
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        r.read_line(&mut h)?;
        let t = h.trim();
        if t.is_empty() {
            break;
        }
        if let Some(v) = t.to_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; len.min(1 << 20)];
    r.read_exact(&mut body)?;
    let u = url::Url::parse(&format!("http://x{target}")).ok();
    let path = u.as_ref().map(|u| u.path().to_string()).unwrap_or_default();
    let q = u.map(|u| u.query_pairs().into_owned().collect()).unwrap_or_default();
    let (rtx, rrx) = std::sync::mpsc::channel();
    let _ = tx.unbounded_send(Req { path, q, body: String::from_utf8_lossy(&body).to_string(), reply: rtx });
    let out = rrx.recv_timeout(std::time::Duration::from_secs(20)).unwrap_or_else(|_| "hết giờ chờ".into());
    write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", out.len(), out)?;
    Ok(())
}

fn describe(w: &gtk::Widget) -> String {
    if let Ok(b) = w.clone().downcast::<gtk::Button>() {
        if let Some(l) = b.label() {
            return l.to_string();
        }
        return format!("nút {}", w.widget_name());
    }
    if let Ok(e) = w.clone().downcast::<gtk::Entry>() {
        // Ô mật khẩu / khoá: không in nội dung (cổng gỡ lỗi đọc qua mạng nội bộ, nhật ký thử…)
        if !gtk::prelude::EntryExt::is_visible(&e) {
            return format!("ô chữ ẩn ({} ký tự)", e.text_length());
        }
        return format!("ô chữ: {}", e.text());
    }
    if w.is::<gtk::DrawingArea>() {
        return format!("thẻ: {}", w.tooltip_text().unwrap_or_default());
    }
    w.type_().name().to_string()
}

fn info() -> String {
    let a = app();
    let f = a.window.focused_widget();
    let (w, h) = a.window.size();
    format!(
        "page={} fullscreen={} size={w}x{h}\nfocus={}\nplayer={}\nsite={}\nprofile={}\ntoast={}\n",
        a.current().map(|p| p.base().menu_key.borrow().clone()).unwrap_or_default(),
        a.is_fullscreen(),
        f.as_ref().map(describe).unwrap_or_default(),
        a.player.borrow().as_ref().map(|p| p.debug()).unwrap_or_else(|| "chưa mở".into()),
        cbtv::prefs::base_url(),
        cbtv::prefs::current_profile().name,
        if a.toast_box.is_visible() { a.toast_label.text().to_string() } else { String::new() }
    )
}

fn handle(r: Req) {
    let reply = r.reply.clone();
    let send = move |s: String| {
        let _ = reply.send(s);
    };
    match r.path.as_str() {
        "/log" => {
            let since = r.q.get("since").and_then(|s| s.parse().ok()).unwrap_or(0);
            let n: usize = r.q.get("n").and_then(|s| s.parse().ok()).unwrap_or(300);
            let (last, lines) = cbtv::log::since(since);
            let skip = lines.len().saturating_sub(n);
            send(format!("#{last}\n{}", lines[skip..].join("\n")));
        }
        "/js" => {
            let code = r.q.get("code").cloned().unwrap_or(r.body);
            match app().player.borrow().clone() {
                Some(p) => p.eval(&code, send),
                None => send("trình phát chưa mở".into()),
            }
        }
        "/diag" => {
            // Chẩn đoán khung trình phát (DIAG_JS trong player.rs): gửi lệnh xuống mọi khung, đọc kết quả sau 800 ms
            match app().player.borrow().clone() {
                Some(p) => {
                    p.eval("window.__diag=[];var fs=document.getElementsByTagName('iframe');for(var i=0;i<fs.length;i++)fs[i].contentWindow.postMessage({chimto:'diag'},'*');window.postMessage({chimto:'diag'},'*');'ok'", |_| {});
                    let p2 = p.clone();
                    crate::ui::after(800, move || p2.eval("JSON.stringify(window.__diag)", send));
                }
                None => send("trình phát chưa mở".into()),
            }
        }
        "/play" => {
            app().play(r.q.get("film").map(|s| s.as_str()).unwrap_or(""), r.q.get("ep").map(|s| s.as_str()).unwrap_or(""), r.q.contains_key("resume"));
            send("ok".into());
        }
        // Thử đường tự chuyển sang WebView: /native?fail=1 (mpv báo lỗi giữa chừng), /native?hang=1|0 (cổng phát nội bộ treo)
        "/native" => {
            let hang = r.q.get("hang").map(|v| v == "1");
            let fail = r.q.get("fail").map(|v| v == "1").unwrap_or(false);
            match app().player.borrow().clone() {
                Some(p) => send(p.debug_native(fail, hang)),
                None => {
                    if let Some(h) = hang {
                        cbtv::media_proxy::debug_hang(h);
                    }
                    send("trình phát chưa mở".into())
                }
            }
        }
        "/open" => {
            app().push(crate::pages::detail::new(r.q.get("url").map(|s| s.as_str()).unwrap_or(""), None, Vec::new()));
            send("ok".into());
        }
        "/pref" => {
            cbtv::prefs::debug_set(r.q.get("key").map(|s| s.as_str()).unwrap_or(""), r.q.get("json").map(|s| s.as_str()).unwrap_or("null"));
            send("ok".into());
        }
        "/prefs" => send(cbtv::prefs::debug_dump()),
        "/labels" => {
            // Chữ đang hiện trong trang (kiểm kết quả mà không cần chụp màn hình)
            fn walk(w: &gtk::Widget, out: &mut Vec<String>) {
                if !w.is_visible() {
                    return;
                }
                if let Some(l) = w.downcast_ref::<gtk::Label>() {
                    let t = l.text().to_string();
                    if !t.trim().is_empty() {
                        out.push(t);
                    }
                }
                if let Some(c) = w.downcast_ref::<gtk::Container>() {
                    for ch in c.children() {
                        walk(&ch, out);
                    }
                }
            }
            let mut out = Vec::new();
            if let Some(p) = app().current() {
                walk(p.base().body.upcast_ref(), &mut out);
            }
            send(out.join("
"));
        }
        "/click" => {
            let text = r.q.get("text").cloned().unwrap_or_default().to_lowercase();
            let n: usize = r.q.get("n").and_then(|s| s.parse().ok()).unwrap_or(0);
            let a = app();
            let root: gtk::Widget = a.window.clone().upcast();
            let found: Vec<gtk::Widget> = keynav::targets(&root).into_iter().filter(|w| describe(w).to_lowercase().contains(&text)).collect();
            match found.get(n) {
                Some(w) => {
                    if let Ok(b) = w.clone().downcast::<gtk::Button>() {
                        b.clicked();
                    } else {
                        w.grab_focus();
                    }
                    send(format!("đã bấm: {}", describe(w)));
                }
                None => send(format!("không thấy nút «{text}» ({})", found.len())),
            }
        }
        "/go" => {
            // /go?to=home|search|history|follows|sites|settings|sync|profile — mở mục menu
            let to = r.q.get("to").cloned().unwrap_or_default();
            let a = app();
            match crate::menu::entries(&a).into_iter().find(|e| e.key == to) {
                Some(e) => {
                    (e.open)();
                    send(format!("đã mở {to}"));
                }
                None => send(format!("không có mục {to}")),
            }
        }
        _ => send(info()),
    }
}
