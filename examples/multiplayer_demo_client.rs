//! Client demo cho `multiplayer_demo_server.rs`. Chạy nhiều lần (client1,
//! client2, ...) ở các terminal khác nhau để có nhiều player cùng lúc.
//!
//! 2 thread: 1 thread nền dùng `ConnectionReader` in liên tục state nhận
//! được từ server (không chặn việc gõ input); thread chính đọc mỗi dòng từ
//! stdin làm 1 số nguyên (-128..127), gửi qua `ConnectionWriter` làm input
//! của tick đó. `Client::connect()` trả sẵn cặp (reader, writer) đã tách —
//! Cyclone không có API nào trả về 1 connection 2 chiều chưa tách (xem
//! nguyên tắc "Opinionated API" trong `net::Connection`), nên đây không
//! phải lựa chọn của caller mà là cách duy nhất `Client::connect()` hoạt
//! động, đúng cho cả GameServer lẫn client thường như file này.
//!
//! Chạy: `cargo run --example multiplayer_demo_client`, gõ số rồi Enter.

use std::io::{self, BufRead, Write};
use std::thread;

use cyclone::net::Client;
use cyclone::protocol::{MessageKind, WireDelta, WireSnapshot};
use cyclone::replication::{apply, send_input};
use cyclone::snapshot::{Snapshot, SnapshotDelta};
use cyclone::TickId;

const ADDR: &str = "127.0.0.1:7878";

fn main() {
    let (mut reader, mut writer) =
        Client::connect(ADDR).expect("connect failed — server đã chạy chưa?");

    thread::spawn(move || {
        // Baseline tái tạo phía client: full Snapshot đầu tiên, sau đó mỗi
        // Delta được replication::apply() lên baseline này — chứng minh
        // pipeline decode + apply hoạt động đúng, không chỉ đếm byte nhận
        // được.
        let mut baseline: Option<Snapshot> = None;

        loop {
            match reader.recv() {
                Ok(packet) => match packet.kind {
                    MessageKind::Snapshot => match WireSnapshot::from_bytes(&packet.payload) {
                        Ok(wire) => {
                            let snapshot = Snapshot::from(&wire);
                            print_snapshot(&snapshot);
                            baseline = Some(snapshot);
                        }
                        Err(err) => println!("[client] snapshot decode error: {err}"),
                    },
                    MessageKind::Delta => match WireDelta::from_bytes(&packet.payload) {
                        Ok(wire) => {
                            let delta = SnapshotDelta::from(&wire);
                            let Some(old) = &baseline else {
                                println!("[client] nhận delta trước khi có baseline, bỏ qua");
                                continue;
                            };
                            // wire.tick không có trong WireDelta (chỉ Snapshot
                            // mới mang tick) — dùng tick gần nhất đã biết + 1
                            // chỉ để hiển thị, không ảnh hưởng nội dung state.
                            let next_tick = TickId(old.tick.0 + 1);
                            let snapshot = apply(old, &delta, next_tick);
                            print_snapshot(&snapshot);
                            baseline = Some(snapshot);
                        }
                        Err(err) => println!("[client] delta decode error: {err}"),
                    },
                    // Server không bao giờ gửi Input — chiều đó chỉ client -> server.
                    MessageKind::Input => {}
                },
                Err(_) => {
                    println!("[client] mất kết nối, thread đọc dừng");
                    break;
                }
            }
        }
    });

    println!("[client] đã kết nối {ADDR}. Gõ 1 số nguyên (vd 1, -1, 5) rồi Enter để di chuyển.");
    let stdin = io::stdin();
    let mut tick: u64 = 0;
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let delta: i8 = match line.trim().parse() {
            Ok(v) => v,
            Err(_) => {
                println!("[client] nhập 1 số nguyên từ -128 tới 127");
                continue;
            }
        };
        if send_input(&mut writer, TickId(tick), vec![delta as u8]).is_err() {
            println!("[client] gửi input thất bại — server có thể đã đóng");
            break;
        }
        tick += 1;
    }
}

fn print_snapshot(snapshot: &Snapshot) {
    print!("[client] tick {} |", snapshot.tick.0);
    for item in &snapshot.items {
        if let [pos] = item.fields[..] {
            print!(" entity {:?} pos={pos}", item.id);
        }
    }
    println!();
    let _ = io::stdout().flush();
}
