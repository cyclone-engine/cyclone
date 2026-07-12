use cyclone::{Commands, InputBatch, Object, TickContext, TickId, World};

struct Player {
    position: i64,
    velocity: i64,
}

impl Object for Player {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, ctx: &TickContext, _cmd: &mut Commands) {
        self.position += self.velocity;
        println!(
            "[tick {}] entity {:?} position = {}",
            ctx.info.tick.0, ctx.info.id, self.position
        );
    }
}

fn main() {
    let mut world = World::new();
    world.spawn(Player {
        position: 0,
        velocity: 1,
    });

    for t in 0..5 {
        world.tick(TickId(t), &InputBatch::new());
    }
}
