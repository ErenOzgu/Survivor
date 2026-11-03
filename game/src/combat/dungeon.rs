//! The dungeon mode (Deadmines): no waves. Packs of four to six wait ahead of the hero, on
//! floor a ray finds and in its line of sight, standing still until the hero comes near or hurts
//! one; then the whole pack comes. Each pack cleared moves the boss clock on (`PACK_SECONDS`),
//! so the bosses arrive as the dungeon is cleared, the last one at the end.

use bevy::prelude::*;

use benilla_world::collision::WorldCollision;

use super::enemies::Enemy;
use super::gear_ui::Toasts;
use super::rules;
use super::visuals::{attach_enemy_visual, Placeholders};
use super::{CombatRng, CombatSet, HeroDied, RestartRun, Run};
use crate::content::waves;
use crate::content::zones::{self, PACK_SECONDS};
use crate::content::EnemyKind;
use crate::game::{floor_under, Hero};

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Packs>().add_systems(
            Update,
            (place_packs, wake_packs, count_packs)
                .chain()
                .in_set(CombatSet::Spawn)
                .run_if(in_dungeon)
                .run_if(super::hero_on_ground),
        );
    }
}

pub fn in_dungeon() -> bool {
    zones::current().def().dungeon
}

/// The boss clock: the run's time, or in a dungeon the packs cleared.
pub fn boss_clock(run: &Run) -> f32 {
    if in_dungeon() {
        run.packs_cleared as f32 * PACK_SECONDS
    } else {
        run.time
    }
}

/// The packs about, and when to look for a spot for the next.
#[derive(Resource, Default)]
struct Packs {
    live: Vec<Pack>,
    next_try: f32,
}

struct Pack {
    members: Vec<Entity>,
    awake: bool,
}

/// A pack member's speed while it waits.
#[derive(Component)]
struct Dormant(f32);

/// Packs waiting at once, and how near the hero wakes one (yards).
const LIVE_PACKS: usize = 2;
const WAKE_RANGE: f32 = 15.0;

/// A spot for a pack: ahead (any way) at a pull's distance, on floor near the hero's height, with
/// nothing solid close in the way.
pub fn pack_spot(world: &WorldCollision, hero: Vec3, rng: &mut rules::Rng) -> Option<Vec3> {
    let start = rng.range(0.0, std::f32::consts::TAU);
    for i in 0..16 {
        let a = start + i as f32 * std::f32::consts::TAU / 16.0;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        for d in [26.0, 20.0, 15.0] {
            let eye = hero + Vec3::Y * 1.5;
            if let Some(hit) = world.ray_body(eye, Dir3::new(dir).ok()?, d) {
                if hit.distance < d * 0.8 {
                    continue;
                }
            }
            let p = hero + dir * d;
            if let Some(y) = floor_under(world, p.with_y(hero.y)) {
                if (y - hero.y).abs() < 5.0 {
                    return Some(p.with_y(y));
                }
            }
        }
    }
    None
}

/// The kinds a pack is made of.
const PACK_KINDS: [EnemyKind; 6] = [
    EnemyKind::Kobold,
    EnemyKind::Kobold,
    EnemyKind::Geomancer,
    EnemyKind::Sapper,
    EnemyKind::Wolf,
    EnemyKind::Spider,
];

#[allow(clippy::too_many_arguments)]
fn place_packs(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    mut packs: ResMut<Packs>,
    mut rng: ResMut<CombatRng>,
    assets: Res<Placeholders>,
    world: WorldCollision,
    heroes: Query<&Transform, With<Hero>>,
    (mut died, mut restarted): (MessageReader<HeroDied>, MessageReader<RestartRun>),
) {
    // A new run: the old packs went with the old run.
    if died.read().count() > 0 || restarted.read().count() > 0 {
        packs.live.clear();
    }
    packs.next_try -= time.delta_secs();
    if packs.live.len() >= LIVE_PACKS || packs.next_try > 0.0 {
        return;
    }
    packs.next_try = 1.5;
    let Ok(hero) = heroes.single() else {
        return;
    };
    let Some(spot) = pack_spot(&world, hero.translation, &mut rng.0) else {
        return;
    };
    let wave = waves::wave_at(run.time);
    let mul = zones::current().def().level_mul;
    let n = 4 + rng.0.below(3);
    let mut members = Vec::new();
    for i in 0..n {
        let kind = PACK_KINDS[rng.0.below(PACK_KINDS.len())];
        let jitter = if i == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(rng.0.range(-2.0, 2.0), 0.0, rng.0.range(-2.0, 2.0))
        };
        let mut enemy = Enemy::new(kind, wave.health_mul * mul, wave.damage_mul * mul);
        let speed = enemy.speed;
        enemy.speed = 0.0;
        let e = commands
            .spawn((
                Name::new(kind.name()),
                enemy,
                Dormant(speed),
                Transform::from_translation(spot + jitter),
                Visibility::default(),
            ))
            .id();
        attach_enemy_visual(&mut commands, e, kind, &assets);
        members.push(e);
    }
    packs.live.push(Pack {
        members,
        awake: false,
    });
}

/// A pack wakes whole when the hero comes near any of it or hurts one.
fn wake_packs(
    mut commands: Commands,
    mut packs: ResMut<Packs>,
    heroes: Query<&Transform, With<Hero>>,
    mut enemies: Query<(&Transform, &mut Enemy, Option<&Dormant>)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    for pack in packs.live.iter_mut().filter(|p| !p.awake) {
        let wake = pack.members.iter().any(|m| {
            enemies.get(*m).is_ok_and(|(xf, e, _)| {
                e.health < e.max_health
                    || rules::flat_dist(xf.translation, hero.translation) < WAKE_RANGE
            })
        });
        if !wake {
            continue;
        }
        pack.awake = true;
        for m in &pack.members {
            if let Ok((_, mut e, Some(d))) = enemies.get_mut(*m) {
                e.speed = d.0;
                commands.entity(*m).remove::<Dormant>();
            }
        }
    }
}

/// A pack whose every member is gone is cleared: the boss clock moves on.
fn count_packs(
    mut packs: ResMut<Packs>,
    mut run: ResMut<Run>,
    mut toasts: ResMut<Toasts>,
    enemies: Query<&Enemy>,
) {
    let before = packs.live.len();
    packs.live.retain(|p| {
        p.members
            .iter()
            .any(|m| enemies.get(*m).is_ok_and(|e| e.alive()))
    });
    let cleared = (before - packs.live.len()) as u32;
    if cleared > 0 {
        run.packs_cleared += cleared;
        toasts.push(
            format!("Paket temizlendi ({})", run.packs_cleared),
            Color::srgb(1.0, 0.85, 0.4),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_params() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = World::new();
        IntoSystem::into_system(place_packs).initialize(&mut world);
        IntoSystem::into_system(wake_packs).initialize(&mut world);
        IntoSystem::into_system(count_packs).initialize(&mut world);
    }
}
