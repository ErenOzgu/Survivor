//! Spell effects from the client (`Spells\*.mdx` with their particles): missiles on projectiles,
//! models on ground areas and XP orbs, bursts for hits, explosions and novas, ice on frozen
//! enemies. Each skill's models come from its spell's visual (`art::SkillFx`); without one the
//! placeholder shapes stay. Caps keep a crowded screen from spawning hundreds of emitters.

use bevy::prelude::*;

use super::art::WowArt;
use super::enemies::Enemy;
use super::skills::{Projectile, Zone};
use super::visuals::{spawn_flash, Placeholders};
use super::xp::Orb;
use super::CombatSet;
use crate::content::SkillId;
use crate::models::ModelRequest;

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxBudget>().add_systems(
            Update,
            (
                dress_projectiles,
                dress_zones,
                dress_orbs,
                frozen_ice,
                tick_bursts,
            )
                .in_set(CombatSet::Ui),
        );
    }
}

/// How many effect instances may live at once, by kind.
const MAX_BURSTS: usize = 24;
const MAX_ORB_MODELS: usize = 90;
const MAX_ICE: usize = 30;
/// Seconds a one-shot burst lives (its particles drain after).
const BURST_LIFE: f32 = 1.4;

/// The size an area model is authored at (yards), to scale it to a skill's radius.
pub fn native_radius(id: SkillId) -> f32 {
    match id {
        SkillId::FrostNova | SkillId::ArcaneExplosion => 10.0,
        SkillId::Blizzard | SkillId::Flamestrike => 8.0,
        _ => 5.0,
    }
}

/// The scale for an area model of `id` covering `radius` yards.
pub fn area_scale(id: SkillId, radius: f32) -> f32 {
    (radius / native_radius(id)).clamp(0.3, 3.0)
}

/// Bursts spawned this frame and alive, to keep within the caps.
#[derive(Resource, Default)]
pub struct FxBudget {
    bursts: usize,
}

/// A one-shot effect, gone after its life.
#[derive(Component)]
pub struct Burst {
    left: f32,
}

/// What kind of burst.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BurstKind {
    /// Where a hit lands.
    Impact,
    /// On the ground: a nova, an explosion, Blink's flash.
    Area,
}

/// A burst for `skill` at `at` (feet or hit point): the client's model if there is one (and room
/// under the cap), else the placeholder flash for areas.
#[allow(clippy::too_many_arguments)]
pub fn burst(
    commands: &mut Commands,
    budget: &mut FxBudget,
    art: &WowArt,
    assets: &Placeholders,
    skill: SkillId,
    kind: BurstKind,
    at: Vec3,
    radius: f32,
) {
    let fx = art.fx.get(&skill);
    let path = match kind {
        BurstKind::Impact => fx.and_then(|f| f.impact.clone()),
        BurstKind::Area => fx.and_then(|f| f.area.clone().or_else(|| f.impact.clone())),
    };
    match path {
        Some(path) if budget.bursts < MAX_BURSTS => {
            budget.bursts += 1;
            let scale = match kind {
                BurstKind::Impact => 1.0,
                BurstKind::Area => area_scale(skill, radius),
            };
            commands.spawn((
                Name::new("spell burst"),
                Burst { left: BURST_LIFE },
                Transform::from_translation(at),
                Visibility::default(),
                ModelRequest::Effect { path, scale },
            ));
        }
        _ if kind == BurstKind::Area => spawn_flash(commands, assets, skill, at, radius),
        _ => {}
    }
}

fn tick_bursts(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut budget: ResMut<FxBudget>,
    mut bursts: Query<(Entity, &mut Burst)>,
) {
    let mut live = 0;
    for (e, mut b) in &mut bursts {
        b.left -= time.delta_secs();
        if b.left <= 0.0 {
            commands.entity(e).try_despawn();
        } else {
            live += 1;
        }
    }
    budget.bursts = live;
}

/// Projectiles wear their skill's missile model.
fn dress_projectiles(
    mut commands: Commands,
    art: Res<WowArt>,
    new: Query<(Entity, &Projectile), Added<Projectile>>,
) {
    for (e, p) in &new {
        if let Some(path) = art.fx.get(&p.skill).and_then(|f| f.missile.clone()) {
            let scale = match p.skill {
                SkillId::WinterBolts => 1.3,
                SkillId::Pyroblast => 1.2,
                _ => 1.0,
            };
            commands
                .entity(e)
                .insert(ModelRequest::Effect { path, scale });
        }
    }
}

/// Ground areas wear their skill's area model, sized to the area.
fn dress_zones(mut commands: Commands, art: Res<WowArt>, new: Query<(Entity, &Zone), Added<Zone>>) {
    for (e, z) in &new {
        if let Some(path) = art.fx.get(&z.skill).and_then(|f| f.area.clone()) {
            commands.entity(e).insert(ModelRequest::Effect {
                path,
                scale: area_scale(z.skill, z.radius),
            });
        }
    }
}

/// XP orbs wear a small green missile model, up to a cap (the rest keep the plain sphere).
fn dress_orbs(
    mut commands: Commands,
    art: Res<WowArt>,
    new: Query<Entity, Added<Orb>>,
    dressed: Query<(), (With<Orb>, With<ModelRequest>)>,
) {
    let Some(path) = art.orb_model.clone() else {
        return;
    };
    let room = MAX_ORB_MODELS.saturating_sub(dressed.iter().count());
    for e in new.iter().take(room) {
        commands.entity(e).insert(ModelRequest::Effect {
            path: path.clone(),
            scale: 0.5,
        });
    }
}

/// The ice on an enemy Frost Nova holds.
#[derive(Component)]
struct IceOn(Entity);

/// Frozen enemies wear Frost Nova's ice while frozen (up to a cap).
fn frozen_ice(
    mut commands: Commands,
    art: Res<WowArt>,
    enemies: Query<(Entity, &Enemy, Option<&IceOn>)>,
) {
    let Some(path) = art
        .fx
        .get(&SkillId::FrostNova)
        .and_then(|f| f.state.clone())
    else {
        return;
    };
    let mut live = enemies.iter().filter(|(_, _, i)| i.is_some()).count();
    for (e, enemy, on) in &enemies {
        match on {
            Some(IceOn(child)) if enemy.frozen <= 0.0 || !enemy.alive() => {
                commands.entity(*child).try_despawn();
                commands.entity(e).remove::<IceOn>();
                live -= 1;
            }
            None if enemy.frozen > 0.0 && enemy.alive() && live < MAX_ICE => {
                let child = commands
                    .spawn((
                        Name::new("ice"),
                        Transform::default(),
                        Visibility::default(),
                        ModelRequest::Effect {
                            path: path.clone(),
                            scale: 0.6,
                        },
                        ChildOf(e),
                    ))
                    .id();
                commands.entity(e).insert(IceOn(child));
                live += 1;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_scale_to_their_radius() {
        assert_eq!(area_scale(SkillId::FrostNova, 10.0), 1.0);
        assert_eq!(area_scale(SkillId::FrostNova, 5.0), 0.5);
        assert_eq!(area_scale(SkillId::Blizzard, 100.0), 3.0, "clamped");
        assert_eq!(area_scale(SkillId::Blizzard, 0.1), 0.3, "clamped");
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(dress_projectiles).initialize(&mut world);
        IntoSystem::into_system(dress_zones).initialize(&mut world);
        IntoSystem::into_system(dress_orbs).initialize(&mut world);
        IntoSystem::into_system(frozen_ice).initialize(&mut world);
        IntoSystem::into_system(tick_bursts).initialize(&mut world);
    }
}
