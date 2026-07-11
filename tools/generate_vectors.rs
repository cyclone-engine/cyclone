//! Sinh golden vector NGOÀI vòng test. Golden file là 1 phần của wire
//! protocol specification — không phải fixture tự sinh trong lúc chạy
//! `cargo test`. Chạy thủ công khi cần cập nhật (protocol version bump,
//! hoặc đổi format có chủ đích), rồi commit các file .bin sinh ra:
//!
//!     cargo run --bin generate_vectors
//!
//! Binary này KHÔNG được gọi từ `cargo test` — nếu golden bị regenerate
//! ngay trong lúc test chạy, test sẽ luôn đúng với chính nó và mất hết ý
//! nghĩa "chuẩn vàng" để các SDK ngôn ngữ khác đối chiếu.

use cyclone::protocol::{NetEntityId, WireSnapshot, WireSnapshotItem};

/// Sinh xong phải giống hệt nhau ở mọi lần chạy — golden vector không được
/// phụ thuộc RNG hay thời gian hệ thống.
fn deterministic_snapshot(entity_count: u32) -> WireSnapshot {
    let items = (0..entity_count)
        .map(|i| WireSnapshotItem {
            id: NetEntityId {
                index: i,
                generation: 0,
            },
            type_id: (i % 3) + 1,
            fields: vec![i as i32, i as i32 * 2, -(i as i32)],
        })
        .collect();

    WireSnapshot { tick: 1000, items }
}

fn main() {
    let out_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/protocol_vectors");

    for &count in &[100u32, 1_000, 10_000] {
        let snapshot = deterministic_snapshot(count);
        let bytes = snapshot.to_bytes();
        let path = format!("{out_dir}/snapshot_{count}.bin");
        std::fs::write(&path, &bytes).expect("write golden vector");
        println!("wrote {path} ({} bytes, {count} entities)", bytes.len());
    }
}
