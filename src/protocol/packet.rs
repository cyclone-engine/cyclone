use super::codec::{read_u16, read_u32, read_u8, write_u16, write_u32, write_u8};
use super::error::ProtocolError;
use super::version::ProtocolVersion;

/// 2 byte ASCII cố định, lưu dạng raw bytes (không phải số nguyên) để
/// không phụ thuộc endian khi nhận diện packet — đây là byte đầu tiên
/// mọi SDK đọc trước khi biết gì khác về payload.
pub const MAGIC: [u8; 2] = *b"CY";

/// Trần trên cho payload_len đọc từ header — chặn ngay khi biết con số,
/// trước khi động tới cursor/split_at. Game realtime không cần packet
/// >1MB/tick; số lớn hơn gần như chắc chắn là input sai hoặc cố tình.
pub const MAX_PACKET_SIZE: usize = 1 << 20;

/// Số byte cố định của header: magic(2) + version(2) + kind(1) +
/// payload_len(4). Public để `net::PacketReader` biết cần tối thiểu bao
/// nhiêu byte trước khi gọi được `peek_payload_len` — cùng một nguồn sự
/// thật với `decode()`, không hard-code lại offset ở module khác.
pub const HEADER_LEN: usize = 9;

/// Loại payload bên trong packet. Thêm loại mới phải append cuối, không đổi
/// số đã gán (đó là hợp đồng đã gửi ra ngoài).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageKind {
    Snapshot = 0,
    Delta = 1,
    /// v0.4: client → server, mang WireInput.
    Input = 2,
}

impl MessageKind {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(value: u8) -> Result<Self, ProtocolError> {
        match value {
            0 => Ok(MessageKind::Snapshot),
            1 => Ok(MessageKind::Delta),
            2 => Ok(MessageKind::Input),
            other => Err(ProtocolError::InvalidMessageKind(other)),
        }
    }
}

/// Lớp framing ngoài cùng, bọc payload đã encode sẵn (WireSnapshot/WireDelta).
/// Packet không biết gì về nội dung payload — chỉ biết kind + độ dài, để
/// SDK có thể tách frame khỏi network stream trước khi parse nội dung.
#[derive(Debug)]
pub struct Packet {
    pub version: ProtocolVersion,
    pub kind: MessageKind,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(kind: MessageKind, payload: Vec<u8>) -> Self {
        Self {
            version: ProtocolVersion::CURRENT,
            kind,
            payload,
        }
    }

    /// Header: magic(2) + version(2 LE) + kind(1) + payload_len(4 LE), sau
    /// đó payload thô. Tổng header cố định 9 byte.
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.payload.len() <= MAX_PACKET_SIZE,
            "Packet payload is {} bytes, exceeds MAX_PACKET_SIZE {MAX_PACKET_SIZE}",
            self.payload.len()
        );
        let mut buf = Vec::with_capacity(9 + self.payload.len());
        buf.extend_from_slice(&MAGIC);
        write_u16(&mut buf, self.version.as_u16());
        write_u8(&mut buf, self.kind.as_u8());
        write_u32(&mut buf, self.payload.len() as u32);
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Decode toàn bộ packet từ một buffer đã nhận đủ (framing ở tầng
    /// network — TCP length-prefixed hay tương đương — là việc của v0.3,
    /// không phải của hàm này). Trả lỗi thay vì panic nếu magic sai,
    /// version không hỗ trợ, hoặc payload_len không khớp phần còn lại.
    pub fn decode(mut buf: &[u8]) -> Result<Self, ProtocolError> {
        let cursor = &mut buf;

        let magic: [u8; 2] = [read_u8(cursor)?, read_u8(cursor)?];
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }

        let version = ProtocolVersion::from_u16(read_u16(cursor)?)?;
        let kind = MessageKind::from_u8(read_u8(cursor)?)?;
        let payload_len = read_u32(cursor)? as usize;
        if payload_len > MAX_PACKET_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                len: payload_len,
                max: MAX_PACKET_SIZE,
            });
        }

        if cursor.len() < payload_len {
            return Err(ProtocolError::UnexpectedEof);
        }
        let (payload, rest) = cursor.split_at(payload_len);
        if !rest.is_empty() {
            return Err(ProtocolError::TrailingBytes);
        }

        Ok(Packet {
            version,
            kind,
            payload: payload.to_vec(),
        })
    }

    /// Đọc payload_len từ header đã nhận đủ HEADER_LEN byte, không đụng gì
    /// tới payload. Dùng bởi `net::PacketReader` để biết cần đợi thêm bao
    /// nhiêu byte từ socket trước khi có đủ dữ liệu gọi `decode()` trên 1
    /// packet trọn vẹn — không validate version ở đây, `decode()` đầy đủ
    /// sẽ làm việc đó khi packet đã ghép xong.
    pub fn peek_payload_len(header: &[u8]) -> Result<usize, ProtocolError> {
        debug_assert!(header.len() >= HEADER_LEN);
        let cursor = &mut &header[..HEADER_LEN];

        let magic: [u8; 2] = [read_u8(cursor)?, read_u8(cursor)?];
        if magic != MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }
        let _version = read_u16(cursor)?;
        let _kind = read_u8(cursor)?;
        let payload_len = read_u32(cursor)? as usize;

        if payload_len > MAX_PACKET_SIZE {
            return Err(ProtocolError::PayloadTooLarge {
                len: payload_len,
                max: MAX_PACKET_SIZE,
            });
        }
        Ok(payload_len)
    }
}
