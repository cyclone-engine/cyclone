# Cyclone — Kiến trúc & luồng thực thi

> Bản tổng hợp kỹ thuật, đi kèm bản trực quan tại [`docs/architecture.html`](./architecture.html).

Cyclone là một **headless deterministic multiplayer server framework** viết bằng Rust, hiện là 1 crate duy nhất (`cyclone`, edition 2024, không có runtime dependency ngoài — `Cargo.toml` chỉ có `criterion` cho benchmark). Dự án đang ở giai đoạn v0.1 → v0.3 theo roadmap trong `README.md`.

Bối cảnh dự án: mục tiêu **không phải** port 1:1 Teeworlds sang Rust, mà là xây một engine tái sử dụng được, trong đó Teeworlds chỉ là gameplay module đầu tiên chạy trên engine. Engine phải hỗ trợ được các thể loại khác (platformer, shooter, racing, sandbox) mà không cần sửa engine — mã C++ Teeworlds chỉ có giá trị tham khảo hành vi, không phải khuôn mẫu kiến trúc.

Crate root `src/lib.rs` khai báo 9 module con, chính là 9 "tầng" của engine:

```rust
pub mod commands;
pub mod entity;
pub mod net;
pub mod object;
pub mod protocol;
pub mod replication;
pub mod runtime;
pub mod snapshot;
pub mod time;
pub mod world;

pub use commands::Commands;
pub use entity::{Entity, EntityId};
pub use object::{Object, TickInfo};
pub use runtime::{Runner, TickScheduler};
pub use time::TickId;
pub use world::World;
```

Roadmap 3 giai đoạn (từ `README.md`):

- **v0.1 — Simulation Foundation**: `Tick Scheduler → World → Entity` (chưa có network)
- **v0.2 — State Replication**: `World → Snapshot → Delta → Network Packet`
- **v0.3 — Networking & Deployment**: `Client → TCP → Session → Tick → World → Snapshot → Delta → Client`

---

## 1. Sơ đồ tầng

Nguyên tắc lặp lại xuyên suốt: mỗi cặp tầng tách **logic thuần** (deterministic, không I/O, dễ test) khỏi **tầng chạm thế giới thật** (đồng hồ hệ thống, socket TCP). Phụ thuộc chỉ đi một chiều, từ dưới lên:

```
time → entity → object/commands → world → runtime
     → snapshot → protocol → replication → net → main
```

| Tầng | Loại | File | Vai trò |
|---|---|---|---|
| `time` | thuần | `src/time/mod.rs` | `TickId(u64)` — đồng hồ logic của simulation |
| `entity` | thuần | `src/entity/*.rs` | `EntityId` (index+generation), `Entity` (alive/enabled/parent/children) |
| `object` / `commands` | thuần | `src/object.rs`, `src/commands.rs` | Trait `Object` (hợp đồng gameplay), `Commands` (hàng đợi ghi trì hoãn) |
| `world` | thuần | `src/world/mod.rs` | Lõi simulation: chạy tick, tạo snapshot |
| `runtime` | **I/O — clock** | `src/runtime/*.rs` | `TickScheduler` (fixed-timestep thuần) + `Runner` (chạm `Instant` thật) |
| `snapshot` | thuần | `src/snapshot/*.rs` | Trạng thái world tại 1 tick, `diff()`/`apply()` |
| `protocol` | thuần | `src/protocol/*.rs` | Wire format nhị phân, encode/decode |
| `replication` | adapter | `src/replication/*.rs` | Nối Snapshot ↔ Protocol ↔ Net |
| `net` | **I/O — socket** | `src/net/*.rs` | TCP thật: `Server`, `Connection`, `PacketReader` |

---

## 2. Chi tiết từng tầng

