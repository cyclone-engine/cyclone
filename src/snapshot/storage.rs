use std::collections::VecDeque;

use super::snapshot::Snapshot;
use crate::time::TickId;

/// Lịch sử snapshot theo tick — chưa gắn với client/viewer cụ thể (đó là
/// việc của v0.3 khi có kết nối thật). Dùng làm baseline cho Delta khi cần
/// so với một tick bất kỳ trong lịch sử gần đây.
pub struct SnapshotStorage {
    history: VecDeque<Snapshot>,
    capacity: usize,
}

impl SnapshotStorage {
    pub fn new(capacity: usize) -> Self {
        Self {
            history: VecDeque::new(),
            capacity,
        }
    }

    pub fn push(&mut self, snapshot: Snapshot) {
        if self.history.len() == self.capacity {
            self.history.pop_front();
        }
        self.history.push_back(snapshot);
    }

    pub fn get(&self, tick: TickId) -> Option<&Snapshot> {
        self.history.iter().find(|s| s.tick == tick)
    }

    pub fn latest(&self) -> Option<&Snapshot> {
        self.history.back()
    }
}
