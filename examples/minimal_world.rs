use cyclone::{Commands, Object, TickId, TickInfo, World};

struct Counter {
    value: i64,
    step: i64,
}

impl Object for Counter {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, info: &TickInfo, _cmd: &mut Commands) {
        self.value += self.step;
        println!(
            "tick {} | entity {:?} | value = {}",
            info.tick.0, info.id, self.value
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
        world.tick(TickId(t));
    }
}
