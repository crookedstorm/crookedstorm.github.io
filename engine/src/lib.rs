//! Game simulation engine for Wanderings and Misadventures.
//!
//! The engine owns world state, simulation logic, and procedural generation.
//! Rendering, DOM interaction, and input handling live in the Astro/TypeScript
//! layer and consume the [`state::InitState`] and [`state::FrameState`] produced
//! here.

pub mod components;
pub mod maze;
pub mod rng;
pub mod state;
pub mod systems;
pub mod world;

use crate::components::GridMotion;
use crate::components::ObjectKind;
use crate::components::Player;
use crate::components::Position;
use crate::components::Sprite;
use crate::components::SpriteId;
use crate::components::Transform;
use crate::components::Treat;
use crate::components::TreatKind;
use crate::components::Velocity;
use crate::maze::Maze;
use crate::maze::MazeConfig;
use crate::rng::Rng;
use crate::state::DestinationInfo;
use crate::state::FrameState;
use crate::state::InitState;
use crate::state::Input;
use crate::state::TilePos;
use crate::state::TreatInfo;
use crate::systems::TILE_SIZE;
use crate::systems::TREAT_MESSAGE_FRAMES;
use crate::world::Entity;
use crate::world::World;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

/// Bumped whenever the engine's public contract changes.
const PROTOCOL_VERSION: u32 = 5;

/// Engine release identifier surfaced to TypeScript as a pipeline smoke test.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Owning simulation context. Constructed once per page load with a seed,
/// then stepped each animation frame with the current input. The wasm-bindgen
/// generated bindings surface this as a JS class: `new Engine(seed)`.
#[wasm_bindgen]
pub struct Engine {
    world: World,
    maze: Maze,
    player: Entity,
    active_destination_href: Option<String>,
    pending_navigation: Option<String>,
    last_collected_treat: Option<TreatKind>,
    treat_message_frames: u32,
}

#[wasm_bindgen]
impl Engine {
    /// Build a fresh engine for the given seed. The same seed reproduces the
    /// same maze, treat placement, and destination layout across runs.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> Engine {
        let mut rng = Rng::from_seed(seed);
        let mut world = World::new();
        let maze = Maze::new(MazeConfig::default(), &mut rng);
        maze.spawn_walls(&mut world);
        maze.populate(&mut world, &mut rng);

        let camp_pos = find_camp(&world).unwrap_or(Position { x: 1, y: 1 });
        let player = world.spawn();
        world.insert(
            player,
            Transform {
                x: camp_pos.x as f32 * TILE_SIZE + TILE_SIZE / 2.0,
                y: camp_pos.y as f32 * TILE_SIZE + TILE_SIZE / 2.0,
            },
        );
        world.insert(player, Velocity { x: 0.0, y: 0.0 });
        world.insert(
            player,
            GridMotion {
                tile_x: camp_pos.x,
                tile_y: camp_pos.y,
                active_direction: None,
                buffered_direction: None,
                frames_remaining: 0,
            },
        );
        world.insert(player, Player::default());
        world.insert(player, Sprite(SpriteId::PlayerRaccoon));

        Engine {
            world,
            maze,
            player,
            active_destination_href: None,
            pending_navigation: None,
            last_collected_treat: None,
            treat_message_frames: 0,
        }
    }

    /// One-time geometry and static-object snapshot for the renderer.
    pub fn init_state(&self) -> JsValue {
        let walls = collect_walls(&self.world);
        let camp = find_camp(&self.world).unwrap_or(Position { x: 1, y: 1 });
        let destinations = collect_destinations(&self.world);
        let treats = collect_treats(&self.world);

        let player_start = self
            .world
            .component::<GridMotion>(self.player)
            .map(|motion| TilePos::new(motion.tile_x, motion.tile_y))
            .or_else(|| {
                self.world
                    .component::<Transform>(self.player)
                    .map(|transform| {
                        TilePos::new(
                            (transform.x / TILE_SIZE) as i32,
                            (transform.y / TILE_SIZE) as i32,
                        )
                    })
            })
            .unwrap_or_else(|| TilePos::new(camp.x, camp.y));

        let state = InitState {
            protocol_version: PROTOCOL_VERSION,
            width: self.maze.width,
            height: self.maze.height,
            tile_size: TILE_SIZE as i32,
            walls,
            camp: TilePos::new(camp.x, camp.y),
            player_start,
            destinations,
            treats,
        };

        serde_wasm_bindgen::to_value(&state).expect("InitState serializes cleanly")
    }

    /// Advance the simulation one frame given the input state, returning the
    /// dynamic snapshot the renderer should draw this frame.
    pub fn step(&mut self, input: JsValue) -> JsValue {
        let input: Input = serde_wasm_bindgen::from_value(input).unwrap_or_default();

        systems::apply_input(&mut self.world, self.player, &input);
        systems::move_with_collision(&mut self.world, &self.maze, self.player);

        let collected_this_frame = systems::collect_treats(&mut self.world, self.player);
        if let Some(kind) = collected_this_frame {
            self.last_collected_treat = Some(kind);
            self.treat_message_frames = TREAT_MESSAGE_FRAMES;
        } else if let Some(player_state) = self.world.component_mut::<Player>(self.player) {
            // The just-collected flag is only set on the frame a treat is
            // picked up; clear it on every other frame.
            player_state.just_collected_treat = false;
        }

        let active_section = systems::active_destination(&self.world, self.player);
        self.active_destination_href = active_section.map(|section| section.href().to_owned());

        if input.enter
            && let Some(href) = &self.active_destination_href
        {
            self.pending_navigation = Some(href.clone());
        }

        let status = self.compute_status();
        if self.treat_message_frames > 0 {
            self.treat_message_frames -= 1;
        }

        let live_treats = collect_live_treats(&self.world);
        let snapshot = self.snapshot(status, live_treats);
        serde_wasm_bindgen::to_value(&snapshot).expect("FrameState serializes cleanly")
    }
}

