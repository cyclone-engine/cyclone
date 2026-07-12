# Cyclone v0.4 — Input Pipeline & Multiplayer Game Loop

> Thiết kế đã chốt qua thảo luận kiến trúc trước khi code. Đây là phiên bản đầu
> tiên Cyclone có luồng game nhiều người hoàn chỉnh: Client → Input → World →
> Snapshot → Client.

## Mục tiêu

```
Client                                              Client
  │                                                    ▲
  ▼                                                    │
WireInput                                        WireSnapshot/WireDelta
  │                                                    ▲
  ▼                                                    │
ConnectionReader (thread riêng)              ConnectionWriter
  │                                                    ▲
  ▼                                                    │
channel (mpsc)                                SnapshotSender.next_message()
  │                                                    ▲
  ▼                                                    │
ClientSession.current_input                World.snapshot(tick)
  │                                                    ▲
  ▼                                                    │
InputBatch  ────────────────────▶  World.tick(tick, &batch)
                                          │
                                    GameServer loop (TickScheduler)
```

**Definition of Done**: `cargo run --example multiplayer_demo`, mở 2 client
thật, điều khiển được — không phải `cargo test` xanh là xong.

## Bảng quyết định (bản cuối)

| # | Quyết định |
|---|---|
| 1 | Thêm `TickContext { info: TickInfo, input: Option<&InputFrame> }`, không nhét input vào `TickInfo` |
| 2 | `GameServer` nhận `on_connect`/`on_disconnect` closure, không tự spawn gameplay |
| 3 | `Connection::into_split(self) -> io::Result<(ConnectionReader, ConnectionWriter)>` thay cho `try_clone()` — loại khả năng gọi nhầm `recv()` trên 2 clone |
| 4 | `CurrentInput` là trạng thái hiện hành (1 slot), không FIFO; server không chờ input, dùng input gần nhất |
| 5 | `MessageKind::Input = 2`, **không** bump `ProtocolVersion` — chưa có client thật nào phụ thuộc V1, bump lúc này là sớm; để dành khi có SDK ngoài thật sự cần phân biệt |
| 6 | Không thêm `receive_snapshot()`/`receive_delta()` ở v0.4 |
| 7 | `Runner` giữ nguyên cho offline simulation; `GameServer` tự viết loop bằng `TickScheduler::advance()` |
| 8 | `ClientSession` giữ toàn bộ state 1 client; `GameServer` chỉ giữ `Vec<ClientSession>`, không nhiều `HashMap<EntityId, X>` |
| 9 | `InputFrame.bytes` là dữ liệu thô, engine không diễn giải cấu trúc |
| 10 | Không thêm `trait PacketSink` — chấp nhận trùng lặp giữa hàm cho `Connection` và hàm `_writer` cho `ConnectionWriter`. Chỉ thêm trait khi có ≥ 3 kiểu triển khai thật |
| 11 | `InputBatch` merge 2 con trỏ song song với `World.entries` (giống `diff()`/`apply()`), không dùng `get(id)` ngẫu nhiên — vì `entries` đã index trực tiếp theo `EntityId.index()` |
| 12 | Không có `World::queue_spawn`/`queue_despawn` riêng — dùng `World::commands() -> Commands<'_>`, tái sử dụng đúng 1 API queue duy nhất cho cả `Object::on_tick` lẫn `GameServer` |
| 13 | Đặt tên: `InputBatch::new()` (rỗng) / `InputBatch::from_items(...)` (có dữ liệu, tự sort) — không dùng `build()` |
| 14 | `last_seen_tick` (mốc cho timeout/idle/AI-takeover sau này) đặt ở `ClientSession`, không đặt trong `CurrentInput` — Single Responsibility: `CurrentInput` chỉ biết input gameplay hiện hành, không biết gì về tick server/connection. Dùng tick của **server** (GameServer tự đếm), không dùng `InputFrame.tick` (client tự khai, không đáng tin, không cùng hệ quy chiếu) |
| 15 | **Opinionated API**: `net::Connection` (2 chiều, chưa tách) ẩn hẳn thành `pub(crate)`, không public. `net::Server::accept()` và `net::Client::connect()` (mới) trả thẳng `(ConnectionReader, ConnectionWriter)` đã tách sẵn — không có cách nào lấy ra 1 connection 2 chiều chưa tách từ public API. Hệ quả: `replication::packet_sender` chỉ còn 1 bộ hàm (`send_snapshot`/`send_delta`/`send_outgoing`/`send_input`) nhận `&mut ConnectionWriter`, bỏ hẳn hậu tố `_writer` và bộ hàm `_conn` song song (quyết định #10 tự nhiên đơn giản hoá vì không còn ai giữ `Connection` để truyền vào). `ClientSession::new()` nhận thẳng `(ConnectionReader, ConnectionWriter)`, không còn `into_split()`/`io::Result` ở tầng session |

## Chi tiết theo module

### `protocol/input.rs` (mới)
```rust
pub struct WireInput {
    pub tick: u64,
    pub bytes: Vec<u8>,   // opaque, gameplay tự encode/decode
}
// WireEncode/WireDecode + to_bytes()/from_bytes(), cùng khuôn WireSnapshot
```
`packet.rs`: thêm `MessageKind::Input = 2`. `version.rs`: giữ nguyên chỉ `V1` — không bump.

### `net/connection.rs` + `net/server.rs` + `net/client.rs` (mới) — Opinionated API
```rust
// connection.rs — pub(crate), KHÔNG export ra ngoài crate
pub(crate) struct Connection { stream: TcpStream, reader: PacketReader }
impl Connection {
    pub(crate) fn new(stream: TcpStream) -> Self
    pub(crate) fn into_split(self) -> io::Result<(ConnectionReader, ConnectionWriter)>
}

// duy nhất 2 kiểu này public — không có "Connection" 2 chiều nào lộ ra ngoài
pub struct ConnectionReader { stream: TcpStream, reader: PacketReader }
impl ConnectionReader {
    pub fn recv(&mut self) -> Result<Packet, ConnectionError>
}
pub struct ConnectionWriter { stream: TcpStream }
impl ConnectionWriter {
    pub fn send(&mut self, packet: Packet) -> Result<(), ConnectionError>
}

// server.rs
impl Server {
    pub fn accept(&self) -> io::Result<(ConnectionReader, ConnectionWriter)>
    // = Connection::new(stream).into_split() — người dùng không thấy Connection
}

// client.rs — mới, đối xứng với Server
pub struct Client;
impl Client {
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<(ConnectionReader, ConnectionWriter)>
}
```

### `replication/packet_sender.rs` (chỉ 1 bộ hàm, nhận `&mut ConnectionWriter`)
```rust
pub fn send_snapshot(conn: &mut ConnectionWriter, snapshot: &Snapshot) -> Result<(), ConnectionError>
pub fn send_delta(conn: &mut ConnectionWriter, delta: &SnapshotDelta) -> Result<(), ConnectionError>
pub fn send_outgoing(conn: &mut ConnectionWriter, msg: &Outgoing) -> Result<(), ConnectionError>
pub fn send_input(conn: &mut ConnectionWriter, tick: TickId, bytes: Vec<u8>) -> Result<(), ConnectionError>
```
Không còn bản `_conn`/`_writer` song song — kể từ khi `Connection` bị ẩn (quyết định #15), không ai còn giữ 1 `Connection` để truyền vào bản `_conn` nữa, nên tự nhiên chỉ còn đúng 1 bộ hàm.

### `input/mod.rs` (mới — tầng thuần)
```rust
pub struct InputFrame { pub tick: TickId, pub bytes: Vec<u8> }

pub struct InputBatch { items: Vec<(EntityId, InputFrame)> }   // bất biến: luôn sort theo EntityId
impl InputBatch {
    pub fn new() -> Self
    pub fn from_items(items: Vec<(EntityId, InputFrame)>) -> Self   // tự sort bên trong
    pub(crate) fn iter(&self) -> impl Iterator<Item = &(EntityId, InputFrame)>
}

pub struct CurrentInput { latest: Option<InputFrame> }   // trạng thái hiện hành, không FIFO
impl CurrentInput {
    pub fn new() -> Self
    pub fn update(&mut self, frame: InputFrame)
    pub fn get(&self) -> Option<&InputFrame>
}
```

### `object.rs` (sửa signature — breaking, có chủ đích)
```rust
pub struct TickInfo { pub id: EntityId, pub tick: TickId }   // không đổi

pub struct TickContext<'a> {
    pub info: TickInfo,
    pub input: Option<&'a InputFrame>,
}

pub trait Object {
    fn type_id(&self) -> u32;
    fn on_spawn(&mut self, _ctx: &TickContext, _cmd: &mut Commands) {}
    fn on_tick(&mut self, ctx: &TickContext, cmd: &mut Commands);
    fn on_despawn(&mut self, _ctx: &TickContext, _cmd: &mut Commands) {}
    fn write_snapshot(&self, _writer: &mut SnapshotWriter) {}
}
```

### `world/mod.rs` (sửa `tick()`, thêm `commands()`)
```rust
pub fn tick(&mut self, tick: TickId, inputs: &InputBatch)
// merge 2 con trỏ: entries[0..len] (đã tự nhiên theo thứ tự EntityId, vì
// entries index trực tiếp bằng EntityId.index()) với inputs.iter() (đã sort)
// → O(entities + players), không gọi get(id) lặp lại.

pub fn commands(&mut self) -> Commands<'_> {
    Commands { next_index: &mut self.next_index, pending: &mut self.pending }
}
```
`examples/minimal_world.rs`/`main.rs` gọi `world.tick(tick, &InputBatch::new())`.

### `session/mod.rs` (mới)
```rust
pub struct ClientSession {
    pub entity: EntityId,
    writer: ConnectionWriter,
    incoming: mpsc::Receiver<WireInput>,
    pub snapshot_sender: SnapshotSender,
    pub current_input: CurrentInput,
    // Metadata vòng đời connection (timeout/idle/AI-takeover sau này) —
    // KHÔNG đặt trong CurrentInput vì đó là dữ liệu gameplay thuần (Single
    // Responsibility: CurrentInput chỉ biết "input hiện hành để
    // Object::on_tick đọc", không biết gì về tick server hay connection).
    last_seen_tick: Option<TickId>,
}
impl ClientSession {
    pub fn new(entity: EntityId, reader: ConnectionReader, writer: ConnectionWriter) -> Self   // infallible, không io::Result
    fn drain_input(&mut self, server_tick: TickId) -> bool   // cập nhật last_seen_tick khi có gói mới
    fn last_seen_tick(&self) -> Option<TickId>                // chưa dùng ở v0.4, để sẵn cho timeout
    fn send_outgoing(&mut self, msg: &Outgoing) -> Result<(), ConnectionError>
}
```
`reader`/`writer` đến từ `Server::accept()` — đã tách sẵn, không còn bước `into_split()`/`io::Result` nào ở tầng session nữa. `reader` chuyển sang thread riêng, vòng lặp `reader.recv()` → decode `WireInput` khi `kind == Input` → gửi qua channel. Socket đóng → channel đóng → `GameServer` phát hiện qua lỗi `Disconnected`.

### `server/mod.rs` (mới — `GameServer`)
```rust
pub struct GameServer {
    world: World,
    scheduler: TickScheduler,
    sessions: Vec<ClientSession>,
    new_connections: mpsc::Receiver<(ConnectionReader, ConnectionWriter)>,
    on_connect: Box<dyn FnMut(&mut Commands) -> EntityId>,
    on_disconnect: Box<dyn FnMut(EntityId, &mut Commands)>,
    current_tick: TickId,   // tick gần nhất đã tick World, dùng làm mốc cho ClientSession::last_seen_tick
}
impl GameServer {
    pub fn bind(
        addr: impl ToSocketAddrs,
        ticks_per_second: u32,
        on_connect: impl FnMut(&mut Commands) -> EntityId + 'static,
        on_disconnect: impl FnMut(EntityId, &mut Commands) + 'static,
    ) -> io::Result<Self>

    pub fn run(&mut self, should_continue: impl FnMut() -> bool)
}
```
Mỗi vòng: (1) drain `new_connections` (đã là `(ConnectionReader, ConnectionWriter)` sẵn, accept-thread tự gọi `Server::accept()`) → `on_connect(&mut world.commands())` → `ClientSession::new(entity, reader, writer)` → thêm vào `sessions`; (2) mỗi session `drain_input()`, phát hiện disconnect → `on_disconnect` + despawn qua `world.commands()`; (3) `scheduler.advance(elapsed)` → dựng `InputBatch::from_items()` từ `current_input` mọi session → `world.tick(tick, &batch)`; (4) `world.snapshot(tick)` → mỗi session `snapshot_sender.next_message()` → `send_outgoing()`.

TODO ghi trong code: *"GameServer hiện gộp drain/tick/broadcast; tách `ReplicationSystem`/`SessionManager` khi có lý do thật, không tách trước."*

### `examples/multiplayer_demo/`
`DemoPlayer: Object` đọc `ctx.input.bytes` trong `on_tick`; `on_connect` spawn `DemoPlayer` qua `Commands`. Client binary: connect, gửi `WireInput` mỗi frame, in `WireSnapshot`/`WireDelta` nhận được.

## Thứ tự code

0. `protocol/input.rs` + `packet.rs`/`version.rs`
1. `net/connection.rs`: `into_split()`, `ConnectionReader`, `ConnectionWriter`
2. `input/mod.rs`: `InputFrame`, `InputBatch`, `CurrentInput`
3. `object.rs` (`TickContext`) + `world/mod.rs` (`tick()` merge, `commands()`) + cập nhật `examples/minimal_world.rs`, `main.rs`, `tests/*`
4. `replication/packet_sender.rs`: `send_*_writer`, `send_input`
5. `session/mod.rs`: `ClientSession` + thread đọc
6. `server/mod.rs`: `GameServer`
7. `examples/multiplayer_demo/`
