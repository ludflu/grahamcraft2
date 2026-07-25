//! Player controller with arrow-key movement and position sync.

use std::time::Duration;

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::models::{CameraPivot, LocalPlayer, RaycastIgnore};
use crate::player_model::spawn_local_avatar;
use crate::state::ClientState;
use crate::voxels::VoxelWorld;

const JUMP_SPEED: f32 = 6.5;
const GRAVITY: f32 = 24.0;
const MAX_FALL_SPEED: f32 = 14.0;
const MAX_FALL_STEP: f32 = 0.35;
const SYNC_INTERVAL: Duration = Duration::from_millis(50);
const MOVE_SPEED: f32 = 5.0;
/// Ursina FirstPersonController default height (eye ray origin).
pub const PLAYER_HEIGHT: f32 = 2.0;
/// Horizontal half-width of the player body for wall collision.
pub const PLAYER_RADIUS: f32 = 0.35;
/// Matches Ursina `mouse_sensitivity = Vec2(40, 40)` scaled for Bevy pixel deltas.
const MOUSE_SENSITIVITY: Vec2 = Vec2::new(0.004, 0.004);
const MIN_PITCH: f32 = -90.0_f32.to_radians();
const MAX_PITCH: f32 = 90.0_f32.to_radians();
/// Third-person offset matching Python `CAMERA_BACK_OFFSET = Vec3(0, 0.25, -6)`.
const CAMERA_SHOULDER: f32 = 0.25;
pub const CAMERA_DISTANCE: f32 = 6.0;
const CAMERA_FOV: f32 = 90.0;
/// Python: `ground.distance <= self.height + 0.1` — use feet tolerance equivalent.
const GROUND_TOLERANCE: f32 = 0.1;

/// Vertical physics state extracted for unit tests and systems.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerPhysics {
    pub gravity_enabled: bool,
    pub grounded: bool,
    pub vertical_velocity: f32,
}

impl Default for PlayerPhysics {
    fn default() -> Self {
        Self {
            gravity_enabled: true,
            grounded: false,
            vertical_velocity: 0.0,
        }
    }
}

/// Snap the player onto ground when close enough (runs before gravity each frame).
pub fn step_ensure_on_ground(y: f32, physics: &mut PlayerPhysics, world: &VoxelWorld, x: f32, z: f32) -> f32 {
    if physics.grounded {
        return y;
    }

    // Don't cancel a jump on the same frame space was pressed.
    if physics.vertical_velocity > 0.0 {
        return y;
    }

    let Some(ground_y) = ground_y_with_fallback(world, x, y, z) else {
        return y;
    };

    if y <= ground_y + GROUND_TOLERANCE {
        physics.vertical_velocity = 0.0;
        physics.grounded = true;
        ground_y
    } else {
        y
    }
}

/// Apply one frame of gravity and ground collision. Returns new feet height.
pub fn step_player_gravity(
    x: f32,
    z: f32,
    mut y: f32,
    physics: &mut PlayerPhysics,
    world: &VoxelWorld,
    dt: f32,
) -> f32 {
    if !physics.gravity_enabled {
        return y;
    }

    if !physics.grounded {
        physics.vertical_velocity -= GRAVITY * dt;
        physics.vertical_velocity = physics.vertical_velocity.max(-MAX_FALL_SPEED);
        let mut step = physics.vertical_velocity * dt;
        if step < 0.0 {
            step = step.max(-MAX_FALL_STEP);
        }
        y += step;
    }

    let Some(ground_y) = ground_y_with_fallback(world, x, y, z) else {
        physics.grounded = false;
        return y;
    };

    if y < ground_y {
        physics.vertical_velocity = 0.0;
        physics.grounded = true;
        return ground_y;
    }

    let on_ground = y - ground_y <= GROUND_TOLERANCE && physics.vertical_velocity <= 0.0;
    if on_ground {
        physics.vertical_velocity = 0.0;
        physics.grounded = true;
        return ground_y;
    }

    physics.grounded = false;
    y
}

const FLOOR_TOP_Y: f32 = 1.0;
/// Treat as a populated world when at least this many blocks are loaded.
const MIN_WORLD_BLOCKS: usize = 100;

