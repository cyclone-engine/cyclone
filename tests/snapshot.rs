use cyclone::snapshot::{diff, DeltaItem, Snapshot, SnapshotItem, SnapshotWriter};
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

    fn write_snapshot(&self, writer: &mut SnapshotWriter) {
        writer.write_i32(self.x);
        writer.write_i32(self.y);
    }
}

// Không override write_snapshot -> mặc định rỗng -> không xuất hiện trong
// snapshot, đúng hành vi "entity không muốn replicate".
struct ServerOnlyTimer;

impl Object for ServerOnlyTimer {
    fn type_id(&self) -> u32 {
        99
    }

    fn on_tick(&mut self, _info: &TickInfo, _cmd: &mut Commands) {}
}

#[test]
fn snapshot_contains_only_replicated_entities() {
    let mut world = World::new();
    world.spawn(Player { x: 0, y: 5 });
    world.spawn(ServerOnlyTimer);

    world.tick(TickId(0));

    let snap = world.snapshot(TickId(0));

    assert_eq!(snap.items.len(), 1);
    assert_eq!(snap.items[0].type_id, 1);
    assert_eq!(snap.items[0].fields, vec![1, 5]);
}

#[test]
fn delta_reports_only_changed_fields_as_diff() {
    let mut world = World::new();
    let id = world.spawn(Player { x: 0, y: 0 });

    let old = Snapshot {
        tick: TickId(0),
        items: vec![SnapshotItem {
            id,
            type_id: 1,
            fields: vec![100, 20, 50],
        }],
    };
    let new = Snapshot {
        tick: TickId(1),
        items: vec![SnapshotItem {
            id,
            type_id: 1,
            fields: vec![110, 20, 60],
        }],
    };

    let delta = diff(Some(&old), &new);

    assert_eq!(
        delta.items,
        vec![DeltaItem::Update {
            id,
            fields: vec![10, 0, 10],
        }]
    );
}

#[test]
fn delta_without_baseline_reports_everything_as_spawn() {
    let mut world = World::new();
    let id = world.spawn(Player { x: 0, y: 0 });

    let new = world.snapshot(TickId(0));
    let delta = diff(None, &new);

    assert_eq!(
        delta.items,
        vec![DeltaItem::Spawn {
            item: SnapshotItem {
                id,
                type_id: 1,
                fields: vec![0, 0],
            },
        }]
    );
}

fn run_and_snapshot() -> Snapshot {
    let mut world = World::new();
    world.spawn(Player { x: 0, y: 3 });
    world.spawn(Player { x: 10, y: -3 });

    for t in 0..20 {
        world.tick(TickId(t));
    }

    world.snapshot(TickId(20))
}

#[test]
fn snapshot_is_deterministic_across_independent_worlds() {
    let a = run_and_snapshot();
    let b = run_and_snapshot();
    assert_eq!(a.items, b.items);
}
