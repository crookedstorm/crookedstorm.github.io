//! Components attached to entities in the [`World`](crate::world::World).
//!
//! Components are plain structs, kept small so the simulation logic in
//! `systems.rs` reads like a straightforward transformation over them. New
//! gameplay behaviors add new structs here rather than new branches inside
//! a god-component.

use serde::Deserialize;
use serde::Serialize;

/// Integer tile-grid position for static entities: walls, treats, camp,
/// destinations. The simulation works in tile space for these; the player
/// uses [`GridMotion`] plus [`Transform`] for tile-aligned smooth movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

/// Continuous-space pixel position for the player. The renderer reads this
/// to blit the raccoon sprite; the movement system updates it by interpolating
/// between tile centers from the authoritative [`GridMotion`] state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
}

/// Per-frame pixel delta surfaced to the renderer. The movement system writes
/// the actual transform change applied this frame so consumers can derive a
/// sense of motion without owning the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Velocity {
    pub x: f32,
    pub y: f32,
}

/// Cardinal movement direction for buffered tile stepping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }
}

/// Authoritative player grid position plus any in-flight tile step. `tile_x`
/// and `tile_y` are the current centered tile while moving; once a step
/// finishes they advance to the destination tile. `buffered_direction` stores
/// the latest held direction so the next step can chain without an extra input
/// edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridMotion {
    pub tile_x: i32,
    pub tile_y: i32,
    pub active_direction: Option<Direction>,
    pub buffered_direction: Option<Direction>,
    pub frames_remaining: u32,
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

/// Treat varieties placed in each generated world. Each kind owns its display
/// name and score value so collection logic and future sprite selection share
/// one authoritative definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TreatKind {
    Cheeseburger,
    Snail,
    Frog,
    Banana,
    Cherries,
    Berries,
    Apple,
}

impl TreatKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Cheeseburger => "Cheeseburger",
            Self::Snail => "Snail",
            Self::Frog => "Little frog",
            Self::Banana => "Banana",
            Self::Cherries => "Cherries",
            Self::Berries => "Berries",
            Self::Apple => "Apple",
        }
    }

    pub fn value(self) -> u32 {
        match self {
            Self::Cheeseburger => 300,
            Self::Snail | Self::Frog => 100,
            Self::Banana | Self::Cherries | Self::Berries | Self::Apple => 50,
        }
    }
}

/// Treat-specific data attached alongside [`ObjectKind::Treat`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Treat {
    pub kind: TreatKind,
}

/// Whether a treat has been collected. Lets treats persist for accounting
/// but be skipped by render and collision systems once removed from play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Collected(pub bool);
