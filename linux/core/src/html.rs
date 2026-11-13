//! Đọc HTML (crate scraper), cùng cách gọi như jsoup / SwiftSoup / AngleSharp của các bản kia: q, q1, txt, at, abs.
//! Bản Windows: Html.cs.

use scraper::{ElementRef, Html, Selector};
use url::Url;

/// Tài liệu kèm địa chỉ gốc (để đổi đường dẫn tương đối ra tuyệt đối).
pub struct Doc {
    pub html: Html,
    pub base: Option<Url>,
    pub location: String,
}

/// Một phần tử trong tài liệu (mang theo địa chỉ gốc của tài liệu).
#[derive(Clone, Copy)]
pub struct El<'a> {
    pub e: ElementRef<'a>,
    base: Option<&'a Url>,
}

fn sel(css: &str) -> Option<Selector> {
    match Selector::parse(css) {
        Ok(s) => Some(s),
        Err(e) => {
            crate::log::d("html", &format!("bộ chọn CSS sai «{css}»: {e:?}"));
            None
        }
    }
}

impl Doc {
    pub fn parse(html: &str, base_uri: &str) -> Doc {
        let doc = Html::parse_document(html);
        let mut base = Url::parse(base_uri).ok();
        // <base href> của trang (ít gặp) — như jsoup
        if let Some(s) = sel("base[href]") {
            if let Some(b) = doc.select(&s).next().and_then(|e| e.value().attr("href")) {
                if let Some(u) = base.as_ref().and_then(|x| x.join(b.trim()).ok()).or_else(|| Url::parse(b.trim()).ok()) {
                    base = Some(u);
                }
            }
        }
        Doc { html: doc, base, location: base_uri.to_string() }
    }

    pub fn root(&self) -> El<'_> {
        El { e: self.html.root_element(), base: self.base.as_ref() }
    }

    pub fn q(&self, css: &str) -> Vec<El<'_>> {
        self.root().q(css)
    }

    pub fn q1(&self, css: &str) -> Option<El<'_>> {
        self.root().q1(css)
    }

    /// Toàn bộ HTML của tài liệu.
    pub fn html_text(&self) -> String {
        self.html.root_element().html()
    }

    /// Đổi đường dẫn (tương đối) ra tuyệt đối theo địa chỉ trang.
    pub fn abs_url(&self, v: &str) -> String {
        abs_with(self.base.as_ref(), v)
    }
}

fn abs_with(base: Option<&Url>, v: &str) -> String {
    let v = v.trim();
    match base {
        Some(b) => b.join(v).map(|u| u.to_string()).unwrap_or_default(),
        None => {
            if v.starts_with("http") {
                v.to_string()
            } else {
                String::new()
            }
        }
    }
}

impl<'a> El<'a> {
    pub fn q(&self, css: &str) -> Vec<El<'a>> {
        match sel(css) {
            Some(s) => self.e.select(&s).map(|e| El { e, base: self.base }).collect(),
            None => Vec::new(),
        }
    }

    pub fn q1(&self, css: &str) -> Option<El<'a>> {
        let s = sel(css)?;
        self.e.select(&s).next().map(|e| El { e, base: self.base })
    }

    /// Chữ của phần tử, khoảng trắng gộp lại (như text() của jsoup; bỏ chữ trong script / style).
    pub fn txt(&self) -> String {
        let mut out = String::new();
        for node in self.e.descendants() {
            if let Some(t) = node.value().as_text() {
                let skip = node.ancestors().take_while(|a| a.id() != self.e.id()).any(|a| {
                    a.value().as_element().map(|el| matches!(el.name(), "script" | "style")).unwrap_or(false)
                });
                if !skip {
                    out.push_str(t);
                }
            }
        }
        crate::util::collapse_spaces(&out)
    }

    /// Chữ thô (kể cả script) — nội dung thẻ script.
    pub fn script_text(&self) -> String {
        self.e.text().collect()
    }

    /// Thuộc tính ("" nếu không có).
    pub fn at(&self, key: &str) -> String {
        self.e.value().attr(key).unwrap_or("").to_string()
    }

    pub fn has(&self, key: &str) -> bool {
        self.e.value().attr(key).is_some()
    }

    /// Đường dẫn tuyệt đối của thuộc tính ("" nếu không có thuộc tính; rỗng → địa chỉ trang, như absUrl của jsoup).
    pub fn abs(&self, key: &str) -> String {
        match self.e.value().attr(key) {
            None => String::new(),
            Some(v) => abs_with(self.base, v),
        }
    }

    pub fn tag(&self) -> &str {
        self.e.value().name()
    }

    pub fn class_has(&self, c: &str) -> bool {
        self.e.value().classes().any(|x| x == c)
    }

    pub fn inner_html(&self) -> String {
        self.e.inner_html()
    }

    pub fn outer_html(&self) -> String {
        self.e.html()
    }

    /// Phần tử cha gần nhất là phần tử (không có → None).
    pub fn parent(&self) -> Option<El<'a>> {
        self.e.parent().and_then(ElementRef::wrap).map(|e| El { e, base: self.base })
    }

    /// Các phần tử con trực tiếp.
    pub fn children(&self) -> Vec<El<'a>> {
        self.e.children().filter_map(ElementRef::wrap).map(|e| El { e, base: self.base }).collect()
    }

    /// Phần tử anh em kế tiếp.
    pub fn next_sibling(&self) -> Option<El<'a>> {
        let mut n = self.e.next_sibling();
        while let Some(x) = n {
            if let Some(e) = ElementRef::wrap(x) {
                return Some(El { e, base: self.base });
            }
            n = x.next_sibling();
        }
        None
    }

    /// Tổ tiên gần nhất khớp bộ chọn (kể cả chính nó) — như closest() của DOM.
    pub fn closest(&self, css: &str) -> Option<El<'a>> {
        let s = sel(css)?;
        let mut cur = Some(self.e);
        while let Some(e) = cur {
            if s.matches(&e) {
                return Some(El { e, base: self.base });
            }
            cur = e.parent().and_then(ElementRef::wrap);
        }
        None
    }

    /// Phần tử có khớp bộ chọn không.
    pub fn is(&self, css: &str) -> bool {
        sel(css).map(|s| s.matches(&self.e)).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_like_jsoup() {
        let d = Doc::parse(
            r#"<html><body><div class="a b"><a href="/phim/x">  Đấu   Phá <script>var x=1</script></a><img data-src="//cdn.x/p.jpg"></div></body></html>"#,
            "https://yanhh3d.ee/trang/2",
        );
        let a = d.q1("div.a a").unwrap();
        assert_eq!(a.txt(), "Đấu Phá");
        assert_eq!(a.abs("href"), "https://yanhh3d.ee/phim/x");
        assert_eq!(d.q1("img").unwrap().abs("data-src"), "https://cdn.x/p.jpg");
        assert_eq!(d.q1("img").unwrap().abs("src"), "");
        assert!(d.q1("div").unwrap().class_has("b"));
        assert_eq!(a.closest("div").unwrap().tag(), "div");
        assert!(d.q("bad[[").is_empty());
    }
}
