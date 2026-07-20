//! Binary-space-partition rooms-and-corridors maze generator.
//!
//! The generator carves rectangular rooms out of an otherwise solid grid and
//! connects them with L-shaped corridors. The result is "easy" in the sense
//! that rooms stay open and the topology is fully connected — no isolated
//! pockets — which suits a navigation-as-discovery world better than a
//! twisting perfect maze.

use crate::components::Collected;
use crate::components::Collider;
use crate::components::ObjectKind;
use crate::components::Position;
use crate::components::Section;
use crate::components::Treat;
use crate::components::TreatKind;
use crate::rng::Rng;
use crate::world::World;

/// Treat distribution placed in each maze. Positions remain seed-driven while
/// the inventory and total available score stay consistent across worlds.
const TREAT_KINDS: [TreatKind; 8] = [
    TreatKind::Cheeseburger,
    TreatKind::Snail,
    TreatKind::Frog,
    TreatKind::Banana,
    TreatKind::Cherries,
    TreatKind::Berries,
    TreatKind::Berries,
    TreatKind::Apple,
];
const TREAT_COUNT: usize = TREAT_KINDS.len();

/// One carved room in tile coordinates. Stored so room centers can be used
/// later to place camp, destinations, and treats.
#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl Room {
    pub fn center(self) -> Position {
        Position {
            x: self.left + self.width / 2,
            y: self.top + self.height / 2,
        }
    }

    /// Returns whether this room can hold a destination building and its
    /// one-tile walkable buffer.
    pub fn can_host_destination(self) -> bool {
        self.width >= 5 && self.height >= 4
    }

    /// Returns the centered door tile for a destination building.
    ///
    /// The room-size constraint leaves a walkable ring around the 3×2
    /// footprint, so corridors arriving at the room center can route around
    /// the building to reach its door.
    pub fn destination_door(self) -> Position {
        debug_assert!(self.can_host_destination());
        self.center()
    }
}

/// Adjustable knobs for generation. Values favor roomy, readable layouts for
/// a short visit rather than dense dungeon crawls.
#[derive(Clone, Copy, Debug)]
pub struct MazeConfig {
    /// Maze width in tiles.
    pub width: i32,
    /// Maze height in tiles.
    pub height: i32,
    /// Minimum room side length in tiles. Five leaves a one-tile buffer
    /// around every destination building's 3×2 footprint.
    pub min_room: i32,
    /// Maximum room side length in tiles.
    pub max_room: i32,
    /// Stop recursing once either child area drops below this many tiles.
    pub min_split_area: i32,
}

impl Default for MazeConfig {
    fn default() -> Self {
        // Matches the canvas size in world.astro: 30 tiles wide, 22 tall.
        Self {
            width: 30,
            height: 22,
            min_room: 5,
            max_room: 8,
            min_split_area: 36,
        }
    }
}

/// Result of generating a maze: solid walls filled in, plus the rooms and
/// corridor tiles so callers can place gameplay objects on open floor.
#[derive(Debug)]
pub struct Maze {
    pub width: i32,
    pub height: i32,
    /// `true` where the tile is solid wall, `false` once carved open.
    pub tiles: Vec<bool>,
    pub rooms: Vec<Room>,
}

impl Maze {
    pub fn new(config: MazeConfig, rng: &mut Rng) -> Self {
        let mut maze = Self {
            width: config.width,
            height: config.height,
            tiles: vec![true; (config.width * config.height) as usize],
            rooms: Vec::new(),
        };

        let root = Room {
            left: 1,
            top: 1,
            width: config.width - 2,
            height: config.height - 2,
        };

        maze.split_root(root, config, rng);

        maze
    }

