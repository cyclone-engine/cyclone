/// Chỉ cho Object ghi dữ liệu, không cho tạo packet hay biết gì về
/// network/serialization. Đổi cách mã hoá field (varint, bit-packing, nén)
/// sau này không ảnh hưởng tới Object.
#[derive(Default)]
pub struct SnapshotWriter {
    fields: Vec<i32>,
}

impl SnapshotWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_i32(&mut self, value: i32) {
        self.fields.push(value);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub(crate) fn finish(self) -> Vec<i32> {
        self.fields
    }
}
