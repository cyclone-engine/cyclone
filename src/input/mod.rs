use crate::entity::EntityId;
use crate::time::TickId;

/// Input thô của 1 entity tại 1 tick. `bytes` không có cấu trúc do engine
/// định nghĩa — gameplay tự encode/decode (xem `protocol::WireInput`, cùng
/// nguyên tắc "opaque payload" như `SnapshotWriter`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFrame {
    pub tick: TickId,
    pub bytes: Vec<u8>,
}

/// Input của mọi entity có kết nối, cho đúng 1 tick — World.tick() dùng để
/// dựng TickContext.input cho từng entity.
///
/// Bất biến bắt buộc: `items` luôn sort theo EntityId, cùng lý do
/// `Snapshot.items` — World.tick() merge 2 con trỏ song song với
/// `entries` (đã tự nhiên theo thứ tự EntityId vì entries index trực tiếp
/// bằng EntityId.index()), không tra cứu ngẫu nhiên. Vi phạm bất biến này
/// (items không sort) khiến merge bỏ sót hoặc gán nhầm input cho entity
/// khác — không có gì phát hiện lỗi này ngoài việc luôn dựng InputBatch qua
/// `from_items()`, không tự ý thêm field `items` public.
pub struct InputBatch {
    items: Vec<(EntityId, InputFrame)>,
}

impl InputBatch {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Sort nội bộ theo EntityId — caller không cần tự sort trước khi gọi.
    pub fn from_items(mut items: Vec<(EntityId, InputFrame)>) -> Self {
        items.sort_by_key(|(id, _)| *id);
        Self { items }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &(EntityId, InputFrame)> {
        self.items.iter()
    }
}

impl Default for InputBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Trạng thái input "hiện hành" của 1 client — không phải hàng đợi FIFO.
/// GameServer đọc `get()` mỗi tick mà không rút phần tử ra; nếu tick hiện
/// tại chưa có gói input mới, `get()` vẫn trả về input gần nhất đã nhận.
/// Server không bao giờ chờ đủ input mới tick — 1 client lag không được
/// phép treo cả server.
#[derive(Default)]
pub struct CurrentInput {
    latest: Option<InputFrame>,
}

impl CurrentInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, frame: InputFrame) {
        self.latest = Some(frame);
    }

    pub fn get(&self) -> Option<&InputFrame> {
        self.latest.as_ref()
    }
}