### 2.1 `time` (`src/time/mod.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TickId(pub u64);
```

Đồng hồ logic của toàn simulation — không có wall-clock ở đây, thuần là số đếm tick. Dùng xuyên suốt mọi tầng khác.

### 2.2 `object.rs` — hợp đồng gameplay

```rust
pub struct TickInfo {
    pub id: EntityId,
    pub tick: TickId,
}

pub trait Object {
    fn type_id(&self) -> u32;
    fn on_spawn(&mut self, _info: &TickInfo, _cmd: &mut Commands) {}
    fn on_tick(&mut self, info: &TickInfo, cmd: &mut Commands);
    fn on_despawn(&mut self, _info: &TickInfo, _cmd: &mut Commands) {}
    fn write_snapshot(&self, _writer: &mut SnapshotWriter) {}
}
```

Nguyên tắc thiết kế then chốt: `Object` **không bao giờ có tham chiếu ngược lại `World`**, không tự truy vấn entity khác — nó chỉ nhận `&TickInfo` (đọc) và `&mut Commands` (ghi ý định) mỗi khi được `World` gọi tới.

### 2.3 `entity/id.rs` — định danh entity

```rust
pub struct EntityId { index: u32, generation: u32 }
impl EntityId {
    pub(crate) fn new(index: u32, generation: u32) -> Self
    pub fn index(&self) -> u32
    pub fn generation(&self) -> u32
}
```

Ghi chú TODO trong code: `generation` hiện luôn = 0 vì chưa có free-list/slot-reuse.

### 2.4 `entity/entity.rs` — metadata vòng đời

```rust
pub struct Entity {
    pub id: EntityId,
    pub enabled: bool,
    pub alive: bool,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
}
```

### 2.5 `commands.rs` — hàng đợi lệnh ghi (write-only, deferred)

```rust
pub(crate) enum PendingCommand {
    Spawn { id: EntityId, parent: Option<EntityId>, object: Box<dyn Object> },
    Despawn(EntityId),
}

pub struct Commands<'a> {
    pub(crate) next_index: &'a mut u32,
    pub(crate) pending: &'a mut Vec<PendingCommand>,
}

impl<'a> Commands<'a> {
    pub fn spawn<T: Object + 'static>(&mut self, obj: T) -> EntityId
    pub fn spawn_with_parent<T: Object + 'static>(&mut self, obj: T, parent: Option<EntityId>) -> EntityId
    pub fn despawn(&mut self, id: EntityId)
}
```

Mọi lệnh spawn/despawn được đẩy vào `pending` chứ không thực thi ngay — `World::flush()` mới thực sự áp dụng, và lệnh mới sinh ra bên trong `on_spawn`/`on_despawn` phải đợi flush của **tick kế tiếp** (tránh đệ quy vô hạn).

### 2.6 `world/mod.rs` — trái tim của simulation

```rust
struct ObjectEntry { entity: Entity, object: Box<dyn Object> }

