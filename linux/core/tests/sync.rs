//! Đồng bộ trong nhà: giao thức (/hello, /pair, /sync, chữ ký) trên kho cài đặt thật, máy chủ thật trên 127.0.0.1 và phía máy
//! khách gọi tới một "máy kia" giả. Không mở cổng 8791/8792, không quảng bá mDNS (không gọi start / open_pairing / sync_all).
//! Kho cài đặt là một cho cả tiến trình → các test chạy lần lượt (khoá LOCK), mỗi test một thư mục dữ liệu riêng.

use cbtv::prefs;
use cbtv::sync_data::{self, SyncHistory, SyncPayload, SyncProfile};
use cbtv::sync_service::{self, http1, SyncEvent, SyncPeer};
use cbtv::util::now_ms;
use cbtv::HistoryItem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::io::AsyncWriteExt;

static LOCK: Mutex<()> = Mutex::new(());
static N: AtomicUsize = AtomicUsize::new(0);

struct Fresh {
    dir: PathBuf,
    _g: MutexGuard<'static, ()>,
}

impl Drop for Fresh {
    fn drop(&mut self) {
        sync_service::set_pair_code(None);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fresh() -> Fresh {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("cbtv-sync-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    prefs::load(Some(dir.clone()));
    sync_service::set_pair_code(None);
    Fresh { dir, _g: g }
}

/// Máy gọi tới (địa chỉ ví dụ dành cho tài liệu, RFC 5737).
const CALLER: &str = "192.0.2.9";

fn req(method: &str, path: &str, body: &[u8], headers: &[(&str, &str)]) -> http1::Message {
    http1::Message {
        method: method.into(),
        path: path.into(),
        headers: headers.iter().map(|(k, v)| (k.to_lowercase(), v.to_string())).collect(),
        body: body.to_vec(),
        remote_host: Some(CALLER.into()),
        ..Default::default()
    }
}

fn answer(r: &http1::Message) -> (http1::Message, serde_json::Value) {
    let raw = sync_service::handle(r);
    let m = http1::parse(&raw, false).expect("trả lời đủ");
    let v = serde_json::from_slice(&m.body).unwrap_or(serde_json::Value::Null);
    (m, v)
}

fn pair_body(device: &str, code: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"device": device, "name": "TX3", "platform": "android", "code": code})).unwrap()
}

/// Ghép máy "tv1" với máy này → khoá chung.
fn pair_tv1() -> String {
    sync_service::set_pair_code(Some("1234"));
    let (m, v) = answer(&req("POST", "/pair", &pair_body("tv1", "1234"), &[("X-Port", "8791")]));
    assert_eq!(m.status, 200);
    v["token"].as_str().unwrap().to_string()
}

fn events() -> (u64, Arc<Mutex<Vec<SyncEvent>>>) {
    let got = Arc::new(Mutex::new(Vec::new()));
    let g = got.clone();
    let id = sync_service::add_listener(move |e| g.lock().unwrap().push(e.clone()));
    (id, got)
}

fn peer(id: &str) -> SyncPeer {
    sync_service::peers().into_iter().find(|p| p.id == id).expect("có máy đã ghép")
}

fn remote_payload(film: &str, time: i64) -> SyncPayload {
    SyncPayload {
        device: "tv1".into(),
        name: "TX3".into(),
        platform: "android".into(),
        profiles: vec![SyncProfile {
            name: "Cả nhà".into(),
            history: vec![SyncHistory::new(film, &film.to_uppercase(), "", "Tập 1", &format!("{film}/1"), time)],
            ..Default::default()
        }],
        sites: sync_data::sync_sites_data(),
        ..Default::default()
    }
}

fn signed(token: &str, device: &str, body: &[u8]) -> http1::Message {
    let time = now_ms().to_string();
    let sig = http1::sign(token, &time, body);
    req("POST", "/sync", body, &[("X-Device", device), ("X-Time", &time), ("X-Sig", &sig), ("X-Port", "8791")])
}

