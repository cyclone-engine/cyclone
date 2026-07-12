use std::io;
use std::net::ToSocketAddrs;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Instant;

use crate::commands::Commands;
use crate::entity::EntityId;
use crate::input::InputBatch;
use crate::net::{ConnectionReader, ConnectionWriter, Server};
use crate::runtime::TickScheduler;
use crate::session::ClientSession;
use crate::time::TickId;
use crate::world::World;

/// Vòng lặp game chính: accept connection mới, gom input, tick World, phát
/// snapshot. Không dùng `Runner` — Runner chỉ phục vụ offline simulation
/// (xem runtime::Runner); GameServer cần chèn drain input trước tick và
/// broadcast sau tick nên tự lái TickScheduler trực tiếp.
///
/// TODO (roadmap): GameServer hiện gộp accept/drain/tick/broadcast trong 1
/// struct. Tách ReplicationSystem/SessionManager riêng khi có lý do thật
/// (ví dụ nhiều chiến lược broadcast khác nhau), không tách trước khi cần.
pub struct GameServer {
    world: World,
    scheduler: TickScheduler,
    sessions: Vec<ClientSession>,
    new_connections: Receiver<(ConnectionReader, ConnectionWriter)>,
    on_connect: OnConnect,
    on_disconnect: OnDisconnect,
    /// Tick gần nhất đã tick World — dùng để ClientSession ghi nhận
    /// `last_seen_tick` khi drain input, không phải để gameplay đọc (0 khi
    /// chưa tick lần nào).
    current_tick: TickId,
}

type OnConnect = Box<dyn FnMut(&mut Commands) -> EntityId>;
type OnDisconnect = Box<dyn FnMut(EntityId, &mut Commands)>;

impl GameServer {
    /// `on_connect`/`on_disconnect` là hook duy nhất engine cấp cho
    /// gameplay để gắn EntityId vào 1 connection — GameServer không bao giờ
    /// tự spawn kiểu Object cụ thể nào (Player/Tee/...), giữ đúng ranh giới
    /// Engine/Gameplay.
    pub fn bind(
        addr: impl ToSocketAddrs,
        ticks_per_second: u32,
        on_connect: impl FnMut(&mut Commands) -> EntityId + 'static,
        on_disconnect: impl FnMut(EntityId, &mut Commands) + 'static,
    ) -> io::Result<Self> {
        let server = Server::bind(addr)?;
        let (tx, rx) = mpsc::channel();

        // Server::accept() blocking -> chạy trên thread riêng để vòng tick
        // chính không bao giờ phải chờ client mới kết nối. Vòng lặp tự kết
        // thúc khi accept() lỗi (bao gồm cả lỗi tách reader/writer bên
        // trong — Server::accept() giờ luôn trả sẵn cặp đã tách, không còn
        // bước tách riêng ở GameServer nữa) hoặc GameServer bị drop
        // (tx.send() lỗi). Không phân biệt "listener chết hẳn" với "1
        // connection lỗi tách" — cả 2 đều hiếm và thường xảy ra cùng lúc
        // (ví dụ hết file descriptor), không đáng thêm logic retry riêng.
        thread::spawn(move || {
            while let Ok(pair) = server.accept() {
                if tx.send(pair).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            world: World::new(),
            scheduler: TickScheduler::new(ticks_per_second),
            sessions: Vec::new(),
            new_connections: rx,
            on_connect: Box::new(on_connect),
            on_disconnect: Box::new(on_disconnect),
            current_tick: TickId(0),
        })
    }

    pub fn run(&mut self, mut should_continue: impl FnMut() -> bool) {
        let mut last = Instant::now();
        while should_continue() {
            self.accept_new_connections();
            self.drain_sessions();

            let now = Instant::now();
            let elapsed = now - last;
            last = now;

            let mut latest_tick = None;
            for tick in self.scheduler.advance(elapsed) {
                let items = self
                    .sessions
                    .iter()
                    .filter_map(|session| {
                        session
                            .current_input
                            .get()
                            .map(|frame| (session.entity, frame.clone()))
                    })
                    .collect();
                self.world.tick(tick, &InputBatch::from_items(items));
                latest_tick = Some(tick);
            }

            if let Some(tick) = latest_tick {
                self.current_tick = tick;
                self.broadcast(tick);
            }
        }
    }

    fn accept_new_connections(&mut self) {
        loop {
            match self.new_connections.try_recv() {
                Ok((reader, writer)) => {
                    let entity = {
                        let mut cmd = self.world.commands();
                        (self.on_connect)(&mut cmd)
                    };
                    let session = ClientSession::new(entity, reader, writer);
                    self.sessions.push(session);
                }
                Err(TryRecvError::Empty) => return,
                // Accept thread đã dừng hẳn (ví dụ listener lỗi) — không
                // còn connection mới nào tới nữa, nhưng session hiện có vẫn
                // chạy bình thường.
                Err(TryRecvError::Disconnected) => return,
            }
        }
    }

    /// Đọc input đang chờ của mọi session (không chặn) và loại session đã
    /// đóng kết nối, gọi `on_disconnect` tương ứng. Dùng `current_tick` của
    /// lần tick gần nhất (không phải tick sắp chạy) làm mốc `last_seen_tick`
    /// — đủ chính xác cho mục đích bookkeeping tương lai, không cần đợi
    /// tick tiếp theo mới biết.
    fn drain_sessions(&mut self) {
        let mut i = 0;
        while i < self.sessions.len() {
            if self.sessions[i].drain_input(self.current_tick) {
                let session = self.sessions.remove(i);
                let mut cmd = self.world.commands();
                (self.on_disconnect)(session.entity, &mut cmd);
            } else {
                i += 1;
            }
        }
    }

    fn broadcast(&mut self, tick: TickId) {
        let snapshot = self.world.snapshot(tick);
        for session in &mut self.sessions {
            let message = session.snapshot_sender.next_message(&snapshot);
            // TODO (roadmap): lỗi gửi ở đây bị bỏ qua âm thầm — session sẽ
            // tự được dọn ở lần drain_sessions() kế tiếp khi thread đọc của
            // nó phát hiện socket đóng. Chấp nhận được cho v0.4 vì lỗi ghi
            // và lỗi đọc trên cùng 1 TCP stream gần như luôn xảy ra cùng
            // nhau (socket đóng thì cả 2 chiều đều lỗi).
            let _ = session.send_outgoing(&message);
        }
    }
}
