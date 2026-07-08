use crate::entity::EntityId;
use crate::object::Object;

pub(crate) enum PendingCommand {
    Spawn {
        id: EntityId,
        parent: Option<EntityId>,
        object: Box<dyn Object>,
    },
    Despawn(EntityId),
    // Future:
    // Move
    // Reparent
    // Enable
    // Disable
}

/// Commands chỉ dành cho ghi: spawn / despawn.
/// Không có cách nào để đọc entity khác qua Commands.
///
/// Lệnh được gom vào hàng đợi và áp dụng ở flush() cuối tick — kể cả khi
/// gọi từ trong on_spawn/on_despawn, lệnh mới phát sinh KHÔNG được xử lý
/// ngay trong flush hiện tại, mà đợi flush của tick kế tiếp. Điều này đảm
/// bảo flush() không đệ quy và luôn kết thúc, dù gameplay có lỡ tạo chuỗi
/// spawn/despawn nối tiếp nhau.
pub struct Commands<'a> {
    pub(crate) next_index: &'a mut u32,
    pub(crate) pending: &'a mut Vec<PendingCommand>,
}

impl<'a> Commands<'a> {
    pub fn spawn<T: Object + 'static>(&mut self, obj: T) -> EntityId {
        self.spawn_with_parent(obj, None)
    }

    pub fn spawn_with_parent<T: Object + 'static>(
        &mut self,
        obj: T,
        parent: Option<EntityId>,
    ) -> EntityId {
        let index = *self.next_index;
        *self.next_index += 1;
        let id = EntityId::new(index, 0);
        self.pending.push(PendingCommand::Spawn {
            id,
            parent,
            object: Box::new(obj),
        });
        id
    }

    pub fn despawn(&mut self, id: EntityId) {
        self.pending.push(PendingCommand::Despawn(id));
    }
}