#[test]
fn hello_and_pairing_by_code() {
    let _f = fresh();
    let (lid, got) = events();
    let (m, v) = answer(&req("GET", "/hello", b"", &[]));
    assert_eq!(m.status, 200);
    assert_eq!(v["app"], "conbangtv");
    assert_eq!(v["platform"], "linux");
    assert_eq!(v["device"], prefs::device_id());
    assert_eq!(v["version"], cbtv::update_check::current_version());
    assert_eq!(v["pairing"], false);
    assert_eq!(answer(&req("GET", "/khac", b"", &[])).0.status, 404);

    // Chưa mở màn Đồng bộ: không cho ghép
    let (m, v) = answer(&req("POST", "/pair", &pair_body("tv1", "1234"), &[]));
    assert_eq!((m.status, v["error"].as_str()), (403, Some("closed")));

    sync_service::set_pair_code(Some("1234"));
    assert_eq!(answer(&req("GET", "/hello", b"", &[])).1["pairing"], true);
    assert_eq!(answer(&req("POST", "/pair", b"{bad", &[])).0.status, 400);
    assert_eq!(answer(&req("POST", "/pair", br#"{"device":"tv1"}"#, &[])).0.status, 400);
    assert_eq!(answer(&req("POST", "/pair", br#"{"device":"","code":"1234"}"#, &[])).0.status, 400);

    // Đoán sai 5 lần: đổi mã
    for i in 0..4 {
        let (m, v) = answer(&req("POST", "/pair", &pair_body("tv1", "0000"), &[]));
        assert_eq!((m.status, v["error"].as_str()), (403, Some("code")), "lần {i}");
        assert_eq!(sync_service::pair_code().as_deref(), Some("1234"));
    }
    answer(&req("POST", "/pair", &pair_body("tv1", "0000"), &[]));
    let code = sync_service::pair_code().unwrap();
    assert_ne!(code, "1234"); // (1 / 10 000 trùng lại — chấp nhận)
    assert!(code.len() == 4 && code.chars().all(|c| c.is_ascii_digit()));
    assert!(sync_service::peers().is_empty());

    // Đúng mã: trả khoá chung, nhớ máy kia + địa chỉ gọi lại (X-Port); mã dùng một lần
    let (m, v) = answer(&req("POST", "/pair", &pair_body("tv1", &code), &[("X-Port", "8791")]));
    assert_eq!(m.status, 200);
    assert_eq!(v["device"], prefs::device_id());
    assert_eq!(v["platform"], "linux");
    let token = v["token"].as_str().unwrap();
    assert!(token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()));
    let p = peer("tv1");
    assert_eq!((p.name.as_str(), p.platform.as_str(), p.token.as_str()), ("TX3", "android", token));
    assert_eq!((p.host.as_deref(), p.port, p.status.as_deref()), (Some(CALLER), Some(8791), Some("Vừa ghép")));
    assert_ne!(sync_service::pair_code().unwrap(), code);
    assert!(got.lock().unwrap().contains(&SyncEvent::Message("Đã ghép với TX3.".into())));

    // Ghép lại cùng máy: thay khoá cũ (không thêm máy thứ hai); không có X-Port thì nhớ địa chỉ, không nhớ cổng
    let code = sync_service::pair_code().unwrap();
    let body = serde_json::to_vec(&serde_json::json!({"device": "tv1", "name": " ", "code": code})).unwrap();
    let (m, v) = answer(&req("POST", "/pair", &body, &[]));
    assert_eq!(m.status, 200);
    assert_eq!(sync_service::peers().len(), 1);
    let p = peer("tv1");
    assert_eq!((p.name.as_str(), p.token.as_str(), p.port), ("Máy khác", v["token"].as_str().unwrap(), None));

    sync_service::unpair("tv1");
    assert!(sync_service::peers().is_empty());
    sync_service::remove_listener(lid);
}

#[test]
fn sync_request_merges_and_answers_signed() {
    let _f = fresh();
    prefs::add_history(HistoryItem {
        film_url: "a".into(),
        title: "A".into(),
        ep_title: "Tập 1".into(),
        ep_url: "a/1".into(),
        time: 10,
        ..Default::default()
    });
    let token = pair_tv1();
    let (lid, got) = events();
    let body = remote_payload("x", 20).to_json();

    // Máy lạ / sai chữ ký / sai khoá / thân hỏng
    assert_eq!(answer(&signed(&token, "khac", &body)).1["error"], "unpaired");
    assert_eq!(answer(&req("POST", "/sync", &body, &[])).0.status, 403);
    let mut bad = signed(&token, "tv1", &body);
    bad.body = remote_payload("y", 20).to_json();
    assert_eq!(answer(&bad).1["error"], "sig");
    assert_eq!(answer(&signed("khoa-khac", "tv1", &body)).0.status, 401);
    assert_eq!(answer(&signed(&token, "tv1", b"[1]")).1["error"], "json");
    assert!(prefs::history().iter().all(|h| h.film_url != "x"));

    // Đúng: gộp vào máy này, trả lại bản đã gộp (cũng ký)
    let (m, _) = answer(&signed(&token, "tv1", &body));
    assert_eq!(m.status, 200);
    assert!(http1::verify(&m, &token));
    let back = SyncPayload::parse(&m.body).unwrap();
    assert_eq!(back.platform, "linux");
    assert_eq!(back.profiles[0].history.iter().map(|h| h.film.as_str()).collect::<Vec<_>>(), ["x", "a"]);
    assert_eq!(prefs::history().iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), ["X", "A"]);
    let p = peer("tv1");
    assert_eq!((p.name.as_str(), p.status.as_deref(), p.host.as_deref(), p.port), ("TX3", Some("Đã nhận dữ liệu mới"), Some(CALLER), Some(8791)));
    assert!(p.last.unwrap() > 0);
    assert!(got.lock().unwrap().contains(&SyncEvent::DataChanged));

    // Gọi lại cùng dữ liệu: đã khớp
    got.lock().unwrap().clear();
    let (m, _) = answer(&signed(&token, "tv1", &body));
    assert_eq!(m.status, 200);
    assert_eq!(peer("tv1").status.as_deref(), Some("Đã khớp"));
    assert!(!got.lock().unwrap().contains(&SyncEvent::DataChanged));
    sync_service::remove_listener(lid);
}

#[tokio::test]
async fn loopback_server() {
    let _f = fresh();
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        loop {
            let Ok((c, _)) = l.accept().await else { return };
            tokio::spawn(sync_service::serve(c));
        }
    });

    let hello = http1::send("127.0.0.1", port, "GET", "/hello", b"", None).await.unwrap();
    assert_eq!(hello.status, 200);
    assert_eq!(hello.remote_host.as_deref(), Some("127.0.0.1"));

    // Ghép qua mạng thật (máy này đóng vai cả hai: mã máy gửi = mã máy này)
    sync_service::set_pair_code(Some("4321"));
    let r = http1::send("127.0.0.1", port, "POST", "/pair", &pair_body(&prefs::device_id(), "4321"), None).await.unwrap();
    assert_eq!(r.status, 200);
    let token = cbtv::json::str(cbtv::json::parse_object(&r.body).as_ref(), "token");
    let p = peer(&prefs::device_id());
    assert_eq!((p.host.as_deref(), p.port), (Some("127.0.0.1"), Some(sync_service::port())));

    // Đồng bộ có ký (X-Device / X-Time / X-Sig do http1::send thêm)
    let body = remote_payload("y", 30).to_json();
    let r = http1::send("127.0.0.1", port, "POST", "/sync", &body, Some(&token)).await.unwrap();
    assert_eq!(r.status, 200);
    assert!(http1::verify(&r, &token));
    let back = SyncPayload::parse(&r.body).unwrap();
    assert!(back.profiles[0].history.iter().any(|h| h.film == "y"));

    // Thân lớn (nhiều gói TCP) vẫn đọc đủ
    let mut big = remote_payload("z", 40);
    big.profiles[0].hist_del = (0..300).map(|i| (format!("https://example.com/phim-rat-dai-{i:04}"), i)).collect();
    let r = http1::send("127.0.0.1", port, "POST", "/sync", &big.to_json(), Some(&token)).await.unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(SyncPayload::parse(&r.body).unwrap().profiles[0].hist_del.len(), 300);
    server.abort();
}

/// Máy kia giả: nhận một yêu cầu, trả lời bằng `reply(yêu cầu)`.
async fn fake_peer(reply: impl FnOnce(http1::Message) -> Vec<u8> + Send + 'static) -> (u16, tokio::task::JoinHandle<()>) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let h = tokio::spawn(async move {
        let (mut c, _) = l.accept().await.unwrap();
        let m = http1::read(&mut c, true).await.unwrap().unwrap();
        c.write_all(&reply(m)).await.unwrap();
        let _ = c.shutdown().await;
    });
    (port, h)
}

