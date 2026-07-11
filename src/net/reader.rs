use crate::protocol::{Packet, ProtocolError, HEADER_LEN};

/// Ghép các mảnh byte rời rạc từ TCP thành Packet hoàn chỉnh. Thuần logic —
/// không chạm socket — nên test được bằng cách feed() từng mảnh nhỏ để mô
/// phỏng TCP phân mảnh gói tin, mà không cần mở kết nối thật. Đối xứng với
/// cách `TickScheduler` tách khỏi `Runner`: state machine thuần, I/O thật
/// (đọc socket) là việc của `Connection`.
///
/// TODO: `try_read_packet()` chỉ trả về 1 packet mỗi lần gọi — nếu 1 lần
/// `feed()` chứa nhiều packet liền nhau (TCP gộp nhiều gói kernel-side),
/// caller phải tự `while let Some(p) = reader.try_read_packet()? { ... }`.
/// Đúng nhưng hơi thô; cân nhắc bọc thành iterator sau này.
#[derive(Default)]
pub struct PacketReader {
    buf: Vec<u8>,
}

impl PacketReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// `Ok(None)` nghĩa là buffer chưa đủ 1 packet — không phải lỗi, gọi
    /// `feed()` thêm rồi thử lại. Lỗi thật (magic sai, payload vượt
    /// MAX_PACKET_SIZE...) trả `Err` ngay, không đợi đủ byte mới báo.
    pub fn try_read_packet(&mut self) -> Result<Option<Packet>, ProtocolError> {
        if self.buf.len() < HEADER_LEN {
            return Ok(None);
        }

        let payload_len = Packet::peek_payload_len(&self.buf[..HEADER_LEN])?;
        let total = HEADER_LEN + payload_len;
        if self.buf.len() < total {
            return Ok(None);
        }

        // TODO (roadmap): drain(..total).collect() allocate 1 Vec<u8> mới
        // cho mỗi packet — đúng cho v0.3, nhưng ở throughput cao (nhiều
        // client, nhiều packet/giây) đây là copy lặp lại tốn kém. Sau này
        // cân nhắc buffer kiểu BytesMut/VecDeque hoặc split_off() để tránh
        // realloc mỗi lần.
        let frame: Vec<u8> = self.buf.drain(..total).collect();
        Packet::decode(&frame).map(Some)
    }
}
