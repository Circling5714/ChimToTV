//! Vượt chặn nhà mạng (tách gói bắt tay TLS theo tên trang) — bản Windows: Unblock.cs. TẠM: chưa chuyển, chuyển thẳng.

use crate::net::Conn;
use tokio::net::TcpStream;

/// Mức đang bật (0 = tắt).
pub fn level() -> i64 {
    crate::prefs::unblock_level()
}

/// Cổng nội bộ: xử lý gói đầu tiên của bên gọi (tách gói bắt tay / tự lên Mức 2) rồi trả kết nối để chuyển tiếp tiếp.
/// None = bỏ kết nối.
pub async fn bridge_first(_client: &mut TcpStream, conn: Conn, _host: &str, _port: u16) -> Option<Conn> {
    Some(conn)
}
