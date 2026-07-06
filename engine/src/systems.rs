//! Simulation systems: movement, collision, treat collection, destination
//! and camp proximity detection. Each is a plain function taking the
//! [`World`](crate::world::World) and maze references it needs. The caller
//! in [`Engine`](crate::Engine) chooses the execution order.

use crate::components::ObjectKind;
use crate::components::Player;
use crate::components::Position;
use crate::components::Section;
use crate::components::Transform;
use crate::components::Velocity;
use crate::maze::Maze;
use crate::state::Input;
use crate::world::Entity;
use crate::world::World;

/// Tile size in pixels. Matches the renderer's tile size in `world.astro`.
pub const TILE_SIZE: f32 = 32.0;

/// Player collision box side, in pixels. One tile wide so the raccoon fits
/// through single-tile corridors.
pub const PLAYER_SIZE: f32 = 32.0;

/// Pixels per step added per frame of held input.
pub const ACCELERATION: f32 = 0.6;

/// Cap on speed magnitude in pixels per step.
pub const MAX_SPEED: f32 = 4.0;

/// Velocity multiplier applied each frame when the related axis has no input.
pub const FRICTION: f32 = 0.85;

/// Snapped-to-zero threshold; tiny drift is clamped to avoid perpetual jitter.
pub const STOP_EPSILON: f32 = 0.1;

/// Points awarded per treat collected. Matches the original TS implementation.
pub const TREAT_VALUE: u32 = 50;

/// Frames the status line lingers on the "treat acquired" message after
/// collection, matching the original TS feel.
pub const TREAT_MESSAGE_FRAMES: u32 = 90;

/// Half the player's collision box, inset slightly so corners fit into
/// corridors without snagging on adjacent walls.
fn player_half_extent() -> f32 {
    PLAYER_SIZE / 2.0 - 4.0
}

/// Returns `true` if the player's bounding box at the given center overlaps
/// any solid wall tile.
fn collides_at(maze: &Maze, center_x: f32, center_y: f32) -> bool {
    let half = player_half_extent();
    let corners = [
        (center_x - half, center_y - half),
        (center_x + half, center_y - half),
        (center_x - half, center_y + half),
        (center_x + half, center_y + half),
    ];

    corners.iter().any(|(cx, cy)| {
        let tile_x = (cx / TILE_SIZE).floor() as i32;
        let tile_y = (cy / TILE_SIZE).floor() as i32;
        maze.is_wall(tile_x, tile_y)
    })
}

/// Apply acceleration along the input axes, clamp speed, and decay velocity
/// on idle axes via friction. Mutates only the player's [`Velocity`].
pub fn apply_input(world: &mut World, player: Entity, input: &Input) {
    let Some(velocity) = world.component_mut::<Velocity>(player) else {
        return;
    };

    let mut intent_x = 0.0_f32;
    let mut intent_y = 0.0_f32;

    if input.left {
        intent_x -= 1.0;
    }
    if input.right {
        intent_x += 1.0;
    }
    if input.up {
        intent_y -= 1.0;
    }
    if input.down {
        intent_y += 1.0;
    }

    if intent_x != 0.0 && intent_y != 0.0 {
        let diagonal_scale = std::f32::consts::SQRT_2.recip();
        intent_x *= diagonal_scale;
        intent_y *= diagonal_scale;
    }

    velocity.x += intent_x * ACCELERATION;
    velocity.y += intent_y * ACCELERATION;

    let speed = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
    if speed > MAX_SPEED {
        let scale = MAX_SPEED / speed;
        velocity.x *= scale;
        velocity.y *= scale;
    }

    if intent_x == 0.0 {
        velocity.x *= FRICTION;
    }
    if intent_y == 0.0 {
        velocity.y *= FRICTION;
    }

    if velocity.x.abs() < STOP_EPSILON {
        velocity.x = 0.0;
    }
    if velocity.y.abs() < STOP_EPSILON {
        velocity.y = 0.0;
    }
}

/// Integrate the player's velocity into its [`Transform`], with per-axis wall
/// collision. The axis that would collide is zeroed instead of moving, so the
/// raccoon slides along walls rather than sticking.
pub fn move_with_collision(world: &mut World, maze: &Maze, player: Entity) {
    let Some(velocity) = world.component::<Velocity>(player).copied() else {
        return;
    };

    // X axis: try the move; if it collides, leave position unchanged and zero
    // the x velocity so subsequent frames don't keep pushing into the wall.
    let current_x = world.component::<Transform>(player).map(|t| t.x);
    let current_y = world.component::<Transform>(player).map(|t| t.y);
    let (Some(current_x), Some(current_y)) = (current_x, current_y) else {
        return;
    };

    let next_x = current_x + velocity.x;
    if !collides_at(maze, next_x, current_y) {
        if let Some(transform) = world.component_mut::<Transform>(player) {
            transform.x = next_x;
        }
    } else if let Some(v) = world.component_mut::<Velocity>(player) {
        v.x = 0.0;
    }

    // Y axis: re-read transform.x in case the X step above changed it.
    let updated_x = world
        .component::<Transform>(player)
        .map(|t| t.x)
        .unwrap_or(current_x);

    let next_y = current_y + velocity.y;
    if !collides_at(maze, updated_x, next_y) {
        if let Some(transform) = world.component_mut::<Transform>(player) {
            transform.y = next_y;
        }
    } else if let Some(v) = world.component_mut::<Velocity>(player) {
        v.y = 0.0;
    }
}

