//! Client demo cho `multiplayer_demo_server.rs` — v0.5: dùng `GameClient`
//! SDK, không tự xử lý TCP/Packet/baseline/apply Delta nữa (so với bản v0.4
//! trước đây, xem lịch sử git). Chạy nhiều lần (client1, client2, ...) ở
//! các terminal khác nhau để có nhiều player cùng lúc.
//!
//! Đọc stdin trên 1 thread riêng (blocking), đẩy số đã gõ qua channel —
//! vòng lặp chính không bị chặn bởi việc chờ người dùng gõ phím, vẫn gọi
//! `client.update()` đều đặn để nhận state mới liên tục.
//!
//! Chạy: `cargo run --example multiplayer_demo_client`, gõ số rồi Enter.

use std::io::{self, BufRead, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cyclone::snapshot::Snapshot;
use cyclone::{ConnectionState, GameClient};

const ADDR: &str = "127.0.0.1:7878";

fn main() {
    let mut client =
        GameClient::connect(ADDR).expect("connect failed — server đã chạy chưa?");

    println!("[client] đã kết nối {ADDR}. Gõ 1 số nguyên (vd 1, -1, 5) rồi Enter để di chuyển.");

    let (tx, rx) = mpsc::channel::<i8>();
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            match line.trim().parse::<i8>() {
                Ok(delta) => {
                    if tx.send(delta).is_err() {
                        break;
                    }
                }
                Err(_) => println!("[client] nhập 1 số nguyên từ -128 tới 127"),
            }
        }
    });

    let mut last_printed_tick: Option<u64> = None;
    loop {
        loop {
            match rx.try_recv() {
                Ok(delta) => client.push_input(vec![delta as u8]),
                Err(mpsc::TryRecvError::Empty) => break,
                // stdin đóng (Ctrl+D hoặc EOF) -> không còn input mới nào
                // nữa, thoát demo — khác với thực tế production nơi
                // GameClient nên tiếp tục chạy dù không còn input tới.
                Err(mpsc::TryRecvError::Disconnected) => {
                    println!("[client] stdin đóng, thoát");
                    return;
                }
            }
        }

        if client.update() == ConnectionState::Disconnected {
            println!("[client] mất kết nối");
            break;
        }

        if let Some(snapshot) = client.snapshot()
            && last_printed_tick != Some(snapshot.tick.0)
        {
            print_snapshot(snapshot);
            last_printed_tick = Some(snapshot.tick.0);
        }

        thread::sleep(Duration::from_millis(16));
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
