//! Voxel entities driven by server block updates.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::models::{block_color, BlockCoord, BlockUpdate, Voxel};

/// Block updates received during connect, applied on startup before play.
#[derive(Resource, Default)]
pub struct PendingBlocks(pub Vec<BlockUpdate>);

/// Tracks rendered voxels and applies server block updates.
#[derive(Resource)]
pub struct VoxelWorld {
    /// Collision grid — always updated synchronously in `apply`.
    blocks: HashMap<BlockCoord, i32>,
    voxels: HashMap<BlockCoord, Entity>,
    cube_mesh: Handle<Mesh>,
    materials: HashMap<i32, Handle<StandardMaterial>>,
}

pub fn setup_voxel_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands.insert_resource(VoxelWorld {
        blocks: HashMap::new(),
        voxels: HashMap::new(),
        cube_mesh,
        materials: HashMap::new(),
    });
}

impl VoxelWorld {
    pub fn cube_mesh(&self) -> Handle<Mesh> {
        self.cube_mesh.clone()
    }

    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn has_block_at(&self, coord: BlockCoord) -> bool {
        self.blocks.contains_key(&coord)
    }

    /// Center of the empty cell where a new block would be placed against `coord`/`normal`.
    pub fn placement_point(coord: BlockCoord, normal: Vec3) -> Vec3 {
        coord.as_vec3() + normal + Vec3::splat(0.5)
    }

    /// Grid raycast for block targeting — ignores the avatar and skips blocks overlapping the player.
    pub fn raycast_blocks(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        player_feet: Option<Vec3>,
    ) -> Option<VoxelRayHit> {
        let dir = direction.try_normalize()?;
        let radius = crate::player::PLAYER_RADIUS;
        let height = crate::player::PLAYER_HEIGHT;

        let mut t = 0.0_f32;
        let mut cell = block_coord_at(origin);
        let mut last_air = cell;

        let step_x = if dir.x >= 0.0 { 1 } else { -1 };
        let step_y = if dir.y >= 0.0 { 1 } else { -1 };
        let step_z = if dir.z >= 0.0 { 1 } else { -1 };

        let inv_dir = Vec3::new(
            if dir.x.abs() < f32::EPSILON {
                f32::INFINITY
            } else {
                1.0 / dir.x
            },
            if dir.y.abs() < f32::EPSILON {
                f32::INFINITY
            } else {
                1.0 / dir.y
            },
            if dir.z.abs() < f32::EPSILON {
                f32::INFINITY
            } else {
                1.0 / dir.z
            },
        );

        let mut t_max = Vec3::new(
            edge_distance(origin.x, cell.x, step_x, inv_dir.x),
            edge_distance(origin.y, cell.y, step_y, inv_dir.y),
            edge_distance(origin.z, cell.z, step_z, inv_dir.z),
        );
        let t_delta = Vec3::new(
            inv_dir.x.abs(),
            inv_dir.y.abs(),
            inv_dir.z.abs(),
        );

        loop {
            if t > max_distance {
                break;
            }

            if self.has_block_at(cell) {
                let skip = player_feet.is_some_and(|feet| {
                    self.block_overlaps_player(cell, feet, radius, height)
                        || self.block_under_player(cell, feet, radius)
                });
                if !skip {
                    let normal = Vec3::new(
                        (last_air.x - cell.x) as f32,
                        (last_air.y - cell.y) as f32,
                        (last_air.z - cell.z) as f32,
                    );
                    return Some(VoxelRayHit {
                        coord: cell,
                        normal,
                        distance: t,
                    });
                }
            } else {
                last_air = cell;
            }

            if t_max.x < t_max.y {
                if t_max.x < t_max.z {
                    t = t_max.x;
                    t_max.x += t_delta.x;
                    cell.x += step_x;
                } else {
                    t = t_max.z;
                    t_max.z += t_delta.z;
                    cell.z += step_z;
                }
            } else if t_max.y < t_max.z {
                t = t_max.y;
                t_max.y += t_delta.y;
                cell.y += step_y;
            } else {
                t = t_max.z;
                t_max.z += t_delta.z;
                cell.z += step_z;
            }
        }

        None
    }

    fn block_under_player(
        &self,
        coord: BlockCoord,
        feet: Vec3,
        radius: f32,
    ) -> bool {
        if coord.y as f32 >= feet.y + 0.5 {
            return false;
        }
        let cx = coord.x as f32 + 0.5;
        let cz = coord.z as f32 + 0.5;
        (cx - feet.x).abs() < radius + 0.5 && (cz - feet.z).abs() < radius + 0.5
    }

