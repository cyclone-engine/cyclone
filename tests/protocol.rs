//! Golden binary test: so khớp với các file .bin CỐ ĐỊNH đã commit trong
//! tests/protocol_vectors/, không phải round-trip tự-so-với-chính-nó.
//! Một SDK ngôn ngữ khác implement parser cho cùng file .bin này phải ra
//! đúng cùng kết quả — đó là ý nghĩa của "golden".

use cyclone::protocol::{
    MessageKind, NetEntityId, Packet, ProtocolError, WireDecode, WireDelta, WireDeltaItem,
    WireEncode, WireSnapshot, WireSnapshotItem, MAGIC, MAX_PACKET_SIZE,
};

fn vector(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/protocol_vectors/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("missing test vector {path}: {e}"))
}

fn encode<T: WireEncode>(value: &T) -> Vec<u8> {
    let mut buf = Vec::new();
    value.encode(&mut buf);
    buf
}

#[test]
fn snapshot_empty_matches_golden_bytes() {
    let bytes = vector("snapshot_empty.bin");

    let decoded = WireSnapshot::decode(&mut bytes.as_slice()).unwrap();
    assert_eq!(decoded.tick, 0);
    assert!(decoded.items.is_empty());

    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn snapshot_player_matches_golden_bytes() {
    let bytes = vector("snapshot_player.bin");

    let decoded = WireSnapshot::decode(&mut bytes.as_slice()).unwrap();
    assert_eq!(decoded.tick, 100);
    assert_eq!(
        decoded.items,
        vec![WireSnapshotItem {
            id: NetEntityId {
                index: 1,
                generation: 0
            },
            type_id: 1,
            fields: vec![10, 20, 100],
        }]
    );

    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn delta_spawn_matches_golden_bytes() {
    let bytes = vector("delta_spawn.bin");

    let decoded = WireDelta::decode(&mut bytes.as_slice()).unwrap();
    assert_eq!(
        decoded.items,
        vec![WireDeltaItem::Spawn(WireSnapshotItem {
            id: NetEntityId {
                index: 2,
                generation: 0
            },
            type_id: 2,
            fields: vec![50, 60, 5],
        })]
    );

    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn delta_update_matches_golden_bytes() {
    let bytes = vector("delta_update.bin");

    let decoded = WireDelta::decode(&mut bytes.as_slice()).unwrap();
    assert_eq!(
        decoded.items,
        vec![WireDeltaItem::Update {
            id: NetEntityId {
                index: 1,
                generation: 0
            },
            fields: vec![5, -3],
        }]
    );

    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn delta_remove_matches_golden_bytes() {
    let bytes = vector("delta_remove.bin");

    let decoded = WireDelta::decode(&mut bytes.as_slice()).unwrap();
    assert_eq!(
        decoded.items,
        vec![WireDeltaItem::Remove {
            id: NetEntityId {
                index: 3,
                generation: 0
            },
        }]
    );

    assert_eq!(encode(&decoded), bytes);
}

#[test]
fn packet_snapshot_empty_matches_golden_bytes() {
    let bytes = vector("packet_snapshot_empty.bin");

    let packet = Packet::decode(&bytes).unwrap();
    assert_eq!(packet.kind, MessageKind::Snapshot);
    assert_eq!(packet.payload, vector("snapshot_empty.bin"));

    assert_eq!(Packet::new(packet.kind, packet.payload).encode(), bytes);
}

#[test]
fn packet_rejects_wrong_magic() {
    let mut bytes = vector("packet_snapshot_empty.bin");
    bytes[0] = b'X';
    assert!(Packet::decode(&bytes).is_err());
}

#[test]
fn packet_rejects_truncated_buffer() {
    let bytes = vector("packet_snapshot_empty.bin");
    assert!(Packet::decode(&bytes[..bytes.len() - 1]).is_err());
}

/// Buffer chỉ 12 byte thật, nhưng item_count tự xưng 4 tỷ. Không được để
/// decode chạy tới Vec::with_capacity(4_000_000_000) trước khi phát hiện
/// buffer không đủ dữ liệu — phải reject ngay khi đọc xong item_count.
#[test]
fn snapshot_decode_rejects_huge_item_count_before_allocating() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0u64.to_le_bytes()); // tick
    bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // item_count = 4 tỷ

    let err = WireSnapshot::decode(&mut bytes.as_slice()).unwrap_err();
    assert_eq!(
        err,
        ProtocolError::TooManyItems {
            count: u32::MAX,
            max: 10_000,
        }
    );
}

#[test]
fn delta_decode_rejects_huge_item_count_before_allocating() {
    let bytes = u32::MAX.to_le_bytes(); // item_count = 4 tỷ, không có payload

    let err = WireDelta::decode(&mut bytes.as_slice()).unwrap_err();
    assert_eq!(
        err,
        ProtocolError::TooManyItems {
            count: u32::MAX,
            max: 10_000,
        }
    );
}

#[test]
fn snapshot_item_decode_rejects_huge_field_count_before_allocating() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u32.to_le_bytes()); // entity index
    bytes.extend_from_slice(&0u32.to_le_bytes()); // generation
    bytes.extend_from_slice(&1u32.to_le_bytes()); // type_id
    bytes.extend_from_slice(&u16::MAX.to_le_bytes()); // field_count = 65535

    let err = WireSnapshotItem::decode(&mut bytes.as_slice()).unwrap_err();
    assert_eq!(
        err,
        ProtocolError::TooManyFields {
            count: u16::MAX,
            max: 10_000,
        }
    );
}

/// Header tự xưng payload_len = 4GB nhưng không có payload thật đi kèm.
/// Phải reject ngay khi đọc xong payload_len, trước khi split_at/to_vec
/// theo con số này.
#[test]
fn packet_decode_rejects_huge_payload_len_before_allocating() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&1u16.to_le_bytes()); // version
    bytes.push(0); // kind = Snapshot
    bytes.extend_from_slice(&u32::MAX.to_le_bytes()); // payload_len = 4 tỷ

    let err = Packet::decode(&bytes).unwrap_err();
    assert_eq!(
        err,
        ProtocolError::PayloadTooLarge {
            len: u32::MAX as usize,
            max: MAX_PACKET_SIZE,
        }
    );
}

#[test]
#[should_panic(expected = "exceeds MAX_FIELD_COUNT")]
fn snapshot_item_encode_panics_on_internal_field_overflow_bug() {
    let item = WireSnapshotItem {
        id: NetEntityId {
            index: 0,
            generation: 0,
        },
        type_id: 0,
        fields: vec![0; 10_001],
    };
    let mut buf = Vec::new();
    item.encode(&mut buf);
}
