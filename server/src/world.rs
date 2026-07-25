use std::collections::HashMap;

/// World bounds and initial floor layout.
pub const WORLD_SIZE_X: i32 = 64;
pub const WORLD_SIZE_Y: i32 = 64;
pub const WORLD_SIZE_Z: i32 = 64;
pub const FLOOR_SIZE: i32 = 48;

const FLOOR_COLORS: [i32; 6] = [1, 2, 3, 4, 5, 6];
const SPAWN_HEIGHT: f32 = 1.0;

/// A coordinate key for a single voxel in the world.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct BlockCoord {
    x: i32,
    y: i32,
    z: i32,
}

/// A placed block returned in world snapshots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlockEntry {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_type: i32,
}

/// A 3D voxel world storing only non-air blocks.
pub struct World {
    size_x: i32,
    size_y: i32,
    size_z: i32,
    blocks: HashMap<BlockCoord, i32>,
}

impl World {
    /// Creates an empty world with the given bounds. Air blocks are not stored.
    pub fn new(size_x: i32, size_y: i32, size_z: i32) -> Self {
        Self {
            size_x,
            size_y,
            size_z,
            blocks: HashMap::new(),
        }
    }

    /// Creates the default playable world with a multicolored floor.
    pub fn create_initial() -> Self {
        let mut world = Self::new(WORLD_SIZE_X, WORLD_SIZE_Y, WORLD_SIZE_Z);
        world.seed_floor();
        world
    }

    /// Default spawn point above the center of the initial floor.
    pub fn spawn_position() -> (f32, f32, f32) {
        (
            FLOOR_SIZE as f32 / 2.0,
            SPAWN_HEIGHT,
            FLOOR_SIZE as f32 / 2.0,
        )
    }

    fn seed_floor(&mut self) {
        for z in 0..FLOOR_SIZE {
            for x in 0..FLOOR_SIZE {
                let block_type = FLOOR_COLORS[((x + z) as usize) % FLOOR_COLORS.len()];
                self.set_block(x, 0, z, block_type);
            }
        }
    }

    /// Restore a walkable 3×3 floor pad under `(x, z)` when blocks are missing.
    pub fn ensure_spawn_pad(&mut self, x: f32, z: f32) -> Vec<BlockEntry> {
        let cx = x.floor() as i32;
        let cz = z.floor() as i32;
        let mut placed = Vec::new();

        for dx in -1..=1 {
            for dz in -1..=1 {
                let bx = cx + dx;
                let bz = cz + dz;
                if self.get_block(bx, 0, bz).unwrap_or(0) != 0 {
                    continue;
                }
                let block_type = FLOOR_COLORS[((bx + bz) as usize) % FLOOR_COLORS.len()];
                if self.set_block(bx, 0, bz, block_type) {
                    placed.push(BlockEntry {
                        x: bx,
                        y: 0,
                        z: bz,
                        block_type,
                    });
                }
            }
        }

        placed
    }

    pub fn size_x(&self) -> i32 {
        self.size_x
    }

    pub fn size_y(&self) -> i32 {
        self.size_y
    }

    pub fn size_z(&self) -> i32 {
        self.size_z
    }

    /// Returns all non-air blocks currently in the world.
    pub fn blocks(&self) -> Vec<BlockEntry> {
        self.blocks
            .iter()
            .map(|(coord, block_type)| BlockEntry {
                x: coord.x,
                y: coord.y,
                z: coord.z,
                block_type: *block_type,
            })
            .collect()
    }

    /// Returns the block type at the given coordinate, or None if out of bounds.
    #[allow(dead_code)]
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        if !Self::in_bounds(self.size_x, self.size_y, self.size_z, x, y, z) {
            return None;
        }
        Some(
            self.blocks
                .get(&BlockCoord { x, y, z })
                .copied()
                .unwrap_or(0),
        )
    }

    /// Places a block at the given coordinate. Air removes the entry. Returns false if out of bounds.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block_type: i32) -> bool {
        if !Self::in_bounds(self.size_x, self.size_y, self.size_z, x, y, z) {
            return false;
        }
        let coord = BlockCoord { x, y, z };
        if block_type == 0 {
            self.blocks.remove(&coord);
        } else {
            self.blocks.insert(coord, block_type);
        }
        true
    }

    /// Removes a block at the given coordinate. Returns false if out of bounds.
    pub fn break_block(&mut self, x: i32, y: i32, z: i32) -> bool {
        self.set_block(x, y, z, 0)
    }

    fn in_bounds(size_x: i32, size_y: i32, size_z: i32, x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < size_x && y < size_y && z < size_z
    }
}

