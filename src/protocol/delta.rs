use crate::snapshot::{DeltaItem, SnapshotDelta};

use super::codec::{
    read_i32, read_u16, read_u32, read_u8, write_i32, write_u16, write_u32, write_u8,
    MAX_FIELD_COUNT, MAX_ITEM_COUNT,
};
use super::error::ProtocolError;
use super::snapshot::{NetEntityId, WireSnapshotItem};
use super::{WireDecode, WireEncode};

const KIND_SPAWN: u8 = 0;
const KIND_UPDATE: u8 = 1;
const KIND_REMOVE: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireDeltaItem {
    Spawn(WireSnapshotItem),
    Update { id: NetEntityId, fields: Vec<i32> },
    Remove { id: NetEntityId },
}

impl From<&DeltaItem> for WireDeltaItem {
    fn from(item: &DeltaItem) -> Self {
        match item {
            DeltaItem::Spawn { item } => WireDeltaItem::Spawn(item.into()),
            DeltaItem::Update { id, fields } => WireDeltaItem::Update {
                id: (*id).into(),
                fields: fields.clone(),
            },
            DeltaItem::Remove { id } => WireDeltaItem::Remove { id: (*id).into() },
        }
    }
}

impl From<&WireDeltaItem> for DeltaItem {
    fn from(item: &WireDeltaItem) -> Self {
        match item {
            WireDeltaItem::Spawn(item) => DeltaItem::Spawn { item: item.into() },
            WireDeltaItem::Update { id, fields } => DeltaItem::Update {
                id: (*id).into(),
                fields: fields.clone(),
            },
            WireDeltaItem::Remove { id } => DeltaItem::Remove { id: (*id).into() },
        }
    }
}

impl WireEncode for WireDeltaItem {
    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            WireDeltaItem::Spawn(item) => {
                write_u8(buf, KIND_SPAWN);
                item.encode(buf);
            }
            WireDeltaItem::Update { id, fields } => {
                write_u8(buf, KIND_UPDATE);
                id.encode(buf);
                assert!(
                    fields.len() <= MAX_FIELD_COUNT as usize,
                    "WireDeltaItem::Update has {} fields, exceeds MAX_FIELD_COUNT {MAX_FIELD_COUNT}",
                    fields.len()
                );
                write_u16(buf, fields.len() as u16);
                for field in fields {
                    write_i32(buf, *field);
                }
            }
            WireDeltaItem::Remove { id } => {
                write_u8(buf, KIND_REMOVE);
                id.encode(buf);
            }
        }
    }
}

impl WireDecode for WireDeltaItem {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError> {
        match read_u8(buf)? {
            KIND_SPAWN => Ok(WireDeltaItem::Spawn(WireSnapshotItem::decode(buf)?)),
            KIND_UPDATE => {
                let id = NetEntityId::decode(buf)?;
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
                Ok(WireDeltaItem::Update { id, fields })
            }
            KIND_REMOVE => Ok(WireDeltaItem::Remove {
                id: NetEntityId::decode(buf)?,
            }),
            other => Err(ProtocolError::InvalidDeltaItemKind(other)),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WireDelta {
    pub items: Vec<WireDeltaItem>,
}

impl From<&SnapshotDelta> for WireDelta {
    fn from(delta: &SnapshotDelta) -> Self {
        Self {
            items: delta.items.iter().map(WireDeltaItem::from).collect(),
        }
    }
}

impl From<&WireDelta> for SnapshotDelta {
    fn from(wire: &WireDelta) -> Self {
        Self {
            items: wire.items.iter().map(DeltaItem::from).collect(),
        }
    }
}

impl WireEncode for WireDelta {
    fn encode(&self, buf: &mut Vec<u8>) {
        assert!(
            self.items.len() <= MAX_ITEM_COUNT as usize,
            "WireDelta has {} items, exceeds MAX_ITEM_COUNT {MAX_ITEM_COUNT}",
            self.items.len()
        );
        write_u32(buf, self.items.len() as u32);
        for item in &self.items {
            item.encode(buf);
        }
    }
}

impl WireDecode for WireDelta {
    fn decode(buf: &mut &[u8]) -> Result<Self, ProtocolError> {
        let item_count = read_u32(buf)?;
        if item_count > MAX_ITEM_COUNT {
            return Err(ProtocolError::TooManyItems {
                count: item_count,
                max: MAX_ITEM_COUNT,
            });
        }
        let mut items = Vec::with_capacity(item_count as usize);
        for _ in 0..item_count {
            items.push(WireDeltaItem::decode(buf)?);
        }
        Ok(Self { items })
    }
}

impl WireDelta {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode(&mut buf);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let mut cursor = bytes;
        let value = Self::decode(&mut cursor)?;
        if !cursor.is_empty() {
            return Err(ProtocolError::TrailingBytes);
        }
        Ok(value)
    }
}
