use super::codec::{read_bytes, write_bytes, MAX_INPUT_BYTES};
use super::error::ProtocolError;
use super::{WireDecode, WireEncode};

/// Wire representation của input 1 client. `bytes` là dữ liệu thô — engine
/// không giả định cấu trúc bên trong, gameplay tự encode/decode (để tương
/// thích nhiều gameplay backend: Object thuần, ECS, Bevy, Hecs... trên
/// cùng 1 wire protocol mà không phải sửa engine).
///
/// Không mang tick: client v0.5 không chạy simulation cục bộ, không có
/// đồng hồ tick nào đáng tin để gửi (xem docs/versions/v0.5/design.md,
/// Bước 7 — Clock Sync để dành cho sau). Gửi 1 con số tự đếm cục bộ trông
/// giống tick server sẽ gây nhầm lẫn thật khi sau này có lag compensation/
/// input ack/replay/rollback — thà không có còn hơn có nhưng sai hệ quy
/// chiếu. Tick "đáng tin" mà InputFrame mang ở phía server là tick CỦA
/// SERVER tại lúc nhận (xem `session::ClientSession::drain_input`), không
/// phải trường nào trên wire này.
///
/// Không mang player/entity id: chiều client → server luôn đi trên 1
/// Connection cụ thể, và ClientSession ở phía server đã biết Connection đó
/// ứng với EntityId nào — nhét id vào đây chỉ là dữ liệu thừa trên wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireInput {
    pub bytes: Vec<u8>,
}

impl WireEncode for WireInput {
    fn encode(&self, buf: &mut Vec<u8>) {
        write_bytes(buf, &self.bytes);
    }
}

impl WireDecode for WireInput {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError> {
        let bytes = read_bytes(buf, MAX_INPUT_BYTES)?;
        Ok(Self { bytes })
    }
}

impl WireInput {
    /// Tiện ích cho SDK/caller không muốn tự quản lý Vec<u8> buffer — cùng
    /// khuôn với WireSnapshot::to_bytes()/from_bytes().
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode(&mut buf);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut cursor = bytes;
        let value = Self::decode(&mut cursor)?;
        if !cursor.is_empty() {
            return Err(ProtocolError::TrailingBytes);
        }
        Ok(value)
    }
}
