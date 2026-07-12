use crate::commands::Commands;
use crate::entity::EntityId;
use crate::input::InputFrame;
use crate::snapshot::SnapshotWriter;
use crate::time::TickId;

/// Metadata thuần của tick — chỉ thông tin thuộc về bản thân tick, không
/// phải dữ liệu runtime khác (input, prediction, authority...). Cố tình giữ
/// nhỏ để không biến thành "god struct" khi engine có thêm khái niệm mới.
pub struct TickInfo {
    pub id: EntityId,
    pub tick: TickId,
}

/// Dữ liệu thuần được World truyền vào Object mỗi lần gọi. Không phải World
/// handle — không có method để truy vấn entity khác.
///
/// `input` tách khỏi `TickInfo` có chủ đích: input là dữ liệu runtime của
/// tick này (có thể None nếu entity không có kết nối nào điều khiển, hoặc
/// chưa nhận gói nào), khác bản chất với metadata tick. Field runtime khác
/// sau này (prediction, authority, latency...) sẽ thêm vào TickContext,
/// không nhét vào TickInfo.
pub struct TickContext<'a> {
    pub info: TickInfo,
    pub input: Option<&'a InputFrame>,
}

/// Object chỉ biết xử lý logic của chính nó và gửi yêu cầu qua Commands.
/// Object không được cấp bất kỳ cách nào để chủ động truy cập World.
pub trait Object {
    /// Định danh schema wire-format của Object này — độc lập với tên type
    /// Rust. Đây là hợp đồng mạng: đổi tên struct không được phép làm đổi
    /// type_id đã từng gửi lên client. Bắt buộc implement, không có default
    /// hợp lý (mặc định 0 sẽ gây đụng độ âm thầm giữa các loại quên khai báo).
    fn type_id(&self) -> u32;

    fn on_spawn(&mut self, _ctx: &TickContext, _cmd: &mut Commands) {}
    fn on_tick(&mut self, ctx: &TickContext, cmd: &mut Commands);
    fn on_despawn(&mut self, _ctx: &TickContext, _cmd: &mut Commands) {}

    /// Object tự nguyện phơi bày state của mình; World không đọc lén,
    /// Object không biết gì về client/network. Mặc định không ghi gì —
    /// tức là không replicate (ví dụ AIController, Timer, MatchManager).
    ///
    /// Contract: cùng type_id() luôn phải ghi cùng số field, cùng thứ tự,
    /// cùng ý nghĩa ở mọi tick — Delta diff dựa vào vị trí, không phải tên
    /// field.
    fn write_snapshot(&self, _writer: &mut SnapshotWriter) {}
}
