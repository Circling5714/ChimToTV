//! JSON: đọc một giá trị bắt đầu tại một vị trí trong chuỗi dài (dữ liệu Next.js `self.__next_f.push`) + đọc khoá an toàn
//! như optString / optInt của org.json. Bản Windows: Json.cs (JsonScan, J). Kết quả dùng `serde_json::Value`.

use serde_json::{Map, Value};

pub type Obj = Map<String, Value>;

/// Đọc một giá trị JSON bắt đầu tại `index` (byte) trong `text`, dừng ngay sau giá trị đó. Không đọc được → None.
/// Chuỗi có `\uD83D` lẻ (nửa cặp) thì thay bằng U+FFFD thay vì báo lỗi (serde_json không nhận).
pub fn scan(text: &str, index: usize) -> Option<Value> {
    if index >= text.len() || !text.is_char_boundary(index) {
        return None;
    }
    let mut p = Scan { u: text.as_bytes(), s: text, i: index };
    p.value().ok()
}

/// JSON chuẩn (API) → Value; hỏng → None.
pub fn parse(data: &[u8]) -> Option<Value> {
    serde_json::from_slice(data).ok()
}

pub fn parse_object(data: &[u8]) -> Option<Obj> {
    match parse(data) {
        Some(Value::Object(o)) => Some(o),
        _ => None,
    }
}

struct Scan<'a> {
    u: &'a [u8],
    s: &'a str,
    i: usize,
}

struct Bad;

impl<'a> Scan<'a> {
    fn ws(&mut self) {
        while self.i < self.u.len() && matches!(self.u[self.i], b' ' | b'\n' | b'\r' | b'\t') {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Value, Bad> {
        self.ws();
        if self.i >= self.u.len() {
            return Err(Bad);
        }
        match self.u[self.i] {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(Value::String),
            b't' => self.literal("true").map(|_| Value::Bool(true)),
            b'f' => self.literal("false").map(|_| Value::Bool(false)),
            b'n' => self.literal("null").map(|_| Value::Null),
            _ => self.number(),
        }
    }

    fn literal(&mut self, w: &str) -> Result<(), Bad> {
        if self.u[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(())
        } else {
            Err(Bad)
        }
    }

    fn object(&mut self) -> Result<Value, Bad> {
        self.i += 1;
        let mut o = Map::new();
        self.ws();
        if self.i < self.u.len() && self.u[self.i] == b'}' {
            self.i += 1;
            return Ok(Value::Object(o));
        }
        loop {
            self.ws();
            if self.i >= self.u.len() || self.u[self.i] != b'"' {
                return Err(Bad);
            }
            let key = self.string()?;
            self.ws();
            if self.i >= self.u.len() || self.u[self.i] != b':' {
                return Err(Bad);
            }
            self.i += 1;
            let v = self.value()?;
            o.entry(key).or_insert(v);
            self.ws();
            if self.i >= self.u.len() {
                return Err(Bad);
            }
            match self.u[self.i] {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Ok(Value::Object(o));
                }
                _ => return Err(Bad),
            }
        }
    }

    fn array(&mut self) -> Result<Value, Bad> {
        self.i += 1;
        let mut a = Vec::new();
        self.ws();
        if self.i < self.u.len() && self.u[self.i] == b']' {
            self.i += 1;
            return Ok(Value::Array(a));
        }
        loop {
            a.push(self.value()?);
            self.ws();
            if self.i >= self.u.len() {
                return Err(Bad);
            }
            match self.u[self.i] {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Ok(Value::Array(a));
                }
                _ => return Err(Bad),
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, Bad> {
        if self.i + 4 > self.u.len() {
            return Err(Bad);
        }
        let h = std::str::from_utf8(&self.u[self.i..self.i + 4]).map_err(|_| Bad)?;
        let v = u32::from_str_radix(h, 16).map_err(|_| Bad)?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, Bad> {
        self.i += 1;
        let mut out = String::new();
        let mut run = self.i;
        while self.i < self.u.len() {
            let c = self.u[self.i];
            if c == b'"' {
                out.push_str(&self.s[run..self.i]);
                self.i += 1;
                return Ok(out);
            }
            if c != b'\\' {
                self.i += 1;
                continue;
            }
            out.push_str(&self.s[run..self.i]);
            self.i += 1;
            if self.i >= self.u.len() {
                return Err(Bad);
            }
            let e = self.u[self.i];
            self.i += 1;
            match e {
                b'"' | b'\\' | b'/' => out.push(e as char),
                b'b' => out.push('\u{8}'),
                b'f' => out.push('\u{c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let v = self.hex4()?;
                    if (0xD800..0xDC00).contains(&v) && self.u[self.i..].starts_with(b"\\u") {
                        let save = self.i;
                        self.i += 2;
                        let lo = self.hex4()?;
                        if (0xDC00..0xE000).contains(&lo) {
                            let cp = 0x10000 + ((v - 0xD800) << 10) + (lo - 0xDC00);
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                        } else {
                            out.push('\u{FFFD}');
                            self.i = save;
                        }
                    } else {
                        out.push(char::from_u32(v).unwrap_or('\u{FFFD}'));
                    }
                }
                _ => {
                    // ký tự thoát lạ: giữ nguyên ký tự (kể cả ký tự nhiều byte)
                    let start = self.i - 1;
                    let mut end = self.i;
                    while end < self.u.len() && !self.s.is_char_boundary(end) {
                        end += 1;
                    }
                    out.push_str(&self.s[start..end]);
                    self.i = end;
                }
            }
            run = self.i;
        }
        Err(Bad)
    }

    fn number(&mut self) -> Result<Value, Bad> {
        let start = self.i;
        let mut dbl = false;
        while self.i < self.u.len() {
            match self.u[self.i] {
                b'0'..=b'9' | b'-' | b'+' => self.i += 1,
                b'.' | b'e' | b'E' => {
                    dbl = true;
                    self.i += 1
                }
                _ => break,
            }
        }
        if self.i == start {
            return Err(Bad);
        }
        let s = &self.s[start..self.i];
        if !dbl {
            if let Ok(n) = s.parse::<i64>() {
                return Ok(Value::from(n));
            }
        }
        s.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).ok_or(Bad)
    }
}

// ---- Đọc khoá an toàn (J của bản Windows) ----

/// Chuỗi (đã bỏ khoảng trắng) của khoá; thiếu / không phải chuỗi → "".
pub fn str(o: Option<&Obj>, key: &str) -> String {
    match o.and_then(|o| o.get(key)) {
        Some(Value::String(s)) => s.trim().to_string(),
        _ => String::new(),
    }
}

pub fn int(o: Option<&Obj>, key: &str, def: i64) -> i64 {
    match o.and_then(|o| o.get(key)) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|d| d as i64)).unwrap_or(def),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(def),
        _ => def,
    }
}