#[cfg(test)]
mod tests {
    use super::World;

    #[test]
    fn set_and_break_block() {
        let mut world = World::new(4, 4, 4);
        assert!(world.set_block(1, 2, 3, 5));
        assert_eq!(world.get_block(1, 2, 3), Some(5));
        assert_eq!(world.blocks().len(), 1);
        assert!(world.break_block(1, 2, 3));
        assert_eq!(world.get_block(1, 2, 3), Some(0));
        assert!(world.blocks().is_empty());
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let world = World::new(4, 4, 4);
        assert_eq!(world.get_block(-1, 0, 0), None);
        assert_eq!(world.get_block(4, 0, 0), None);
    }

    #[test]
    fn empty_world_stores_nothing() {
        let world = World::new(64, 64, 64);
        assert!(world.blocks().is_empty());
    }

    #[test]
    fn create_initial_seeds_multicolored_floor() {
        let world = World::create_initial();
        assert_eq!(world.blocks().len(), (super::FLOOR_SIZE * super::FLOOR_SIZE) as usize);
        assert_eq!(world.get_block(0, 0, 0), Some(1));
        assert_eq!(world.get_block(1, 0, 0), Some(2));
    }

    #[test]
    fn spawn_position_is_above_floor_center() {
        let (x, y, z) = World::spawn_position();
        assert_eq!(x, super::FLOOR_SIZE as f32 / 2.0);
        assert_eq!(y, super::SPAWN_HEIGHT);
        assert_eq!(z, super::FLOOR_SIZE as f32 / 2.0);
    }

    #[test]
    fn ensure_spawn_pad_restores_missing_floor() {
        let mut world = World::create_initial();
        for dx in -1..=1 {
            for dz in -1..=1 {
                world.break_block(24 + dx, 0, 24 + dz);
            }
        }
        assert_eq!(world.get_block(24, 0, 24), Some(0));

        let placed = world.ensure_spawn_pad(24.0, 24.0);
        assert_eq!(placed.len(), 9);
        assert_ne!(world.get_block(24, 0, 24), Some(0));
    }

    #[test]
    fn set_block_out_of_bounds_returns_false() {
        let mut world = World::new(4, 4, 4);
        assert!(!world.set_block(-1, 0, 0, 1));
        assert!(!world.set_block(4, 0, 0, 1));
    }

    #[test]
    fn set_air_removes_existing_block() {
        let mut world = World::new(4, 4, 4);
        assert!(world.set_block(0, 0, 0, 3));
        assert!(world.set_block(0, 0, 0, 0));
        assert_eq!(world.get_block(0, 0, 0), Some(0));
        assert!(world.blocks().is_empty());
    }

    #[test]
    fn overwrite_block_type() {
        let mut world = World::new(4, 4, 4);
        assert!(world.set_block(1, 1, 1, 2));
        assert!(world.set_block(1, 1, 1, 7));
        assert_eq!(world.get_block(1, 1, 1), Some(7));
        assert_eq!(world.blocks().len(), 1);
    }

    #[test]
    fn blocks_lists_all_placed_voxels() {
        let mut world = World::new(8, 8, 8);
        assert!(world.set_block(0, 0, 0, 1));
        assert!(world.set_block(2, 3, 4, 5));
        let mut entries = world.blocks();
        entries.sort_by_key(|b| (b.x, b.y, b.z));
        assert_eq!(
            entries,
            vec![
                super::BlockEntry {
                    x: 0,
                    y: 0,
                    z: 0,
                    block_type: 1
                },
                super::BlockEntry {
                    x: 2,
                    y: 3,
                    z: 4,
                    block_type: 5
                },
            ]
        );
    }
}
