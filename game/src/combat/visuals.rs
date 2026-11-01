//! Placeholder visuals: simple unlit meshes and colours for enemies, projectiles, ground areas,
//! pulses and orbs. Enemy bodies are made in one place, [`attach_enemy_visual`], so it can later
//! switch to the real creature models (`crate::models`, being built separately).

use std::collections::HashMap;

use bevy::prelude::*;

use crate::content::{EnemyKind, SkillId};

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, build_placeholders)
            .add_systems(Update, animate_flashes.in_set(super::CombatSet::Act));
    }
}

/// Shared meshes and materials for every placeholder.
#[derive(Resource)]
pub struct Placeholders {
    pub enemy: HashMap<EnemyKind, (Handle<Mesh>, Handle<StandardMaterial>)>,
    pub eye: (Handle<Mesh>, Handle<StandardMaterial>),
    /// Per skill: its colour as an opaque material (projectiles) and a see-through one (areas).
    pub solid: HashMap<SkillId, Handle<StandardMaterial>>,
    pub glass: HashMap<SkillId, Handle<StandardMaterial>>,
    pub ball: Handle<Mesh>,
    pub bolt: Handle<Mesh>,
    /// A flat disc of radius 1; scale it to an area's radius.
    pub disc: Handle<Mesh>,
    pub orb: (Handle<Mesh>, Handle<StandardMaterial>),
}

fn unlit(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn rgb(c: [f32; 3]) -> Color {
    Color::srgb(c[0], c[1], c[2])
}

fn build_placeholders(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let all_kinds = [
        EnemyKind::Kobold,
        EnemyKind::Wolf,
        EnemyKind::Spider,
        EnemyKind::GnollChief,
        EnemyKind::DefiasBoss,
    ];
    let mut enemy = HashMap::new();
    for kind in all_kinds {
        let d = kind.def();
        // Body shapes (feet at the origin is handled by the child transform below).
        let mesh = match kind {
            // Small upright figure.
            EnemyKind::Kobold | EnemyKind::GnollChief | EnemyKind::DefiasBoss => {
                meshes.add(Capsule3d::new(0.4, 0.8))
            }
            // Long, low body.
            EnemyKind::Wolf => meshes.add(Cuboid::new(0.6, 0.6, 1.4)),
            // Wide, flat body.
            EnemyKind::Spider => meshes.add(Sphere::new(0.6)),
        };
        enemy.insert(kind, (mesh, materials.add(unlit(rgb(d.color)))));
    }
    let mut solid = HashMap::new();
    let mut glass = HashMap::new();
    for s in SkillId::ALL {
        let c = s.def().color;
        solid.insert(s, materials.add(unlit(rgb(c))));
        glass.insert(
            s,
            materials.add(StandardMaterial {
                base_color: Color::srgba(c[0], c[1], c[2], 0.35),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            }),
        );
    }
    commands.insert_resource(Placeholders {
        enemy,
        eye: (
            meshes.add(Cuboid::new(0.2, 0.12, 0.2)),
            materials.add(unlit(Color::srgb(1.0, 0.9, 0.2))),
        ),
        solid,
        glass,
        ball: meshes.add(Sphere::new(0.35)),
        bolt: meshes.add(Cuboid::new(0.18, 0.18, 0.8)),
        disc: meshes.add(Cylinder::new(1.0, 0.08)),
        orb: (
            meshes.add(Sphere::new(0.22)),
            materials.add(unlit(Color::srgb(0.3, 1.0, 0.45))),
        ),
    });
}

/// Gives a freshly spawned enemy entity its look. The entity's `Transform` is the enemy's feet;
/// the body is a child. This is the one place to switch to the creature models API.
pub fn attach_enemy_visual(
    commands: &mut Commands,
    enemy: Entity,
    kind: EnemyKind,
    assets: &Placeholders,
) {
    let d = kind.def();
    let (mesh, mat) = assets.enemy[&kind].clone();
    let (body_y, body_scale) = match kind {
        EnemyKind::Wolf => (0.45, Vec3::splat(d.scale)),
        EnemyKind::Spider => (0.35, Vec3::new(1.3, 0.55, 1.3) * d.scale),
        _ => (0.8 * d.scale, Vec3::splat(d.scale)),
    };
    let eye_at = match kind {
        EnemyKind::Wolf => Vec3::new(0.0, 0.7, -0.75) * d.scale,
        EnemyKind::Spider => Vec3::new(0.0, 0.5, -0.65) * d.scale,
        _ => Vec3::new(0.0, 1.35, -0.35) * d.scale,
    };
    commands.entity(enemy).with_children(|p| {
        p.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_xyz(0.0, body_y, 0.0).with_scale(body_scale),
        ));
        p.spawn((
            Mesh3d(assets.eye.0.clone()),
            MeshMaterial3d(assets.eye.1.clone()),
            Transform::from_translation(eye_at),
        ));
    });
}

/// A short-lived expanding disc: the visual of a nova, an explosion or a blast.
#[derive(Component)]
pub struct Flash {
    pub age: f32,
    pub life: f32,
    pub radius: f32,
}

pub fn spawn_flash(
    commands: &mut Commands,
    assets: &Placeholders,
    skill: SkillId,
    at: Vec3,
    radius: f32,
) {
    commands.spawn((
        Name::new("flash"),
        Flash {
            age: 0.0,
            life: 0.35,
            radius,
        },
        Mesh3d(assets.disc.clone()),
        MeshMaterial3d(assets.glass[&skill].clone()),
        Transform::from_translation(at + Vec3::Y * 0.3).with_scale(Vec3::new(0.1, 1.0, 0.1)),
    ));
}

fn animate_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut Flash, &mut Transform)>,
) {
    for (e, mut f, mut xf) in &mut flashes {
        f.age += time.delta_secs();
        if f.age >= f.life {
            commands.entity(e).try_despawn();
            continue;
        }
        let r = f.radius * (0.3 + 0.7 * f.age / f.life);
        xf.scale = Vec3::new(r, 1.0, r);
    }
}
