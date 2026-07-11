use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use cyclone::protocol::{NetEntityId, WireDecode, WireSnapshot, WireSnapshotItem};

/// Cùng công thức xác định với tools/generate_vectors.rs, nhưng tách riêng
/// vì benchmark cần thêm mốc 5000 không nằm trong golden vector đã commit
/// (golden chỉ giữ những quy mô cần làm chuẩn cho SDK khác, không phải mọi
/// mốc benchmark).
fn build_snapshot(entity_count: u32) -> WireSnapshot {
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

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_encode");
    for &count in &[100u32, 1_000, 5_000, 10_000] {
        let snapshot = build_snapshot(count);
        let size = snapshot.to_bytes().len();
        eprintln!(
            "[info] {count} entities -> {size} bytes ({:.1} bytes/entity)",
            size as f64 / count as f64
        );

        group.bench_with_input(BenchmarkId::from_parameter(count), &snapshot, |b, snap| {
            b.iter(|| black_box(snap).to_bytes());
        });
    }
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot_decode");
    for &count in &[100u32, 1_000, 5_000, 10_000] {
        let bytes = build_snapshot(count).to_bytes();

        group.bench_with_input(BenchmarkId::from_parameter(count), &bytes, |b, bytes| {
            b.iter(|| WireSnapshot::decode(&mut black_box(bytes.as_slice())).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
