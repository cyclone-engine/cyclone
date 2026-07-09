use super::error::ProtocolError;

/// Trần trên cho mọi count đọc từ buffer chưa tin tưởng (item_count,
/// field_count...) trước khi dùng làm Vec::with_capacity. Không phải giới
/// hạn nghiệp vụ (game thật không cần 10k field/entity) — đây thuần là
/// chặn DoS bằng con số bịa ra trong 4 byte input.
pub const MAX_ITEM_COUNT: u32 = 10_000;
pub const MAX_FIELD_COUNT: u16 = 10_000;

/// Mọi số nguyên trên wire đều little-endian, tường minh qua to/from_le_bytes
/// — không dùng transmute, không phụ thuộc target endianness hay layout
/// struct của Rust. Đây là quyết định khoá cứng cho protocol version V1;
/// đổi endianness là breaking change, phải bump ProtocolVersion.
pub trait WireEncode {
    fn encode(&self, buf: &mut Vec<u8>);
}

/// Decode nhận buffer dạng con trỏ cursor (`&mut &[u8]`) — mỗi lần đọc xong
/// một trường thì tự rút ngắn slice, để composite type gọi decode() của
/// trường con liên tiếp mà không cần track offset thủ công.
pub trait WireDecode: Sized {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError>;
}

pub fn write_u8(buf: &mut Vec<u8>, value: u8) {
    buf.push(value);
}

pub fn write_u16(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

pub fn write_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

pub fn write_u64(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_le_bytes());
}

pub fn write_i32(buf: &mut Vec<u8>, value: i32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8], ProtocolError> {
    if buf.len() < n {
        return Err(ProtocolError::UnexpectedEof);
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

pub fn read_u8(buf: &mut &[u8]) -> Result<u8, ProtocolError> {
    Ok(take(buf, 1)?[0])
}

pub fn read_u16(buf: &mut &[u8]) -> Result<u16, ProtocolError> {
    let bytes: [u8; 2] = take(buf, 2)?.try_into().unwrap();
    Ok(u16::from_le_bytes(bytes))
}

pub fn read_u32(buf: &mut &[u8]) -> Result<u32, ProtocolError> {
    let bytes: [u8; 4] = take(buf, 4)?.try_into().unwrap();
    Ok(u32::from_le_bytes(bytes))
}

pub fn read_u64(buf: &mut &[u8]) -> Result<u64, ProtocolError> {
    let bytes: [u8; 8] = take(buf, 8)?.try_into().unwrap();
    Ok(u64::from_le_bytes(bytes))
}

pub fn read_i32(buf: &mut &[u8]) -> Result<i32, ProtocolError> {
    let bytes: [u8; 4] = take(buf, 4)?.try_into().unwrap();
    Ok(i32::from_le_bytes(bytes))
}
