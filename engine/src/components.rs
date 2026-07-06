//! Components attached to entities in the [`World`](crate::world::World).
//!
//! Components are plain structs, kept small so the simulation logic in
//! `systems.rs` reads like a straightforward transformation over them. New
//! gameplay behaviors add new structs here rather than new branches inside
//! a god-component.

use serde::Serialize;

/// Integer tile-grid position for static entities: walls, treats, camp,
/// destinations. The simulation works in tile space for these; the player
/// uses [`Transform`] for sub-tile smooth movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// Continuous-space pixel position for the player. The renderer reads this
/// to blit the raccoon sprite; collision converts to tile space when checking
/// against the maze grid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
}

/// Continuous-space velocity in pixels per step. Applied to a [`Transform`]
/// when integrating motion.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

/// Marks an entity as a solid wall occupying its tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Collider;

/// Kinds of objects that fill map tiles or override default rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectKind {
    Camp,
    Destination { section: Section },
    Treat,
}

/// Top-level site section navigated to from the world. Sections are stable
/// so Renaissance rendering and URL routing agree even as enums grow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Section {
    About,
    Blog,
    Projects,
}

impl Section {
    /// URL path navigated to when the player enters the destination.
    pub fn href(self) -> &'static str {
        match self {
            Self::About => "/about/",
            Self::Blog => "/blog/",
            Self::Projects => "/projects/",
        }
    }

    /// Human-readable label surfaced in the world status line.
    pub fn label(self) -> &'static str {
        match self {
            Self::About => "About Brooke",
            Self::Blog => "Field notes and posts",
            Self::Projects => "Projects and experiments",
        }
    }
}

/// Player-facing sprite identifier. Drawing lives in TypeScript; this only
/// selects which registered sprite sheet the renderer blits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpriteId {
    PlayerRaccoon,
    Camp,
    Treat,
    Destination,
}

/// Associates an entity with a visible sprite on the renderer side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Sprite(pub SpriteId);

/// Marks an entity as the player and carries its current lifecycle state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Player {
    /// Not-yet-banked points carried from treats since the last snapshot.
    pub score: u32,
    /// Set true on the frame a treat is collected, cleared by the snapshot.
    pub just_collected_treat: bool,
}

/// Whether a treat has been collected. Lets treats persist for accounting
/// but be skipped by render and collision systems once removed from play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Collected(pub bool);
