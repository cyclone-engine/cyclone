use std::io::{Read, Write};
use std::net::TcpStream;

use crate::protocol::Packet;

use super::error::ConnectionError;
use super::reader::PacketReader;

/// Chỗ duy nhất chạm TcpStream thật trong crate — giống vai trò `Runner`
/// với wall-clock: mọi thứ khác (Packet, PacketReader) đều thuần logic,
/// test được không cần socket.
///
/// Connection chỉ biết `Packet` — không biết `Snapshot`/`WireSnapshot`/
/// `SnapshotDelta`. Encode domain object thành Packet là việc của tầng gọi
/// nó (xem `replication::send_snapshot`/`send_delta`), để Connection không
/// phình to khi sau này thêm compression/encryption/UDP/QUIC/websocket —
/// mỗi thứ đó chỉ cần đổi cách `send`/`recv` chạm socket, không đụng gì
/// tới việc ai biết encode Snapshot.
///
/// TODO: nếu thêm transport khác ngoài TCP (UnixSocket, MemoryTransport,
/// QUIC...), tách 1 trait `Transport { read, write }` và generic hoá
/// Connection theo nó, thay vì gắn cứng TcpStream như hiện tại.
pub struct Connection {
    stream: TcpStream,
    reader: PacketReader,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            reader: PacketReader::new(),
        }
    }

    // TODO (roadmap): packet.encode() allocate 1 Vec<u8> mới mỗi lần gọi
    // send(). Đúng cho v0.3; nếu sau này cần giảm allocation ở throughput
    // cao, thêm Packet::encode_into(&mut Vec<u8>) để Connection tái dùng 1
    // buffer riêng thay vì cấp phát lại mỗi packet.
    pub fn send(&mut self, packet: Packet) -> Result<(), ConnectionError> {
        self.stream.write_all(&packet.encode())?;
        Ok(())
    }

    /// Blocking: đọc thêm từ socket cho tới khi `PacketReader` ghép đủ 1
    /// Packet hoàn chỉnh. TCP có thể giao gói tin rời rạc thành nhiều lần
    /// `read()` — vòng lặp này là nơi duy nhất chịu trách nhiệm chờ đợi đó,
    /// tách khỏi logic ghép byte thuần trong `PacketReader`.
    pub fn recv(&mut self) -> Result<Packet, ConnectionError> {
        loop {
            if let Some(packet) = self.reader.try_read_packet()? {
                return Ok(packet);
            }

            let mut buf = [0u8; 4096];
            let n = self.stream.read(&mut buf)?;
            if n == 0 {
                return Err(ConnectionError::Closed);
            }
            self.reader.feed(&buf[..n]);
        }
    }
}
