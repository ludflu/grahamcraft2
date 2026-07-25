//! Player body vs voxel collision — must match the Rust client (`client/src/player.rs`).

pub const PLAYER_RADIUS: f32 = 0.35;
pub const PLAYER_HEIGHT: f32 = 2.0;

/// True when a 1×1×1 block at `(bx, by, bz)` intersects the player body AABB.
pub fn block_overlaps_player(bx: i32, by: i32, bz: i32, feet_x: f32, feet_y: f32, feet_z: f32) -> bool {
    let (min_x, max_x, min_y, max_y, min_z, max_z) =
        player_aabb(feet_x, feet_y, feet_z, PLAYER_RADIUS, PLAYER_HEIGHT);
    aabb_overlap(
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
    )
}

fn player_aabb(
    x: f32,
    feet_y: f32,
    z: f32,
    radius: f32,
    height: f32,
) -> (f32, f32, f32, f32, f32, f32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_at_chest_height_overlaps_player() {
        assert!(block_overlaps_player(24, 1, 24, 24.0, 1.0, 24.0));
    }

    #[test]
    fn block_above_head_does_not_overlap() {
        assert!(!block_overlaps_player(24, 3, 24, 24.0, 1.0, 24.0));
    }

    #[test]
    fn block_beside_player_does_not_overlap() {
        assert!(!block_overlaps_player(26, 1, 24, 24.0, 1.0, 24.0));
    }
}