impl Engine {
    fn compute_status(&self) -> String {
        if self.treat_message_frames > 0
            && let Some(kind) = self.last_collected_treat
        {
            return format!("{} acquired! +{}", kind.display_name(), kind.value());
        }

        if let Some(href) = &self.active_destination_href {
            let label = section_label_for_href(href).unwrap_or("");
            return format!("{label}. Press Enter to enter.");
        }

        if systems::is_near_camp(&self.world, self.player) {
            return "Standing at camp.".to_owned();
        }

        "Adventuring…".to_owned()
    }

    fn snapshot(&mut self, status: String, treats: Vec<TreatInfo>) -> FrameState {
        let player = self
            .world
            .component::<Player>(self.player)
            .copied()
            .unwrap_or_default();
        let transform = self
            .world
            .component::<Transform>(self.player)
            .copied()
            .unwrap_or(Transform { x: 0.0, y: 0.0 });
        let velocity = self
            .world
            .component::<Velocity>(self.player)
            .copied()
            .unwrap_or(Velocity { x: 0.0, y: 0.0 });

        let pending_navigation = self.pending_navigation.take();

        FrameState {
            protocol_version: PROTOCOL_VERSION,
            player_x: transform.x,
            player_y: transform.y,
            player_vx: velocity.x,
            player_vy: velocity.y,
            score: player.score,
            treats,
            status,
            active_destination_href: self.active_destination_href.clone(),
            pending_navigation,
            just_collected_treat: player.just_collected_treat,
        }
    }
}

fn find_camp(world: &World) -> Option<Position> {
    for entity in world.entities_with::<ObjectKind>() {
        if let Some(ObjectKind::Camp) = world.component::<ObjectKind>(entity)
            && let Some(position) = world.component::<Position>(entity)
        {
            return Some(*position);
        }
    }
    None
}

fn collect_walls(world: &World) -> Vec<TilePos> {
    let mut walls = Vec::new();
    for entity in world.entities_with::<crate::components::Collider>() {
        if let Some(Position { x, y }) = world.component::<Position>(entity) {
            walls.push(TilePos::new(*x, *y));
        }
    }
    walls
}

fn collect_destinations(world: &World) -> Vec<DestinationInfo> {
    let mut destinations = Vec::new();
    for entity in world.entities_with::<ObjectKind>() {
        if let Some(ObjectKind::Destination { section }) = world.component::<ObjectKind>(entity)
            && let Some(Position { x, y }) = world.component::<Position>(entity)
        {
            destinations.push(DestinationInfo {
                x: *x,
                y: *y,
                href: section.href().to_owned(),
                label: section.label().to_owned(),
            });
        }
    }
    destinations
}

fn collect_treats(world: &World) -> Vec<TreatInfo> {
    let mut treats = Vec::new();
    for entity in world.entities_with::<ObjectKind>() {
        if let Some(ObjectKind::Treat) = world.component::<ObjectKind>(entity)
            && let Some(Position { x, y }) = world.component::<Position>(entity)
            && let Some(Treat { kind }) = world.component::<Treat>(entity)
        {
            treats.push(TreatInfo::new(*x, *y, *kind));
        }
    }
    treats
}

/// Treats that have not yet been collected, including the kind the renderer
/// uses to select an asset.
fn collect_live_treats(world: &World) -> Vec<TreatInfo> {
    let mut treats = Vec::new();
    for entity in world.entities_with::<ObjectKind>() {
        let is_live_treat = matches!(
            world.component::<ObjectKind>(entity),
            Some(ObjectKind::Treat)
        ) && !matches!(
            world.component::<crate::components::Collected>(entity),
            Some(crate::components::Collected(true))
        );
        if is_live_treat
            && let Some(Position { x, y }) = world.component::<Position>(entity)
            && let Some(Treat { kind }) = world.component::<Treat>(entity)
        {
            treats.push(TreatInfo::new(*x, *y, *kind));
        }
    }
    treats
}

fn section_label_for_href(href: &str) -> Option<&'static str> {
    match href {
        "/about/" => Some("About Brooke"),
        "/blog/" => Some("Field notes and posts"),
        "/projects/" => Some("Projects and experiments"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Engine;
    use crate::components::TreatKind;

    #[test]
    fn treat_status_names_the_collected_treat_and_value() {
        let mut engine = Engine::new(7);
        engine.last_collected_treat = Some(TreatKind::Cheeseburger);
        engine.treat_message_frames = 1;

        assert_eq!(engine.compute_status(), "Cheeseburger acquired! +300");
    }
}
