//! Simulation systems: movement, treat collection, destination detection,
//! and camp proximity checks. Each is a plain function taking the
//! [`World`](crate::world::World) and maze references it needs. The caller
//! in [`Engine`](crate::Engine) chooses the execution order.

use crate::components::Collider;
use crate::components::Direction;
use crate::components::GridMotion;
use crate::components::ObjectKind;
use crate::components::Player;
use crate::components::Position;
use crate::components::Section;
use crate::components::Transform;
use crate::components::Treat;
use crate::components::TreatKind;
use crate::components::Velocity;
use crate::maze::Maze;
use crate::state::Input;
use crate::world::Entity;
use crate::world::World;

/// Tile size in pixels. Matches the renderer's tile size in `world.astro`.
pub const TILE_SIZE: f32 = 32.0;

/// Frames used to travel one full tile. Short enough to stay responsive,
/// long enough to read as a gentle glide between grid centers.
pub const GRID_STEP_FRAMES: u32 = 8;

/// Frames the status line lingers on the "treat acquired" message after
/// collection, matching the original TS feel.
pub const TREAT_MESSAGE_FRAMES: u32 = 90;

fn tile_center(tile: i32) -> f32 {
    tile as f32 * TILE_SIZE + TILE_SIZE / 2.0
}

fn centered_transform(tile_x: i32, tile_y: i32) -> Transform {
    Transform {
        x: tile_center(tile_x),
        y: tile_center(tile_y),
    }
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn preferred_direction(input: &Input) -> Option<Direction> {
    if input.preferred_direction.is_some() {
        return input.preferred_direction;
    }

    if input.left && !input.right {
        return Some(Direction::Left);
    }
    if input.right && !input.left {
        return Some(Direction::Right);
    }
    if input.up && !input.down {
        return Some(Direction::Up);
    }
    if input.down && !input.up {
        return Some(Direction::Down);
    }

    None
}

fn player_tile_position(world: &World, player: Entity) -> Option<Position> {
    if let Some(motion) = world.component::<GridMotion>(player) {
        return Some(Position {
            x: motion.tile_x,
            y: motion.tile_y,
        });
    }

    world
        .component::<Transform>(player)
        .map(|transform| Position {
            x: (transform.x / TILE_SIZE) as i32,
            y: (transform.y / TILE_SIZE) as i32,
        })
}

fn can_step(world: &World, maze: &Maze, tile_x: i32, tile_y: i32, direction: Direction) -> bool {
    let (dx, dy) = direction.delta();
    let destination_x = tile_x + dx;
    let destination_y = tile_y + dy;

    if maze.is_wall(destination_x, destination_y) {
        return false;
    }

    !world.entities_with::<Collider>().into_iter().any(|entity| {
        world
            .component::<Position>(entity)
            .is_some_and(|position| position.x == destination_x && position.y == destination_y)
    })
}

fn start_buffered_step(world: &World, motion: &mut GridMotion, maze: &Maze) {
    if motion.active_direction.is_some() {
        return;
    }

    let Some(direction) = motion.buffered_direction else {
        return;
    };

    if !can_step(world, maze, motion.tile_x, motion.tile_y, direction) {
        return;
    }

    motion.active_direction = Some(direction);
    motion.frames_remaining = GRID_STEP_FRAMES;
}

fn interpolated_transform(motion: &GridMotion) -> Transform {
    let Some(direction) = motion.active_direction else {
        return centered_transform(motion.tile_x, motion.tile_y);
    };

    let (dx, dy) = direction.delta();
    let start = centered_transform(motion.tile_x, motion.tile_y);
    let end = centered_transform(motion.tile_x + dx, motion.tile_y + dy);
    let completed_frames = GRID_STEP_FRAMES - motion.frames_remaining;
    let progress = completed_frames as f32 / GRID_STEP_FRAMES as f32;
    let eased_progress = smoothstep(progress);

    Transform {
        x: start.x + (end.x - start.x) * eased_progress,
        y: start.y + (end.y - start.y) * eased_progress,
    }
}

/// Record the latest held direction so grid movement can turn cleanly at the
/// next tile center. When no movement keys are held, the buffer is cleared.
pub fn apply_input(world: &mut World, player: Entity, input: &Input) {
    let Some(motion) = world.component_mut::<GridMotion>(player) else {
        return;
    };

    motion.buffered_direction = preferred_direction(input);
}

/// Advance the player by at most one frame of tile-step motion. The player
/// moves only between adjacent open tiles and always lands exactly centered on
/// the grid. The latest buffered direction is reused at each tile boundary so
/// held input can continue straight or turn smoothly.
pub fn move_with_collision(world: &mut World, maze: &Maze, player: Entity) {
    let Some(mut motion) = world.component::<GridMotion>(player).copied() else {
        return;
    };

    let previous_transform = world
        .component::<Transform>(player)
        .copied()
        .unwrap_or_else(|| centered_transform(motion.tile_x, motion.tile_y));

    start_buffered_step(world, &mut motion, maze);

    let next_transform = if motion.active_direction.is_some() {
        motion.frames_remaining -= 1;
        let transform = interpolated_transform(&motion);

        if motion.frames_remaining == 0 {
            let direction = motion
                .active_direction
                .expect("active direction exists while finishing a grid step");
            let (dx, dy) = direction.delta();
            motion.tile_x += dx;
            motion.tile_y += dy;
            motion.active_direction = None;
            start_buffered_step(world, &mut motion, maze);
            centered_transform(motion.tile_x, motion.tile_y)
        } else {
            transform
        }
    } else {
        centered_transform(motion.tile_x, motion.tile_y)
    };

    let velocity = Velocity {
        x: next_transform.x - previous_transform.x,
        y: next_transform.y - previous_transform.y,
    };

    world.insert(player, motion);
    world.insert(player, next_transform);
    world.insert(player, velocity);
}

/// Collect any uncollected treat overlapping the player's current tile.
/// Returns the collected kind so the caller can report its name and value.
/// Sets the player's `just_collected_treat` flag for the snapshot.
pub fn collect_treats(world: &mut World, player: Entity) -> Option<TreatKind> {
    let Position {
        x: player_tile_x,
        y: player_tile_y,
    } = player_tile_position(world, player)?;

    let treat_entities = world.entities_with::<ObjectKind>();
    let mut collected_kind = None;

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
        let Some(Treat { kind }) = world.component::<Treat>(entity).copied() else {
            continue;
        };

        if (player_tile_x - x).abs() <= 1 && (player_tile_y - y).abs() <= 1 {
            world.insert(entity, crate::components::Collected(true));
            if let Some(player_state) = world.component_mut::<Player>(player) {
                player_state.score += kind.value();
                player_state.just_collected_treat = true;
            }
            collected_kind = Some(kind);
        }
    }

    collected_kind
}

