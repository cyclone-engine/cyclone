use std::cell::RefCell;
use std::rc::Rc;

use cyclone::{Commands, Object, TickId, TickInfo, World};

// Lưu ý: RefCell ở đây chỉ để test thu thập log quan sát được, không phải
// cơ chế truy cập trong lõi engine — Object vẫn chỉ nhận &mut self + Commands.
struct Recorder {
    value: i64,
    step: i64,
    log: Rc<RefCell<Vec<i64>>>,
}

impl Object for Recorder {
    fn type_id(&self) -> u32 {
        1
    }

    fn on_tick(&mut self, _info: &TickInfo, _cmd: &mut Commands) {
        self.value += self.step;
        self.log.borrow_mut().push(self.value);
    }
}

fn run_simulation() -> Vec<i64> {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut world = World::new();
    world.spawn(Recorder {
        value: 0,
        step: 3,
        log: log.clone(),
    });
    world.spawn(Recorder {
        value: 10,
        step: -1,
        log: log.clone(),
    });

    for t in 0..10 {
        world.tick(TickId(t));
    }

    log.borrow().clone()
}

#[test]
fn same_input_produces_same_output() {
    let first = run_simulation();
    let second = run_simulation();
    assert_eq!(first, second);
}
