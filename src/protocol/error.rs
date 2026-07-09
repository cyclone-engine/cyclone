use std::fmt;

/// Lỗi decode: input đến từ network/file ngoài, không được tin tưởng.
/// Không bao giờ panic/index-out-of-bounds khi buffer thiếu byte hay
/// header sai — luôn trả Result để caller (Rust server hay SDK khác) xử lý.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    UnexpectedEof,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidMessageKind(u8),
    InvalidDeltaItemKind(u8),
    TrailingBytes,
    /// item_count đọc từ buffer vượt giới hạn cho phép, TRƯỚC khi biết
    /// buffer có thực sự chứa từng đó item hay không — chặn ở đây để
    /// không Vec::with_capacity theo một số attacker tự xưng (vd 4 tỷ).
    TooManyItems { count: u32, max: u32 },
    TooManyFields { count: u16, max: u16 },
    /// payload_len trong header vượt MAX_PACKET_SIZE — chặn trước khi
    /// split_at/to_vec theo con số này, phòng khi tương lai buffer đến từ
    /// TCP stream đã đủ lớn để không bị UnexpectedEof chặn giùm.
    PayloadTooLarge { len: usize, max: usize },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "unexpected end of buffer"),
            Self::InvalidMagic => write!(f, "invalid packet magic"),
            Self::UnsupportedVersion(v) => write!(f, "unsupported protocol version: {v}"),
            Self::InvalidMessageKind(k) => write!(f, "invalid message kind: {k}"),
            Self::InvalidDeltaItemKind(k) => write!(f, "invalid delta item kind: {k}"),
            Self::TrailingBytes => write!(f, "trailing bytes after decoding"),
            Self::TooManyItems { count, max } => {
                write!(f, "item_count {count} exceeds max {max}")
            }
            Self::TooManyFields { count, max } => {
                write!(f, "field_count {count} exceeds max {max}")
            }
            Self::PayloadTooLarge { len, max } => {
                write!(f, "payload_len {len} exceeds max {max}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}
