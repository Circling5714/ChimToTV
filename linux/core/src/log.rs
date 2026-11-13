//! Nhật ký nhỏ: ghi vào ~/.local/share/conbangtv/log.txt (giữ ~500 KB) + vòng 2 000 dòng trong bộ nhớ (cổng gỡ lỗi /log).
//! Bản Windows: Log.cs.

use std::io::Write;
use std::sync::Mutex;

struct State {
    lines: std::collections::VecDeque<(u64, String)>,
    seq: u64,
    file: Option<std::path::PathBuf>,
}

static STATE: Mutex<State> = Mutex::new(State { lines: std::collections::VecDeque::new(), seq: 0, file: None });

/// Gọi một lần lúc mở app (sau Prefs::load).
pub fn start() {
    let f = crate::prefs::data_dir().join("log.txt");
    if let Ok(m) = std::fs::metadata(&f) {
        if m.len() > 500_000 {
            let _ = std::fs::rename(&f, f.with_extension("txt.1"));
        }
    }
    STATE.lock().unwrap().file = Some(f);
}

/// Ghi một dòng nhật ký: `log::d("net", "…")`.
pub fn d(tag: &str, message: &str) {
    let (date, hms) = stamp();
    let line = format!("{hms} {tag}: {message}");
    if std::env::var_os("CONBANG_DEBUG").is_some() {
        eprintln!("{line}");
    }
    let mut st = STATE.lock().unwrap();
    st.seq += 1;
    let seq = st.seq;
    st.lines.push_back((seq, line.clone()));
    while st.lines.len() > 2000 {
        st.lines.pop_front();
    }
    if let Some(f) = st.file.clone() {
        if let Ok(mut h) = std::fs::OpenOptions::new().create(true).append(true).open(f) {
            let _ = writeln!(h, "{date} {line}");
        }
    }
}

/// Các dòng có số thứ tự > since; trả kèm số thứ tự cuối để lần sau hỏi tiếp.
pub fn since(since: u64) -> (u64, Vec<String>) {
    let st = STATE.lock().unwrap();
    (st.seq, st.lines.iter().filter(|(s, _)| *s > since).map(|(_, t)| t.clone()).collect())
}

fn stamp() -> (String, String) {
    let ms = crate::util::now_ms();
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt: libc::time_t = (ms / 1000) as libc::time_t;
    unsafe {
        libc::localtime_r(&tt, &mut tm);
    }
    (
        format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday),
        format!("{:02}:{:02}:{:02}.{:03}", tm.tm_hour, tm.tm_min, tm.tm_sec, ms % 1000),
    )
}
