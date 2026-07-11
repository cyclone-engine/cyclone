use std::fmt;
use std::io;

use crate::protocol::ProtocolError;

/// Lỗi ở tầng Connection: hoặc I/O thật (socket đóng, timeout...) hoặc lỗi
/// protocol đã decode được (magic sai, packet quá lớn...). Gộp 2 nguồn lỗi
/// khác bản chất vào 1 enum để `Connection::recv()` dùng `?` xuyên suốt.
#[derive(Debug)]
pub enum ConnectionError {
    Io(io::Error),
    Protocol(ProtocolError),
    /// Peer đóng socket giữa chừng (read trả về 0 byte) — không phải lỗi
    /// I/O theo nghĩa `io::Error`, cần phân biệt để caller biết đây là
    /// disconnect bình thường, không phải sự cố mạng.
    Closed,
}

impl From<io::Error> for ConnectionError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ProtocolError> for ConnectionError {
    fn from(e: ProtocolError) -> Self {
        Self::Protocol(e)
    }
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::Closed => write!(f, "connection closed by peer"),
        }
    }
}

impl std::error::Error for ConnectionError {}
