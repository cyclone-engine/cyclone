//! Demo v0.4: chạy `GameServer` thật trên TCP, chấp nhận nhiều client, mỗi
//! client điều khiển 1 `DemoPlayer` bằng input gõ từ bàn phím (xem
//! `multiplayer_demo_client.rs`). Đây là "Definition of Done" của v0.4 —
//! không phải `cargo test` xanh, mà là chạy được server + 2 client thật
//! cùng lúc và thấy vị trí đổi theo input gõ vào.
//!
//! Chạy: `cargo run --example multiplayer_demo_server`
//! Rồi ở 2 terminal khác: `cargo run --example multiplayer_demo_client`

use cyclone::snapshot::SnapshotWriter;
use cyclone::{Commands, GameServer, Object, TickContext};

const ADDR: &str = "127.0.0.1:7878";

/// Gameplay tối thiểu: đọc byte đầu tiên của input (i8) làm delta vị trí
/// mỗi tick. `bytes` không có cấu trúc do engine định nghĩa — DemoPlayer tự
/// quyết định nghĩa của nó (đây chính là "input chỉ là bytes" ở
/// input/mod.rs: engine không biết gì về ý nghĩa 1 byte này).
struct DemoPlayer {
    position: i32,
}

impl Object for DemoPlayer {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, ctx: &TickContext, _cmd: &mut Commands) {
        if let Some(&byte) = ctx.input.and_then(|input| input.bytes.first()) {
            self.position += byte as i8 as i32;
        }
    }

    fn write_snapshot(&self, writer: &mut SnapshotWriter) {
        writer.write_i32(self.position);
    }
}

fn main() {
    let mut server = GameServer::bind(
        ADDR,
        20,
        |cmd: &mut Commands| {
            let id = cmd.spawn(DemoPlayer { position: 0 });
            println!("[server] player joined -> {id:?} (position = 0)");
            id
        },
        |id, cmd: &mut Commands| {
            println!("[server] player left -> {id:?}");
            cmd.despawn(id);
        },
    )
    .expect("bind failed");

    println!("[server] listening on {ADDR}, ticking at 20/s — Ctrl+C để dừng");
    server.run(|| true);
}
