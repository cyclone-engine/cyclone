//! Encode domain object (`Snapshot`/`SnapshotDelta`) thành `Packet` rồi gửi
//! qua `Connection`. Tách khỏi `Connection` có chủ đích: `Connection` chỉ
//! biết `Packet`, còn việc biết cách encode `Snapshot`/`SnapshotDelta` là
//! kiến thức riêng của tầng replication.
//!
//! Tên file cố ý không phải `transport.rs`: transport thật (TCP/UDP/QUIC)
//! là việc của `net::Connection`. File này là "Replication Encoder" —
//! Snapshot/Delta -> Packet — nên đặt tên theo đúng việc nó làm.
//!
//! TODO (roadmap): `send_snapshot`/`send_delta` gần như trùng lặp
//! (Wire::from -> Packet::new -> conn.send) — cố ý chưa generic hoá vì chỉ
//! có 2 hàm, tách riêng đọc rõ hơn generic. Nếu số message kind tăng lên
//! đáng kể (Ping/Ack/Login/Disconnect/Input...), cân nhắc 1 trait
//! `IntoPacket`/`PacketEncoder` để giảm số hàm `send_*` lặp lại.

use crate::net::{Connection, ConnectionError};
use crate::protocol::{MessageKind, Packet, WireDelta, WireSnapshot};
use crate::snapshot::{Snapshot, SnapshotDelta};

use super::Outgoing;

pub fn send_snapshot(conn: &mut Connection, snapshot: &Snapshot) -> Result<(), ConnectionError> {
    let wire = WireSnapshot::from(snapshot);
    conn.send(Packet::new(MessageKind::Snapshot, wire.to_bytes()))
}

pub fn send_delta(conn: &mut Connection, delta: &SnapshotDelta) -> Result<(), ConnectionError> {
    let wire = WireDelta::from(delta);
    conn.send(Packet::new(MessageKind::Delta, wire.to_bytes()))
}

/// Nối trực tiếp kết quả `SnapshotSender::next_message` vào `Connection` —
/// chỗ khép vòng pipeline `World::tick() -> SnapshotSender -> socket`.
pub fn send_outgoing(conn: &mut Connection, message: &Outgoing) -> Result<(), ConnectionError> {
    match message {
        Outgoing::Snapshot(snapshot) => send_snapshot(conn, snapshot),
        Outgoing::Delta(delta) => send_delta(conn, delta),
    }
}
