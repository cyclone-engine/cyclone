use super::item::SnapshotItem;
use crate::time::TickId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub tick: TickId,
    pub items: Vec<SnapshotItem>,
}

impl Snapshot {
    pub fn new(tick: TickId) -> Self {
        Self {
            tick,
            items: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, item: SnapshotItem) {
        self.items.push(item);
    }

    /// Bắt buộc gọi trước khi diff: Delta merge tuyến tính O(n) dựa vào
    /// items đã sắp xếp theo EntityId, không cần HashMap.
    pub(crate) fn sort(&mut self) {
        self.items.sort_by_key(|item| item.id);
    }
}
