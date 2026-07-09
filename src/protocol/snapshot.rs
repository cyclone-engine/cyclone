use crate::entity::EntityId;
use crate::snapshot::{Snapshot, SnapshotItem};

use super::codec::{
    read_i32, read_u16, read_u32, read_u64, write_i32, write_u16, write_u32, write_u64,
    MAX_FIELD_COUNT, MAX_ITEM_COUNT,
};
use super::error::ProtocolError;
use super::{WireDecode, WireEncode};

/// Wire representation của EntityId. Tách khỏi `crate::entity::EntityId`
/// có chủ đích: internal EntityId có thể đổi field/kiểu (ví dụ nén bit)
/// mà không làm đổi hợp đồng mạng, miễn convert (From) được cập nhật theo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetEntityId {
    pub index: u32,
    pub generation: u32,
}

impl From<EntityId> for NetEntityId {
    fn from(id: EntityId) -> Self {
        Self {
            index: id.index(),
            generation: id.generation(),
        }
    }
}

impl WireEncode for NetEntityId {
    fn encode(&self, buf: &mut Vec<u8>) {
        write_u32(buf, self.index);
        write_u32(buf, self.generation);
    }
}

impl WireDecode for NetEntityId {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError> {
        Ok(Self {
            index: read_u32(buf)?,
            generation: read_u32(buf)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireSnapshotItem {
    pub id: NetEntityId,
    pub type_id: u32,
    pub fields: Vec<i32>,
}

impl From<&SnapshotItem> for WireSnapshotItem {
    fn from(item: &SnapshotItem) -> Self {
        Self {
            id: item.id.into(),
            type_id: item.type_id,
            fields: item.fields.clone(),
        }
    }
}

impl WireEncode for WireSnapshotItem {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.id.encode(buf);
        write_u32(buf, self.type_id);
        // Encode là code nội bộ (không phải input mạng): nếu fields.len()
        // vượt u16, `as u16` sẽ wrap âm thầm và gửi packet sai mà không ai
        // biết. Panic ở đây — vượt MAX_FIELD_COUNT là bug trong Object,
        // không phải trạng thái hợp lệ cần xử lý mềm.
        assert!(
            self.fields.len() <= MAX_FIELD_COUNT as usize,
            "WireSnapshotItem has {} fields, exceeds MAX_FIELD_COUNT {MAX_FIELD_COUNT}",
            self.fields.len()
        );
        write_u16(buf, self.fields.len() as u16);
        for field in &self.fields {
            write_i32(buf, *field);
        }
    }
}

impl WireDecode for WireSnapshotItem {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError> {
        let id = NetEntityId::decode(buf)?;
        let type_id = read_u32(buf)?;
        let field_count = read_u16(buf)?;
        if field_count > MAX_FIELD_COUNT {
            return Err(ProtocolError::TooManyFields {
                count: field_count,
                max: MAX_FIELD_COUNT,
            });
        }
        let mut fields = Vec::with_capacity(field_count as usize);
        for _ in 0..field_count {
            fields.push(read_i32(buf)?);
        }
        Ok(Self { id, type_id, fields })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireSnapshot {
    pub tick: u64,
    pub items: Vec<WireSnapshotItem>,
}

impl From<&Snapshot> for WireSnapshot {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            tick: snapshot.tick.0,
            items: snapshot.items.iter().map(WireSnapshotItem::from).collect(),
        }
    }
}

impl WireEncode for WireSnapshot {
    fn encode(&self, buf: &mut Vec<u8>) {
        write_u64(buf, self.tick);
        assert!(
            self.items.len() <= MAX_ITEM_COUNT as usize,
            "WireSnapshot has {} items, exceeds MAX_ITEM_COUNT {MAX_ITEM_COUNT}",
            self.items.len()
        );
        write_u32(buf, self.items.len() as u32);
        for item in &self.items {
            item.encode(buf);
        }
    }
}

impl WireDecode for WireSnapshot {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError> {
        let tick = read_u64(buf)?;
        let item_count = read_u32(buf)?;
        if item_count > MAX_ITEM_COUNT {
            return Err(ProtocolError::TooManyItems {
                count: item_count,
                max: MAX_ITEM_COUNT,
            });
        }
        let mut items = Vec::with_capacity(item_count as usize);
        for _ in 0..item_count {
            items.push(WireSnapshotItem::decode(buf)?);
        }
        Ok(Self { tick, items })
    }
}
