use std::time::Instant;

use super::scheduler::TickScheduler;
use crate::input::InputBatch;
use crate::world::World;

/// Lớp duy nhất chạm vào wall-clock thật. Scheduler bên trong vẫn thuần túy.
pub struct Runner {
    scheduler: TickScheduler,
}

impl Runner {
    pub fn new(ticks_per_second: u32) -> Self {
        Self {
            scheduler: TickScheduler::new(ticks_per_second),
        }
    }

    /// Chỉ phục vụ offline simulation (không networking) — luôn tick với
    /// InputBatch rỗng. GameServer (v0.4) không dùng Runner: nó cần chèn
    /// drain input trước tick và broadcast snapshot sau tick, nên tự viết
    /// loop riêng bằng TickScheduler::advance() thay vì mở rộng Runner
    /// thành runtime mạng.
    pub fn run(&mut self, world: &mut World, mut should_continue: impl FnMut() -> bool) {
        let mut last = Instant::now();
        while should_continue() {
            let now = Instant::now();
            let elapsed = now - last;
            last = now;
            for tick in self.scheduler.advance(elapsed) {
                world.tick(tick, &InputBatch::new());
            }
        }
    }
}
