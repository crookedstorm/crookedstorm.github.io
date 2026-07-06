//! Lightweight, hand-rolled entity component system.
//!
//! Entities are plain `u32` indices. Components live in a `Vec` slot map
//! surfaced by [`World::component_mut`] and [`World::component`]. Systems
//! are plain functions that borrow the world; the caller chooses the
//! execution order. No parallelism, no queries beyond what the calling
//! system enumerates. This keeps the dependency surface small and the
//! migration path to `bevy_ecs` low-friction if the simulation grows.

use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;

/// Identifier for an entity. Stays stable even after other entities die.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Entity(pub u32);

/// Sparse storage of a single component type indexed by `Entity`.
///
/// Using `HashMap<Entity, Box<Any>>` keeps the implementation trivial to
/// read and lifts to a more cache-friendly layout only if profiling calls
/// for it later. Component instances are stored `Box`ed because Rust's
/// `Any` requires sized types with `'static` lifetimes.
struct ComponentStorage {
    entries: HashMap<Entity, Box<dyn Any>>,
}

impl ComponentStorage {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.entries.insert(entity, Box::new(component));
    }

    fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.entries.get(&entity)?.downcast_ref::<T>()
    }

    fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.entries.get_mut(&entity)?.downcast_mut::<T>()
    }

    fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.entries
            .get(&entity)
            .is_some_and(|component| component.is::<T>())
    }

    fn remove(&mut self, entity: Entity) {
        self.entries.remove(&entity);
    }

    fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entries.keys().copied()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The simulation root: holds entity ids and all component storages.
pub struct World {
    next_entity: u32,
    components: HashMap<TypeId, ComponentStorage>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_entity: 0,
            components: HashMap::new(),
        }
    }

    /// Spawn a new entity with no components attached.
    pub fn spawn(&mut self) -> Entity {
        let entity = Entity(self.next_entity);
        self.next_entity += 1;
        entity
    }

    /// Attach a component to an entity, replacing any prior of the same type.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.components
            .entry(TypeId::of::<T>())
            .or_insert_with(ComponentStorage::new)
            .insert(entity, component);
    }

    pub fn component<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.components.get(&TypeId::of::<T>())?.get::<T>(entity)
    }

    pub fn component_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.components
            .get_mut(&TypeId::of::<T>())?
            .get_mut::<T>(entity)
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.components
            .get(&TypeId::of::<T>())
            .is_some_and(|storage| storage.has::<T>(entity))
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) {
        if let Some(storage) = self.components.get_mut(&TypeId::of::<T>()) {
            storage.remove(entity);
        }
    }

    /// Iterator over every entity that owns component type `T`.
    /// Components stored under the same storage are unordered, so callers
    /// relying on stable order should collect and sort explicitly.
    pub fn entities_with<T: 'static>(&self) -> Vec<Entity> {
        self.components
            .get(&TypeId::of::<T>())
            .map(|storage| storage.entities().collect())
            .unwrap_or_default()
    }

    pub fn component_count<T: 'static>(&self) -> usize {
        self.components
            .get(&TypeId::of::<T>())
            .map(ComponentStorage::len)
            .unwrap_or_default()
    }

    pub fn entity_count(&self) -> u32 {
        self.next_entity
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Entity;
    use super::World;

    #[derive(PartialEq, Debug)]
    struct Position {
        x: i32,
        y: i32,
    }

    #[derive(PartialEq, Debug)]
    struct Tag(&'static str);

    #[test]
    fn spawn_assigns_unique_entities() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        assert_ne!(a, b);
    }

    #[test]
    fn insert_and_read_component() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Position { x: 1, y: 2 });

        assert_eq!(
            world.component::<Position>(entity),
            Some(&Position { x: 1, y: 2 })
        );
    }

    #[test]
    fn component_mut_updates_in_place() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Position { x: 0, y: 0 });

        if let Some(position) = world.component_mut::<Position>(entity) {
            position.x = 5;
            position.y = 7;
        }

        assert_eq!(
            world.component::<Position>(entity),
            Some(&Position { x: 5, y: 7 })
        );
    }

    #[test]
    fn entities_with_returns_owners_of_component() {
        let mut world = World::new();
        let entity_a = world.spawn();
        let entity_b = world.spawn();
        let entity_c = world.spawn();

        world.insert(entity_a, Tag("alpha"));
        world.insert(entity_c, Tag("gamma"));
        world.insert(entity_a, Position { x: 1, y: 1 });
        world.insert(entity_b, Position { x: 2, y: 2 });

        let tagged: Vec<Entity> = world.entities_with::<Tag>();
        assert_eq!(tagged.len(), 2);
        assert!(tagged.contains(&entity_a));
        assert!(tagged.contains(&entity_c));
        assert!(!tagged.contains(&entity_b));

        let positioned: Vec<Entity> = world.entities_with::<Position>();
        assert_eq!(positioned.len(), 2);
    }

    #[test]
    fn remove_component_detaches_safely() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Tag("alpha"));
        world.remove_component::<Tag>(entity);
        assert!(!world.has::<Tag>(entity));
    }

    #[test]
    fn missing_component_returns_none() {
        let mut world = World::new();
        let entity = world.spawn();
        assert!(world.component::<Position>(entity).is_none());
        assert!(!world.has::<Position>(entity));
    }
}
