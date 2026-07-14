use crate::net::{ConnectionError, ConnectionWriter};
use crate::protocol::{MessageKind, Packet, WireInput};

/// Input đã sẵn sàng gửi đi — tách khỏi `LatestInput` (trạng thái chờ gửi
/// bên trong) để `take()` trả ra dữ liệu thuần, còn việc gửi (encode +
/// `conn.send()`) nằm ở `send_input()` riêng, không lẫn vào nhau. Không
/// mang tick — xem `protocol::WireInput` vì sao.
pub(crate) struct OutgoingInput {
    pub bytes: Vec<u8>,
}

/// Chiều client → server: gửi input thô. Cố tình KHÔNG đặt trong
/// `replication` — Input không phải "replication" (không có quyết định
/// full/delta gì cả, chỉ encode+gửi 1 chiều), đây là kiến thức riêng của
/// tầng client, đối xứng với việc `client::snapshot_cache` tự decode
/// Snapshot/Delta thẳng thay vì qua `replication` khi nhận.
pub(crate) fn send_input(
    conn: &mut ConnectionWriter,
    input: OutgoingInput,
) -> Result<(), ConnectionError> {
    let wire = WireInput { bytes: input.bytes };
    conn.send(Packet::new(MessageKind::Input, wire.to_bytes()))
}

/// Input "chờ gửi" của client — chỉ giữ **đúng 1 giá trị mới nhất**, không
/// phải hàng đợi. Đặt tên `LatestInput` (không phải `ClientInput` chung
/// chung) để tên tự nói lên giới hạn này.
///
/// `push()` GHI ĐÈ giá trị đang chờ. Đúng nếu input là trạng thái liên tục
/// (ví dụ "đang giữ phím di chuyển", chỉ cần biết trạng thái mới nhất) —
/// SAI nếu input là các lệnh rời rạc cần giữ hết. Ví dụ cùng 1 frame gọi
/// `push(shoot)` rồi `push(move_right)` trước khi `update()` kịp gửi:
/// `shoot` bị mất, chỉ `move_right` được gửi đi. Nếu gameplay cần loại
/// input đó (bắn/nhảy/dash — mỗi lệnh phải tới server, không được ghi đè
/// lẫn nhau), `LatestInput` KHÔNG phù hợp — cần 1 kiểu khác kiểu hàng đợi
/// thật (ví dụ `ClientInputQueue`), chưa có ở v0.5, thêm khi có nhu cầu
/// thật thay vì đoán trước hình dạng.
///
/// `take()` CONSUME — mỗi lần `push()` chỉ được gửi đúng 1 lần ở `update()`
/// kế tiếp, không lặp lại vô hạn (khác `CurrentInput` phía server — server
/// cần trạng thái hiện hành vì nó tick liên tục và phải luôn có input để
/// dùng, không chờ; client gửi command rời rạc, không consume sẽ khiến
/// input cũ bị gửi lại mãi kể cả sau khi game đã "nhả phím").
pub(crate) struct LatestInput {
    pending: Option<Vec<u8>>,
}

impl LatestInput {
    pub(crate) fn new() -> Self {
        Self { pending: None }
    }

    pub(crate) fn push(&mut self, bytes: Vec<u8>) {
        self.pending = Some(bytes);
    }

    pub(crate) fn take(&mut self) -> Option<OutgoingInput> {
        self.pending.take().map(|bytes| OutgoingInput { bytes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_is_sent_exactly_once_not_resent_every_take() {
        let mut input = LatestInput::new();
        input.push(vec![1]);

        assert_eq!(input.take().map(|o| o.bytes), Some(vec![1]));
        // Lần take() thứ 2 mà không push() thêm gì mới -> None, không gửi
        // lại mãi (đúng bug đã sửa: trước đây flush() không consume).
        assert!(input.take().is_none());
    }

    #[test]
    fn push_overwrites_pending_value_not_pending() {
        let mut input = LatestInput::new();
        input.push(vec![1]);
        input.push(vec![2]); // ghi đè, chưa take() lần nào

        assert_eq!(input.take().map(|o| o.bytes), Some(vec![2]));
        assert!(input.take().is_none());
    }

    #[test]
    fn pushing_twice_before_take_loses_the_first_value() {
        // Ghi lại đúng giới hạn đã nêu trong doc: push(shoot) rồi
        // push(move_right) trong cùng 1 frame -> shoot mất. Đây KHÔNG phải
        // bug cần sửa ở v0.5 — chỉ là hành vi phải nhớ khi dùng LatestInput
        // cho input dạng lệnh rời rạc.
        let mut input = LatestInput::new();
        input.push(b"shoot".to_vec());
        input.push(b"move_right".to_vec());

        assert_eq!(input.take().map(|o| o.bytes), Some(b"move_right".to_vec()));
    }
}
