//! Connection thật qua TCP loopback (127.0.0.1) — không mock stream, để
//! xác nhận PacketReader + Connection hoạt động đúng với 1 socket kernel
//! thật, kể cả khi TCP giao dữ liệu rời rạc.

use std::thread;

use cyclone::net::{Connection, Server};
use cyclone::protocol::MessageKind;
use cyclone::snapshot::{Snapshot, SnapshotDelta, SnapshotItem};
use cyclone::{Object, TickId, World};

struct Player;
impl Object for Player {
    fn type_id(&self) -> u32 {
        1
    }
    fn on_tick(&mut self, _info: &cyclone::TickInfo, _cmd: &mut cyclone::Commands) {}
}

#[test]
fn sends_snapshot_and_receives_it_on_the_other_end() {
    let server = Server::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let mut world = World::new();
    let id = world.spawn(Player);
    let snapshot = Snapshot {
        tick: TickId(7),
        items: vec![SnapshotItem {
            id,
            type_id: 1,
            fields: vec![10, 20, 30],
        }],
    };

    let client_thread = thread::spawn(move || {
        let stream = std::net::TcpStream::connect(addr).unwrap();
        let mut conn = Connection::new(stream);
        cyclone::replication::send_snapshot(&mut conn, &snapshot).unwrap();
    });

    let mut server_conn = server.accept().unwrap();
    let packet = server_conn.recv().unwrap();
    client_thread.join().unwrap();

    assert_eq!(packet.kind, MessageKind::Snapshot);
    let wire = cyclone::protocol::WireSnapshot::from_bytes(&packet.payload).unwrap();
    assert_eq!(wire.tick, 7);
    assert_eq!(wire.items.len(), 1);
    assert_eq!(wire.items[0].fields, vec![10, 20, 30]);
}

#[test]
fn sends_delta_and_receives_it_on_the_other_end() {
    let server = Server::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let mut world = World::new();
    let id = world.spawn(Player);
    let delta = SnapshotDelta {
        items: vec![cyclone::snapshot::DeltaItem::Update {
            id,
            fields: vec![1, -1],
        }],
    };

    let client_thread = thread::spawn(move || {
        let stream = std::net::TcpStream::connect(addr).unwrap();
        let mut conn = Connection::new(stream);
        cyclone::replication::send_delta(&mut conn, &delta).unwrap();
    });

    let mut server_conn = server.accept().unwrap();
    let packet = server_conn.recv().unwrap();
    client_thread.join().unwrap();

    assert_eq!(packet.kind, MessageKind::Delta);
    let wire = cyclone::protocol::WireDelta::from_bytes(&packet.payload).unwrap();
    assert_eq!(wire.items.len(), 1);
}

#[test]
fn send_outgoing_dispatches_snapshot_then_delta_from_sender() {
    let server = Server::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let mut world = World::new();
    let id = world.spawn(Player);
    let tick0 = Snapshot {
        tick: TickId(0),
        items: vec![SnapshotItem {
            id,
            type_id: 1,
            fields: vec![0],
        }],
    };
    let tick1 = Snapshot {
        tick: TickId(1),
        items: vec![SnapshotItem {
            id,
            type_id: 1,
            fields: vec![5],
        }],
    };

    let client_thread = thread::spawn(move || {
        let stream = std::net::TcpStream::connect(addr).unwrap();
        let mut conn = Connection::new(stream);
        let mut sender = cyclone::replication::SnapshotSender::new();

        let first = sender.next_message(&tick0);
        cyclone::replication::send_outgoing(&mut conn, &first).unwrap();

        let second = sender.next_message(&tick1);
        cyclone::replication::send_outgoing(&mut conn, &second).unwrap();
    });

    let mut server_conn = server.accept().unwrap();
    let first_packet = server_conn.recv().unwrap();
    let second_packet = server_conn.recv().unwrap();
    client_thread.join().unwrap();

    assert_eq!(first_packet.kind, MessageKind::Snapshot);
    assert_eq!(second_packet.kind, MessageKind::Delta);
}

#[test]
fn recv_reports_closed_when_peer_disconnects_without_sending() {
    let server = Server::bind("127.0.0.1:0").unwrap();
    let addr = server.local_addr().unwrap();

    let client_thread = thread::spawn(move || {
        let _stream = std::net::TcpStream::connect(addr).unwrap();
        // Đóng ngay, không gửi gì.
    });

    let mut server_conn = server.accept().unwrap();
    client_thread.join().unwrap();

    assert!(matches!(
        server_conn.recv(),
        Err(cyclone::net::ConnectionError::Closed)
    ));
}
