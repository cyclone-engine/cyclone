use cyclone::{Commands, Object, TickId, TickInfo, World};

struct Player {
    position: i64,
    velocity: i64,
}

impl Object for Player {
    fn on_tick(&mut self, info: &TickInfo, _cmd: &mut Commands) {
        self.position += self.velocity;
        println!(
            "[tick {}] entity {:?} position = {}",
            info.tick.0, info.id, self.position
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
        world.tick(TickId(t));
    }
}
