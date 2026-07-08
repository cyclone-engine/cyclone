use std::time::Duration;

use crate::time::TickId;

/// Logic thuần: không đọc wall-clock, không I/O. Nhận elapsed time,
/// trả về danh sách tick cần chạy. Test/replay/benchmark có thể drive
/// trực tiếp mà không cần Runner thật.
pub struct TickScheduler {
    tick_duration: Duration,
    accumulator: Duration,
    current_tick: u64,
}

impl TickScheduler {
    pub fn new(ticks_per_second: u32) -> Self {
        Self {
            tick_duration: Duration::from_secs_f64(1.0 / ticks_per_second as f64),
            accumulator: Duration::ZERO,
            current_tick: 0,
        }
    }

    pub fn advance(&mut self, elapsed: Duration) -> Vec<TickId> {
        self.accumulator += elapsed;
        let mut ticks = Vec::new();
        while self.accumulator >= self.tick_duration {
            self.accumulator -= self.tick_duration;
            ticks.push(TickId(self.current_tick));
            self.current_tick += 1;
        }
        ticks
    }
}