/// Returns the [`Section`] whose destination door occupies the player's tile.
/// Used by the engine to request a one-time navigation on arrival.
pub fn active_destination(world: &World, player: Entity) -> Option<Section> {
    let Position {
        x: player_tile_x,
        y: player_tile_y,
    } = player_tile_position(world, player)?;

    for entity in world.entities_with::<ObjectKind>() {
        if let Some(ObjectKind::Destination { section }) = world.component::<ObjectKind>(entity)
            && let Some(Position { x, y }) = world.component::<Position>(entity)
            && player_tile_x == *x
            && player_tile_y == *y
        {
            return Some(*section);
        }
    }

    None
}

/// Returns `true` when the player is within one tile of the camp marker.
pub fn is_near_camp(world: &World, player: Entity) -> bool {
    let Some(Position {
        x: player_tile_x,
        y: player_tile_y,
    }) = player_tile_position(world, player)
    else {
        return false;
    };

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
    use crate::components::Collider;
    use crate::components::Direction;
    use crate::components::GridMotion;
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

    fn empty_maze(width: i32, height: i32) -> Maze {
        Maze {
            width,
            height,
            tiles: vec![false; (width * height) as usize],
            rooms: Vec::new(),
        }
    }

    fn build_world_with_player_at(tile_x: i32, tile_y: i32) -> (World, Entity, Maze) {
        let mut rng = Rng::from_seed(7);
        let mut world = World::new();
        let maze = Maze::new(MazeConfig::default(), &mut rng);

        let player = world.spawn();
        world.insert(player, centered_transform(tile_x, tile_y));
        world.insert(player, Velocity { x: 0.0, y: 0.0 });
        world.insert(
            player,
            GridMotion {
                tile_x,
                tile_y,
                active_direction: None,
                buffered_direction: None,
                frames_remaining: 0,
            },
        );
        world.insert(player, Player::default());

        (world, player, maze)
    }

    fn step_player(world: &mut World, maze: &Maze, player: Entity, input: Input) {
        apply_input(world, player, &input);
        move_with_collision(world, maze, player);
    }

    #[test]
    fn latest_held_direction_becomes_the_buffered_direction() {
        let (mut world, player, _maze) = build_world_with_player_at(3, 3);
        let input = Input {
            up: true,
            down: false,
            left: false,
            right: true,
            preferred_direction: Some(Direction::Up),
        };

        apply_input(&mut world, player, &input);

        let motion = world.component::<GridMotion>(player).unwrap();
        assert_eq!(motion.buffered_direction, Some(Direction::Up));
    }

    #[test]
    fn move_with_collision_starts_a_grid_step_immediately() {
        let (mut world, player, _maze) = build_world_with_player_at(3, 3);
        let maze = empty_maze(8, 8);

        step_player(
            &mut world,
            &maze,
            player,
            Input {
                up: false,
                down: false,
                left: false,
                right: true,
                preferred_direction: Some(Direction::Right),
            },
        );

        let transform = world.component::<Transform>(player).unwrap();
        let velocity = world.component::<Velocity>(player).unwrap();

        assert!(transform.x > tile_center(3));
        assert_eq!(transform.y, tile_center(3));
        assert!(velocity.x > 0.0);
        assert_eq!(velocity.y, 0.0);
    }

    #[test]
    fn grid_step_lands_exactly_on_the_next_tile_center() {
        let (mut world, player, _maze) = build_world_with_player_at(3, 3);
        let maze = empty_maze(8, 8);
        let input = Input {
            up: false,
            down: false,
            left: false,
            right: true,
            preferred_direction: Some(Direction::Right),
        };

        for _ in 0..GRID_STEP_FRAMES {
            step_player(&mut world, &maze, player, input);
        }

        let motion = world.component::<GridMotion>(player).unwrap();
        let transform = world.component::<Transform>(player).unwrap();

        assert_eq!(motion.tile_x, 4);
        assert_eq!(motion.tile_y, 3);
        assert_eq!(transform.x, tile_center(4));
        assert_eq!(transform.y, tile_center(3));
    }

    #[test]
    fn buffered_turn_starts_after_arriving_at_the_next_tile() {
        let (mut world, player, _maze) = build_world_with_player_at(3, 3);
        let maze = empty_maze(8, 8);
        let move_right = Input {
            up: false,
            down: false,
            left: false,
            right: true,
            preferred_direction: Some(Direction::Right),
        };
        let turn_up = Input {
            up: true,
            down: false,
            left: false,
            right: true,
            preferred_direction: Some(Direction::Up),
        };

        for _ in 0..(GRID_STEP_FRAMES - 1) {
            step_player(&mut world, &maze, player, move_right);
        }

        step_player(&mut world, &maze, player, turn_up);

        let motion = world.component::<GridMotion>(player).unwrap();
        let transform = world.component::<Transform>(player).unwrap();
        assert_eq!(motion.tile_x, 4);
        assert_eq!(motion.tile_y, 3);
        assert_eq!(transform.x, tile_center(4));
        assert_eq!(transform.y, tile_center(3));
        assert_eq!(motion.active_direction, Some(Direction::Up));

        step_player(&mut world, &maze, player, turn_up);

        let transform = world.component::<Transform>(player).unwrap();
        assert!(transform.y < tile_center(3));
        assert_eq!(transform.x, tile_center(4));
    }

    #[test]
    fn blocked_step_keeps_the_player_centered_on_the_tile() {
        let (mut world, player, _maze) = build_world_with_player_at(3, 3);
        let mut maze = empty_maze(8, 8);
        let blocked_index = (3 * maze.width + 4) as usize;
        maze.tiles[blocked_index] = true;

        step_player(
            &mut world,
            &maze,
            player,
            Input {
                up: false,
                down: false,
                left: false,
                right: true,
                preferred_direction: Some(Direction::Right),
            },
        );

        let motion = world.component::<GridMotion>(player).unwrap();
        let transform = world.component::<Transform>(player).unwrap();
        let velocity = world.component::<Velocity>(player).unwrap();

        assert_eq!(motion.tile_x, 3);
        assert_eq!(motion.tile_y, 3);
        assert_eq!(motion.active_direction, None);
        assert_eq!(transform.x, tile_center(3));
        assert_eq!(transform.y, tile_center(3));
        assert_eq!(velocity.x, 0.0);
        assert_eq!(velocity.y, 0.0);
    }

    #[test]
    fn dynamic_collider_blocks_a_grid_step() {
        let (mut world, player, _maze) = build_world_with_player_at(3, 3);
        let maze = empty_maze(8, 8);
        let collider = world.spawn();
        world.insert(collider, Position { x: 4, y: 3 });
        world.insert(collider, Collider);

        step_player(
            &mut world,
            &maze,
            player,
            Input {
                up: false,
                down: false,
                left: false,
                right: true,
                preferred_direction: Some(Direction::Right),
            },
        );

        let motion = world.component::<GridMotion>(player).unwrap();
        assert_eq!(motion.tile_x, 3);
        assert_eq!(motion.tile_y, 3);
        assert_eq!(motion.active_direction, None);
    }

    #[test]
    fn collecting_a_treat_awards_points_and_marks_player() {
        let (mut world, player, _) = build_world_with_player_at(15, 5);

        let treat = world.spawn();
        world.insert(treat, Position { x: 15, y: 5 });
        world.insert(treat, ObjectKind::Treat);
        world.insert(
            treat,
            Treat {
                kind: TreatKind::Cheeseburger,
            },
        );
        world.insert(treat, Collected(false));

        let collected = collect_treats(&mut world, player);
        assert_eq!(collected, Some(TreatKind::Cheeseburger));

        let player_state = world.component::<Player>(player).unwrap();
        assert_eq!(player_state.score, 300);
        assert!(player_state.just_collected_treat);

        let treat_state = world.component::<Collected>(treat).unwrap();
        assert_eq!(treat_state.0, true);
    }

    #[test]
    fn active_destination_requires_the_door_tile() {
        let (mut world, player, _) = build_world_with_player_at(10, 8);

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
    fn active_destination_ignores_adjacent_tiles() {
        let (mut world, player, _) = build_world_with_player_at(10, 7);

        let dest = world.spawn();
        world.insert(dest, Position { x: 10, y: 8 });
        world.insert(
            dest,
            ObjectKind::Destination {
                section: Section::About,
            },
        );

        assert_eq!(active_destination(&world, player), None);
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
