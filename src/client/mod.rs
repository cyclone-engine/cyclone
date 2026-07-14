mod connection;
mod error;
mod input;
mod snapshot_cache;
mod state;

pub use error::ClientError;
pub use state::ConnectionState;

use std::net::ToSocketAddrs;

use crate::snapshot::Snapshot;

use connection::ClientConnection;
use input::{send_input, LatestInput};
use snapshot_cache::SnapshotCache;

/// SDK client chính thức: game chỉ cần `connect()`, `update()` mỗi frame,
/// `snapshot()` để đọc state, `push_input()` để gửi input — không tự xử lý
/// TCP, không tự decode Packet, không tự quản lý baseline/apply Delta.
///
/// Không chạy simulation cục bộ (không `World`/`Object` ở client) — chỉ
/// nhận state từ server và gửi input, không prediction/interpolation/
/// rollback (xem docs/versions/v0.5/design.md).
pub struct GameClient {
    connection: ClientConnection,
    snapshots: SnapshotCache,
    input: LatestInput,
    state: ConnectionState,
}

impl GameClient {
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self, ClientError> {
        let connection = ClientConnection::connect(addr)?;
        Ok(Self {
            connection,
            snapshots: SnapshotCache::new(),
            input: LatestInput::new(),
            state: ConnectionState::Connected,
        })
    }

    /// Gọi mỗi frame: gửi input đang chờ trước, rồi mới xử lý packet đã
    /// tới (cập nhật snapshot cache). Cố tình theo thứ tự này — input gửi
    /// ở frame này tác động tick KẾ TIẾP của server, không phải state vừa
    /// nhận được; gửi trước khi đọc phản ánh đúng quan hệ nhân quả đó, dù
    /// về mặt kỹ thuật thứ tự 2 việc này độc lập (không việc nào phụ thuộc
    /// kết quả của việc kia trong cùng 1 lần `update()`).
    ///
    /// Không nhận `dt` — client không chạy simulation cục bộ nên không cần
    /// biết elapsed time (xem Bước 5, design.md).
    pub fn update(&mut self) -> ConnectionState {
        if self.state == ConnectionState::Disconnected {
            return self.state;
        }

        if let Some(outgoing) = self.input.take()
            && send_input(self.connection.writer(), outgoing).is_err()
        {
            self.state = ConnectionState::Disconnected;
            return self.state;
        }

        for packet in self.connection.drain() {
            self.snapshots.update(packet);
        }
        if self.connection.is_disconnected() {
            self.state = ConnectionState::Disconnected;
        }

        self.state
    }

    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshots.current()
    }

    pub fn push_input(&mut self, bytes: Vec<u8>) {
        self.input.push(bytes);
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }
}