fn ground_y_with_fallback(world: &VoxelWorld, x: f32, feet_y: f32, z: f32) -> Option<f32> {
    world
        .ground_y_at(x, feet_y, z)
        .or_else(|| (world.block_count() >= MIN_WORLD_BLOCKS).then_some(FLOOR_TOP_Y))
}

pub fn snap_to_ground_y(x: f32, z: f32, world: &VoxelWorld) -> Option<f32> {
    ground_y_with_fallback(world, x, 0.0, z)
}

#[derive(Component)]
pub struct PlayerController;

/// Yaw (player) and pitch (camera pivot) in radians — mirrors Ursina FirstPersonController.
#[derive(Component, Clone, Copy)]
pub struct CameraOrbit {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

/// Horizontal forward/right from yaw, matching Ursina player `forward` / `right`.
pub fn horizontal_forward(yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * -Vec3::Z
}

pub fn horizontal_right(yaw: f32) -> Vec3 {
    Quat::from_rotation_y(yaw) * Vec3::X
}

pub fn spawn_player(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    client: Res<ClientState>,
    world: Res<VoxelWorld>,
    mut remote: ResMut<crate::remote_players::RemotePlayers>,
) {
    remote.set_local_player(client.player_id.clone());

    let player = commands
        .spawn((
            LocalPlayer,
            PlayerController,
            CameraOrbit::default(),
            Transform::from_translation(client.spawn_position),
            Visibility::default(),
        ))
        .id();

    let pivot = commands
        .spawn((
            CameraPivot,
            RaycastIgnore,
            Transform::from_xyz(0.0, PLAYER_HEIGHT, 0.0),
            Visibility::default(),
        ))
        .id();
    commands.entity(player).add_child(pivot);

    commands.entity(pivot).with_children(|parent| {
        parent.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection {
                fov: CAMERA_FOV.to_radians(),
                ..default()
            }),
            Transform::from_translation(Vec3::new(0.0, CAMERA_SHOULDER, CAMERA_DISTANCE))
                .looking_at(Vec3::new(0.0, -CAMERA_SHOULDER, 0.0), Vec3::Y),
        ));
    });

    spawn_local_avatar(
        &mut commands,
        world.cube_mesh(),
        &mut materials,
        player,
    );
}

pub fn grab_cursor(mut window: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = window.get_single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::Locked;
        window.cursor_options.visible = false;
    }
}

pub fn player_look(
    mut motion: EventReader<MouseMotion>,
    mut player: Query<(&mut Transform, &mut CameraOrbit), With<PlayerController>>,
    mut pivot: Query<&mut Transform, (With<CameraPivot>, Without<PlayerController>)>,
) {
    let mut delta = Vec2::ZERO;
    for event in motion.read() {
        delta += event.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    let Ok((mut player_tf, mut orbit)) = player.get_single_mut() else {
        return;
    };
    let Ok(mut pivot_tf) = pivot.get_single_mut() else {
        return;
    };

    // Inverted horizontal look; vertical uses standard (mouse up = look up).
    orbit.yaw -= delta.x * MOUSE_SENSITIVITY.y;
    orbit.pitch = (orbit.pitch - delta.y * MOUSE_SENSITIVITY.x).clamp(MIN_PITCH, MAX_PITCH);

    player_tf.rotation = Quat::from_rotation_y(orbit.yaw);
    pivot_tf.rotation = Quat::from_rotation_x(orbit.pitch);
}

/// Horizontal displacement from keyboard input this frame (WASD and arrow keys).
pub fn movement_delta_from_keys(keys: &ButtonInput<KeyCode>, yaw: f32, dt: f32) -> Vec3 {
    let speed = MOVE_SPEED * dt;
    let forward = horizontal_forward(yaw);
    let right = horizontal_right(yaw);
    let mut delta = Vec3::ZERO;

    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        delta += forward * speed;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        delta -= forward * speed;
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        delta -= right * speed;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        delta += right * speed;
    }

    delta
}

