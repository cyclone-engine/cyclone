use std::io;
use std::net::{TcpStream, ToSocketAddrs};

use super::connection::{Connection, ConnectionReader, ConnectionWriter};

/// Đối xứng với `Server`: không có state (không cần sống lâu sau khi kết
/// nối xong — `ConnectionReader`/`ConnectionWriter` trả về mới là thứ
/// người dùng giữ), chỉ là 1 hàm dựng kết nối đặt tên cho rõ vai trò và dễ
/// tìm cạnh `Server::bind()`/`accept()` trong docs.
pub struct Client;

impl Client {
    /// `connect()` trả thẳng `(ConnectionReader, ConnectionWriter)` đã
    /// tách sẵn — cùng nguyên tắc "Opinionated API" với `Server::accept()`:
    /// không có cách nào lấy ra 1 connection 2 chiều chưa tách để phải tự
    /// quyết định có tách hay không.
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<(ConnectionReader, ConnectionWriter)> {
        let stream = TcpStream::connect(addr)?;
        Connection::new(stream).into_split()
    }
}
