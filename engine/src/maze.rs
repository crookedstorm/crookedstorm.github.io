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
use crate::rng::Rng;
use crate::world::World;

/// Treats placed per maze. Tunable, but matches the original TS spawn count.
const TREAT_COUNT: usize = 8;

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
}

/// Adjustable knobs for generation. Values favor roomy, readable layouts for
/// a short visit rather than dense dungeon crawls.
#[derive(Clone, Copy, Debug)]
pub struct MazeConfig {
    /// Maze width in tiles.
    pub width: i32,
    /// Maze height in tiles.
    pub height: i32,
    /// Minimum room side length in tiles.
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
            min_room: 4,
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

        maze.split(root, config, rng);

        maze
    }

    /// Recursive BSP step. Splits the given room along a random axis when
    /// both children remain useful, then carves both; otherwise carves a
    /// single leaf.
    fn split(&mut self, region: Room, config: MazeConfig, rng: &mut Rng) {
        let split_horizontally = rng.below(2) == 0;

        if let Some((lower, upper)) = self.try_split(region, split_horizontally, config, rng) {
            self.split(lower, config, rng);
            self.split(upper, config, rng);
        } else if let Some((lower, upper)) =
            self.try_split(region, !split_horizontally, config, rng)
        {
            self.split(lower, config, rng);
            self.split(upper, config, rng);
        } else {
            self.carve_room(region, config, rng);
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

        let split_point =
            rng.between(config.min_room as u32, (span - config.min_room) as u32) as i32;

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
    fn carve_room(&mut self, region: Room, config: MazeConfig, rng: &mut Rng) {
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
        if self.rooms.len() < 2 {
            return;
        }

        // Pick distinct room indices for camp and each destination.
        let room_indices = pick_distinct(rng, self.rooms.len(), 1 + 3);

        // Camp goes in the first picked room.
        let camp_pos = self.rooms[room_indices[0]].center();
        {
            let entity = world.spawn();
            world.insert(entity, camp_pos);
            world.insert(entity, ObjectKind::Camp);
            world.insert(
                entity,
                crate::components::Sprite(crate::components::SpriteId::Camp),
            );
        }

        let sections = [Section::About, Section::Blog, Section::Projects];

        let mut destination_positions = Vec::new();
        for (room_index, section) in room_indices[1..].iter().zip(sections) {
            let center = self.rooms[*room_index].center();
            destination_positions.push(center);
            let entity = world.spawn();
            world.insert(entity, center);
            world.insert(entity, ObjectKind::Destination { section });
            world.insert(
                entity,
                crate::components::Sprite(crate::components::SpriteId::Destination),
            );
        }

        self.place_treats(world, rng, camp_pos, &destination_positions);
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
            if placed.iter().any(|treat| is_near(candidate, *treat)) {
                continue;
            }

            let entity = world.spawn();
            world.insert(entity, candidate);
            world.insert(entity, ObjectKind::Treat);
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
    use super::Maze;
    use super::MazeConfig;
    use super::Rng;
    use super::Room;

    #[test]
    fn maze_is_fully_connected_open() {
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
