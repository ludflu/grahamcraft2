//! Ray picking for placing and breaking voxels.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use crate::models::{BlockCoord, CameraPivot, LocalPlayer, RaycastIgnore, Voxel};
use crate::player::{PLAYER_HEIGHT, PLAYER_RADIUS};
use crate::state::ClientState;
use crate::ui::{aim_screen_position, CrosshairBar, CrosshairSmooth};
use crate::voxels::VoxelWorld;

const AIM_DISTANCE: f32 = 24.0;
/// Screen-space smoothing (higher = snappier, lower = steadier).
const CROSSHAIR_SMOOTH: f32 = 0.28;

pub struct AimTarget {
    pub aim_point: Vec3,
    pub coord: BlockCoord,
    pub normal: Vec3,
}

/// Screen-center ray from the eye pivot (matches the Python client's aim origins).
pub fn aim_ray(
    camera: &Camera,
    camera_tf: &GlobalTransform,
    pivot_tf: Option<&GlobalTransform>,
    window: &Window,
) -> Option<Ray3d> {
    let cursor = aim_screen_position(window);
    let world_ray = camera.viewport_to_world(camera_tf, cursor).ok()?;
    let direction = world_ray.direction;
    let origin = pivot_tf
        .map(|pivot| pivot.translation() + *direction * 0.05)
        .unwrap_or(world_ray.origin);
    Some(Ray3d::new(origin, direction))
}

pub fn query_aim_target(
    ray_cast: &mut MeshRayCast,
    world: &VoxelWorld,
    ray: Ray3d,
    player_feet: Option<Vec3>,
    ignore: &Query<Entity, With<RaycastIgnore>>,
    voxels: &Query<(Entity, &Voxel)>,
) -> Option<AimTarget> {
    let filter = |entity: Entity| voxels.get(entity).is_ok() && !ignore.contains(entity);
    let settings = RayCastSettings::default()
        .with_filter(&filter)
        .with_visibility(RayCastVisibility::Any)
        .never_early_exit();

    for (entity, hit) in ray_cast.cast_ray(ray, &settings) {
        if ray.origin.distance(hit.point) > AIM_DISTANCE {
            continue;
        }

        let Ok((_, voxel)) = voxels.get(*entity) else {
            continue;
        };

        if player_feet.is_some_and(|feet| world.is_floor_under_player(voxel.coord, feet)) {
            continue;
        }

        return Some(AimTarget {
            aim_point: VoxelWorld::placement_point(voxel.coord, hit.normal),
            coord: voxel.coord,
            normal: hit.normal,
        });
    }

    None
}

pub fn placement_coord(hit: &AimTarget) -> BlockCoord {
    BlockCoord::new(
        hit.coord.x + hit.normal.x as i32,
        hit.coord.y + hit.normal.y as i32,
        hit.coord.z + hit.normal.z as i32,
    )
}

/// Draw the crosshair at the screen projection of the block placement point.
pub fn update_crosshair_position(
    time: Res<Time>,
    mut smooth: ResMut<CrosshairSmooth>,
    mut ray_cast: MeshRayCast,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    pivot: Query<&GlobalTransform, With<CameraPivot>>,
    player: Query<&GlobalTransform, With<LocalPlayer>>,
    ignore: Query<Entity, With<RaycastIgnore>>,
    voxels: Query<(Entity, &Voxel)>,
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
    let pivot_tf = pivot.get_single().ok();

    let raw = aim_ray(camera, camera_tf, pivot_tf, window).and_then(|ray| {
        let target = query_aim_target(
            &mut ray_cast,
            &world,
            ray,
            player_feet,
            &ignore,
            &voxels,
        )?;
        camera.world_to_viewport(camera_tf, target.aim_point).ok()
    });

    let dt = time.delta_secs();
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
    mut ray_cast: MeshRayCast,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    pivot: Query<&GlobalTransform, With<CameraPivot>>,
    player: Query<&GlobalTransform, With<LocalPlayer>>,
    ignore: Query<Entity, With<RaycastIgnore>>,
    voxels: Query<(Entity, &Voxel)>,
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
    let Some(ray) = aim_ray(
        camera,
        camera_tf,
        pivot.get_single().ok(),
        window,
    ) else {
        return;
    };

    let player_feet = player.get_single().ok().map(|tf| tf.translation());

    if mouse.just_pressed(MouseButton::Left) {
        let Some(hit) = query_aim_target(
            &mut ray_cast,
            &world,
            ray,
            player_feet,
            &ignore,
            &voxels,
        ) else {
            return;
        };

        let coord = placement_coord(&hit);
        if player_feet.is_none_or(|feet| {
            world.can_place_block_at(coord, feet, PLAYER_RADIUS, PLAYER_HEIGHT)
        }) {
            client.session.place_block(coord);
        }
    }

    if mouse.just_pressed(MouseButton::Right) {
        if let Some(hit) = query_aim_target(
            &mut ray_cast,
            &world,
            ray,
            player_feet,
            &ignore,
            &voxels,
        ) {
            client.session.break_block(hit.coord);
        }
    }
}
