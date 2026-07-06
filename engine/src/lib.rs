//! Game simulation engine for Wanderings and Misadventures.
//!
//! The engine owns world state, simulation logic, and procedural generation.
//! Rendering, DOM interaction, and input handling live in the Astro/TypeScript
//! layer and consume the [`FrameState`] produced here.

use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

/// Engine release identifier surfaced to TypeScript as a pipeline smoke test.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Full simulation snapshot returned to TypeScript each frame.
///
/// Kept empty for the scaffolding stage. Gameplay fields land here as systems
/// come online: player position, sprite ids, status text, score, destinations,
/// collected treats, and so on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameState {
    /// Bumped whenever the engine's public contract changes.
    pub protocol_version: u32,
}

impl FrameState {
    fn snapshot() -> Self {
        Self {
            protocol_version: 1,
        }
    }
}

/// Produce a fresh frame snapshot. With no systems wired up yet this returns a
/// constant `FrameState`, but it exercises the serde-wasm-bindgen bridge so the
/// TypeScript pipeline can be validated end to end during scaffolding.
#[wasm_bindgen]
pub fn step() -> JsValue {
    let state = FrameState::snapshot();
    serde_wasm_bindgen::to_value(&state).expect("FrameState serializes cleanly")
}
