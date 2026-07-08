use crate::commands::Commands;
use crate::entity::EntityId;
use crate::time::TickId;

/// Dữ liệu thuần được World truyền vào Object mỗi lần gọi.
/// Không phải World handle — không có method để truy vấn entity khác.
pub struct TickInfo {
    pub id: EntityId,
    pub tick: TickId,
}

/// Object chỉ biết xử lý logic của chính nó và gửi yêu cầu qua Commands.
/// Object không được cấp bất kỳ cách nào để chủ động truy cập World.
pub trait Object {
    fn on_spawn(&mut self, _info: &TickInfo, _cmd: &mut Commands) {}
    fn on_tick(&mut self, info: &TickInfo, cmd: &mut Commands);
    fn on_despawn(&mut self, _info: &TickInfo, _cmd: &mut Commands) {}
}
