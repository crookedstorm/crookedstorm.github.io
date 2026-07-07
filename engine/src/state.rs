//! Serialization boundary between the Rust engine and the TypeScript layer.
//!
//! [`InitState`] is produced once on engine construction: the maze geometry,
//! static object positions, and the player start. [`FrameState`] is produced
//! every step: dynamic player state, score, status text, and any navigation
//! the engine is signalling to the renderer. [`Input`] flows the other way:
//! the pressed-key set for the frame.

use crate::components::Direction;
use serde::Deserialize;
use serde::Serialize;

/// Per-frame input gathered by TypeScript from keyboard state. The booleans
/// report which cardinal keys are held; `preferred_direction` records the most
/// recently pressed held direction so buffered grid movement can resolve turns
/// predictably when several keys are down.
#[derive(Clone, Copy, Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub preferred_direction: Option<Direction>,
    pub enter: bool,
}

/// Tile-space coordinate pair surfaced to the renderer.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilePos {
    pub x: i32,
    pub y: i32,
}

impl TilePos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Static destination surfaced once at init so the renderer can draw labels
/// and link targets without re-querying the engine.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DestinationInfo {
    pub x: i32,
    pub y: i32,
    pub href: String,
    pub label: String,
}

/// One-shot snapshot of the world geometry and static object placement.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitState {
    pub protocol_version: u32,
    pub width: i32,
    pub height: i32,
    pub tile_size: i32,
    pub walls: Vec<TilePos>,
    pub camp: TilePos,
    pub player_start: TilePos,
    pub destinations: Vec<DestinationInfo>,
    pub treats: Vec<TilePos>,
}

/// Per-frame snapshot consumed by the renderer. Carries only dynamic data so
/// the wasm-to-JS payload stays small.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameState {
    pub protocol_version: u32,
    pub player_x: f32,
    pub player_y: f32,
    pub player_vx: f32,
    pub player_vy: f32,
    pub score: u32,
    /// Live treat positions, excluding any collected so far.
    pub treats: Vec<TilePos>,
    pub status: String,
    /// `Some(href)` when the player is standing on a destination this frame.
    /// Lets the renderer prompt "press Enter to enter".
    pub active_destination_href: Option<String>,
    /// `Some(href)` when the engine requests a top-level page transition.
    /// TypeScript performs `window.location.href = ...` and the engine clears
    /// this on the next step.
    pub pending_navigation: Option<String>,
    /// Set the frame a treat is collected so the renderer can play a sound.
    pub just_collected_treat: bool,
}
