//! Enemy looks and placeholder visuals. Enemies get a real WoW creature model (`crate::models`)
//! picked by model folder from the player's client tables, with a simple unlit placeholder body
//! shown until it is ready (and kept when there is no data). Projectiles, ground areas, pulses and
//! orbs are still placeholder meshes.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::content::{EnemyKind, SkillId};
use crate::models::{self, AutoLocomotion, ModelAnim, ModelData, ModelPlaceholder, ModelRequest};

const ALL_KINDS: [EnemyKind; 5] = [
    EnemyKind::Kobold,
    EnemyKind::Wolf,
    EnemyKind::Spider,
    EnemyKind::GnollChief,
    EnemyKind::DefiasBoss,
];

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, build_placeholders).add_systems(
            Update,
            (
                resolve_enemy_displays.in_set(super::CombatSet::Spawn),
                animate_flashes.in_set(super::CombatSet::Act),
            ),
        );
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
    /// The creature display each enemy kind is drawn with, once the client tables are read.
    /// A kind missing here keeps its placeholder body.
    pub displays: HashMap<EnemyKind, u32>,
    /// Whether [`Self::displays`] has been filled (or there is no data to fill it from).
    pub displays_resolved: bool,
}

/// The model folder an enemy kind's creature display is picked from. The Defias leader is a
/// character-model NPC with no folder of its own; it keeps its placeholder for now.
pub fn model_folder(kind: EnemyKind) -> Option<&'static str> {
    match kind {
        EnemyKind::Kobold => Some("Creature\\Kobold"),
        EnemyKind::Wolf => Some("Creature\\Wolf"),
        EnemyKind::Spider => Some("Creature\\Spider"),
        EnemyKind::GnollChief => Some("Creature\\Gnoll"),
        EnemyKind::DefiasBoss => None,
    }
}

/// Once the client tables are in, picks a creature display for every enemy kind.
fn resolve_enemy_displays(mut assets: ResMut<Placeholders>, data: Option<Res<ModelData>>) {
    if assets.displays_resolved {
        return;
    }
    let Some(data) = data else { return };
    for kind in ALL_KINDS {
        let Some(folder) = model_folder(kind) else {
            continue;
        };
        match models::creature_display_in_folder(&data, folder) {
            Some(id) => {
                info!("rnsurvivors: {:?} drawn with creature display {id}", kind);
                assets.displays.insert(kind, id);
            }
            None if data.tables.is_some() => {
                warn!(
                    "rnsurvivors: no creature display under {folder}, {kind:?} stays a placeholder"
                );
            }
            None => {}
        }
    }
    assets.displays_resolved = true;
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
    let mut enemy = HashMap::new();
    for kind in ALL_KINDS {
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
    for s in SkillId::every() {
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
        displays: HashMap::new(),
        displays_resolved: false,
    });
}

/// Gives a freshly spawned enemy entity its look. The entity's `Transform` is the enemy's feet
/// (the model faces its −Z). It asks for the kind's creature model, which runs, attacks and dies
/// with the enemy (see `enemies::animate_enemies`); the placeholder body shows until the model
/// is ready and stays if there is none.
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
            ModelPlaceholder,
        ));
        p.spawn((
            Mesh3d(assets.eye.0.clone()),
            MeshMaterial3d(assets.eye.1.clone()),
            Transform::from_translation(eye_at),
            ModelPlaceholder,
        ));
    });
    if let Some(&display_id) = assets.displays.get(&kind) {
        commands.entity(enemy).insert((
            ModelRequest::Creature { display_id },
            ModelAnim::default(),
            AutoLocomotion::default(),
        ));
    }
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

#[cfg(test)]
mod system_params {
    use super::*;
    use bevy::ecs::system::IntoSystem;

    fn init<M>(world: &mut World, s: impl IntoSystem<(), (), M>) {
        let mut s = IntoSystem::into_system(s);
        s.initialize(world);
    }

    /// Each system's parameters are compatible (no conflicting queries); Bevy checks it here.
    #[test]
    fn systems_initialize() {
        let mut world = World::new();
        init(&mut world, animate_flashes);
        init(&mut world, resolve_enemy_displays);
        init(&mut world, build_placeholders);
    }
}
