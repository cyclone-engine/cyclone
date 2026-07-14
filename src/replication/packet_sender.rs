//! Encode domain object (`Snapshot`/`SnapshotDelta`) thành `Packet` rồi gửi
//! qua `ConnectionWriter`. Tách khỏi `net` có chủ đích: `ConnectionWriter`
//! chỉ biết `Packet`, còn việc biết cách encode `Snapshot`/`SnapshotDelta`
//! là kiến thức riêng của tầng replication.
//!
//! Tên file cố ý không phải `transport.rs`: transport thật (TCP/UDP/QUIC)
//! là việc của `net`. File này là "Replication Encoder" — Snapshot/Delta ->
//! Packet — nên đặt tên theo đúng việc nó làm.
//!
//! Chỉ 1 bộ hàm, nhận `&mut ConnectionWriter` — kể từ khi `Server::accept()`/
//! `Client::connect()` luôn trả `(ConnectionReader, ConnectionWriter)` đã
//! tách sẵn (Opinionated API, xem `net::Connection`), không còn ai giữ 1
//! connection 2 chiều chưa tách để cần bản `_conn` riêng nữa.
//!
//! TODO (roadmap): `send_snapshot`/`send_delta` gần như trùng lặp
//! (Wire::from -> Packet::new -> conn.send) — cố ý chưa generic hoá, chỉ 2
//! hàm nhỏ. Nếu số message kind tăng đáng kể (Ping/Ack/Login/Disconnect...),
//! cân nhắc 1 trait `IntoPacket`/`PacketEncoder`.
//!
//! v0.5: `send_input` đã chuyển sang `client::input` (không còn ở đây nữa).
//! Input không phải "replication" — không có quyết định full/delta gì cả,
//! chỉ là encode+gửi 1 chiều client -> server, đúng vai trò của tầng
//! `client`, đối xứng với việc `client::snapshot_cache` tự decode
//! Snapshot/Delta thẳng chứ không qua `replication` (Quyết định #6, v0.4).

use crate::net::{ConnectionError, ConnectionWriter};
use crate::protocol::{MessageKind, Packet, WireDelta, WireSnapshot};
use crate::snapshot::{Snapshot, SnapshotDelta};

use super::Outgoing;

pub fn send_snapshot(conn: &mut ConnectionWriter, snapshot: &Snapshot) -> Result<(), ConnectionError> {
    let wire = WireSnapshot::from(snapshot);
    conn.send(Packet::new(MessageKind::Snapshot, wire.to_bytes()))
}

pub fn send_delta(conn: &mut ConnectionWriter, delta: &SnapshotDelta) -> Result<(), ConnectionError> {
    let wire = WireDelta::from(delta);
    conn.send(Packet::new(MessageKind::Delta, wire.to_bytes()))
}

/// Nối trực tiếp kết quả `SnapshotSender::next_message` vào `ConnectionWriter`
/// — chỗ khép vòng pipeline `World::tick() -> SnapshotSender -> socket`.
pub fn send_outgoing(conn: &mut ConnectionWriter, message: &Outgoing) -> Result<(), ConnectionError> {
    match message {
        Outgoing::Snapshot(snapshot) => send_snapshot(conn, snapshot),
        Outgoing::Delta(delta) => send_delta(conn, delta),
    }
}