/// Collect any uncollected treat overlapping the player's tile. Returns `true`
/// if a treat was collected this frame. Sets the player's `just_collected_treat`
/// flag for the snapshot to surface to the renderer.
pub fn collect_treats(world: &mut World, player: Entity) -> bool {
    let Some((player_center_x, player_center_y)) = world
        .component::<Transform>(player)
        .map(|transform| (transform.x, transform.y))
    else {
        return false;
    };

    let player_tile_x = (player_center_x / TILE_SIZE) as i32;
    let player_tile_y = (player_center_y / TILE_SIZE) as i32;

    let treat_entities = world.entities_with::<ObjectKind>();
    let mut collected_any = false;

    for entity in treat_entities {
        let is_treat = matches!(
            world.component::<ObjectKind>(entity),
            Some(ObjectKind::Treat)
        );
        if !is_treat {
            continue;
        }

        let already_collected = matches!(
            world.component::<crate::components::Collected>(entity),
            Some(crate::components::Collected(true))
        );
        if already_collected {
            continue;
        }

        let Some(Position { x, y }) = world.component::<Position>(entity).copied() else {
            continue;
        };

        // Treat collision uses tile proximity: same tile or one of the eight
        // neighbors of the player. Generous enough that running past a treat
        // at speed still collects it.
        if (player_tile_x - x).abs() <= 1 && (player_tile_y - y).abs() <= 1 {
            world.insert(entity, crate::components::Collected(true));
            if let Some(player_state) = world.component_mut::<Player>(player) {
                player_state.score += TREAT_VALUE;
                player_state.just_collected_treat = true;
            }
            collected_any = true;
        }
    }

    collected_any
}

/// Returns the [`Section`] whose destination overlaps the player's tile, if
/// any. Used by the engine to drive the "press Enter to enter" prompt and the
/// pending-navigation signal.
pub fn active_destination(world: &World, player: Entity) -> Option<Section> {
    let transform = world.component::<Transform>(player)?;
    let player_tile_x = (transform.x / TILE_SIZE) as i32;
    let player_tile_y = (transform.y / TILE_SIZE) as i32;

    for entity in world.entities_with::<ObjectKind>() {
        if let Some(ObjectKind::Destination { section }) = world.component::<ObjectKind>(entity)
            && let Some(Position { x, y }) = world.component::<Position>(entity)
            && (player_tile_x - x).abs() <= 1
            && (player_tile_y - y).abs() <= 1
        {
            return Some(*section);
        }
    }

    None
}

/// Returns `true` when the player is within one tile of the camp marker.
pub fn is_near_camp(world: &World, player: Entity) -> bool {
    let Some(transform) = world.component::<Transform>(player) else {
        return false;
    };
    let player_tile_x = (transform.x / TILE_SIZE) as i32;
    let player_tile_y = (transform.y / TILE_SIZE) as i32;

    for entity in world.entities_with::<ObjectKind>() {
        if let Some(ObjectKind::Camp) = world.component::<ObjectKind>(entity)
            && let Some(Position { x, y }) = world.component::<Position>(entity)
            && (player_tile_x - x).abs() <= 1
            && (player_tile_y - y).abs() <= 1
        {
            return true;
        }
    }

    false
}