pub fn num(o: Option<&Obj>, key: &str) -> Option<f64> {
    match o.and_then(|o| o.get(key)) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
}

pub fn bool(o: Option<&Obj>, key: &str, def: bool) -> bool {
    match o.and_then(|o| o.get(key)) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true" || (s != "false" && def),
        _ => def,
    }
}

pub fn obj<'a>(o: Option<&'a Obj>, key: &str) -> Option<&'a Obj> {
    o.and_then(|o| o.get(key)).and_then(|v| v.as_object())
}

pub fn arr<'a>(o: Option<&'a Obj>, key: &str) -> Option<&'a Vec<Value>> {
    o.and_then(|o| o.get(key)).and_then(|v| v.as_array())
}

/// Các phần tử là đối tượng của mảng.
pub fn objects(a: Option<&Vec<Value>>) -> Vec<&Obj> {
    a.map(|a| a.iter().filter_map(|v| v.as_object()).collect()).unwrap_or_default()
}

/// Giá trị dạng chữ của khoá bất kỳ kiểu (số → chữ số), không có → "".
pub fn any(o: Option<&Obj>, key: &str) -> String {
    match o.and_then(|o| o.get(key)) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else {
                n.as_f64().map(|d| fmt_f64(d)).unwrap_or_default()
            }
        }
        Some(Value::Bool(b)) => if *b { "True" } else { "False" }.to_string(),
        Some(v) => v.to_string(),
    }
}

/// Số thực ra chữ như C# InvariantCulture ("2.5", "3").
pub fn fmt_f64(d: f64) -> String {
    if d.fract() == 0.0 && d.abs() < 1e15 {
        format!("{}", d as i64)
    } else {
        let s = format!("{d}");
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_stops_after_value() {
        let t = r#"xx {"a":[1,2.5,"xA\n"],"b":{"c":null,"d":true}} rest"#;
        let v = scan(t, 3).unwrap();
        assert_eq!(v["a"][0], 1);
        assert_eq!(v["a"][1], 2.5);
        assert_eq!(v["a"][2], "xA\n");
        assert_eq!(v["b"]["d"], true);
        assert!(scan(t, 0).is_none());
        // nửa cặp lẻ → U+FFFD, cặp đủ → ký tự
        assert_eq!(scan(r#""a\ud83db""#, 0).unwrap(), "a\u{FFFD}b");
        assert_eq!(scan(r#""😀""#, 0).unwrap(), "😀");
        assert_eq!(scan(r#""Đấu Phá""#, 0).unwrap(), "Đấu Phá");
    }

    #[test]
    fn safe_getters() {
        let o = parse_object(br#"{"s":" x ","n":"12","d":3.0,"b":"true","o":{"k":1},"a":[{"x":1},2]}"#).unwrap();
        assert_eq!(str(Some(&o), "s"), "x");
        assert_eq!(int(Some(&o), "n", 0), 12);
        assert_eq!(int(Some(&o), "d", 0), 3);
        assert!(bool(Some(&o), "b", false));
        assert_eq!(int(obj(Some(&o), "o"), "k", 0), 1);
        assert_eq!(objects(arr(Some(&o), "a")).len(), 1);
        assert_eq!(any(Some(&o), "d"), "3");
        assert_eq!(any(Some(&o), "zz"), "");
    }
}
