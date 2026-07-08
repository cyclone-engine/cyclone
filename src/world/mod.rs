use crate::commands::{Commands, PendingCommand};
use crate::entity::{Entity, EntityId};
use crate::object::{Object, TickInfo};
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

    /// Chạy đúng 1 tick: mỗi object tự cập nhật chính nó, không object nào
    /// có quyền truy cập object khác. Commands do các object phát ra được
    /// gom lại và áp dụng sau khi toàn bộ object đã tick xong (deterministic).
    pub fn tick(&mut self, tick: TickId) {
        for i in 0..self.entries.len() {
            if !self.entries[i].entity.alive || !self.entries[i].entity.enabled {
                continue;
            }
            let id = self.entries[i].entity.id;
            let info = TickInfo { id, tick };
            let mut cmd = Commands {
                next_index: &mut self.next_index,
                pending: &mut self.pending,
            };
            self.entries[i].object.on_tick(&info, &mut cmd);
        }
        self.flush(tick);
    }

    /// Xử lý đúng 1 pass các lệnh đang có tại thời điểm gọi. Lệnh mới phát
    /// sinh từ on_spawn/on_despawn (nếu object gọi cmd.spawn/despawn bên
    /// trong callback đó) sẽ nằm lại trong `self.pending` và đợi flush của
    /// tick kế tiếp — không xử lý đệ quy trong lần gọi này, để tránh treo
    /// engine nếu gameplay lỡ tạo chuỗi spawn/despawn nối tiếp vô hạn.
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

        let info = TickInfo { id, tick };
        let mut cmd = Commands {
            next_index: &mut self.next_index,
            pending: &mut self.pending,
        };
        self.entries[idx].object.on_spawn(&info, &mut cmd);
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

        let info = TickInfo { id, tick };
        let mut cmd = Commands {
            next_index: &mut self.next_index,
            pending: &mut self.pending,
        };
        self.entries[idx].object.on_despawn(&info, &mut cmd);
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
