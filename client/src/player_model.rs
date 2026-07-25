//! Low-poly blocky avatars for local and remote players.

use bevy::prelude::*;
use sha2::{Digest, Sha256};

use crate::models::{block_color, RaycastIgnore, BLOCK_PALETTE};

const SKIN: Color = Color::srgb(1.0, 0.835, 0.667);

#[derive(Clone, Copy)]
pub(crate) struct AvatarColors {
    skin: Color,
    body: Color,
    limbs: Color,
}

const LOCAL_AVATAR: AvatarColors = AvatarColors {
    skin: SKIN,
    body: Color::srgb(0.0, 0.5, 1.0),
    limbs: Color::srgb(0.0, 0.0, 1.0),
};

struct BodyPart {
    position: Vec3,
    scale: Vec3,
    slot: &'static str,
}

const BODY_PARTS: [BodyPart; 6] = [
    BodyPart {
        position: Vec3::new(0.0, 1.55, 0.0),
        scale: Vec3::new(0.45, 0.45, 0.45),
        slot: "skin",
    },
    BodyPart {
        position: Vec3::new(0.0, 1.0, 0.0),
        scale: Vec3::new(0.5, 0.65, 0.3),
        slot: "body",
    },
    BodyPart {
        position: Vec3::new(-0.15, 0.35, 0.0),
        scale: Vec3::new(0.22, 0.7, 0.22),
        slot: "limbs",
    },
    BodyPart {
        position: Vec3::new(0.15, 0.35, 0.0),
        scale: Vec3::new(0.22, 0.7, 0.22),
        slot: "limbs",
    },
    BodyPart {
        position: Vec3::new(-0.38, 1.0, 0.0),
        scale: Vec3::new(0.2, 0.6, 0.2),
        slot: "body",
    },
    BodyPart {
        position: Vec3::new(0.38, 1.0, 0.0),
        scale: Vec3::new(0.2, 0.6, 0.2),
        slot: "body",
    },
];

fn colors_for_player(player_id: &str) -> AvatarColors {
    let digest = Sha256::digest(player_id.as_bytes());
    AvatarColors {
        skin: SKIN,
        body: block_color((digest[0] as i32 % BLOCK_PALETTE.len() as i32) + 1),
        limbs: block_color((digest[1] as i32 % BLOCK_PALETTE.len() as i32) + 1),
    }
}

fn tint_for_slot(colors: AvatarColors, slot: &str) -> Color {
    match slot {
        "skin" => colors.skin,
        "limbs" => colors.limbs,
        _ => colors.body,
    }
}

pub fn spawn_blocky_avatar(
    commands: &mut Commands,
    cube_mesh: Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    colors: AvatarColors,
) -> Entity {
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    commands.entity(parent).add_child(root);

    for part in BODY_PARTS {
        let material = materials.add(StandardMaterial {
            base_color: tint_for_slot(colors, part.slot),
            perceptual_roughness: 0.9,
            ..default()
        });
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                RaycastIgnore,
                Mesh3d(cube_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(part.position).with_scale(part.scale),
                Visibility::default(),
            ));
        });
    }

    root
}

pub fn spawn_local_avatar(
    commands: &mut Commands,
    cube_mesh: Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    player: Entity,
) {
    spawn_blocky_avatar(commands, cube_mesh, materials, player, LOCAL_AVATAR);
}

pub fn spawn_remote_avatar(
    commands: &mut Commands,
    cube_mesh: Handle<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    player_id: &str,
    position: Vec3,
) -> Entity {
    let colors = colors_for_player(player_id);
    let root = commands
        .spawn((
            Transform::from_translation(position),
            Visibility::default(),
        ))
        .id();
    spawn_blocky_avatar(commands, cube_mesh, materials, root, colors);
    root
}
