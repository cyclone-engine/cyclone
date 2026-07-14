use super::item::SnapshotItem;
use super::snapshot::Snapshot;
use crate::entity::EntityId;
use crate::time::TickId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeltaItem {
    Spawn { item: SnapshotItem },
    Update { id: EntityId, fields: Vec<i32> },
    Remove { id: EntityId },
}

impl DeltaItem {
    /// EntityId mà item này nói tới, bất kể biến thể — dùng bởi `apply()`
    /// để merge tuyến tính `old.items` với `delta.items` mà không cần
    /// match lặp lại 3 nhánh ở mỗi điểm gọi.
    pub fn id(&self) -> EntityId {
        match self {
            DeltaItem::Spawn { item } => item.id,
            DeltaItem::Update { id, .. } => *id,
            DeltaItem::Remove { id } => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDelta {
    pub tick: TickId,
    pub items: Vec<DeltaItem>,
}

/// Delta chỉ là phép toán thuần trên 2 Snapshot — không biết gì về World
/// hay Object. Yêu cầu: `old` và `new` phải đã sort theo EntityId (Snapshot
/// tự làm việc này khi World tạo ra), để merge tuyến tính thay vì cần
/// HashMap.
///
/// `delta.tick` luôn lấy từ `new.tick` — đây là tick mà delta này áp dụng
/// TỚI (baseline + delta = state tại tick này), không phải tick của baseline.
pub fn diff(old: Option<&Snapshot>, new: &Snapshot) -> SnapshotDelta {
    let mut delta = SnapshotDelta {
        tick: new.tick,
        items: Vec::new(),
    };

    let Some(old) = old else {
        for item in &new.items {
            delta.items.push(DeltaItem::Spawn { item: item.clone() });
        }
        return delta;
    };

    let mut oi = 0usize;
    let mut ni = 0usize;

    while oi < old.items.len() && ni < new.items.len() {
        let o = &old.items[oi];
        let n = &new.items[ni];

        if o.id == n.id {
            // Cùng EntityId -> phải cùng loại entity; cùng type_id -> phải
            // cùng schema field. Vi phạm nghĩa là slot bị tái sử dụng mà
            // quên bump generation, hoặc write_snapshot không ổn định.
            debug_assert_eq!(
                o.type_id, n.type_id,
                "EntityId reused without generation bump"
            );
            debug_assert_eq!(
                o.fields.len(),
                n.fields.len(),
                "Snapshot schema mismatch"
            );

            let diffed: Vec<i32> = o
                .fields
                .iter()
                .zip(n.fields.iter())
                .map(|(a, b)| b - a)
                .collect();

            if diffed.iter().any(|&v| v != 0) {
                delta.items.push(DeltaItem::Update {
                    id: n.id,
                    fields: diffed,
                });
            }

            oi += 1;
            ni += 1;
        } else if o.id < n.id {
            delta.items.push(DeltaItem::Remove { id: o.id });
            oi += 1;
        } else {
            delta.items.push(DeltaItem::Spawn { item: n.clone() });
            ni += 1;
        }
    }

    while oi < old.items.len() {
        delta.items.push(DeltaItem::Remove {
            id: old.items[oi].id,
        });
        oi += 1;
    }

    while ni < new.items.len() {
        delta.items.push(DeltaItem::Spawn {
            item: new.items[ni].clone(),
        });
        ni += 1;
    }

    delta
}