pub struct World {
    entries: Vec<ObjectEntry>,
    next_index: u32,
    pending: Vec<PendingCommand>,
}
```

Storage hiện tại là `Vec<Box<dyn Object>>` — README ghi rõ đây là lựa chọn tạm để ưu tiên đơn giản, chưa phải archetype/SoA cuối cùng; sẽ đánh giá lại sau khi có benchmark thật.

```rust
impl World {
    pub fn new() -> Self
    pub fn spawn<T: Object + 'static>(&mut self, obj: T) -> EntityId
    pub fn entity(&self, id: EntityId) -> Option<&Entity>
    pub fn tick(&mut self, tick: TickId)
    pub fn snapshot(&self, tick: TickId) -> Snapshot
    fn flush(&mut self, tick: TickId)
    fn apply_spawn(&mut self, id: EntityId, parent: Option<EntityId>, object: Box<dyn Object>, tick: TickId)
    fn despawn_entity(&mut self, id: EntityId, tick: TickId)
}
```

**`World::tick()`** — vòng lặp cốt lõi:

```rust
pub fn tick(&mut self, tick: TickId) {
    for i in 0..self.entries.len() {
        if !self.entries[i].entity.alive || !self.entries[i].entity.enabled { continue; }
        let id = self.entries[i].entity.id;
        let info = TickInfo { id, tick };
        let mut cmd = Commands { next_index: &mut self.next_index, pending: &mut self.pending };
        self.entries[i].object.on_tick(&info, &mut cmd);
    }
    self.flush(tick);
}
```

Gọi `on_tick()` của mỗi object còn sống & enabled theo thứ tự entry (deterministic), gom mọi `Commands` phát sinh, rồi `flush()` một lần cuối tick.

**`World::snapshot()`** — cầu nối sang tầng Snapshot:

```rust
pub fn snapshot(&self, tick: TickId) -> Snapshot {
    let mut snapshot = Snapshot::new(tick);
    for entry in &self.entries {
        if !entry.entity.alive { continue; }
        let mut writer = SnapshotWriter::new();
        entry.object.write_snapshot(&mut writer);
        if writer.is_empty() { continue; } // object không muốn replicate
        snapshot.push(SnapshotItem { id: entry.entity.id, type_id: entry.object.type_id(), fields: writer.finish() });
    }
    snapshot.sort(); // bất biến bắt buộc: sort theo EntityId
    snapshot
}
```

World là nguồn Snapshot duy nhất trong crate (comment trong code nói rõ nếu sau này gắn ECS backend khác thì đây là chỗ tách trait `ReplicationBackend`).

### 2.7 `runtime/*.rs` — vòng lặp tick + wall-clock

`scheduler.rs` — logic thuần, không I/O, không đọc wall-clock thật:

```rust
pub struct TickScheduler {
    tick_duration: Duration,
    accumulator: Duration,
    current_tick: u64,
}
impl TickScheduler {
    pub fn new(ticks_per_second: u32) -> Self
    pub fn advance(&mut self, elapsed: Duration) -> Vec<TickId>
}
```

`advance()` dùng thuật toán fixed-timestep tích lũy (accumulator pattern): cộng dồn `elapsed`, mỗi khi đủ `tick_duration` thì pop ra 1 `TickId` và tăng `current_tick`. Trả về `Vec<TickId>` để 1 lần gọi `advance()` có thể sinh ra 0, 1, hoặc nhiều tick (catch-up khi frame bị trễ).

`runner.rs` — nơi duy nhất chạm `Instant` thật:

```rust
pub struct Runner { scheduler: TickScheduler }
impl Runner {
    pub fn new(ticks_per_second: u32) -> Self
    pub fn run(&mut self, world: &mut World, mut should_continue: impl FnMut() -> bool) {
        let mut last = Instant::now();
        while should_continue() {
            let now = Instant::now();
            let elapsed = now - last;
            last = now;
            for tick in self.scheduler.advance(elapsed) {
                world.tick(tick);
            }
        }
    }
}
```

Lưu ý: `Runner::run()` **hiện chưa gọi** bất kỳ hàm nào của Snapshot/Replication/Net — nó chỉ drive `World`. Việc snapshot + gửi mạng phải do code gọi (server loop tùy biến) thực hiện thêm sau mỗi `world.tick()`.

### 2.8 `snapshot/*.rs` — trạng thái thuần túy, chưa biết gì về mạng

```rust
pub struct SnapshotItem { pub id: EntityId, pub type_id: u32, pub fields: Vec<i32> }

pub struct Snapshot { pub tick: TickId, pub items: Vec<SnapshotItem> }
impl Snapshot {
    pub fn new(tick: TickId) -> Self
    pub(crate) fn push(&mut self, item: SnapshotItem)
    pub(crate) fn sort(&mut self) // sort theo EntityId — bất biến bắt buộc cho diff()/apply()
}

#[derive(Default)]
pub struct SnapshotWriter { fields: Vec<i32> }
impl SnapshotWriter {
    pub fn new() -> Self
    pub fn write_i32(&mut self, value: i32)
    pub(crate) fn is_empty(&self) -> bool
    pub(crate) fn finish(self) -> Vec<i32>
}
```

`delta.rs` — thuật toán diff giữa 2 Snapshot (merge tuyến tính O(n), dựa trên `items` đã sort):

```rust
pub enum DeltaItem {
    Spawn { item: SnapshotItem },
    Update { id: EntityId, fields: Vec<i32> },
    Remove { id: EntityId },
}
pub struct SnapshotDelta { pub items: Vec<DeltaItem> }

pub fn diff(old: Option<&Snapshot>, new: &Snapshot) -> SnapshotDelta
```

Logic: nếu `old` là `None` → mọi item của `new` thành `Spawn`. Nếu có `old`, quét 2 mảng cùng lúc theo con trỏ (đã sort theo `EntityId`): id trùng → tính `fields[i] = new - old` (chỉ tạo `Update` nếu có field khác 0); id chỉ có ở `old` → `Remove`; id chỉ có ở `new` → `Spawn`.

`applier.rs` (trong `snapshot/`) — phép toán ngược `(old, delta) → new`:

```rust
pub fn apply(old: &Snapshot, delta: &SnapshotDelta, tick: TickId) -> Snapshot
```

`storage.rs` — lịch sử snapshot theo tick:

```rust
pub struct SnapshotStorage { history: VecDeque<Snapshot>, capacity: usize }
impl SnapshotStorage {
    pub fn new(capacity: usize) -> Self
    pub fn push(&mut self, snapshot: Snapshot)
    pub fn get(&self, tick: TickId) -> Option<&Snapshot>
    pub fn latest(&self) -> Option<&Snapshot>
}
```

### 2.9 `protocol/*.rs` — wire format nhị phân, không biết gì về World/socket

`codec.rs` — nguyên thủy đọc/ghi little-endian:

```rust
pub const MAX_ITEM_COUNT: u32 = 10_000;
pub const MAX_FIELD_COUNT: u16 = 10_000;

pub trait WireEncode { fn encode(&self, buf: &mut Vec<u8>); }
pub trait WireDecode: Sized { fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError>; }
```

Cả `MAX_ITEM_COUNT`/`MAX_FIELD_COUNT` tồn tại để chặn DoS (không cấp phát `Vec` theo số attacker tự khai trong 4 byte input).

`packet.rs` — lớp framing ngoài cùng:

```rust
pub const MAGIC: [u8; 2] = *b"CY";
pub const MAX_PACKET_SIZE: usize = 1 << 20;
pub const HEADER_LEN: usize = 9; // magic(2)+version(2)+kind(1)+payload_len(4)

#[repr(u8)]
pub enum MessageKind { Snapshot = 0, Delta = 1 }

pub struct Packet { pub version: ProtocolVersion, pub kind: MessageKind, pub payload: Vec<u8> }
impl Packet {
    pub fn new(kind: MessageKind, payload: Vec<u8>) -> Self
    pub fn encode(&self) -> Vec<u8>
    pub fn decode(mut buf: &[u8]) -> Result<Self, ProtocolError>
    pub fn peek_payload_len(header: &[u8]) -> Result<usize, ProtocolError> // dùng bởi net::PacketReader
}
```

`snapshot.rs` (trong `protocol/`) — "Wire" version của Snapshot:

```rust
pub struct NetEntityId { pub index: u32, pub generation: u32 }
pub struct WireSnapshotItem { pub id: NetEntityId, pub type_id: u32, pub fields: Vec<i32> }
pub struct WireSnapshot { pub tick: u64, pub items: Vec<WireSnapshotItem> }
impl WireSnapshot {
    pub fn to_bytes(&self) -> Vec<u8>
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError>
}
```

`delta.rs` (trong `protocol/`) — tương tự cho `SnapshotDelta`:

```rust
pub enum WireDeltaItem {
    Spawn(WireSnapshotItem),
    Update { id: NetEntityId, fields: Vec<i32> },
    Remove { id: NetEntityId },
}
pub struct WireDelta { pub items: Vec<WireDeltaItem> }
```

`error.rs`:

```rust
pub enum ProtocolError {
    UnexpectedEof, InvalidMagic, UnsupportedVersion(u16),
    InvalidMessageKind(u8), InvalidDeltaItemKind(u8), TrailingBytes,
    TooManyItems { count: u32, max: u32 },
    TooManyFields { count: u16, max: u16 },
    PayloadTooLarge { len: usize, max: usize },
}
```

Không bao giờ panic khi decode input mạng — luôn trả `Result`.

### 2.10 `replication/*.rs` — cầu nối Snapshot ↔ Protocol ↔ Net

`sender.rs` — quyết định "khi nào gửi full snapshot, khi nào gửi delta" (thuần logic, không chạm socket):

```rust
pub enum Outgoing { Snapshot(Snapshot), Delta(SnapshotDelta) }

pub struct SnapshotSender { baseline: Option<Snapshot> }
impl SnapshotSender {
    pub fn new() -> Self
    pub fn next_message(&mut self, latest: &Snapshot) -> Outgoing {
        let message = match &self.baseline {
            None => Outgoing::Snapshot(latest.clone()),
            Some(baseline) => Outgoing::Delta(diff(Some(baseline), latest)),
        };
        self.baseline = Some(latest.clone());
        message
    }
}
```

Mỗi client giữ 1 `SnapshotSender` riêng: lần đầu gửi full `Snapshot`, các lần sau gọi `snapshot::diff()` để chỉ gửi phần thay đổi.

`applier.rs` (trong `replication/`) — `pub fn apply(old, delta, tick) -> Snapshot`, dùng phía nhận (client SDK) để tái tạo state mới từ baseline + delta.

`packet_sender.rs` — Snapshot/Delta → Packet → gửi qua `net::Connection`:

```rust
pub fn send_snapshot(conn: &mut Connection, snapshot: &Snapshot) -> Result<(), ConnectionError> {
    let wire = WireSnapshot::from(snapshot);
    conn.send(Packet::new(MessageKind::Snapshot, wire.to_bytes()))
}

pub fn send_delta(conn: &mut Connection, delta: &SnapshotDelta) -> Result<(), ConnectionError> {
    let wire = WireDelta::from(delta);
    conn.send(Packet::new(MessageKind::Delta, wire.to_bytes()))
}

pub fn send_outgoing(conn: &mut Connection, message: &Outgoing) -> Result<(), ConnectionError> {
    match message {
        Outgoing::Snapshot(snapshot) => send_snapshot(conn, snapshot),
        Outgoing::Delta(delta) => send_delta(conn, delta),
    }
}
```

### 2.11 `net/*.rs` — I/O thật (TCP), tách biệt logic thuần

`server.rs` — accept loop chặn (blocking), v0.3 chưa dùng async:

```rust
pub struct Server { listener: TcpListener }
impl Server {
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self>
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr>
    pub fn accept(&self) -> io::Result<Connection> {
        let (stream, _addr) = self.listener.accept()?;
        Ok(Connection::new(stream))
    }
}
```

`reader.rs` — state machine ghép byte thuần, không chạm socket:

```rust
#[derive(Default)]
pub struct PacketReader { buf: Vec<u8> }
impl PacketReader {
    pub fn new() -> Self
    pub fn feed(&mut self, bytes: &[u8])
    pub fn try_read_packet(&mut self) -> Result<Option<Packet>, ProtocolError> {
        if self.buf.len() < HEADER_LEN { return Ok(None); }
        let payload_len = Packet::peek_payload_len(&self.buf[..HEADER_LEN])?;
        let total = HEADER_LEN + payload_len;
        if self.buf.len() < total { return Ok(None); }
        let frame: Vec<u8> = self.buf.drain(..total).collect();
        Packet::decode(&frame).map(Some)
    }
}
```

`connection.rs` — nơi duy nhất chạm `TcpStream` thật:

```rust
pub struct Connection { stream: TcpStream, reader: PacketReader }
impl Connection {
    pub fn new(stream: TcpStream) -> Self
    pub fn send(&mut self, packet: Packet) -> Result<(), ConnectionError> {
        self.stream.write_all(&packet.encode())?;
        Ok(())
    }
    pub fn recv(&mut self) -> Result<Packet, ConnectionError> {
        loop {
            if let Some(packet) = self.reader.try_read_packet()? { return Ok(packet); }
            let mut buf = [0u8; 4096];
            let n = self.stream.read(&mut buf)?;
            if n == 0 { return Err(ConnectionError::Closed); }
            self.reader.feed(&buf[..n]);
        }
    }
}
```

`Connection` **chỉ biết `Packet`** — hoàn toàn không biết `Snapshot`/`WireSnapshot`/`SnapshotDelta`; encode domain object thành `Packet` là việc của tầng `replication` gọi vào nó.

`error.rs`:

```rust
pub enum ConnectionError { Io(io::Error), Protocol(ProtocolError), Closed }
```

### 2.12 `main.rs` và `examples/minimal_world.rs`

`src/main.rs` hiện tại **chưa** implement server thật (chưa dùng `net::Server`/`runtime::Runner`) — nó chỉ là demo World giống hệt `examples/minimal_world.rs`:

```rust
use cyclone::{Commands, Object, TickId, TickInfo, World};

struct Player { position: i64, velocity: i64 }
impl Object for Player {
    fn type_id(&self) -> u32 { 1 }
    fn on_tick(&mut self, info: &TickInfo, _cmd: &mut Commands) {
        self.position += self.velocity;
        println!("[tick {}] entity {:?} position = {}", info.tick.0, info.id, self.position);
    }
}

fn main() {
    let mut world = World::new();
    world.spawn(Player { position: 0, velocity: 1 });
    for t in 0..5 {
        world.tick(TickId(t));
    }
}
```

Cả `main.rs` và `examples/minimal_world.rs` **không dùng** `Runner`/`TickScheduler` (không có wall-clock thật), không dùng `net`/`replication`/`snapshot` — chỉ demo tầng Object/World/Commands/Entity thuần (v0.1). Pipeline networking đầy đủ (v0.2/v0.3) hiện chỉ được minh chứng qua test `tests/net_loopback.rs`, chưa có trong `main.rs`.

---

## 3. Luồng thực thi tổng thể

Chuỗi gọi hàm cụ thể, theo đúng thứ tự (phần networking dựa trên cách `tests/net_loopback.rs` minh chứng, vì `main.rs` hiện tại chưa wire đủ):

### Giai đoạn khởi động

1. `main()` (hoặc server code tương lai) tạo `World::new()`.
2. Gọi `world.spawn(obj)` nhiều lần để khởi tạo entity ban đầu — bên trong `World::spawn()` tạo tạm 1 `Commands`, gọi `cmd.spawn(obj)` (đẩy `PendingCommand::Spawn`), rồi gọi ngay `self.flush(TickId(0))` để áp dụng lập tức.
3. *(Networking, khi có)* `Server::bind(addr)` mở `TcpListener`; server loop gọi `server.accept()` chặn cho tới khi có client → trả về `Connection::new(stream)`.

### Giai đoạn tick loop chính

4. `Runner::run(&mut self, world, should_continue)` — vòng `while should_continue()`: đo `elapsed = Instant::now() - last`, gọi `self.scheduler.advance(elapsed)` → trả về `Vec<TickId>` (0, 1, hay nhiều tick tùy tốc độ frame). Với mỗi `TickId`: gọi `world.tick(tick)`.
5. `World::tick(tick)`: lặp qua từng entry còn `alive && enabled`, dựng `TickInfo` và `Commands`, gọi `object.on_tick(&info, &mut cmd)` (nơi gameplay code chạy). Sau khi duyệt hết: gọi `self.flush(tick)` áp dụng mọi `PendingCommand::Spawn`/`Despawn` đã hàng đợi (gọi `on_spawn`/`on_despawn`, cascade xuống children).

### Giai đoạn snapshot + replication

6. Server code gọi `let snapshot = world.snapshot(tick)`: lặp qua entry còn `alive`, tạo `SnapshotWriter`, gọi `entry.object.write_snapshot(&mut writer)`; nếu rỗng thì bỏ qua; ngược lại tạo `SnapshotItem`, `snapshot.push(item)`; cuối cùng `snapshot.sort()`.
7. Với mỗi client, server giữ 1 `SnapshotSender`, gọi `sender.next_message(&snapshot)`: lần đầu trả `Outgoing::Snapshot`; các lần sau gọi `snapshot::diff(Some(baseline), latest)` → trả `Outgoing::Delta`; cập nhật baseline.
8. Server gọi `replication::send_outgoing(&mut conn, &message)`: convert sang `WireSnapshot`/`WireDelta` → `to_bytes()` → `Packet::new(kind, bytes)` → `conn.send(packet)`. `Connection::send()` gọi `packet.encode()` (thêm header 9 byte) rồi `stream.write_all()` — điểm duy nhất chạm socket thật để ghi.

### Giai đoạn nhận

9. `Connection::recv()` — vòng lặp blocking: gọi `self.reader.try_read_packet()`; nếu buffer chưa đủ header/payload → `Ok(None)`, đọc thêm từ `stream.read()` rồi `feed()` tiếp (xử lý TCP phân mảnh gói tin); nếu đủ → `drain` 1 frame trọn vẹn, `Packet::decode()` → `Ok(Some(packet))`.
10. Caller đọc `packet.kind` để phân biệt Snapshot/Delta, decode payload: `WireSnapshot::from_bytes()` hoặc `WireDelta::from_bytes()`.
11. Phía nhận tái tạo state: convert Wire type về type nội bộ (gọi lại `snapshot.sort()`), nếu là Delta thì gọi `replication::apply(&old, &delta, tick)` để merge tuyến tính ra `Snapshot` mới, cập nhật baseline cục bộ.

### Sơ đồ luồng (top-down)

```
main.rs / server loop
      │
      ▼
runtime::Runner::run()  ──uses──▶ runtime::TickScheduler::advance()
      │  (world.tick(tick) mỗi TickId)
      ▼
world::World::tick()  ──calls──▶ object::Object::on_tick() / on_spawn() / on_despawn()
      │                          (gameplay code, dùng commands::Commands để spawn/despawn)
      │                          entity::Entity (metadata sống/chết, parent/children)
      ▼
world::World::snapshot()  ──calls──▶ object::Object::write_snapshot()
      │                              snapshot::SnapshotWriter
      ▼
snapshot::Snapshot / SnapshotItem
      │
      ▼
replication::SnapshotSender::next_message()  ──calls──▶ snapshot::diff()
      │  (Outgoing::Snapshot | Outgoing::Delta)
      ▼
replication::send_outgoing()  ──calls──▶ protocol::WireSnapshot::from() / WireDelta::from()
      │                                  protocol::Packet::new()
      ▼
net::Connection::send()  ──calls──▶ protocol::Packet::encode()  ──▶ TcpStream::write_all()

      (chiều ngược lại khi nhận)

TcpStream::read() ──▶ net::PacketReader::feed()/try_read_packet()
      │  ──calls──▶ protocol::Packet::peek_payload_len() / Packet::decode()
      ▼
net::Connection::recv() → protocol::Packet
      │
      ▼
protocol::WireSnapshot::from_bytes() / WireDelta::from_bytes()
      │  ──convert──▶ snapshot::Snapshot / SnapshotDelta
      ▼
replication::apply(old, delta, tick) → snapshot::Snapshot mới (dùng ở client/SDK)
```

---

## 4. Nguyên tắc ranh giới kiến trúc

Các quy tắc cứng do người dùng đặt ra ngày 2026-07-07, áp dụng cho mọi phân tích/đề xuất/code trong dự án:

1. **C++ chỉ là tài liệu tham khảo hành vi, không phải khuôn mẫu kiến trúc.** Khi đọc C++ phải phân tích domain rồi thiết kế lại API phù hợp cho Cyclone.
2. **Giai đoạn hiện tại: 1 crate duy nhất, chỉ tách module/domain.** Không tự ý đề xuất tách nhiều crate — việc tách crate chỉ làm sau khi ranh giới module đã ổn định.
3. **Ranh giới Engine/Gameplay tuyệt đối.** Engine không bao giờ được biết các khái niệm gameplay cụ thể (Tee, Hook, Weapon, Character, CTF, DDRace, Tuning, CharacterCore…). Chiều phụ thuộc luôn là Gameplay → Engine. Phép thử nhanh: xóa toàn bộ code gameplay, engine vẫn phải compile được.
4. **Snapshot độc lập hoàn toàn với Protocol.** Snapshot không được biết UDP/Packet/Ack/Huffman/Chunk. Chiều phụ thuộc: World → Snapshot → Protocol → Transport, không đảo ngược.
5. **Protocol chỉ là adapter** (Snapshot ↔ Packet), không thuộc gameplay, không phải trung tâm engine. Có thể có nhiều implementation song song (legacy, protocol mới, replay, debug) mà engine không cần biết đang dùng cái nào.
6. **Gameplay là plugin của Engine**, không thiết kế engine chỉ để phục vụ riêng Teeworlds.
7. **Không tối ưu/trừu tượng hoá quá sớm.** Không chia nhiều crate, ECS hoàn chỉnh, tối ưu compile, hay abstraction cho nhu cầu tương lai xa nếu hiện tại chưa cần thật.

Ngoài ra, hai nguyên tắc kỹ thuật quan sát được trong code (không phải quy tắc đặt ra trước, mà là pattern đã thực hiện nhất quán):

- **`Object` không bao giờ biết `World`, `Snapshot`, hay mạng** — chỉ biết `TickInfo`, `Commands`, `SnapshotWriter`.
- **Mỗi cặp tầng tách logic thuần khỏi I/O thật**: `TickScheduler` (thuần) vs `Runner` (chạm `Instant`); `PacketReader` (thuần) vs `Connection` (chạm `TcpStream`).

---

## 5. Ghi chú cho công việc tiếp theo

- `main.rs` chưa wire pipeline networking đầy đủ — cần ghép `net::Server` + `runtime::Runner` + `replication` thành 1 server loop thật (hiện chỉ có test `tests/net_loopback.rs` minh chứng cách ghép).
- Nhiều TODO trong code (ví dụ ở `sender.rs`, `connection.rs`, `reader.rs`) đánh dấu các điểm tối ưu hoá tương lai: giảm clone/allocation trong `SnapshotSender`/`send_outgoing`, gộp quản lý nhiều connection, tách trait `Transport` để hỗ trợ UDP song song với TCP.
- `EntityId.generation` hiện luôn = 0 — chưa có free-list/slot-reuse khi despawn rồi spawn lại vào cùng slot.
- Storage của `World` (`Vec<Box<dyn Object>>`) là lựa chọn tạm; sẽ đánh giá lại SoA/archetype/enum dispatch sau khi có benchmark thật (xem `benches/protocol_bench.rs`).
