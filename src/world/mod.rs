use crate::commands::{Commands, PendingCommand};
use crate::entity::{Entity, EntityId};
use crate::input::InputBatch;
use crate::object::{Object, TickContext, TickInfo};
use crate::snapshot::{Snapshot, SnapshotItem, SnapshotWriter};
use crate::time::TickId;

/// Entity + Object của cùng một slot luôn đi cùng nhau — tránh trường hợp
/// hai Vec riêng biệt bị lệch index (entity chết nhưng object còn, hoặc
/// ngược lại).
struct ObjectEntry {
    entity: Entity,
    object: Box<dyn Object>,
}

/// World là nơi sở hữu toàn bộ tri thức: danh sách entity, thứ tự tick,
/// vòng đời spawn/despawn. Object không bao giờ có tham chiếu ngược lại World.
pub struct World {
    entries: Vec<ObjectEntry>,
    next_index: u32,
    pending: Vec<PendingCommand>,
}

impl World {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_index: 0,
            pending: Vec::new(),
        }
    }

    /// Spawn trực tiếp, dùng khi thiết lập World ban đầu (trước khi tick chạy).
    /// Flush ngay với TickId(0) — CHỈ đúng lúc setup, trước vòng tick thật.
    /// Không dùng hàm này để spawn khi World đã tick được 1 lúc rồi (ví dụ
    /// player join giữa ván) — on_spawn() sẽ nhận nhầm tick 0. Muốn spawn
    /// đúng tick hiện tại lúc World đang chạy, dùng `commands()` rồi để
    /// `tick()` kế tiếp tự flush với tick thật.
    pub fn spawn<T: Object + 'static>(&mut self, obj: T) -> EntityId {
        let id = {
            let mut cmd = Commands {
                next_index: &mut self.next_index,
                pending: &mut self.pending,
            };
            cmd.spawn(obj)
        };
        self.flush(TickId(0));
        id
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entries
            .get(id.index() as usize)
            .map(|e| &e.entity)
            .filter(|e| e.id == id)
    }

    /// Commands dùng ngoài vòng tick bình thường (ví dụ GameServer spawn
    /// entity cho connection mới). Lệnh chỉ vào hàng đợi `pending` — không
    /// flush ngay ở đây, việc flush thật sự chờ `tick()` kế tiếp gọi, nên
    /// on_spawn/on_despawn luôn nhận đúng tick hiện tại, không hardcode.
    /// Đây là API queue DUY NHẤT của World — Object::on_tick cũng nhận
    /// Commands dựng theo đúng cách này, tránh 2 API cùng ý nghĩa.
    pub fn commands(&mut self) -> Commands<'_> {
        Commands {
            next_index: &mut self.next_index,
            pending: &mut self.pending,
        }
    }

    /// Chạy đúng 1 tick: mỗi object tự cập nhật chính nó, không object nào
    /// có quyền truy cập object khác. Commands do các object phát ra được
    /// gom lại và áp dụng sau khi toàn bộ object đã tick xong (deterministic).
    ///
    /// `inputs` được merge với `entries` bằng 2 con trỏ song song (giống
    /// `snapshot::diff()`/`apply()`), không tra cứu ngẫu nhiên theo từng
    /// entity — vì `entries` đã tự nhiên theo thứ tự EntityId tăng dần
    /// (index trực tiếp bằng EntityId.index(), xem apply_spawn) và
    /// `InputBatch` luôn giữ bất biến đã sort. Độ phức tạp O(entities +
    /// input.len()), không phụ thuộc cách nào lớn hơn cách kia.
    pub fn tick(&mut self, tick: TickId, inputs: &InputBatch) {
        let mut input_iter = inputs.iter().peekable();

        for i in 0..self.entries.len() {
            let id = self.entries[i].entity.id;

            // Input orphan (trỏ tới id nhỏ hơn id hiện tại — entity đã
            // despawn, disabled, hoặc chưa từng tồn tại) bị bỏ qua ở đây,
            // kể cả khi entity hiện tại không alive/enabled: nếu vậy vòng
            // while này sẽ tự loại input của nó ở lượt entity kế tiếp.
            while let Some((iid, _)) = input_iter.peek() {
                if *iid < id {
                    input_iter.next();
                } else {
                    break;
                }
            }

            if !self.entries[i].entity.alive || !self.entries[i].entity.enabled {
                continue;
            }

            let input = match input_iter.peek() {
                Some((iid, _)) if *iid == id => {
                    let (_, frame) = input_iter.next().unwrap();
                    Some(frame)
                }
                _ => None,
            };

            let ctx = TickContext {
                info: TickInfo { id, tick },
                input,
            };
            let mut cmd = Commands {
                next_index: &mut self.next_index,
                pending: &mut self.pending,
            };
            self.entries[i].object.on_tick(&ctx, &mut cmd);
        }
        self.flush(tick);
    }

    /// Tổng hợp state hiện tại thành 1 Snapshot: mỗi entity còn sống tự
    /// nguyện phơi field của mình qua write_snapshot(), World chỉ đứng ra
    /// gọi lần lượt — đây là 1 read-pass thuần (&self khắp nơi), không
    /// đụng Commands, không có vấn đề mượn nào cả.
    ///
    /// TODO (roadmap): `World` hiện là nguồn Snapshot duy nhất trong crate.
    /// Nếu sau này cần cắm backend ECS khác (Bevy, Hecs, Flecs...) vào cùng
    /// pipeline replication, đây là chỗ tách 1 trait kiểu
    /// `ReplicationBackend { fn snapshot(&self, tick: TickId) -> Snapshot }`.
    /// Chưa tách bây giờ vì chỉ có 1 implementer (World) — trait với đúng 1
    /// impl không phải điểm mở rộng thật, chữ ký (&self vs ECS World khác,
    /// cần query gì, tick truyền sao) gần như chắc chắn phải đổi khi có
    /// implementer thứ 2 thật sự xuất hiện.
    pub fn snapshot(&self, tick: TickId) -> Snapshot {
        let mut snapshot = Snapshot::new(tick);
        for entry in &self.entries {
            if !entry.entity.alive {
                continue;
            }
            let mut writer = SnapshotWriter::new();
            entry.object.write_snapshot(&mut writer);
            if writer.is_empty() {
                // Không ghi gì -> không replicate (ví dụ AIController, Timer).
                continue;
            }
            snapshot.push(SnapshotItem {
                id: entry.entity.id,
                type_id: entry.object.type_id(),
                fields: writer.finish(),
            });
        }
        snapshot.sort();
        snapshot
    }

    /// Xử lý đúng 1 pass các lệnh đang có tại thời điểm gọi. Lệnh mới phát
    /// sinh từ on_spawn/on_despawn (nếu object gọi cmd.spawn/despawn bên
    /// trong callback đó) sẽ nằm lại trong `self.pending` và đợi flush của
    /// tick kế tiếp — không xử lý đệ quy trong lần gọi này, để tránh treo
    /// engine nếu gameplay lỡ tạo chuỗi spawn/despawn nối tiếp nhau.
    fn flush(&mut self, tick: TickId) {
        let ops = std::mem::take(&mut self.pending);
        for op in ops {
            match op {
                PendingCommand::Spawn {
                    id,
                    parent,
                    object,
                } => self.apply_spawn(id, parent, object, tick),
                PendingCommand::Despawn(id) => self.despawn_entity(id, tick),
            }
        }
    }

    fn apply_spawn(
        &mut self,
        id: EntityId,
        parent: Option<EntityId>,
        object: Box<dyn Object>,
        tick: TickId,
    ) {
        let entity = Entity {
            id,
            enabled: true,
            alive: true,
            parent,
            children: Vec::new(),
        };
        let idx = id.index() as usize;
        let entry = ObjectEntry { entity, object };
        if idx == self.entries.len() {
            self.entries.push(entry);
        } else {
            self.entries[idx] = entry;
        }

        if let Some(pid) = parent {
            if let Some(p) = self.entries.get_mut(pid.index() as usize) {
                if p.entity.id == pid {
                    p.entity.children.push(id);
                }
            }
        }

        let ctx = TickContext {
            info: TickInfo { id, tick },
            input: None,
        };
        let mut cmd = Commands {
            next_index: &mut self.next_index,
            pending: &mut self.pending,
        };
        self.entries[idx].object.on_spawn(&ctx, &mut cmd);
    }

    /// Đệ quy xuống children khi entity bị despawn (cascade), và dọn tham
    /// chiếu id khỏi parent.children nếu entity bị despawn lẻ (không qua
    /// cascade). An toàn không vô hạn vì cây parent/children hiện chỉ được
    /// tạo lúc spawn_with_parent (child luôn là entity mới) nên không thể
    /// có chu trình — giả định này sẽ cần xét lại nếu sau này thêm Reparent.
    fn despawn_entity(&mut self, id: EntityId, tick: TickId) {
        let idx = id.index() as usize;
        if idx >= self.entries.len() || self.entries[idx].entity.id != id || !self.entries[idx].entity.alive
        {
            return;
        }

        if let Some(pid) = self.entries[idx].entity.parent {
            if let Some(p) = self.entries.get_mut(pid.index() as usize) {
                if p.entity.id == pid {
                    p.entity.children.retain(|c| *c != id);
                }
            }
        }

        let children = self.entries[idx].entity.children.clone();

        let ctx = TickContext {
            info: TickInfo { id, tick },
            input: None,
        };
        let mut cmd = Commands {
            next_index: &mut self.next_index,
            pending: &mut self.pending,
        };
        self.entries[idx].object.on_despawn(&ctx, &mut cmd);
        self.entries[idx].entity.alive = false;

        for child in children {
            self.despawn_entity(child, tick);
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