/// Apply WASD / arrow-key movement with wall collision.
pub fn apply_horizontal_move(
    pos: Vec3,
    keys: &ButtonInput<KeyCode>,
    yaw: f32,
    dt: f32,
    world: &VoxelWorld,
) -> Vec3 {
    let delta = movement_delta_from_keys(keys, yaw, dt);
    if delta.length_squared() > 0.0 {
        world.resolve_horizontal_move(pos, delta, PLAYER_RADIUS, PLAYER_HEIGHT)
    } else {
        pos
    }
}

pub fn player_move(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut client: ResMut<ClientState>,
    mut player: Query<(&mut Transform, &CameraOrbit), With<PlayerController>>,
    world: Res<VoxelWorld>,
) {
    let Ok((mut transform, orbit)) = player.get_single_mut() else {
        return;
    };

    let speed = MOVE_SPEED * time.delta_secs();
    transform.translation = apply_horizontal_move(
        transform.translation,
        &keys,
        orbit.yaw,
        time.delta_secs(),
        &world,
    );

    if !client.gravity_enabled && keys.pressed(KeyCode::Space) {
        transform.translation.y += speed;
    }

    if client.gravity_enabled && keys.just_pressed(KeyCode::Space) && client.grounded {
        client.grounded = false;
        client.vertical_velocity = JUMP_SPEED;
    }
}

pub fn ensure_on_ground(
    mut client: ResMut<ClientState>,
    mut player: Query<&mut Transform, With<LocalPlayer>>,
    world: Res<VoxelWorld>,
) {
    let Ok(mut transform) = player.get_single_mut() else {
        return;
    };

    let pos = transform.translation;
    let mut physics = PlayerPhysics {
        gravity_enabled: client.gravity_enabled,
        grounded: client.grounded,
        vertical_velocity: client.vertical_velocity,
    };
    transform.translation.y = step_ensure_on_ground(pos.y, &mut physics, &world, pos.x, pos.z);
    client.grounded = physics.grounded;
    client.vertical_velocity = physics.vertical_velocity;
}

pub fn player_gravity(
    time: Res<Time>,
    mut client: ResMut<ClientState>,
    mut player: Query<&mut Transform, With<PlayerController>>,
    world: Res<VoxelWorld>,
) {
    let Ok(mut transform) = player.get_single_mut() else {
        return;
    };

    let pos = transform.translation;
    let mut physics = PlayerPhysics {
        gravity_enabled: client.gravity_enabled,
        grounded: client.grounded,
        vertical_velocity: client.vertical_velocity,
    };
    transform.translation.y = step_player_gravity(
        pos.x,
        pos.z,
        pos.y,
        &mut physics,
        &world,
        time.delta_secs(),
    );
    client.grounded = physics.grounded;
    client.vertical_velocity = physics.vertical_velocity;
}

pub fn snap_to_ground(
    transform: &mut Transform,
    client: &mut ClientState,
    world: &VoxelWorld,
) {
    if let Some(y) = snap_to_ground_y(
        transform.translation.x,
        transform.translation.z,
        world,
    ) {
        transform.translation.y = y;
        client.grounded = true;
        client.vertical_velocity = 0.0;
    }
}

pub fn sync_position(mut client: ResMut<ClientState>, player: Query<&Transform, With<PlayerController>>) {
    let Ok(transform) = player.get_single() else {
        return;
    };

    let pos = transform.translation;
    let airborne = !client.grounded;
    let landed = client.grounded && client.was_airborne;
    client.was_airborne = airborne;

    let moved = (pos - client.last_position).length_squared() > 0.0001;

    if airborne && !landed {
        if client.last_sync.elapsed() < SYNC_INTERVAL {
            return;
        }
    } else if !(landed || moved) {
        return;
    }

    client
        .session
        .send_position(pos.x, pos.y, pos.z);
    client.last_position = pos;
    client.last_sync = std::time::Instant::now();
}

