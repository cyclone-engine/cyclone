use super::error::ProtocolError;

/// Version của wire protocol — không liên quan tới version crate Rust
/// (Cargo.toml). Đây là hợp đồng mạng: SDK ngôn ngữ khác so version này
/// để quyết định có đọc được packet hay không, trước khi đụng tới payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProtocolVersion {
    V1 = 1,
}

impl ProtocolVersion {
    pub const CURRENT: ProtocolVersion = ProtocolVersion::V1;

    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn from_u16(value: u16) -> Result<Self, ProtocolError> {
        match value {
            1 => Ok(ProtocolVersion::V1),
            other => Err(ProtocolError::UnsupportedVersion(other)),
        }
    }
}
