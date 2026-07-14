use std::fmt;
use std::io;

/// Lỗi ở tầng GameClient. Chỉ có 1 biến thể ở v0.5 — decode lỗi bị nuốt có
/// chủ đích bên trong `SnapshotCache` (xem docs/versions/v0.5/design.md),
/// không lộ ra thành `ClientError` mà game phải xử lý mỗi frame.
#[derive(Debug)]
pub enum ClientError {
    Io(io::Error),
}

impl From<io::Error> for ClientError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ClientError {}
