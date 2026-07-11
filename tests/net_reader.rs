//! PacketReader phải ghép đúng packet dù byte đến rời rạc qua nhiều lần
//! feed() — mô phỏng TCP phân mảnh, không cần mở socket thật.

use cyclone::net::PacketReader;
use cyclone::protocol::{MessageKind, Packet};

fn sample_packet_bytes() -> Vec<u8> {
    Packet::new(MessageKind::Snapshot, vec![1, 2, 3, 4, 5]).encode()
}

#[test]
fn reads_packet_fed_in_one_piece() {
    let mut reader = PacketReader::new();
    reader.feed(&sample_packet_bytes());

    let packet = reader.try_read_packet().unwrap().unwrap();
    assert_eq!(packet.kind, MessageKind::Snapshot);
    assert_eq!(packet.payload, vec![1, 2, 3, 4, 5]);
}

#[test]
fn reads_packet_fed_byte_by_byte() {
    let bytes = sample_packet_bytes();
    let mut reader = PacketReader::new();

    for i in 0..bytes.len() - 1 {
        reader.feed(&bytes[i..i + 1]);
        assert!(
            reader.try_read_packet().unwrap().is_none(),
            "chưa đủ byte nhưng đã trả về packet ở vị trí {i}"
        );
    }
    reader.feed(&bytes[bytes.len() - 1..]);

    let packet = reader.try_read_packet().unwrap().unwrap();
    assert_eq!(packet.payload, vec![1, 2, 3, 4, 5]);
}

#[test]
fn reads_two_packets_fed_concatenated_in_arbitrary_chunks() {
    let mut bytes = sample_packet_bytes();
    bytes.extend(Packet::new(MessageKind::Delta, vec![9, 9]).encode());

    let mut reader = PacketReader::new();
    // Chia thành các mảnh 3 byte bất kỳ, không theo ranh giới packet.
    for chunk in bytes.chunks(3) {
        reader.feed(chunk);
    }

    let first = reader.try_read_packet().unwrap().unwrap();
    assert_eq!(first.kind, MessageKind::Snapshot);
    assert_eq!(first.payload, vec![1, 2, 3, 4, 5]);

    let second = reader.try_read_packet().unwrap().unwrap();
    assert_eq!(second.kind, MessageKind::Delta);
    assert_eq!(second.payload, vec![9, 9]);

    assert!(reader.try_read_packet().unwrap().is_none());
}

#[test]
fn rejects_wrong_magic_immediately_without_waiting_for_full_payload() {
    let mut bytes = sample_packet_bytes();
    bytes[0] = b'X';

    let mut reader = PacketReader::new();
    // Chỉ feed đúng phần header — lỗi phải lộ ra ngay, không cần đợi payload.
    reader.feed(&bytes[..9]);

    assert!(reader.try_read_packet().is_err());
}
