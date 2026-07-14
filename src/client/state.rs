/// Trạng thái kết nối, game đọc qua `GameClient::state()`/kết quả trả về
/// của `update()`.
///
/// Không có `Connecting`: `GameClient::connect()` là hàm blocking, khi trả
/// `Ok` thì đã ở `Connected` luôn — không có đường nào dẫn tới 1 trạng thái
/// "đang kết nối" quan sát được ở v0.5. Thêm lại khi có API connect bất
/// đồng bộ thật sự cần nó (đừng thêm biến thể cho state không ai tới được).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}
