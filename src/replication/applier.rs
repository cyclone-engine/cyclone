use crate::snapshot::{DeltaItem, Snapshot, SnapshotDelta, SnapshotItem};

/// Phép toán ngược của `snapshot::diff`: (old, delta) -> new. Tick lấy
/// thẳng từ `delta.tick` (v0.5: SnapshotDelta/WireDelta tự mang tick thật
/// trên wire, không còn phải để caller tự đếm và đoán — xem docs/versions/v0.5).
///
/// Yêu cầu (bất biến, không kiểm tra runtime ngoài debug_assert): `old.items`
/// đã sort theo EntityId — bất biến Snapshot tự giữ — và `delta.items` đã ở
/// thứ tự EntityId tăng dần — bất biến mà `diff()` luôn đảm bảo vì nó quét
/// tuyến tính 2 danh sách đã sort. Nhờ vậy `apply()` merge tuyến tính O(n),
/// đối xứng hoàn toàn với cách `diff()` được viết, không cần HashMap.
///
/// TODO (roadmap, chưa phải bug ở v0.5): `apply()` vẫn giả định delta luôn
/// đến đúng thứ tự, không mất gói — packet reorder/loss/retransmit thật vẫn
/// chưa được xử lý, chỉ riêng việc "tick nào" đã hết phải đoán.
///
/// TODO (roadmap): phía nhận hiện phải tự làm "Packet -> decode -> apply ->
/// cập nhật baseline" thủ công (xem `tests/replication_applier.rs`). v0.5
/// gói pipeline này lại trong `client::SnapshotCache`, đối xứng với
/// `SnapshotSender` ở phía gửi.
pub fn apply(old: &Snapshot, delta: &SnapshotDelta) -> Snapshot {
    let tick = delta.tick;
    let mut items: Vec<SnapshotItem> = Vec::with_capacity(old.items.len());
    let mut oi = 0usize;
    let mut di = 0usize;

    while oi < old.items.len() && di < delta.items.len() {
        let old_id = old.items[oi].id;
        let delta_id = delta.items[di].id();

        if old_id < delta_id {
            // Entity không đổi ở tick này — giữ nguyên.
            items.push(old.items[oi].clone());
            oi += 1;
        } else if old_id == delta_id {
            match &delta.items[di] {
                DeltaItem::Update { fields, .. } => {
                    let mut item = old.items[oi].clone();
                    debug_assert_eq!(
                        item.fields.len(),
                        fields.len(),
                        "Delta field count mismatch với Snapshot cũ — schema đổi giữa 2 tick"
                    );
                    for (field, diff) in item.fields.iter_mut().zip(fields) {
                        *field += diff;
                    }
                    items.push(item);
                }
                DeltaItem::Remove { .. } => {
                    // Không push — entity bị loại khỏi snapshot mới.
                }
                DeltaItem::Spawn { .. } => {
                    debug_assert!(false, "Spawn cho EntityId đã tồn tại trong old snapshot");
                }
            }
            oi += 1;
            di += 1;
        } else {
            match &delta.items[di] {
                DeltaItem::Spawn { item } => items.push(item.clone()),
                _ => debug_assert!(
                    false,
                    "Update/Remove cho EntityId không có trong old snapshot"
                ),
            }
            di += 1;
        }
    }

    items.extend(old.items[oi..].iter().cloned());

    for item in &delta.items[di..] {
        match item {
            DeltaItem::Spawn { item } => items.push(item.clone()),
            _ => debug_assert!(
                false,
                "Update/Remove cho EntityId không có trong old snapshot"
            ),
        }
    }

    // Merge trên 2 danh sách đã sort luôn cho kết quả sort — không cần
    // gọi lại Snapshot::sort().
    let mut result = Snapshot::new(tick);
    for item in items {
        result.push(item);
    }
    result
}
