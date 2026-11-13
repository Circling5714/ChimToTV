//! Đi bằng phím mũi tên / remote theo vị trí trên màn hình (như bản TV / Windows KeyNav.cs): từ mục đang chọn, tìm mục gần
//! nhất theo hướng bấm (cách tính của FocusFinder trên Android: khoảng cách chính × 13 + lệch ngang / dọc).

use gtk::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

pub fn dir_of(key: &gdk::keys::Key) -> Option<Dir> {
    use gdk::keys::constants as k;
    match *key {
        k::Left | k::KP_Left => Some(Dir::Left),
        k::Right | k::KP_Right => Some(Dir::Right),
        k::Up | k::KP_Up => Some(Dir::Up),
        k::Down | k::KP_Down => Some(Dir::Down),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub l: f64,
    pub t: f64,
    pub r: f64,
    pub b: f64,
}

/// Mục chọn được: nút, ô chữ, thẻ phim (DrawingArea nhận phím) — đang hiện, bật.
fn is_target(w: &gtk::Widget) -> bool {
    w.can_focus()
        && w.is_sensitive()
        && w.is_visible()
        && w.is_mapped()
        && w.allocated_width() > 1
        && (w.is::<gtk::Button>() || w.is::<gtk::Entry>() || w.is::<gtk::DrawingArea>() || w.is::<gtk::Switch>())
}

/// Các mục chọn được trong root.
pub fn targets(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut out = Vec::new();
    fn walk(w: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
        if !w.is_visible() {
            return;
        }
        if is_target(w) {
            out.push(w.clone());
            return; // không đi vào trong nút
        }
        if let Some(c) = w.downcast_ref::<gtk::Container>() {
            for ch in c.children() {
                walk(&ch, out);
            }
        }
    }
    walk(root, &mut out);
    out
}

/// Khung của w theo toạ độ của root (None: không nằm trong root / chưa xếp chỗ).
pub fn bounds(w: &gtk::Widget, root: &gtk::Widget) -> Option<Rect> {
    let (x, y) = w.translate_coordinates(root, 0, 0)?;
    let (wd, ht) = (w.allocated_width() as f64, w.allocated_height() as f64);
    Some(Rect { l: x as f64, t: y as f64, r: x as f64 + wd, b: y as f64 + ht })
}

/// Dải vuốt ngang chứa w (None: không nằm trong dải).
fn strip_of(w: &gtk::Widget) -> Option<gtk::Widget> {
    let mut cur = w.parent();
    while let Some(p) = cur {
        if p.widget_name() == "strip" {
            return Some(p);
        }
        cur = p.parent();
    }
    None
}

/// Ô đang thấy (cả một phần) trong dải.
fn in_view(t: &gtk::Widget, s: &gtk::Widget) -> bool {
    match bounds(t, s) {
        Some(r) => r.r > 12.0 && r.l < s.allocated_width() as f64 - 12.0,
        None => false,
    }
}

fn gap(a1: f64, a2: f64, b1: f64, b2: f64) -> f64 {
    if b2 < a1 {
        return a1 - b2;
    }
    if b1 > a2 {
        return b1 - a2;
    }
    // Chồng nhau: ưu tiên mục thẳng hàng tâm
    ((a1 + a2) / 2.0 - (b1 + b2) / 2.0).abs() * 0.1
}

/// Mục kế tiếp theo hướng (None = không còn mục nào theo hướng đó). Ô của dải vuốt ngang đang khuất chỉ chọn được bằng
/// ◄ ► trong chính dải đó (▲ ▼ chọn ô đang thấy của dải kia).
pub fn next(root: &gtk::Widget, current: &gtk::Widget, dir: Dir) -> Option<gtk::Widget> {
    let cur = bounds(current, root)?;
    let cur_strip = strip_of(current);
    let horizontal = matches!(dir, Dir::Left | Dir::Right);
    let (mut best, mut best_row): (Option<gtk::Widget>, Option<gtk::Widget>) = (None, None);
    let (mut best_score, mut best_row_score) = (f64::MAX, f64::MAX);
    for t in targets(root) {
        if &t == current {
            continue;
        }
        let ts = strip_of(&t);
        // ◄ ► trong dải: chỉ đi trong chính dải (cuối dải thì đứng yên; ◄ ở đầu dải ra được menu bên trái — ngoài dải)
        if cur_strip.is_some() && horizontal && ts.is_some() && ts != cur_strip {
            continue;
        }
        if let Some(s) = &ts {
            if !(horizontal && Some(s) == cur_strip.as_ref()) && !in_view(&t, s) {
                continue;
            }
        }
        let Some(r) = bounds(&t, root) else { continue };
        let (cx, cy) = ((cur.l + cur.r) / 2.0, (cur.t + cur.b) / 2.0);
        let (rx, ry) = ((r.l + r.r) / 2.0, (r.t + r.b) / 2.0);
        let (major, minor) = match dir {
            Dir::Down => {
                if ry <= cy + 2.0 || r.b <= cur.b {
                    continue;
                }
                ((r.t - cur.b).max(0.0), gap(cur.l, cur.r, r.l, r.r))
            }
            Dir::Up => {
                if ry >= cy - 2.0 || r.t >= cur.t {
                    continue;
                }
                ((cur.t - r.b).max(0.0), gap(cur.l, cur.r, r.l, r.r))
            }
            Dir::Right => {
                if rx <= cx + 2.0 || r.r <= cur.r {
                    continue;
                }
                ((r.l - cur.r).max(0.0), gap(cur.t, cur.b, r.t, r.b))
            }
            Dir::Left => {
                if rx >= cx - 2.0 || r.l >= cur.l {
                    continue;
                }
                ((cur.l - r.r).max(0.0), gap(cur.t, cur.b, r.t, r.b))
            }
        };
        let score = 13.0 * major * major + minor * minor;
        if score < best_score {
            best_score = score;
            best = Some(t.clone());
        }
        // ◄ / ►: mục cùng hàng (chồng nhau theo chiều dọc) được ưu tiên — hàng nút xuống dòng thì không nhảy sang dòng kia
        if horizontal && r.b > cur.t + 2.0 && r.t < cur.b - 2.0 && score < best_row_score {
            best_row_score = score;
            best_row = Some(t);
        }
    }
    best_row.or(best)
}

/// Mục đầu tiên trong root (theo thứ tự trên → dưới, trái → phải).
pub fn first(root: &gtk::Widget) -> Option<gtk::Widget> {
    let mut v: Vec<(gtk::Widget, Rect)> = targets(root).into_iter().filter_map(|t| bounds(&t, root).map(|r| (t, r))).collect();
    v.sort_by(|a, b| {
        let (ta, tb) = ((a.1.t / 8.0).round(), (b.1.t / 8.0).round());
        ta.partial_cmp(&tb).unwrap_or(std::cmp::Ordering::Equal).then(a.1.l.partial_cmp(&b.1.l).unwrap_or(std::cmp::Ordering::Equal))
    });
    v.into_iter().next().map(|x| x.0)
}

/// w nằm trong root không.
pub fn is_inside(w: &gtk::Widget, root: &gtk::Widget) -> bool {
    w == root || w.is_ancestor(root)
}

/// Chọn mục và cuộn cho thấy (chừa lề để thấy cả tiêu đề nhóm phía trên).
pub fn focus_and_show(w: &gtk::Widget) {
    w.grab_focus();
    show(w);
}

/// Cuộn các vùng cuộn bọc ngoài cho thấy w.
pub fn show(w: &gtk::Widget) {
    let mut cur = w.parent();
    while let Some(p) = cur {
        if let Ok(sw) = p.clone().downcast::<gtk::ScrolledWindow>() {
            if let Some(content) = sw.child().and_then(|c| c.downcast::<gtk::Bin>().ok()).and_then(|v| v.child()) {
                if let Some((x, y)) = w.translate_coordinates(&content, 0, 0) {
                    let (wd, ht) = (w.allocated_width() as f64, w.allocated_height() as f64);
                    if sw.widget_name() == "strip" {
                        reveal(&sw.hadjustment(), x as f64 - 30.0, x as f64 + wd + 30.0);
                    } else {
                        reveal(&sw.vadjustment(), y as f64 - 70.0, y as f64 + ht + 40.0);
                    }
                }
            }
        }
        cur = p.parent();
    }
}

fn reveal(adj: &gtk::Adjustment, a: f64, b: f64) {
    let (v, page) = (adj.value(), adj.page_size());
    let max = (adj.upper() - page).max(adj.lower());
    let target = if a < v {
        a
    } else if b > v + page {
        b - page
    } else {
        return;
    };
    adj.set_value(target.clamp(adj.lower(), max));
}

/// Mục có tên (widget_name) là name trong root — chọn lại ô sau khi dựng lại trang.
pub fn find_named(root: &gtk::Widget, name: &str) -> Option<gtk::Widget> {
    targets(root).into_iter().find(|t| t.widget_name() == name)
}