    /// Split the root into four regions before applying randomized BSP splits.
    ///
    /// Every region is large enough for one buffered destination room. This
    /// guarantees a camp plus three destination rooms without retrying a seed.
    fn split_root(&mut self, root: Room, config: MazeConfig, rng: &mut Rng) {
        let minimum_span = config.min_room * 2 + 1;
        if root.width < minimum_span || root.height < minimum_span {
            let _ = self.split(root, config, rng);
            return;
        }

        let left_width = root.width / 2;
        let top_height = root.height / 2;
        let right_width = root.width - left_width - 1;
        let bottom_height = root.height - top_height - 1;
        let north_west = Room {
            left: root.left,
            top: root.top,
            width: left_width,
            height: top_height,
        };
        let north_east = Room {
            left: root.left + left_width + 1,
            top: root.top,
            width: right_width,
            height: top_height,
        };
        let south_west = Room {
            left: root.left,
            top: root.top + top_height + 1,
            width: left_width,
            height: bottom_height,
        };
        let south_east = Room {
            left: root.left + left_width + 1,
            top: root.top + top_height + 1,
            width: right_width,
            height: bottom_height,
        };

        let north_west_anchor = self.split(north_west, config, rng);
        let north_east_anchor = self.split(north_east, config, rng);
        let south_west_anchor = self.split(south_west, config, rng);
        let south_east_anchor = self.split(south_east, config, rng);

        self.connect(north_west_anchor, north_east_anchor);
        self.connect(north_west_anchor, south_west_anchor);
        self.connect(north_east_anchor, south_east_anchor);
        self.connect(south_west_anchor, south_east_anchor);
    }

    /// Recursive BSP step. Splits the given room along a random axis when
    /// both children remain useful, then connects a representative room center
    /// from each child subtree. Returns a room center from the connected
    /// subtree so parents can continue joining larger regions.
    fn split(&mut self, region: Room, config: MazeConfig, rng: &mut Rng) -> Position {
        let split_horizontally = rng.below(2) == 0;

        if let Some((lower, upper)) = self.try_split(region, split_horizontally, config, rng) {
            let lower_anchor = self.split(lower, config, rng);
            let upper_anchor = self.split(upper, config, rng);
            self.connect(lower_anchor, upper_anchor);
            lower_anchor
        } else if let Some((lower, upper)) =
            self.try_split(region, !split_horizontally, config, rng)
        {
            let lower_anchor = self.split(lower, config, rng);
            let upper_anchor = self.split(upper, config, rng);
            self.connect(lower_anchor, upper_anchor);
            lower_anchor
        } else {
            self.carve_room(region, config, rng)
        }
    }

    /// Attempt to split `region` along the given axis. Returns the two
    /// children only if both remain above the configured minimum area.
    fn try_split(
        &self,
        region: Room,
        horizontal: bool,
        config: MazeConfig,
        rng: &mut Rng,
    ) -> Option<(Room, Room)> {
        let span = if horizontal {
            region.height
        } else {
            region.width
        };

        if span < config.min_room * 2 + 1 {
            return None;
        }

        // Leave one wall tile at the split and at least `min_room` tiles in
        // both children.
        let split_point =
            rng.between(config.min_room as u32, (span - config.min_room - 1) as u32) as i32;

        let (lower, upper) = if horizontal {
            (
                Room {
                    left: region.left,
                    top: region.top,
                    width: region.width,
                    height: split_point,
                },
                Room {
                    left: region.left,
                    top: region.top + split_point + 1,
                    width: region.width,
                    height: region.height - split_point - 1,
                },
            )
        } else {
            (
                Room {
                    left: region.left,
                    top: region.top,
                    width: split_point,
                    height: region.height,
                },
                Room {
                    left: region.left + split_point + 1,
                    top: region.top,
                    width: region.width - split_point - 1,
                    height: region.height,
                },
            )
        };

        let lower_area = lower.width * lower.height;
        let upper_area = upper.width * upper.height;

        if lower_area < config.min_split_area || upper_area < config.min_split_area {
            return None;
        }

        Some((lower, upper))
    }

    /// Carve a leaf room centered inside the leaf region, leaving a margin
    /// for walls. Records the room for later placement of camp, destinations,
    /// and treats.
    fn carve_room(&mut self, region: Room, config: MazeConfig, rng: &mut Rng) -> Position {
        let room_width = rng.between(
            config.min_room as u32,
            config.max_room.min(region.width) as u32,
        ) as i32;
        let room_height = rng.between(
            config.min_room as u32,
            config.max_room.min(region.height) as u32,
        ) as i32;

        let max_left = region.width - room_width;
        let max_top = region.height - room_height;

        let left = region.left + rng.below(max_left.max(0) as u32) as i32;
        let top = region.top + rng.below(max_top.max(0) as u32) as i32;

        let room = Room {
            left,
            top,
            width: room_width,
            height: room_height,
        };

        self.carve_rect(room.left, room.top, room.width, room.height);
        self.rooms.push(room);
        room.center()
    }

