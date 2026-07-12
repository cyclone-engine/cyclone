use std::io::{self, Read, Write};
use std::net::TcpStream;

use crate::protocol::Packet;

use super::error::ConnectionError;
use super::reader::PacketReader;

/// Chi tiết nội bộ của `net` — KHÔNG export ra ngoài crate (xem
/// `net::mod`). Cyclone chủ trương "Opinionated API": chỉ có đúng 1 cách
/// lấy 1 connection — `Server::accept()` hoặc `Client::connect()` — và cả
/// hai đều trả thẳng `(ConnectionReader, ConnectionWriter)` đã tách sẵn.
/// Không có API nào trả về `Connection` 2 chiều để người dùng phải tự hỏi
/// "lúc nào nên tách, lúc nào không" — Cyclone quyết định giùm: luôn tách.
///
/// `Connection` chỉ tồn tại như bước trung gian bên trong `Server`/`Client`
/// để dùng chung logic `into_split()`, không có lý do gì để lộ ra public.
pub(crate) struct Connection {
    stream: TcpStream,
    reader: PacketReader,
}

impl Connection {
    pub(crate) fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            reader: PacketReader::new(),
        }
    }

    /// Tách 1 Connection 2 chiều thành 1 nửa đọc + 1 nửa ghi có thể sống
    /// trên 2 thread khác nhau (session giữ ConnectionWriter, 1 thread
    /// riêng giữ ConnectionReader và gọi recv() blocking).
    ///
    /// Đặt tên `into_split` (không phải `split`) để nói rõ semantics: hàm
    /// tiêu thụ `self`, sau lệnh này không còn Connection nào cả, chỉ còn
    /// 2 nửa độc lập — cùng quy ước với `TcpStream::into_split()` bên
    /// tokio.
    ///
    /// Có chủ đích KHÔNG dùng `Connection::try_clone()` kiểu clone nguyên
    /// Connection: clone như vậy vẫn còn `recv()`/`send()` sống trên cả 2
    /// bản, không có gì ngăn 2 thread cùng gọi recv() trên chung 1 socket
    /// và làm hỏng thứ tự byte trên wire. Tách hẳn thành 2 kiểu khác nhau
    /// (ConnectionReader không có send(), ConnectionWriter không có recv())
    /// khiến lỗi này không thể xảy ra — sai là không compile được, không
    /// phải "nhớ đừng gọi nhầm".
    pub(crate) fn into_split(self) -> io::Result<(ConnectionReader, ConnectionWriter)> {
        let write_stream = self.stream.try_clone()?;
        Ok((
            ConnectionReader {
                stream: self.stream,
                reader: self.reader,
            },
            ConnectionWriter {
                stream: write_stream,
            },
        ))
    }
}

/// Nửa đọc — thứ duy nhất người dùng crate thấy để nhận dữ liệu từ 1
/// connection TCP. Không có `send()`.
pub struct ConnectionReader {
    stream: TcpStream,
    reader: PacketReader,
}

impl ConnectionReader {
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

/// Nửa ghi — thứ duy nhất người dùng crate thấy để gửi dữ liệu qua 1
/// connection TCP. Không có `recv()`.
pub struct ConnectionWriter {
    stream: TcpStream,
}

impl ConnectionWriter {
    // TODO (roadmap): packet.encode() allocate 1 Vec<u8> mới mỗi lần gọi
    // send(). Đúng cho v0.3/v0.4; nếu sau này cần giảm allocation ở
    // throughput cao, thêm Packet::encode_into(&mut Vec<u8>) để tái dùng 1
    // buffer riêng thay vì cấp phát lại mỗi packet.
    pub fn send(&mut self, packet: Packet) -> Result<(), ConnectionError> {
        self.stream.write_all(&packet.encode())?;
        Ok(())
    }
}