/// Count of treats still uncollected, used by the status line.
pub fn remaining_treats(world: &World) -> u32 {
    let mut count = 0_u32;
    for entity in world.entities_with::<ObjectKind>() {
        if !matches!(
            world.component::<ObjectKind>(entity),
            Some(ObjectKind::Treat)
        ) {
            continue;
        }
        if matches!(
            world.component::<crate::components::Collected>(entity),
            Some(crate::components::Collected(true))
        ) {
            continue;
        }
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Collected;
    use crate::components::ObjectKind;
    use crate::components::Player;
    use crate::components::Position;
    use crate::components::Section;
    use crate::components::Transform;
    use crate::components::Velocity;
    use crate::maze::Maze;
    use crate::maze::MazeConfig;
    use crate::rng::Rng;
    use crate::state::Input;
    use crate::world::World;

    fn build_world_with_player_at(tile_x: i32, tile_y: i32) -> (World, Entity, Maze) {
        let mut rng = Rng::from_seed(7);
        let mut world = World::new();
        let maze = Maze::new(MazeConfig::default(), &mut rng);
        maze.spawn_walls(&mut world);

        let player = world.spawn();
        world.insert(
            player,
            Transform {
                x: tile_x as f32 * TILE_SIZE + TILE_SIZE / 2.0,
                y: tile_y as f32 * TILE_SIZE + TILE_SIZE / 2.0,
            },
        );
        world.insert(player, Velocity { x: 0.0, y: 0.0 });
        world.insert(player, Player::default());

        (world, player, maze)
    }

    #[test]
    fn acceleration_increases_velocity_in_input_direction() {
        let (mut world, player, maze_idx) = build_world_with_player_at(15, 5);
        let _ = maze_idx;
        let input = Input {
            up: false,
            down: false,
            left: true,
            right: false,
            enter: false,
        };

        apply_input(&mut world, player, &input);
        let velocity = world.component::<Velocity>(player).unwrap();
        assert!(velocity.x < 0.0, "should accelerate left: {:?}", velocity);
        assert!(velocity.y.abs() < f32::EPSILON);
    }

    #[test]
    fn friction_decays_idle_axes() {
        let (mut world, player, _) = build_world_with_player_at(15, 5);
        world.insert(player, Velocity { x: 2.0, y: 0.0 });

        let input = Input::default();
        apply_input(&mut world, player, &input);

        let velocity = world.component::<Velocity>(player).unwrap();
        assert!(
            velocity.x < 2.0,
            "friction should decay velocity: {:?}",
            velocity
        );
    }

    #[test]
    fn move_with_collision_advances_on_open_floor() {
        let (mut world, player, maze) = build_world_with_player_at(15, 5);
        world.insert(player, Velocity { x: 3.0, y: 0.0 });

        move_with_collision(&mut world, &maze, player);

        let transform = world.component::<Transform>(player).unwrap();
        assert!(
            transform.x > 15.0 * TILE_SIZE,
            "should have moved right: {}",
            transform.x
        );
    }

    #[test]
    fn move_with_collision_blocks_walls() {
        let (mut world, player, maze) = build_world_with_player_at(15, 5);

        // Build a guaranteed wall by querying the maze for one near the player.
        let wall_tile = (0..maze.width)
            .flat_map(|x| (0..maze.height).map(move |y| (x, y)))
            .find(|(x, y)| maze.is_wall(*x, *y))
            .expect("maze has walls");

        // Place the player just inside the wall so a small step into it collides.
        world.insert(
            player,
            Transform {
                x: wall_tile.0 as f32 * TILE_SIZE - 2.0,
                y: wall_tile.1 as f32 * TILE_SIZE + TILE_SIZE / 2.0,
            },
        );
        world.insert(player, Velocity { x: 4.0, y: 0.0 });

        move_with_collision(&mut world, &maze, player);

        let velocity = world.component::<Velocity>(player).unwrap();
        assert_eq!(velocity.x, 0.0, "x velocity should be zeroed on wall hit");
    }

    #[test]
    fn collecting_a_treat_awards_points_and_marks_player() {
        let (mut world, player, _) = build_world_with_player_at(15, 5);

        let treat = world.spawn();
        world.insert(treat, Position { x: 15, y: 5 });
        world.insert(treat, ObjectKind::Treat);
        world.insert(treat, Collected(false));

        let collected = collect_treats(&mut world, player);
        assert!(collected);

        let player_state = world.component::<Player>(player).unwrap();
        assert_eq!(player_state.score, TREAT_VALUE);
        assert!(player_state.just_collected_treat);

        let treat_state = world.component::<Collected>(treat).unwrap();
        assert_eq!(treat_state.0, true);
    }

    #[test]
    fn active_destination_reports_overlapping_section() {
        let (mut world, player, _) = build_world_with_player_at(10, 7);

        let dest = world.spawn();
        world.insert(dest, Position { x: 10, y: 8 });
        world.insert(
            dest,
            ObjectKind::Destination {
                section: Section::About,
            },
        );

        let section = active_destination(&world, player);
        assert_eq!(section, Some(Section::About));
    }

    #[test]
    fn is_near_camp_detects_adjacency() {
        let (mut world, player, _) = build_world_with_player_at(12, 8);

        let camp = world.spawn();
        world.insert(camp, Position { x: 13, y: 7 });
        world.insert(camp, ObjectKind::Camp);

        assert!(is_near_camp(&world, player));
    }

    #[test]
    fn remaining_treats_counts_uncollected_only() {
        let (mut world, _player, _) = build_world_with_player_at(15, 5);

        let treat_a = world.spawn();
        world.insert(treat_a, Position { x: 1, y: 1 });
        world.insert(treat_a, ObjectKind::Treat);
        world.insert(treat_a, Collected(false));

        let treat_b = world.spawn();
        world.insert(treat_b, Position { x: 2, y: 2 });
        world.insert(treat_b, ObjectKind::Treat);
        world.insert(treat_b, Collected(true));

        assert_eq!(remaining_treats(&world), 1);
    }
}
