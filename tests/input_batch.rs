//! World::tick() merge InputBatch với entries bằng 2 con trỏ song song
//! (giống snapshot::diff()/apply()) thay vì tra cứu ngẫu nhiên theo từng
//! entity — test này xác nhận merge đúng ở các trường hợp biên: entity có
//! input, entity không có input, và input "orphan" (id đã despawn, không
//! còn entity sống nào khớp).

use std::cell::RefCell;
use std::rc::Rc;

use cyclone::{Commands, InputBatch, InputFrame, Object, TickContext, TickId, World};

struct Recorder {
    log: Rc<RefCell<Vec<Option<Vec<u8>>>>>,
}

impl Object for Recorder {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, ctx: &TickContext, _cmd: &mut Commands) {
        self.log
            .borrow_mut()
            .push(ctx.input.map(|f| f.bytes.clone()));
    }
}

fn frame(tick: u64, byte: u8) -> InputFrame {
    InputFrame {
        tick: TickId(tick),
        bytes: vec![byte],
    }
}

#[test]
fn each_entity_receives_only_its_own_input() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut world = World::new();
    let a = world.spawn(Recorder { log: log.clone() });
    let b = world.spawn(Recorder { log: log.clone() });
    let c = world.spawn(Recorder { log: log.clone() });

    // Cố tình đưa vào KHÔNG theo thứ tự EntityId — from_items() phải tự sort.
    let batch = InputBatch::from_items(vec![
        (c, frame(0, 30)),
        (a, frame(0, 10)),
    ]);

    world.tick(TickId(0), &batch);

    let entries = log.borrow();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0], Some(vec![10])); // a có input
    assert_eq!(entries[1], None); // b không có input
    assert_eq!(entries[2], Some(vec![30])); // c có input

    let _ = b;
}

#[test]
fn orphan_input_for_despawned_entity_is_ignored_without_panicking() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut world = World::new();
    let a = world.spawn(Recorder { log: log.clone() });
    let b = world.spawn(Recorder { log: log.clone() });

    world.commands().despawn(a);
    world.tick(TickId(0), &InputBatch::new()); // flush despawn của a
    log.borrow_mut().clear();

    // Input trỏ tới `a` đã despawn (id nhỏ hơn b) + input hợp lệ cho `b`.
    let batch = InputBatch::from_items(vec![(a, frame(1, 77)), (b, frame(1, 5))]);
    world.tick(TickId(1), &batch);

    let entries = log.borrow();
    // Chỉ b tick (a đã despawn, không còn on_tick) — input orphan của a
    // không được gán nhầm sang b hay làm crash.
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0], Some(vec![5]));
}
