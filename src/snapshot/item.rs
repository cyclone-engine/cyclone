use crate::entity::EntityId;

/// Một entity trong Snapshot: identity (`id`) và metadata (`type_id`) tách
/// biệt rõ ràng khỏi state data (`fields`) — không trộn hai khái niệm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotItem {
    pub id: EntityId,
    pub type_id: u32,
    pub fields: Vec<i32>,
}