    /// True for floor voxels directly under the player's feet column.
    pub fn is_floor_under_player(&self, coord: BlockCoord, feet: Vec3) -> bool {
        self.block_under_player(coord, feet, crate::player::PLAYER_RADIUS)
    }

    /// True when placing a block at `coord` would intersect the player body.
    pub fn block_overlaps_player(
        &self,
        coord: BlockCoord,
        feet: Vec3,
        radius: f32,
        height: f32,
    ) -> bool {
        let (min_x, max_x, min_y, max_y, min_z, max_z) =
            player_aabb(feet.x, feet.y, feet.z, radius, height);
        aabb_overlap(
            (min_x, max_x),
            (min_y, max_y),
            (min_z, max_z),
            (
                coord.x as f32,
                (coord.x + 1) as f32,
                coord.y as f32,
                (coord.y + 1) as f32,
                coord.z as f32,
                (coord.z + 1) as f32,
            ),
        )
    }

    /// False when the cell is occupied or would trap the player inside a block.
    pub fn can_place_block_at(
        &self,
        coord: BlockCoord,
        feet: Vec3,
        radius: f32,
        height: f32,
    ) -> bool {
        !self.has_block_at(coord)
            && !self.block_overlaps_player(coord, feet, radius, height)
    }

    /// True when any of the 3×3 columns under `(x, z)` contains a block.
    pub fn has_support_at(&self, x: f32, z: f32) -> bool {
        column_coords(x).into_iter().any(|ix| {
            column_coords(z)
                .into_iter()
                .any(|iz| self.has_block_at(BlockCoord::new(ix, 0, iz)))
        })
    }

