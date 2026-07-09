#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId {
    index: u32,
    generation: u32,
}

impl EntityId {
    // TODO(v0.2+): generation hiện luôn = 0 vì spawn() chưa tái sử dụng slot
    // đã despawn (không có free list). Khi thêm slot reuse, generation phải
    // tăng lên mỗi lần một index được cấp lại để id cũ không còn trỏ đúng
    // entity mới chiếm cùng slot.
    pub(crate) fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
}
