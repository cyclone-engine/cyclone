//! apply() phải là phép toán ngược chính xác của diff(): với mọi
//! (old, new) hợp lệ, apply(old, diff(old, new), new.tick) == new.

use cyclone::replication::apply;
use cyclone::snapshot::{diff, Snapshot, SnapshotItem};
use cyclone::{Commands, Object, TickId, TickInfo, World};

struct Player {
    x: i32,
    y: i32,
}

impl Object for Player {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, _info: &TickInfo, _cmd: &mut Commands) {
        self.x += 1;
    }

    fn write_snapshot(&self, writer: &mut cyclone::snapshot::SnapshotWriter) {
        writer.write_i32(self.x);
        writer.write_i32(self.y);
    }
}

#[test]
fn apply_reverses_diff_for_updates() {
    let mut world = World::new();
    world.spawn(Player { x: 0, y: 5 });
    world.spawn(Player { x: 100, y: -3 });

    let old = world.snapshot(TickId(0));
    world.tick(TickId(0));
    world.tick(TickId(1));
    let new = world.snapshot(TickId(2));

    let delta = diff(Some(&old), &new);
    let applied = apply(&old, &delta, new.tick);

    assert_eq!(applied, new);
}

#[test]
fn apply_handles_spawn_update_remove_mixed() {
    let mut world = World::new();
    let a = world.spawn(Player { x: 0, y: 0 });
    let b = world.spawn(Player { x: 10, y: 10 });

    let old = Snapshot {
        tick: TickId(0),
        items: vec![
            SnapshotItem {
                id: a,
                type_id: 1,
                fields: vec![0, 0],
            },
            SnapshotItem {
                id: b,
                type_id: 1,
                fields: vec![10, 10],
            },
        ],
    };

    let c = world.spawn(Player { x: 99, y: 99 });
    let new = Snapshot {
        tick: TickId(1),
        items: vec![
            // a bị remove
            SnapshotItem {
                id: b,
                type_id: 1,
                fields: vec![11, 10], // update
            },
            SnapshotItem {
                id: c,
                type_id: 1,
                fields: vec![99, 99], // spawn
            },
        ],
    };

    let delta = diff(Some(&old), &new);
    let applied = apply(&old, &delta, new.tick);

    assert_eq!(applied, new);
}

#[test]
fn apply_from_empty_baseline_equals_full_snapshot() {
    let mut world = World::new();
    world.spawn(Player { x: 1, y: 2 });
    let new = world.snapshot(TickId(0));

    let empty = Snapshot::new(TickId(0));
    let delta = diff(None, &new);
    let applied = apply(&empty, &delta, new.tick);

    assert_eq!(applied, new);
}