pub fn toggle_gravity(keys: Res<ButtonInput<KeyCode>>, mut client: ResMut<ClientState>) {
    if keys.just_pressed(KeyCode::KeyG) {
        client.gravity_enabled = !client.gravity_enabled;
        if !client.gravity_enabled {
            client.vertical_velocity = 0.0;
        }
        info!(
            "Gravity {}",
            if client.gravity_enabled { "on" } else { "off" }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BlockCoord;

    const FLOOR_SIZE: i32 = 48;
    const SPAWN: Vec3 = Vec3::new(24.0, 1.0, 24.0);
    const DT: f32 = 1.0 / 60.0;

    fn server_floor_world() -> VoxelWorld {
        VoxelWorld::from_floor(FLOOR_SIZE)
    }

    fn simulate_frame(
        x: &mut f32,
        y: &mut f32,
        z: &mut f32,
        physics: &mut PlayerPhysics,
        world: &VoxelWorld,
        dx: f32,
        dz: f32,
    ) {
        *x += dx;
        *z += dz;
        *y = step_ensure_on_ground(*y, physics, world, *x, *z);
        *y = step_player_gravity(*x, *z, *y, physics, world, DT);
    }

    fn snap_spawn(y: &mut f32, physics: &mut PlayerPhysics, world: &VoxelWorld, x: f32, z: f32) {
        if let Some(ground) = snap_to_ground_y(x, z, world) {
            *y = ground;
            physics.grounded = true;
            physics.vertical_velocity = 0.0;
        }
    }

    #[test]
    fn ensure_on_ground_does_not_cancel_jump() {
        let world = server_floor_world();
        let mut physics = PlayerPhysics::default();
        physics.grounded = false;
        physics.vertical_velocity = JUMP_SPEED;

        let y = step_ensure_on_ground(1.0, &mut physics, &world, SPAWN.x, SPAWN.z);

        assert_eq!(y, 1.0);
        assert!(!physics.grounded);
        assert_eq!(physics.vertical_velocity, JUMP_SPEED);
    }

    #[test]
    fn player_jumps_and_lands() {
        let world = server_floor_world();
        let x = SPAWN.x;
        let mut y = SPAWN.y;
        let z = SPAWN.z;
        let mut physics = PlayerPhysics::default();
        snap_spawn(&mut y, &mut physics, &world, x, z);

        physics.grounded = false;
        physics.vertical_velocity = JUMP_SPEED;

        let mut max_y = y;
        let mut peaked = false;
        for _ in 0..120 {
            y = step_player_gravity(x, z, y, &mut physics, &world, DT);
            max_y = max_y.max(y);
            if physics.vertical_velocity <= 0.0 && max_y > SPAWN.y + 0.05 {
                peaked = true;
            }
        }

        assert!(peaked, "jump should reach apex and start falling");
        assert!(max_y > SPAWN.y + 0.05, "jump should leave the ground (max_y={max_y})");
        assert!(physics.grounded, "should land back on floor");
        assert!((y - SPAWN.y).abs() <= GROUND_TOLERANCE);
    }

    #[test]
    fn walking_into_stack_does_not_raise_height() {
        let world = VoxelWorld::from_block_coords([
            BlockCoord::new(10, 0, 10),
            BlockCoord::new(11, 0, 10),
            BlockCoord::new(12, 0, 10),
            BlockCoord::new(12, 1, 10),
            BlockCoord::new(12, 2, 10),
        ]);

        let mut x = 10.5;
        let mut y = 1.0;
        let z = 10.5;
        let mut physics = PlayerPhysics::default();
        physics.grounded = true;

        for _ in 0..60 {
            let delta = Vec3::new(MOVE_SPEED * DT, 0.0, 0.0);
            let pos = world.resolve_horizontal_move(
                Vec3::new(x, y, z),
                delta,
                PLAYER_RADIUS,
                PLAYER_HEIGHT,
            );
            x = pos.x;
            y = step_player_gravity(x, z, y, &mut physics, &world, DT);
        }

        assert!(x < 12.0, "blocked by stack, x={x}");
        assert!((y - 1.0).abs() <= GROUND_TOLERANCE, "should stay on floor, y={y}");
    }

    #[test]
    fn spawn_has_floor_support() {
        let world = server_floor_world();
        assert_eq!(
            world.ground_y_at(SPAWN.x, SPAWN.y, SPAWN.z),
            Some(1.0),
            "server spawn must sit on y=0 floor top"
        );
    }

    #[test]
    fn player_stays_on_floor_after_spawn_snap() {
        let world = server_floor_world();
        let mut x = SPAWN.x;
        let mut y = SPAWN.y;
        let mut z = SPAWN.z;
        let mut physics = PlayerPhysics::default();

        snap_spawn(&mut y, &mut physics, &world, x, z);

        for frame in 0..300 {
            simulate_frame(&mut x, &mut y, &mut z, &mut physics, &world, 0.0, 0.0);
            assert!(
                y >= 1.0 - GROUND_TOLERANCE,
                "fell below floor on frame {frame}: y={y}"
            );
        }
        assert!(physics.grounded, "expected grounded after settling");
    }

    #[test]
    fn player_does_not_fall_through_while_moving_with_wasd() {
        let world = server_floor_world();
        let mut x = SPAWN.x;
        let mut y = SPAWN.y;
        let mut z = SPAWN.z;
        let mut physics = PlayerPhysics::default();
        snap_spawn(&mut y, &mut physics, &world, x, z);

        let speed = MOVE_SPEED * DT;
        for frame in 0..600 {
            // Ping-pong on the floor so we stay over collidable voxels.
            let dir = if (frame / 120) % 2 == 0 { 1.0 } else { -1.0 };
            simulate_frame(&mut x, &mut y, &mut z, &mut physics, &world, speed * dir, 0.0);
            assert!(
                y >= 1.0 - GROUND_TOLERANCE,
                "fell through while moving on frame {frame}: pos=({x},{y},{z})"
            );
            assert!(
                world.ground_y_at(x, y, z).is_some(),
                "lost floor support on frame {frame}: pos=({x},{y},{z})"
            );
        }
    }

    #[test]
    fn player_lands_when_dropped_from_height() {
        let world = server_floor_world();
        let mut x = SPAWN.x;
        let mut y = 20.0;
        let mut z = SPAWN.z;
        let mut physics = PlayerPhysics::default();

        for _frame in 0..600 {
            simulate_frame(&mut x, &mut y, &mut z, &mut physics, &world, 0.0, 0.0);
            if physics.grounded {
                assert!((y - 1.0).abs() <= GROUND_TOLERANCE, "landed at y={y}");
                return;
            }
        }
        panic!("never landed after 600 frames, y={y}");
    }

    #[test]
    fn server_spawn_offsets_have_floor_support() {
        let world = server_floor_world();
        for dx in -3..=3 {
            for dz in -3..=3 {
                let x = SPAWN.x + dx as f32;
                let z = SPAWN.z + dz as f32;
                assert_eq!(
                    world.ground_y_at(x, SPAWN.y, z),
                    Some(1.0),
                    "missing floor at spawn offset ({dx}, {dz})"
                );
            }
        }
    }

    #[test]
    fn large_timestep_does_not_tunnel_through_floor() {
        let world = server_floor_world();
        let mut physics = PlayerPhysics::default();
        let mut y = 5.0;

        for frame in 0..40 {
            y = step_player_gravity(SPAWN.x, SPAWN.z, y, &mut physics, &world, 0.25);
            assert!(
                y >= 1.0 - GROUND_TOLERANCE,
                "tunneled through floor on frame {frame}: y={y}"
            );
        }
        assert!(physics.grounded);
    }

    fn world_with_spawn_hole(size: i32) -> VoxelWorld {
        let blocks = (0..size).flat_map(|x| {
            (0..size).filter_map(move |z| {
                if (23..=25).contains(&x) && (23..=25).contains(&z) {
                    None
                } else {
                    Some(BlockCoord::new(x, 0, z))
                }
            })
        });
        VoxelWorld::from_block_coords(blocks)
    }

    #[test]
    fn hole_at_spawn_has_no_grid_ground() {
        let world = world_with_spawn_hole(48);
        assert!(world.block_count() >= MIN_WORLD_BLOCKS);
        assert!(!world.has_support_at(SPAWN.x, SPAWN.z));
        assert_eq!(world.ground_y_at(SPAWN.x, SPAWN.y, SPAWN.z), None);
    }

    #[test]
    fn hole_at_spawn_still_supported_when_world_loaded() {
        let world = world_with_spawn_hole(48);
        let mut x = SPAWN.x;
        let mut y = SPAWN.y;
        let mut z = SPAWN.z;
        let mut physics = PlayerPhysics::default();
        snap_spawn(&mut y, &mut physics, &world, x, z);

        for frame in 0..300 {
            simulate_frame(&mut x, &mut y, &mut z, &mut physics, &world, 0.0, 0.0);
            assert!(
                y >= FLOOR_TOP_Y - GROUND_TOLERANCE,
                "fell through missing spawn pad on frame {frame}: y={y}"
            );
        }
    }

    #[test]
    fn spawn_without_blocks_falls_immediately() {
        let world = VoxelWorld::from_block_coords([]);
        let mut physics = PlayerPhysics::default();
        let mut y = SPAWN.y;

        snap_spawn(&mut y, &mut physics, &world, SPAWN.x, SPAWN.z);
        assert!(
            !physics.grounded,
            "snap should fail when collision grid is empty"
        );

        y = step_player_gravity(SPAWN.x, SPAWN.z, y, &mut physics, &world, DT);
        assert!(y < SPAWN.y, "expected to fall when no blocks are loaded");
    }

    fn keys_with_pressed(key: KeyCode) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(key);
        keys
    }

    #[test]
    fn wasd_keys_move_player_on_open_floor() {
        let world = server_floor_world();
        let start = SPAWN;
        let yaw = 0.0;

        let cases = [
            (KeyCode::KeyW, Vec3::new(0.0, 0.0, -1.0)),
            (KeyCode::KeyS, Vec3::new(0.0, 0.0, 1.0)),
            (KeyCode::KeyA, Vec3::new(-1.0, 0.0, 0.0)),
            (KeyCode::KeyD, Vec3::new(1.0, 0.0, 0.0)),
        ];

        for (key, expected_dir) in cases {
            let keys = keys_with_pressed(key);
            let moved = apply_horizontal_move(start, &keys, yaw, DT, &world);
            assert_ne!(
                moved, start,
                "{key:?} should move the player (still at {moved:?})"
            );
            let delta = (moved - start).normalize();
            assert!(
                delta.dot(expected_dir) > 0.99,
                "{key:?}: expected dir {expected_dir:?}, got delta {:?}",
                moved - start
            );
        }
    }

    #[test]
    fn arrow_keys_match_wasd_movement() {
        let world = server_floor_world();
        let start = SPAWN;
        let yaw = 0.0;

        let pairs = [
            (KeyCode::ArrowUp, KeyCode::KeyW),
            (KeyCode::ArrowDown, KeyCode::KeyS),
            (KeyCode::ArrowLeft, KeyCode::KeyA),
            (KeyCode::ArrowRight, KeyCode::KeyD),
        ];

        for (arrow, wasd) in pairs {
            let arrow_pos =
                apply_horizontal_move(start, &keys_with_pressed(arrow), yaw, DT, &world);
            let wasd_pos = apply_horizontal_move(start, &keys_with_pressed(wasd), yaw, DT, &world);
            assert_eq!(
                arrow_pos, wasd_pos,
                "{arrow:?} should match {wasd:?}"
            );
        }
    }

    #[test]
    fn wasd_movement_accumulates_over_multiple_frames() {
        let world = server_floor_world();
        let mut pos = SPAWN;
        let yaw = 0.0;

        for _ in 0..30 {
            pos = apply_horizontal_move(pos, &keys_with_pressed(KeyCode::KeyD), yaw, DT, &world);
        }

        assert!(
            pos.x > SPAWN.x + 0.5,
            "holding D should travel right over 30 frames, pos={pos:?}"
        );
        assert!(
            (pos.y - SPAWN.y).abs() <= GROUND_TOLERANCE,
            "horizontal move should not change height, y={}",
            pos.y
        );
    }

    #[test]
    fn no_keys_pressed_does_not_move() {
        let world = server_floor_world();
        let keys = ButtonInput::<KeyCode>::default();
        let moved = apply_horizontal_move(SPAWN, &keys, 0.0, DT, &world);
        assert_eq!(moved, SPAWN);
    }
}
