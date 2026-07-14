use std::net::ToSocketAddrs;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::net::{Client, ConnectionWriter};
use crate::protocol::Packet;

use super::error::ClientError;

/// Kết nối tới `GameServer`: nửa đọc sống trên thread riêng — blocking
/// `recv()` như `net::ConnectionReader` vốn có, đẩy `Packet` thô qua channel
/// (không tự decode ở đây; decode Snapshot/Delta là kiến thức riêng của
/// `SnapshotCache`, giống cách thread đọc của `ClientSession` không tự diễn
/// giải input). Nửa ghi giữ lại để `GameClient::update()` dùng gửi input —
/// khi `ClientConnection` (và `writer` bên trong) bị drop, socket tự
/// shutdown, thread đọc thoát vòng lặp (xem `Drop for ConnectionWriter`
/// trong `net/connection.rs`), không leak.
///
/// TODO (roadmap, production): TCP không phát hiện peer chết ngay — nếu
/// server crash mà không đóng socket đúng cách (rút dây mạng, kill -9...),
/// `is_disconnected()` có thể không true trong một khoảng thời gian dài
/// (phụ thuộc OS keepalive timeout, thường rất lâu). Cần heartbeat/ping-pong
/// + timeout ở tầng ứng dụng để phát hiện nhanh hơn. Chưa cần cho demo/v0.5.
pub(crate) struct ClientConnection {
    writer: ConnectionWriter,
    incoming: Receiver<Packet>,
    disconnected: bool,
}

impl ClientConnection {
    pub(crate) fn connect(addr: impl ToSocketAddrs) -> Result<Self, ClientError> {
        let (mut reader, writer) = Client::connect(addr)?;
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            while let Ok(packet) = reader.recv() {
                if tx.send(packet).is_err() {
                    break; // GameClient đã bị drop, dừng đọc.
                }
            }
        });

        Ok(Self {
            writer,
            incoming: rx,
            disconnected: false,
        })
    }

    /// Rút hết packet đang chờ (không chặn), trả về theo đúng thứ tự nhận.
    /// Trả `Vec<Packet>` sở hữu thay vì callback — caller lặp bằng `for`
    /// bình thường, không phải mượn 2 field của `GameClient` cùng lúc qua
    /// closure (dễ vướng borrow hơn khi SDK mở rộng thêm việc phải làm khi
    /// nhận packet, ví dụ emit event sau này).
    ///
    /// Nếu connection đã đóng (channel disconnected), cờ nội bộ được set —
    /// đọc qua `is_disconnected()` sau khi gọi hàm này.
    pub(crate) fn drain(&mut self) -> Vec<Packet> {
        let mut packets = Vec::new();
        loop {
            match self.incoming.try_recv() {
                Ok(packet) => packets.push(packet),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.disconnected = true;
                    break;
                }
            }
        }
        packets
    }

    pub(crate) fn is_disconnected(&self) -> bool {
        self.disconnected
    }

    pub(crate) fn writer(&mut self) -> &mut ConnectionWriter {
        &mut self.writer
    }
}
