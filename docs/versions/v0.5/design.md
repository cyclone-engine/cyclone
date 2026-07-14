# Cyclone v0.5 — Client SDK & hoàn thiện Multiplayer Pipeline

> World Geometry (Map/Collision/Vec2/Raycast) **không** làm ở v0.5 — dời sang
> v0.6. v0.5 chỉ hoàn thiện networking: sau v0.5, Rust client không phải tự
> xử lý TCP/Packet/baseline/apply nữa.

## Mục tiêu

v0.4 đã có `GameServer` + Input Pipeline + Snapshot Replication. v0.5 xây
`GameClient` SDK chính thức — API chuẩn để sau này SDK Unity/Unreal/Godot
bám theo.

## Bước 0 — `SnapshotDelta`/`WireDelta` mang tick thật

Hiện `SnapshotDelta`/`WireDelta` không có `tick`, client phải đoán
`old.tick + 1` — sai khi packet delay/mất/tới muộn.

```rust
// snapshot/delta.rs
pub struct SnapshotDelta {
    pub tick: TickId,
    pub items: Vec<DeltaItem>,
}
// diff(old, new) -> SnapshotDelta { tick: new.tick, items }

// protocol/delta.rs
pub struct WireDelta {
    pub tick: u64,
    pub items: Vec<WireDeltaItem>,
}

// replication/applier.rs
pub fn apply(old: &Snapshot, delta: &SnapshotDelta) -> Snapshot
// bỏ tham số tick rời — đọc delta.tick
```
Không bump `ProtocolVersion` (chưa có client production, chưa cần backward compat).

## Bước 1 — `src/client/` module

```
src/client/
    mod.rs             -- GameClient
    connection.rs       -- ClientConnection (reader thread + channel, mirror ClientSession)
    snapshot_cache.rs    -- SnapshotCache, ClientDiagnostics
    input.rs              -- LatestInput, OutgoingInput, send_input
    state.rs                -- ConnectionState
    error.rs                 -- ClientError
```
`pub use client::GameClient;` ở `lib.rs`.

## Bước 2 — `GameClient`

```rust
pub struct GameClient {
    connection: ClientConnection,
    snapshots: SnapshotCache,
    input: ClientInput,
    state: ConnectionState,
}

impl GameClient {
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self, ClientError>
    pub fn update(&mut self) -> ConnectionState
    pub fn snapshot(&self) -> Option<&Snapshot>
    pub fn push_input(&mut self, bytes: Vec<u8>)
    pub fn state(&self) -> ConnectionState
}
```
Game không biết TCP/Packet/wire format/delta apply.

## Bước 3 — `SnapshotCache`

```rust
pub struct ClientDiagnostics { pub decode_errors: u64, pub packets_received: u64 }

pub struct SnapshotCache {
    current: Option<Snapshot>,
    diagnostics: ClientDiagnostics,   // gộp thành struct riêng, không phải u64 rời
}
impl SnapshotCache {
    pub fn update(&mut self, packet: Packet)   // match packet.kind, decode, apply() lên baseline
    pub fn current(&self) -> Option<&Snapshot>
}
```
Decode lỗi: giữ nguyên state cũ, **không im lặng hoàn toàn** — đếm vào
`ClientDiagnostics.decode_errors` (`packets_received` cũng đếm mọi packet
tới, không chỉ lỗi), nội bộ, `#[cfg(test)]` mới expose getter, game không
bắt buộc đọc. Không throw mỗi frame. Gộp thành struct riêng (không phải
`u64` rời trong `SnapshotCache`) để có sẵn chỗ mở rộng — TODO (v0.6+):
`GameClient::diagnostics()`/`enum ClientEvent { DecodeError, Disconnected }`
khi có nhu cầu thật.

## Bước 4 — `LatestInput` (đổi tên từ `ClientInput`)

**Không đối xứng tuyệt đối** với `CurrentInput` phía server — 3 điểm khác
biệt có chủ đích, phát hiện khi review kỹ:

1. **Không có tick giả.** `WireInput`/`OutgoingInput` **không mang tick** —
   client v0.5 không có đồng hồ tick nào đáng tin để gửi; 1 bộ đếm cục bộ
   trông giống tick server sẽ gây nhầm lẫn thật khi sau này có lag
   compensation/input ack/replay/rollback. `InputFrame.tick` phía server
   (đọc trong `Object::on_tick` qua `ctx.input`) lấy từ tick **của server**
   tại lúc `ClientSession::drain_input()` nhận gói — không phải trường nào
   trên wire.
2. **`take()` consume, không phải `flush()` peek.** Server cần "trạng thái
   hiện hành" (tick liên tục, luôn cần input để dùng, không chờ). Client
   gửi command rời rạc ("nhấn phải" / "nhả phải") — nếu không consume,
   `push_input(right)` 1 lần sẽ bị gửi lại ở mọi `update()` sau đó mãi mãi,
   kể cả sau khi game đã "nhả phím". `take()` đảm bảo mỗi `push()` chỉ được
   gửi đúng 1 lần.
3. **Đổi tên `ClientInput` → `LatestInput`** để tên tự nói lên giới hạn:
   chỉ giữ đúng 1 giá trị mới nhất, không phải hàng đợi. `push(shoot)` rồi
   `push(move_right)` trong cùng 1 frame (trước khi `update()` kịp gửi) sẽ
   làm mất `shoot` — đúng với input dạng "trạng thái liên tục" (giữ phím di
   chuyển), SAI với input dạng lệnh rời rạc (bắn/nhảy/dash, mỗi lệnh phải
   tới server, không được ghi đè). Chưa sửa gì ở v0.5 — chỉ đặt tên trung
   thực; input dạng hàng đợi thật (`ClientInputQueue`) để dành khi có nhu
   cầu thật.