fn save_peer(token: &str, port: u16) {
    prefs::set_sync_peers_raw(serde_json::json!([
        {"id": "tv9", "name": "TV", "platform": "android", "token": token, "host": "127.0.0.1", "port": port}
    ]));
}

#[tokio::test]
async fn client_syncs_with_peer() {
    let _f = fresh();
    prefs::add_history(HistoryItem { film_url: "a".into(), title: "A".into(), time: 10, ..Default::default() });
    let token = "khoa-chung";
    let sent: Arc<Mutex<Option<http1::Message>>> = Arc::new(Mutex::new(None));
    let s2 = sent.clone();
    let (port, h) = fake_peer(move |m| {
        // máy kia gộp dữ liệu máy này vào dữ liệu của nó rồi trả lại (ký)
        let mine = SyncPayload::parse(&m.body).unwrap();
        let mut theirs = remote_payload("z", 50);
        theirs.name = "Tivi phòng khách".into();
        theirs.profiles[0] = sync_data::SyncMerge::profile(&theirs.profiles[0], &mine.profiles[0]);
        *s2.lock().unwrap() = Some(m);
        http1::response(200, &theirs.to_json(), Some("khoa-chung"))
    })
    .await;
    save_peer(token, port);
    assert!(sync_service::sync_peer(&peer("tv9")).await);
    h.await.unwrap();
    let m = sent.lock().unwrap().take().unwrap();
    assert_eq!((m.method.as_str(), m.path.as_str()), ("POST", "/sync"));
    assert!(http1::verify(&m, token));
    assert_eq!(m.headers["x-device"], prefs::device_id());
    assert_eq!(m.headers["x-port"], sync_service::port().to_string());
    assert_eq!(SyncPayload::parse(&m.body).unwrap().profiles[0].history[0].film, "a");
    assert_eq!(prefs::history().iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), ["Z", "A"]);
    let p = peer("tv9");
    assert_eq!((p.name.as_str(), p.status.as_deref(), p.port), ("Tivi phòng khách", Some("Đã nhận dữ liệu mới"), Some(port)));
    assert!(p.last.is_some());

    // Máy kia đã bỏ ghép
    let (port, h) = fake_peer(|_| http1::response(403, br#"{"error":"unpaired"}"#, None)).await;
    save_peer(token, port);
    assert!(!sync_service::sync_peer(&peer("tv9")).await);
    h.await.unwrap();
    assert_eq!(peer("tv9").status.as_deref(), Some("Máy kia đã bỏ ghép — ghép lại"));

    // Trả lời không ký đúng → không nhận
    let (port, h) = fake_peer(|_| http1::response(200, &remote_payload("q", 60).to_json(), Some("khoa-khac"))).await;
    save_peer(token, port);
    assert!(!sync_service::sync_peer(&peer("tv9")).await);
    h.await.unwrap();
    assert_eq!(peer("tv9").status.as_deref(), Some("Lỗi 200"));
    assert!(prefs::history().iter().all(|h| h.film_url != "q"));

    // Không ai nghe ở cổng đó
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = l.local_addr().unwrap().port();
    drop(l);
    save_peer(token, dead);
    assert!(!sync_service::sync_peer(&peer("tv9")).await);
    assert_eq!(peer("tv9").status.as_deref(), Some("Không kết nối được"));

    // Không biết địa chỉ, không thấy trong mạng
    prefs::set_sync_peers_raw(serde_json::json!([{"id": "tv9", "name": "TV", "token": token}]));
    assert!(!sync_service::sync_peer(&peer("tv9")).await);
    assert_eq!(peer("tv9").status.as_deref(), Some("Không thấy máy trong mạng"));
}

#[tokio::test]
async fn pair_errors_from_other_side() {
    let _f = fresh();
    // http1::send báo lỗi rõ ràng khi máy kia đóng kết nối không trả lời
    let (port, h) = fake_peer(|_| Vec::new()).await;
    let e = http1::send("127.0.0.1", port, "POST", "/pair", b"{}", None).await.unwrap_err();
    h.await.unwrap();
    assert_eq!(e.to_string(), "máy kia đóng kết nối");
    // Danh sách máy đã ghép hỏng một mục: bỏ mục đó, giữ mục khác
    prefs::set_sync_peers_raw(serde_json::json!([{"id": 5}, {"id": "ok", "token": "t"}, "rác"]));
    assert_eq!(sync_service::peers().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["ok"]);
    let st = sync_service::status();
    assert_eq!(st.peers.len(), 1);
    assert!(!st.busy && !st.listening && st.auto);
    assert!(!st.address.is_empty() && !st.device_name.is_empty());
}
