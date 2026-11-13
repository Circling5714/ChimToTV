//! Mã hồ sơ NextDNS của app (DNS mã hoá, xem src/doh.rs): đọc keys/nextdns.txt ở gốc dự án lúc dựng và đóng vào tệp chạy
//! (biến CBTV_NEXTDNS_ID, đọc bằng option_env!) — như bản Windows đóng tệp này vào .exe. Tệp không có trên git: không có thì
//! bản dựng bỏ NextDNS (DNS mã hoá đi thẳng Cloudflare → Google). Không in mã ra đâu cả.

use std::path::Path;

fn main() {
    let file = Path::new("../../keys/nextdns.txt");
    let dir = Path::new("../../keys");
    if file.exists() {
        println!("cargo:rerun-if-changed=../../keys/nextdns.txt");
        if let Ok(text) = std::fs::read_to_string(file) {
            let id = text.trim();
            // Chỉ chữ thường + số (như bản Windows) — tệp sai thì bỏ NextDNS
            if !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
                println!("cargo:rustc-env=CBTV_NEXTDNS_ID={id}");
            }
        }
    } else if dir.is_dir() {
        // Có thư mục keys mà chưa có tệp: thêm tệp vào là dựng lại
        println!("cargo:rerun-if-changed=../../keys");
    } else {
        // Không có thư mục keys (repo mới lấy về): chỉ chạy lại khi sửa build.rs — thêm keys/nextdns.txt sau thì
        // `touch linux/core/build.rs` (hoặc cargo clean) để đóng mã vào
        println!("cargo:rerun-if-changed=build.rs");
    }
}
