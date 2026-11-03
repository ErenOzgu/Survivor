//! The hero's health: contact damage from enemies, regeneration, death and the run reset.

use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;

use super::enemies::{Corpse, Enemy, EnemyBolt};
use super::portal::MapCleared;
use super::rules::{self, HERO_MAX_HEALTH, HERO_REGEN};
use super::skills::{Projectile, SkillTimers, Zone};
use super::visuals::Flash;
use super::xp::Orb;
use super::{hero_on_ground, CombatSet, DebugStart, HeroDied, RestartRun, Run};
use crate::game::{Hero, HeroSpeed, Start};

pub struct HeroPlugin;

impl Plugin for HeroPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(HeroHealth::full())
            .init_resource::<DeathNotice>()
            .add_systems(
                Update,
                (apply_stats, contact_damage)
                    .chain()
                    .in_set(CombatSet::Act)
                    .run_if(hero_on_ground),
            )
            .add_systems(
                Update,
                (check_death, reset_run).chain().in_set(CombatSet::Reset),
            );
    }
}

/// The hero's health, and a short invulnerability (after Blink).
#[derive(Resource, Debug, Clone, Copy)]
pub struct HeroHealth {
    pub current: f32,
    pub max: f32,
    pub invulnerable: f32,
}

impl HeroHealth {
    pub fn full() -> Self {
        Self {
            current: HERO_MAX_HEALTH,
            max: HERO_MAX_HEALTH,
            invulnerable: 0.0,
        }
    }

    pub fn fraction(&self) -> f32 {
        (self.current / self.max).clamp(0.0, 1.0)
    }

    /// A hit on the hero from an attack (not contact): nothing while invulnerable or in god
    /// mode.
    pub fn take(&mut self, damage: f32, god: bool) {
        if god || self.invulnerable > 0.0 {
            return;
        }
        self.current = rules::apply_damage(self.current, damage).0;
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }
}

/// The "you died" banner: the last run's summary and how long it stays up (real seconds).
#[derive(Resource, Default)]
pub struct DeathNotice {
    pub text: String,
    pub left: f32,
}

/// The passives' hero stats: more maximum health (the gain also heals) and a faster run.
fn apply_stats(
    run: Res<Run>,
    mut health: ResMut<HeroHealth>,
    mut speed: ResMut<HeroSpeed>,
    mut base_speed: Local<Option<f32>>,
) {
    let stats = rules::Stats::from_passives(&run.passives);
    if health.max != stats.max_health {
        let gain = stats.max_health - health.max;
        health.max = stats.max_health;
        health.current = (health.current + gain.max(0.0)).min(health.max);
    }
    let base = *base_speed.get_or_insert(speed.0);
    let want = base * stats.speed_mul;
    if speed.0 != want {
        speed.0 = want;
    }
}

/// Touching enemies hurt every second they touch; the hero regenerates slowly.
fn contact_damage(
    time: Res<Time>,
    debug: Res<DebugStart>,
    mut health: ResMut<HeroHealth>,
    heroes: Query<&Transform, With<Hero>>,
    enemies: Query<(&Transform, &Enemy)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    health.invulnerable = (health.invulnerable - dt).max(0.0);
    health.heal(HERO_REGEN * dt);
    if debug.god || health.invulnerable > 0.0 {
        return;
    }
    let dps: f32 = enemies
        .iter()
        .filter(|(xf, e)| {
            e.frozen <= 0.0
                && rules::touching(rules::flat_dist(xf.translation, hero.translation), e.radius)
        })
        .map(|(_, e)| e.dps)
        .sum();
    let (h, _) = rules::apply_damage(health.current, dps * dt);
    health.current = h;
}

pub(super) fn check_death(health: Res<HeroHealth>, mut died: MessageWriter<HeroDied>) {
    if health.current <= 0.0 {
        died.write(HeroDied);
    }
}

/// Dying returns the hero to the map start with a fresh run: skills, level and XP reset, every
/// enemy, projectile, area and orb cleared. Leaving through the portal (`MapCleared`) does the
/// same without the death banner (the portal shows its own summary).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn reset_run(
    mut commands: Commands,
    mut died: MessageReader<HeroDied>,
    mut cleared: MessageReader<MapCleared>,
    mut restarted: MessageReader<RestartRun>,
    start: Res<Start>,
    debug: Res<DebugStart>,
    mut run: ResMut<Run>,
    mut health: ResMut<HeroHealth>,
    mut timers: ResMut<SkillTimers>,
    mut notice: ResMut<DeathNotice>,
    mut heroes: Query<(&mut Transform, &mut Hero)>,
    clutter: Query<
        Entity,
        Or<(
            With<Enemy>,
            With<Projectile>,
            With<Zone>,
            With<Orb>,
            With<Flash>,
            With<Corpse>,
            With<EnemyBolt>,
        )>,
    >,
) {
    let died = died.read().count() > 0;
    let cleared = cleared.read().count() > 0 || restarted.read().count() > 0;
    if !died && !cleared {
        return;
    }
    if died {
        let mins = (run.time / 60.0) as u32;
        let secs = (run.time % 60.0) as u32;
        notice.text = format!(
            "Öldün! {mins}:{secs:02} dayandın, level {}, {} düşman. Haritanın başına dönüldü.",
            run.level, run.kills
        );
        notice.left = 5.0;
        info!("rnsurvivors: {}", notice.text);
    }

    for e in &clutter {
        commands.entity(e).try_despawn();
    }
    *run = Run::new(&debug);
    *health = HeroHealth::full();
    timers.0.clear();
    if let Ok((mut xf, mut hero)) = heroes.single_mut() {
        xf.translation = wow_to_bevy(start.0.wow());
        // Re-snaps to the ground once the start's terrain is resident again.
        hero.grounded = false;
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
        init(&mut world, contact_damage);
        init(&mut world, apply_stats);
        init(&mut world, check_death);
        init(&mut world, reset_run);
    }
}