    fn carve_rect(&mut self, left: i32, top: i32, width: i32, height: i32) {
        for y in top..(top + height) {
            for x in left..(left + width) {
                if self.in_bounds(x, y) {
                    let index = self.index(x, y);
                    self.tiles[index] = false;
                }
            }
        }
    }

    /// Connect two room centers with an L-shaped corridor. Carves in a one-tile
    /// strip along the horizontal then vertical segment. Walls alongside the
    /// corridor remain from the original solid grid.
    pub fn connect(&mut self, a: Position, b: Position) {
        // Horizontal segment.
        let (x_start, x_end) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
        for x in x_start..=x_end {
            if self.in_bounds(x, a.y) {
                let index = self.index(x, a.y);
                self.tiles[index] = false;
            }
        }
        // Vertical segment.
        let (y_start, y_end) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
        for y in y_start..=y_end {
            if self.in_bounds(b.x, y) {
                let index = self.index(b.x, y);
                self.tiles[index] = false;
            }
        }
    }

    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width && y < self.height
    }

    /// Returns `true` if the given tile is solid wall (out of bounds counts
    /// as solid so callers can clamp cleanly).
    pub fn is_wall(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return true;
        }
        self.tiles[self.index(x, y)]
    }

    /// Spawn wall colliders for every solid tile. The renderer reads these
    /// as solid blocks and the simulation reads them as obstacles.
    pub fn spawn_walls(&self, world: &mut World) {
        for y in 0..self.height {
            for x in 0..self.width {
                if self.is_wall(x, y) {
                    let entity = world.spawn();
                    world.insert(entity, Position { x, y });
                    world.insert(entity, Collider);
                }
            }
        }
    }

    /// Place camp, destinations, and treats in generated rooms.
    pub fn populate(&self, world: &mut World, rng: &mut Rng) {
        let sections = [Section::About, Section::Blog, Section::Projects];
        let destination_room_candidates: Vec<usize> = self
            .rooms
            .iter()
            .enumerate()
            .filter_map(|(index, room)| room.can_host_destination().then_some(index))
            .collect();

        assert!(
            destination_room_candidates.len() >= sections.len()
                && self.rooms.len() > sections.len(),
            "maze must provide a buffered room for every destination and a separate camp room"
        );

        let destination_room_indices: Vec<usize> =
            pick_distinct(rng, destination_room_candidates.len(), sections.len())
                .into_iter()
                .map(|index| destination_room_candidates[index])
                .collect();
        let camp_room_candidates: Vec<usize> = (0..self.rooms.len())
            .filter(|index| !destination_room_indices.contains(index))
            .collect();
        let camp_room_index =
            camp_room_candidates[rng.below(camp_room_candidates.len() as u32) as usize];

        let camp_pos = self.rooms[camp_room_index].center();
        let camp = world.spawn();
        world.insert(camp, camp_pos);
        world.insert(camp, ObjectKind::Camp);
        world.insert(
            camp,
            crate::components::Sprite(crate::components::SpriteId::Camp),
        );

        let mut destination_positions = Vec::new();
        for (room_index, section) in destination_room_indices.iter().zip(sections) {
            let door = self.rooms[*room_index].destination_door();
            destination_positions.push(door);
            let entity = world.spawn();
            world.insert(entity, door);
            world.insert(entity, ObjectKind::Destination { section });
            world.insert(
                entity,
                crate::components::Sprite(crate::components::SpriteId::Destination),
            );

            self.place_destination_building_collision(world, door);
        }

        self.place_treats(world, rng, camp_pos, &destination_positions);
    }

    /// Marks a destination building as impassable while leaving its door open.
    ///
    /// The destination position is the centered door tile in the lower row of
    /// the sprite's 3×2 footprint. The roof overhang is visual-only.
    fn place_destination_building_collision(&self, world: &mut World, door: Position) {
        for x in (door.x - 1)..=(door.x + 1) {
            self.place_collider(world, Position { x, y: door.y - 1 });
        }

        self.place_collider(
            world,
            Position {
                x: door.x - 1,
                y: door.y,
            },
        );
        self.place_collider(
            world,
            Position {
                x: door.x + 1,
                y: door.y,
            },
        );
    }

    fn place_collider(&self, world: &mut World, position: Position) {
        let entity = world.spawn();
        world.insert(entity, position);
        world.insert(entity, Collider);
    }

    /// Scatter `TREAT_COUNT` treats across open floor tiles, rejecting any
    /// tile adjacent (Chebyshev distance <= 1) to camp, a destination, or a
    /// previously placed treat.
    fn place_treats(
        &self,
        world: &mut World,
        rng: &mut Rng,
        camp: Position,
        destinations: &[Position],
    ) {
        let mut candidates = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.is_wall(x, y) {
                    candidates.push(Position { x, y });
                }
            }
        }

        let mut placed: Vec<Position> = Vec::new();

        while placed.len() < TREAT_COUNT && !candidates.is_empty() {
            let idx = rng.below(candidates.len() as u32) as usize;
            let candidate = candidates.swap_remove(idx);

            if is_near(candidate, camp) {
                continue;
            }
            if destinations.iter().any(|dest| is_near(candidate, *dest)) {
                continue;
            }
            if is_blocked_by_collider(world, candidate) {
                continue;
            }
            if placed.iter().any(|treat| is_near(candidate, *treat)) {
                continue;
            }

            let treat_kind = TREAT_KINDS[placed.len()];
            let entity = world.spawn();
            world.insert(entity, candidate);
            world.insert(entity, ObjectKind::Treat);
            world.insert(entity, Treat { kind: treat_kind });
            world.insert(entity, Collected(false));
            world.insert(
                entity,
                crate::components::Sprite(crate::components::SpriteId::Treat),
            );
            placed.push(candidate);
        }
    }
}