```rust
pub struct LatestInput {
    pending: Option<Vec<u8>>,
}
pub struct OutgoingInput { pub bytes: Vec<u8> }

pub(crate) fn send_input(conn: &mut ConnectionWriter, input: OutgoingInput) -> Result<(), ConnectionError>
```
`push_input(bytes)` ghi đè; `update()` gọi `take()` — consume, không gửi
lại nếu chưa `push()` gì mới. Không xử lý event queue/button history/key
repeat — đó là tầng gameplay. Cần "trạng thái hiện hành liên tục" kiểu
heartbeat/keepalive thì đó là tính năng khác, thêm riêng sau.

`send_input` sống trong `client::input`, **không** trong `replication`.
Input không phải "replication" (không có quyết định full/delta gì cả, chỉ
encode+gửi 1 chiều client → server) — đối xứng việc `SnapshotCache` tự
decode Snapshot/Delta thẳng thay vì qua `replication` khi nhận (Quyết định
#6, v0.4). `replication::packet_sender` giờ chỉ còn `send_snapshot`/
`send_delta`/`send_outgoing`.

## Bước 5 — `ClientConnection`

Tách `ConnectionReader`/`ConnectionWriter` giống `ClientSession`: reader
sống trên thread riêng, đẩy `Packet` thô qua channel (không tự decode ở
thread đọc).

```rust
pub(crate) fn drain(&mut self) -> Vec<Packet>   // sở hữu, không callback
pub(crate) fn is_disconnected(&self) -> bool
```
`drain()` trả `Vec<Packet>` sở hữu thay vì nhận callback `FnMut(Packet)` —
`GameClient::update()` lặp bằng `for` bình thường thay vì phải mượn
`self.connection` và `self.snapshots` cùng lúc qua closure. Không phải tối
ưu nhất (alloc `Vec` mỗi lần gọi) nhưng rõ ràng hơn và không vướng borrow
khi SDK mở rộng thêm việc phải làm lúc nhận packet (ví dụ emit event sau
này).

**Fix thread leak (`net::Connection`, ảnh hưởng cả `ClientConnection` lẫn
`ClientSession`):** trước đây drop `ConnectionWriter` một mình không đóng
fd của `ConnectionReader` đang sống trên thread khác (2 fd `try_clone()`
riêng biệt) — thread đọc treo mãi trong `recv()` nếu peer không tự đóng
trước. Thêm `impl Drop for ConnectionWriter { self.stream.shutdown(Both) }`
— `shutdown()` tác động toàn bộ socket kernel bên dưới, không riêng fd gọi
nó, nên `read()` đang block ở fd còn lại nhận EOF ngay. Test thực nghiệm:
`tests/net_loopback.rs::dropping_writer_alone_unblocks_reader_on_the_same_side`
(drop `ConnectionWriter` một mình, đo thời gian `ConnectionReader` trên
thread khác unblock — phải < 2s, không treo mãi).

**TODO (roadmap, production):** TCP không phát hiện peer chết ngay (rút
dây mạng, kill -9...) — `is_disconnected()` có thể không `true` trong thời
gian dài. Cần heartbeat/ping-pong + timeout ở tầng ứng dụng. Chưa cần cho
demo/v0.5.

Không có `update(dt)` ở v0.5 — client không chạy simulation cục bộ, chỉ
nhận state + gửi input. Thêm `dt` khi thật sự cần interpolation/prediction.

**Thứ tự trong `update()`: gửi input trước, nhận/áp dụng snapshot sau** —
input gửi ở frame này tác động tick KẾ TIẾP của server, không phải state
vừa nhận; gửi trước phản ánh đúng quan hệ nhân quả, dù về kỹ thuật 2 việc
độc lập trong cùng 1 lần gọi.

## Bước 6 — `ConnectionState`

```rust
pub enum ConnectionState { Connected, Disconnected }
```
Không có `Connecting` — `GameClient::connect()` là hàm blocking, khi trả
`Ok` thì đã ở `Connected` luôn, không có đường nào dẫn tới 1 trạng thái
"đang kết nối" quan sát được ở v0.5. Thêm lại khi có API connect bất đồng
bộ thật sự cần nó.

## Bước 7 — Không làm Clock Sync ở v0.5

Không thêm `client/clock.rs`. Chưa có mục đích sử dụng cụ thể (không
interpolation/prediction/rollback/lag-compensation ở v0.5) — `InputFrame.tick`
đã chứng minh tick client không cùng hệ quy chiếu server, thêm field/module
mà chưa có use-case rõ dễ lặp lại vết đó. Để dành tới khi có nhu cầu thật.

## Kiến trúc sau v0.5

```
Cyclone Core
server/   -- GameServer, ClientSession
client/   -- GameClient, SnapshotCache, LatestInput
protocol/ -- Snapshot, Delta, Input
net/      -- Connection (private), ConnectionReader/Writer, Server, Client
```

## Không làm trong v0.5 (dời sang v0.6 — World Geometry)

Vec2, TileMap, collision (`test_box`/`move_box`), raycast, Camera,
Animation, Sprite, Asset, Scene. Không có `PhysicsSystem`/`Simulation::tick()`/
AI system ở v0.6 — gameplay tự gọi `map.move_box()` trong `on_tick()` của
chính nó (xem thảo luận trong session — TileMap tĩnh, không cần đi qua
`TickContext` như `InputBatch`).

## Roadmap

v0.1 Core Object Model · v0.2 Snapshot · v0.3 Network Layer · v0.4
Multiplayer Server · **v0.5 Client SDK (hiện tại)** · v0.6 World Geometry ·
v0.7 Port game Rust đầu tiên.

Cyclone giữ vai trò networked simulation SDK, không biến thành game engine
có sẵn renderer/physics/UI.