    #[cfg(test)]
    pub fn from_block_coords(blocks: impl IntoIterator<Item = BlockCoord>) -> Self {
        let mut map = HashMap::new();
        for coord in blocks {
            map.insert(coord, 1);
        }
        Self {
            blocks: map,
            voxels: HashMap::new(),
            cube_mesh: Handle::default(),
            materials: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub fn from_floor(size: i32) -> Self {
        let coords = (0..size).flat_map(|x| (0..size).map(move |z| BlockCoord::new(x, 0, z)));
        Self::from_block_coords(coords)
    }

    /// Walkable surface height under `(x, z)` near `feet_y`.
    pub fn ground_y_at(&self, x: f32, feet_y: f32, z: f32) -> Option<f32> {
        let mut best: Option<f32> = None;

        for ix in column_coords(x) {
            for iz in column_coords(z) {
                if let Some(top) = self.column_ground(ix, iz, feet_y) {
                    best = Some(best.map_or(top, |current| current.max(top)));
                }
            }
        }

        best
    }

    /// Highest supporting surface in one column at or below the player's feet.
    fn column_ground(&self, ix: i32, iz: i32, feet_y: f32) -> Option<f32> {
        for by in (0..=64).rev() {
            let coord = BlockCoord::new(ix, by, iz);
            if !self.blocks.contains_key(&coord) {
                continue;
            }
            let top = by as f32 + 1.0;
            let bottom = by as f32;
            if top <= feet_y + GROUND_TOLERANCE {
                return Some(top);
            }
            // Feet clipping down through the top face (landing / penetration resolve).
            if feet_y < top && feet_y > bottom {
                return Some(top);
            }
            // Block is above the feet — keep scanning down for the actual floor.
        }
        None
    }

    /// True when the player body AABB overlaps any block at `(x, feet_y, z)`.
    pub fn player_body_intersects(&self, x: f32, feet_y: f32, z: f32, radius: f32, height: f32) -> bool {
        let (min_x, max_x, min_y, max_y, min_z, max_z) = player_aabb(x, feet_y, z, radius, height);

        let bx0 = min_x.floor() as i32;
        let bx1 = max_x.floor() as i32;
        let by0 = min_y.floor() as i32;
        let by1 = max_y.floor() as i32;
        let bz0 = min_z.floor() as i32;
        let bz1 = max_z.floor() as i32;

        for bx in bx0..=bx1 {
            for by in by0..=by1 {
                for bz in bz0..=bz1 {
                    if !self.has_block_at(BlockCoord::new(bx, by, bz)) {
                        continue;
                    }
                    if aabb_overlap(
                        (min_x, max_x),
                        (min_y, max_y),
                        (min_z, max_z),
                        (
                            bx as f32,
                            (bx + 1) as f32,
                            by as f32,
                            (by + 1) as f32,
                            bz as f32,
                            (bz + 1) as f32,
                        ),
                    ) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Apply horizontal movement with axis-separated wall collision.
    pub fn resolve_horizontal_move(
        &self,
        pos: Vec3,
        delta: Vec3,
        radius: f32,
        height: f32,
    ) -> Vec3 {
        let mut result = pos;

        if delta.x.abs() > f32::EPSILON {
            let candidate = Vec3::new(pos.x + delta.x, pos.y, pos.z);
            if !self.player_body_intersects(candidate.x, candidate.y, candidate.z, radius, height) {
                result.x = candidate.x;
            }
        }

        if delta.z.abs() > f32::EPSILON {
            let candidate = Vec3::new(result.x, pos.y, pos.z + delta.z);
            if !self.player_body_intersects(candidate.x, candidate.y, candidate.z, radius, height) {
                result.z = candidate.z;
            }
        }

        result
    }

    pub fn apply(
        &mut self,
        commands: &mut Commands,
        materials: &mut Assets<StandardMaterial>,
        update: BlockUpdate,
    ) {
        if update.block_type == 0 {
            self.blocks.remove(&update.coord);
            self.remove(commands, update.coord);
            return;
        }
        self.blocks.insert(update.coord, update.block_type);
        self.upsert(commands, materials, update.coord, update.block_type);
    }
}

/// Result of a voxel-grid raycast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelRayHit {
    pub coord: BlockCoord,
    pub normal: Vec3,
    pub distance: f32,
}

fn block_coord_at(point: Vec3) -> BlockCoord {
    BlockCoord::new(
        point.x.floor() as i32,
        point.y.floor() as i32,
        point.z.floor() as i32,
    )
}

fn edge_distance(origin: f32, cell: i32, step: i32, inv_dir: f32) -> f32 {
    if inv_dir.is_infinite() {
        return f32::INFINITY;
    }
    let next_boundary = if step > 0 {
        (cell + 1) as f32
    } else {
        cell as f32
    };
    (next_boundary - origin) * inv_dir
}

/// Sample the column under a coordinate and its immediate neighbors.
fn column_coords(v: f32) -> [i32; 3] {
    let base = v.floor() as i32;
    [base - 1, base, base + 1]
}

/// Shared with ground checks in this module.
pub(crate) const GROUND_TOLERANCE: f32 = 0.1;

fn player_aabb(x: f32, feet_y: f32, z: f32, radius: f32, height: f32) -> (f32, f32, f32, f32, f32, f32) {
    (
        x - radius,
        x + radius,
        feet_y,
        feet_y + height,
        z - radius,
        z + radius,
    )
}

fn aabb_overlap(
    (min_x, max_x): (f32, f32),
    (min_y, max_y): (f32, f32),
    (min_z, max_z): (f32, f32),
    (bmin_x, bmax_x, bmin_y, bmax_y, bmin_z, bmax_z): (f32, f32, f32, f32, f32, f32),
) -> bool {
    min_x < bmax_x
        && max_x > bmin_x
        && min_y < bmax_y
        && max_y > bmin_y
        && min_z < bmax_z
        && max_z > bmin_z
}

impl VoxelWorld {
    fn remove(&mut self, commands: &mut Commands, coord: BlockCoord) {
        if let Some(entity) = self.voxels.remove(&coord) {
            commands.entity(entity).despawn();
        }
    }

    fn upsert(
        &mut self,
        commands: &mut Commands,
        materials: &mut Assets<StandardMaterial>,
        coord: BlockCoord,
        block_type: i32,
    ) {
        if let Some(entity) = self.voxels.remove(&coord) {
            commands.entity(entity).despawn();
        }

        let material = self
            .materials
            .entry(block_type)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: block_color(block_type),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .clone();

        let position = coord.as_vec3() + Vec3::new(0.0, 0.5, 0.0);
        let entity = commands
            .spawn((
                Voxel { coord },
                Mesh3d(self.cube_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(position),
            ))
            .id();

        self.voxels.insert(coord, entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BlockCoord;

    fn world_with_blocks(blocks: impl IntoIterator<Item = BlockCoord>) -> VoxelWorld {
        VoxelWorld::from_block_coords(blocks)
    }

    #[test]
    fn ground_y_at_returns_top_of_highest_block_in_column() {
        let world = world_with_blocks([
            BlockCoord::new(3, 0, 5),
            BlockCoord::new(3, 2, 5),
        ]);

        assert_eq!(world.ground_y_at(3.4, 10.0, 5.9), Some(3.0));
        assert_eq!(world.ground_y_at(99.0, 10.0, 5.0), None);
    }

    #[test]
    fn ground_y_at_finds_floor_while_falling() {
        let world = world_with_blocks([BlockCoord::new(3, 0, 5)]);

        assert_eq!(world.ground_y_at(3.4, 10.0, 5.9), Some(1.0));
        assert_eq!(world.ground_y_at(3.4, 0.2, 5.9), Some(1.0));
    }

    #[test]
    fn ground_y_at_does_not_step_onto_block_stack() {
        let world = world_with_blocks([
            BlockCoord::new(3, 0, 5),
            BlockCoord::new(3, 1, 5),
        ]);

        assert_eq!(world.ground_y_at(3.4, 1.0, 5.9), Some(1.0));
    }

    #[test]
    fn horizontal_move_blocked_by_wall() {
        let world = world_with_blocks([
            BlockCoord::new(0, 0, 0),
            BlockCoord::new(1, 0, 0),
            BlockCoord::new(2, 0, 0),
            BlockCoord::new(3, 0, 0),
            BlockCoord::new(3, 1, 0),
            BlockCoord::new(3, 2, 0),
        ]);

        let start = Vec3::new(1.5, 1.0, 0.5);
        let moved = world.resolve_horizontal_move(
            start,
            Vec3::new(2.0, 0.0, 0.0),
            crate::player::PLAYER_RADIUS,
            crate::player::PLAYER_HEIGHT,
        );

        assert!(
            moved.x < 3.0,
            "should not walk into a 3-block stack, x={}",
            moved.x
        );
        assert!((moved.y - start.y).abs() < f32::EPSILON);
    }

    #[test]
    fn ground_y_at_prefers_floor_over_ceiling() {
        let world = world_with_blocks([
            BlockCoord::new(3, 0, 5),
            BlockCoord::new(3, 4, 5),
        ]);

        assert_eq!(world.ground_y_at(3.4, 1.0, 5.9), Some(1.0));
    }

    #[test]
    fn raycast_skips_blocks_under_player_and_hits_wall_ahead() {
        let world = world_with_blocks([
            BlockCoord::new(5, 0, 5),
            BlockCoord::new(5, 0, 0),
            BlockCoord::new(5, 1, 0),
            BlockCoord::new(5, 2, 0),
        ]);

        let feet = Vec3::new(5.5, 1.0, 5.5);
        let origin = Vec3::new(5.5, 2.0, 8.0);
        let direction = Vec3::new(0.0, 0.0, -1.0);

        let hit = world
            .raycast_blocks(origin, direction, 24.0, Some(feet))
            .expect("should hit the wall beyond the player");

        assert_eq!(hit.coord, BlockCoord::new(5, 2, 0));
        assert_eq!(hit.normal, Vec3::Z);
    }

    #[test]
    fn raycast_skips_floor_under_player() {
        let world = world_with_blocks([BlockCoord::new(5, 0, 5)]);

        let feet = Vec3::new(5.5, 1.0, 5.5);
        let origin = Vec3::new(5.5, 2.0, 8.0);
        let direction = Vec3::new(0.0, -0.3, -1.0).normalize();

        assert!(world
            .raycast_blocks(origin, direction, 24.0, Some(feet))
            .is_none());
    }

    #[test]
    fn placement_coord_offsets_along_face_normal() {
        let hit = BlockCoord::new(3, 1, 4);
        let normal = Vec3::NEG_Z;
        let placed = BlockCoord::new(
            hit.x + normal.x as i32,
            hit.y + normal.y as i32,
            hit.z + normal.z as i32,
        );
        assert_eq!(placed, BlockCoord::new(3, 1, 3));
    }

    #[test]
    fn placement_point_is_center_of_adjacent_cell() {
        let point = VoxelWorld::placement_point(BlockCoord::new(1, 0, 2), Vec3::NEG_Z);
        assert_eq!(point, Vec3::new(1.5, 0.5, 1.5));
    }

    #[test]
    fn cannot_place_block_that_overlaps_player() {
        let world = VoxelWorld::from_floor(48);
        let feet = Vec3::new(24.0, 1.0, 24.0);
        let radius = crate::player::PLAYER_RADIUS;
        let height = crate::player::PLAYER_HEIGHT;

        assert!(!world.can_place_block_at(
            BlockCoord::new(24, 1, 24),
            feet,
            radius,
            height,
        ));
        assert!(world.can_place_block_at(
            BlockCoord::new(26, 1, 24),
            feet,
            radius,
            height,
        ));
    }
}
