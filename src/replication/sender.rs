use crate::snapshot::{diff, Snapshot, SnapshotDelta};

/// Kết quả `SnapshotSender` quyết định cần gửi — caller (Connection) mới
/// là nơi thật sự đẩy bytes ra socket. `SnapshotSender` không tự gọi
/// Connection, để test được logic "khi nào full/khi nào delta" mà không
/// cần mở socket, đối xứng với cách `TickScheduler` tách khỏi `Runner`.
///
/// TODO (roadmap): `Outgoing` chỉ nên chứa message thuộc tầng replication
/// (Snapshot/Delta). Khi có message thuộc vòng đời kết nối (Ping/Ack/Hello/
/// Disconnect...), ranh giới đúng là để chúng đi thẳng qua `net::Connection`
/// bằng `Packet` thô — không nhét vào đây — nếu không `Outgoing` sẽ dần
/// trùng lặp với `MessageKind` và mất lý do tồn tại riêng.
pub enum Outgoing {
    Snapshot(Snapshot),
    Delta(SnapshotDelta),
}

/// Tracks the replication state for a single client.
///
/// v0.3 only stores the baseline snapshot to decide between
/// full snapshot and delta transmission.
///
/// Future versions may extend this type with acknowledgement,
/// retransmission and bandwidth state.
///
/// TODO (roadmap): `baseline` clone toàn bộ `Snapshot` mỗi tick
/// (`next_message` + dòng cập nhật baseline). Đúng cho v0.3 (đơn giản, dễ
/// verify), nhưng với nhiều client cùng theo dõi 1 World thì đây là clone
/// lặp lại tốn kém. Sau này cân nhắc `Arc<Snapshot>` (clone chỉ tăng
/// refcount) hoặc tham chiếu qua `SnapshotStorage` đã có sẵn thay vì mỗi
/// `SnapshotSender` tự giữ bản sao riêng.
pub struct SnapshotSender {
    baseline: Option<Snapshot>,
}

impl SnapshotSender {
    pub fn new() -> Self {
        Self { baseline: None }
    }

    pub fn next_message(&mut self, latest: &Snapshot) -> Outgoing {
        let message = match &self.baseline {
            None => Outgoing::Snapshot(latest.clone()),
            Some(baseline) => Outgoing::Delta(diff(Some(baseline), latest)),
        };
        self.baseline = Some(latest.clone());
        message
    }
}

impl Default for SnapshotSender {
    fn default() -> Self {
        Self::new()
    }
}
