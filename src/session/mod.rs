use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::entity::EntityId;
use crate::input::{CurrentInput, InputFrame};
use crate::net::{ConnectionError, ConnectionReader, ConnectionWriter};
use crate::protocol::{MessageKind, WireInput};
use crate::replication::{send_outgoing as send_outgoing_packet, Outgoing, SnapshotSender};
use crate::time::TickId;

/// Toàn bộ state của 1 client, gộp lại 1 chỗ thay vì nhiều
/// `HashMap<EntityId, X>` song song trong GameServer (snapshot state,
/// input state, sau này ping/RTT/ack...) — ClientSession là ranh giới rõ
/// ràng giữa network state và game state.
pub struct ClientSession {
    pub entity: EntityId,
    writer: ConnectionWriter,
    incoming: Receiver<WireInput>,
    pub snapshot_sender: SnapshotSender,
    pub current_input: CurrentInput,
    /// Tick server tại lần `drain_input()` gần nhất thực sự nhận được gói
    /// input mới — metadata vòng đời connection (timeout/idle/AI-takeover
    /// sau này), không phải dữ liệu gameplay nên KHÔNG đặt trong
    /// `CurrentInput` (CurrentInput chỉ nên biết đúng 1 việc: input hiện
    /// hành để Object::on_tick đọc). Chưa dùng ở v0.4, chỉ ghi nhận.
    last_seen_tick: Option<TickId>,
}

impl ClientSession {
    /// `reader`/`writer` đến từ `Server::accept()` — đã tách sẵn, không có
    /// bước "split" nào ở đây nữa (xem `net::Connection`). `reader` chuyển
    /// sang sống trên thread riêng, blocking `recv()`; `writer` giữ lại
    /// trong session, GameServer dùng để broadcast. Thread đọc không biết
    /// gì về Snapshot/World — chỉ decode WireInput và đẩy qua channel; việc
    /// diễn giải input là việc của gameplay khi World.tick() chạy.
    pub fn new(entity: EntityId, mut reader: ConnectionReader, writer: ConnectionWriter) -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || loop {
            match reader.recv() {
                Ok(packet) if packet.kind == MessageKind::Input => {
                    if let Ok(input) = WireInput::from_bytes(&packet.payload) {
                        // Lỗi send() nghĩa là GameServer đã bỏ session này
                        // (Receiver bị drop) — không còn ai đọc, dừng thread.
                        if tx.send(input).is_err() {
                            break;
                        }
                    }
                    // WireInput hỏng: bỏ qua gói này, không đóng kết nối vì
                    // 1 gói input lỗi không đáng làm rớt cả session.
                }
                // Packet kind khác Input trên chiều client -> server hiện
                // chưa có ý nghĩa gì — bỏ qua, không coi là lỗi.
                Ok(_) => {}
                // Socket đóng hoặc lỗi protocol không phục hồi được -> kết
                // thúc thread, channel tự đóng, GameServer phát hiện qua
                // drain_input() trả về true ở lần gọi kế tiếp.
                Err(_) => break,
            }
        });

        Self {
            entity,
            writer,
            incoming: rx,
            snapshot_sender: SnapshotSender::new(),
            current_input: CurrentInput::new(),
            last_seen_tick: None,
        }
    }

    /// Đọc hết input đang chờ trong channel (không chặn), cập nhật
    /// `current_input` với gói mới nhất — không FIFO, các gói cũ hơn trong
    /// cùng lần drain bị ghi đè, đúng ngữ nghĩa "trạng thái hiện hành".
    /// `server_tick` là tick hiện tại của server (không phải tick client tự
    /// khai trong gói), dùng để cập nhật `last_seen_tick`.
    ///
    /// Trả về `true` nếu phát hiện thread đọc đã kết thúc (connection đóng)
    /// — GameServer dùng tín hiệu này để gọi on_disconnect và loại session.
    pub fn drain_input(&mut self, server_tick: TickId) -> bool {
        loop {
            match self.incoming.try_recv() {
                Ok(wire) => {
                    self.current_input.update(InputFrame {
                        tick: TickId(wire.tick),
                        bytes: wire.bytes,
                    });
                    self.last_seen_tick = Some(server_tick);
                }
                Err(TryRecvError::Empty) => return false,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
    }

    /// Tick server gần nhất session này thực sự có gói input mới — chưa
    /// dùng ở v0.4 (để sẵn cho timeout/idle/AI-takeover: so
    /// `current_tick - last_seen_tick` với 1 ngưỡng).
    pub fn last_seen_tick(&self) -> Option<TickId> {
        self.last_seen_tick
    }

    pub fn send_outgoing(&mut self, message: &Outgoing) -> Result<(), ConnectionError> {
        send_outgoing_packet(&mut self.writer, message)
    }
}