/// Returns `true` when `a` is within one tile (Chebyshev distance <= 1) of `b`.
fn is_near(a: Position, b: Position) -> bool {
    (a.x - b.x).abs() <= 1 && (a.y - b.y).abs() <= 1
}

/// Returns whether an existing solid object occupies a tile.
fn is_blocked_by_collider(world: &World, position: Position) -> bool {
    world.entities_with::<Collider>().into_iter().any(|entity| {
        world
            .component::<Position>(entity)
            .is_some_and(|occupied| *occupied == position)
    })
}

/// Pick `count` distinct indices in `[0, limit)` without replacement.
fn pick_distinct(rng: &mut Rng, limit: usize, count: usize) -> Vec<usize> {
    let mut pool: Vec<usize> = (0..limit).collect();
    let mut picked = Vec::with_capacity(count.min(limit));

    while picked.len() < count && !pool.is_empty() {
        let idx = rng.below(pool.len() as u32) as usize;
        picked.push(pool.remove(idx));
    }

    picked
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::Maze;
    use super::MazeConfig;
    use super::Rng;
    use super::Room;
    use crate::components::Collider;
    use crate::components::ObjectKind;
    use crate::components::Position;
    use crate::components::Treat;
    use crate::components::TreatKind;
    use crate::world::World;

    fn count_reachable_open_tiles(maze: &Maze) -> usize {
        let start = (0..maze.height)
            .flat_map(|y| (0..maze.width).map(move |x| (x, y)))
            .find(|(x, y)| !maze.is_wall(*x, *y));

        let Some((start_x, start_y)) = start else {
            return 0;
        };

        let mut seen = vec![false; (maze.width * maze.height) as usize];
        let mut queue = VecDeque::from([(start_x, start_y)]);
        let mut reachable = 0usize;

        while let Some((x, y)) = queue.pop_front() {
            if x < 0 || y < 0 || x >= maze.width || y >= maze.height {
                continue;
            }
            if maze.is_wall(x, y) {
                continue;
            }

            let index = (y * maze.width + x) as usize;
            if seen[index] {
                continue;
            }
            seen[index] = true;
            reachable += 1;

            queue.push_back((x + 1, y));
            queue.push_back((x - 1, y));
            queue.push_back((x, y + 1));
            queue.push_back((x, y - 1));
        }

        reachable
    }

    fn reachable_positions(maze: &Maze, world: &World, start: Position) -> Vec<Position> {
        let collision_positions: Vec<Position> = world
            .entities_with::<Collider>()
            .into_iter()
            .filter_map(|entity| world.component::<Position>(entity).copied())
            .collect();
        let mut seen = vec![false; (maze.width * maze.height) as usize];
        let mut queue = VecDeque::from([start]);
        let mut reachable = Vec::new();

        while let Some(position) = queue.pop_front() {
            if maze.is_wall(position.x, position.y) || collision_positions.contains(&position) {
                continue;
            }

            let index = (position.y * maze.width + position.x) as usize;
            if seen[index] {
                continue;
            }
            seen[index] = true;
            reachable.push(position);

            queue.push_back(Position {
                x: position.x + 1,
                y: position.y,
            });
            queue.push_back(Position {
                x: position.x - 1,
                y: position.y,
            });
            queue.push_back(Position {
                x: position.x,
                y: position.y + 1,
            });
            queue.push_back(Position {
                x: position.x,
                y: position.y - 1,
            });
        }

        reachable
    }

    #[test]
    fn maze_keeps_a_solid_border() {
        let mut rng = Rng::from_seed(1234);
        let config = MazeConfig::default();
        let maze = Maze::new(config, &mut rng);

        // At least one room should have been carved.
        assert!(!maze.rooms.is_empty());

        // The border must remain solid so the player cannot walk off the map.
        for x in 0..config.width {
            assert!(maze.is_wall(x, 0), "top border should be solid at x={x}");
            assert!(
                maze.is_wall(x, config.height - 1),
                "bottom border should be solid at x={x}"
            );
        }
        for y in 0..config.height {
            assert!(maze.is_wall(0, y), "left border should be solid at y={y}");
            assert!(
                maze.is_wall(config.width - 1, y),
                "right border should be solid at y={y}"
            );
        }
    }

    #[test]
    fn generated_maze_has_one_connected_open_region() {
        let config = MazeConfig::default();
        let maze = Maze::new(config, &mut Rng::from_seed(1234));

        let total_open_tiles = (0..maze.height)
            .flat_map(|y| (0..maze.width).map(move |x| (x, y)))
            .filter(|(x, y)| !maze.is_wall(*x, *y))
            .count();
        let reachable_open_tiles = count_reachable_open_tiles(&maze);

        assert!(
            total_open_tiles > 0,
            "maze should carve at least one open tile"
        );
        assert_eq!(
            reachable_open_tiles, total_open_tiles,
            "every open tile should be reachable from every other open tile"
        );
    }

    #[test]
    fn maze_rooms_are_carved_open() {
        let mut rng = Rng::from_seed(7);
        let maze = Maze::new(MazeConfig::default(), &mut rng);

        for room in &maze.rooms {
            for tile_x in room.left..room.left + room.width {
                for tile_y in room.top..room.top + room.height {
                    assert!(
                        !maze.is_wall(tile_x, tile_y),
                        "room tile ({}, {}) should be carved open",
                        tile_x,
                        tile_y
                    );
                }
            }
        }
    }

    #[test]
    fn corridors_connect_room_centers() {
        let mut rng = Rng::from_seed(99);
        let mut maze = Maze::new(MazeConfig::default(), &mut rng);

        if maze.rooms.len() < 2 {
            return;
        }

        let a = maze.rooms[0].center();
        let b = maze.rooms[1].center();

        maze.connect(a, b);

        // The horizontal corridor at y=a.y should be open from a.x to b.x.
        let (x_lo, x_hi) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
        for x in x_lo..=x_hi {
            assert!(
                !maze.is_wall(x, a.y),
                "corridor should be open at ({}, {})",
                x,
                a.y
            );
        }
    }

    #[test]
    fn populated_maze_contains_the_expected_treat_distribution() {
        let mut rng = Rng::from_seed(2026);
        let maze = Maze::new(MazeConfig::default(), &mut rng);
        let mut world = World::new();

        maze.populate(&mut world, &mut rng);

        let kinds: Vec<TreatKind> = world
            .entities_with::<Treat>()
            .into_iter()
            .filter_map(|entity| world.component::<Treat>(entity).map(|treat| treat.kind))
            .collect();

        let count = |target: TreatKind| kinds.iter().filter(|kind| **kind == target).count();

        assert_eq!(kinds.len(), 8);
        assert_eq!(count(TreatKind::Cheeseburger), 1);
        assert_eq!(count(TreatKind::Snail), 1);
        assert_eq!(count(TreatKind::Frog), 1);
        assert_eq!(count(TreatKind::Banana), 1);
        assert_eq!(count(TreatKind::Cherries), 1);
        assert_eq!(count(TreatKind::Berries), 2);
        assert_eq!(count(TreatKind::Apple), 1);
        assert_eq!(kinds.iter().map(|kind| kind.value()).sum::<u32>(), 750);
    }

    #[test]
    fn destination_buildings_have_collision_footprints_with_open_doors() {
        let mut rng = Rng::from_seed(2026);
        let maze = Maze::new(MazeConfig::default(), &mut rng);
        let mut world = World::new();

        maze.populate(&mut world, &mut rng);

        let doors: Vec<Position> = world
            .entities_with::<ObjectKind>()
            .into_iter()
            .filter(|entity| {
                matches!(
                    world.component::<ObjectKind>(*entity),
                    Some(ObjectKind::Destination { .. })
                )
            })
            .filter_map(|entity| world.component::<Position>(entity).copied())
            .collect();
        let collision_positions: Vec<Position> = world
            .entities_with::<Collider>()
            .into_iter()
            .filter_map(|entity| world.component::<Position>(entity).copied())
            .collect();

        assert_eq!(doors.len(), 3);
        assert_eq!(collision_positions.len(), 15);
        for door in doors {
            for x in (door.x - 1)..=(door.x + 1) {
                assert!(collision_positions.contains(&Position { x, y: door.y - 1 }));
            }
            assert!(collision_positions.contains(&Position {
                x: door.x - 1,
                y: door.y,
            }));
            assert!(collision_positions.contains(&Position {
                x: door.x + 1,
                y: door.y,
            }));
            assert!(!collision_positions.contains(&door));
        }

        let treat_positions: Vec<Position> = world
            .entities_with::<Treat>()
            .into_iter()
            .filter_map(|entity| world.component::<Position>(entity).copied())
            .collect();
        assert!(
            treat_positions
                .iter()
                .all(|position| !collision_positions.contains(position))
        );
    }

    #[test]
    fn destination_rooms_have_a_walkable_buffer_across_generated_seeds() {
        for seed in 0..128 {
            let mut rng = Rng::from_seed(seed);
            let maze = Maze::new(MazeConfig::default(), &mut rng);
            let mut world = World::new();

            maze.populate(&mut world, &mut rng);

            let destination_positions: Vec<Position> = world
                .entities_with::<ObjectKind>()
                .into_iter()
                .filter(|entity| {
                    matches!(
                        world.component::<ObjectKind>(*entity),
                        Some(ObjectKind::Destination { .. })
                    )
                })
                .filter_map(|entity| world.component::<Position>(entity).copied())
                .collect();

            assert_eq!(
                destination_positions.len(),
                3,
                "seed {seed}, rooms: {:?}",
                maze.rooms
            );
            for door in &destination_positions {
                let room = maze
                    .rooms
                    .iter()
                    .copied()
                    .find(|room| {
                        door.x >= room.left
                            && door.x < room.left + room.width
                            && door.y >= room.top
                            && door.y < room.top + room.height
                    })
                    .expect("destination door should be in a room");

                assert!(room.can_host_destination(), "seed {seed}");
                assert!(door.x - 2 >= room.left, "seed {seed}");
                assert!(door.x + 2 < room.left + room.width, "seed {seed}");
                assert!(door.y - 2 >= room.top, "seed {seed}");
                assert!(door.y + 1 < room.top + room.height, "seed {seed}");
            }

            let camp_position = world
                .entities_with::<ObjectKind>()
                .into_iter()
                .find_map(|entity| {
                    matches!(
                        world.component::<ObjectKind>(entity),
                        Some(ObjectKind::Camp)
                    )
                    .then(|| world.component::<Position>(entity).copied())
                    .flatten()
                })
                .expect("populated maze should contain a camp");
            let reachable = reachable_positions(&maze, &world, camp_position);
            let treat_positions: Vec<Position> = world
                .entities_with::<Treat>()
                .into_iter()
                .filter_map(|entity| world.component::<Position>(entity).copied())
                .collect();

            for target in destination_positions.iter().chain(treat_positions.iter()) {
                assert!(
                    reachable.contains(target),
                    "seed {seed}, target: {target:?}"
                );
            }
        }
    }

    #[test]
    fn same_seed_reproduces_maze() {
        let config = MazeConfig::default();

        let maze_a = Maze::new(config, &mut Rng::from_seed(2026));
        let maze_b = Maze::new(config, &mut Rng::from_seed(2026));

        assert_eq!(maze_a.tiles, maze_b.tiles);
        assert_eq!(maze_a.rooms.len(), maze_b.rooms.len());
    }

    #[test]
    fn room_center_is_inside_the_room() {
        let room = Room {
            left: 4,
            top: 6,
            width: 5,
            height: 5,
        };
        let center = room.center();
        assert!((4..9).contains(&center.x));
        assert!((6..11).contains(&center.y));
    }
}
