use super::EntityId;

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub enabled: bool,
    pub alive: bool,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
}
