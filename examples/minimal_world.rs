use cyclone::{Commands, InputBatch, Object, TickContext, TickId, World};

struct Counter {
    value: i64,
    step: i64,
}

impl Object for Counter {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, ctx: &TickContext, _cmd: &mut Commands) {
        self.value += self.step;
        println!(
            "tick {} | entity {:?} | value = {}",
            ctx.info.tick.0, ctx.info.id, self.value
        );
    }
}

fn main() {
    let mut world = World::new();
    world.spawn(Counter { value: 0, step: 1 });
    world.spawn(Counter {
        value: 100,
        step: -2,
    });

    for t in 0..5 {
        world.tick(TickId(t), &InputBatch::new());
    }
}
