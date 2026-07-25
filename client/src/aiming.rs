//! Ray picking for placing and breaking voxels.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use crate::models::{BlockCoord, LocalPlayer};
use crate::state::ClientState;
use crate::ui::{aim_screen_position, CrosshairBar, CrosshairSmooth};
use crate::voxels::{VoxelRayHit, VoxelWorld};

const AIM_DISTANCE: f32 = 12.0;
/// Screen-space smoothing (higher = snappier, lower = steadier).
const CROSSHAIR_SMOOTH: f32 = 0.28;

pub struct AimTarget {
    pub face_center: Vec3,
    pub coord: BlockCoord,
    pub normal: Vec3,
}

pub fn aim_ray(camera: &Camera, camera_tf: &GlobalTransform, window: &Window) -> Option<Ray3d> {
    let cursor = aim_screen_position(window);
    camera.viewport_to_world(camera_tf, cursor).ok()
}

pub fn query_aim_target(
    world: &VoxelWorld,
    ray: Ray3d,
    player_feet: Option<Vec3>,
) -> Option<AimTarget> {
    let direction = *ray.direction;
    let hit = world.raycast_blocks(ray.origin, direction, AIM_DISTANCE, player_feet)?;
    Some(aim_target_from_hit(hit))
}

fn aim_target_from_hit(hit: VoxelRayHit) -> AimTarget {
    AimTarget {
        face_center: VoxelWorld::face_center(hit.coord, hit.normal),
        coord: hit.coord,
        normal: hit.normal,
    }
}

/// Draw the crosshair at the screen projection of the targeted block face.
pub fn update_crosshair_position(
    time: Res<Time>,
    mut smooth: ResMut<CrosshairSmooth>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    player: Query<&GlobalTransform, With<LocalPlayer>>,
    world: Res<VoxelWorld>,
    mut bars: Query<(&mut Node, &mut Visibility, &CrosshairBar)>,
) {
    let Ok(window) = window.get_single() else {
        return;
    };
    let Ok((camera, camera_tf)) = camera.get_single() else {
        return;
    };

    let player_feet = player.get_single().ok().map(|tf| tf.translation());

    let raw = aim_ray(camera, camera_tf, window).and_then(|ray| {
        let target = query_aim_target(&world, ray, player_feet)?;
        camera.world_to_viewport(camera_tf, target.face_center).ok()
    });

    let dt = time.delta_secs();
    // Frame-rate independent smoothing toward the projected face center.
    let blend = 1.0 - (1.0 - CROSSHAIR_SMOOTH).powf(dt * 60.0);

    let show_at = match raw {
        Some(target) => {
            let pos = match smooth.position {
                Some(current) => current.lerp(target, blend),
                None => target,
            };
            smooth.position = Some(pos);
            Some(pos)
        }
        None => {
            smooth.position = None;
            None
        }
    };

    for (mut node, mut visibility, bar) in &mut bars {
        match show_at {
            Some(screen) => {
                *visibility = Visibility::Visible;
                node.left = Val::Px(screen.x - bar.half_width);
                node.top = Val::Px(screen.y - bar.half_height);
            }
            None => {
                *visibility = Visibility::Hidden;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_block_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    player: Query<&GlobalTransform, With<LocalPlayer>>,
    world: Res<VoxelWorld>,
    client: Res<ClientState>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        std::process::exit(0);
    }

    let Ok(window) = window.get_single() else {
        return;
    };
    let Ok((camera, camera_tf)) = camera.get_single() else {
        return;
    };
    let Some(ray) = aim_ray(camera, camera_tf, window) else {
        return;
    };

    let player_feet = player.get_single().ok().map(|tf| tf.translation());

    if mouse.just_pressed(MouseButton::Left) {
        if let Some(hit) = query_aim_target(&world, ray, player_feet) {
            let target = hit.coord.as_vec3() + hit.normal;
            let coord = BlockCoord::new(
                target.x.round() as i32,
                target.y.round() as i32,
                target.z.round() as i32,
            );
            if player_feet.is_some_and(|feet| {
                world.can_place_block_at(
                    coord,
                    feet,
                    crate::player::PLAYER_RADIUS,
                    crate::player::PLAYER_HEIGHT,
                )
            }) {
                client.session.place_block(coord);
            }
        }
    }

    if mouse.just_pressed(MouseButton::Right) {
        if let Some(hit) = query_aim_target(&world, ray, player_feet) {
            client.session.break_block(hit.coord);
        }
    }
}
