//! Enemies: the wave spawner around the hero, chasing on the terrain, status effects, their
//! model's animations, deaths and corpses.

use bevy::prelude::*;

use benilla_assets::AdtTile;
use benilla_world::terrain_stream::{terrain_height_under_cached, TerrainStreamer};

use super::rules;
use super::visuals::{attach_enemy_visual, Placeholders};
use super::xp::spawn_orb;
use super::{hero_on_ground, CombatRng, CombatSet, Run};
use crate::content::waves::{self, LEASH, SPAWN_RING_MAX, SPAWN_RING_MIN};
use crate::content::EnemyKind;
use crate::game::Hero;
use crate::models::{AnimState, ModelAnim, ModelStatus};

pub struct EnemiesPlugin;

impl Plugin for EnemiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Spawner>()
            .add_systems(
                Update,
                spawn_waves.in_set(CombatSet::Spawn).run_if(hero_on_ground),
            )
            .add_systems(
                Update,
                (move_enemies, animate_enemies)
                    .chain()
                    .in_set(CombatSet::Act)
                    .run_if(hero_on_ground),
            )
            .add_systems(
                Update,
                (kill_enemies, tick_corpses).in_set(CombatSet::Resolve),
            );
    }
}

/// One enemy. Its `Transform` is its feet on the terrain.
#[derive(Component, Debug)]
pub struct Enemy {
    #[allow(dead_code)] // read by phase 3 (elite/boss rules)
    pub kind: EnemyKind,
    pub health: f32,
    pub speed: f32,
    /// Contact damage per second, with the wave's multiplier applied.
    pub dps: f32,
    pub radius: f32,
    pub xp: u32,
    /// Seconds left frozen (no move, no contact damage).
    pub frozen: f32,
    /// Seconds left slowed, and the fraction of speed taken.
    pub slow_left: f32,
    pub slow: f32,
    /// Seconds left of the "just hit" pulse.
    pub hit_flash: f32,
    /// Hit since its animation last looked (a flinch may play).
    pub just_hit: bool,
    /// Seconds to its next attack swing while touching the hero.
    pub attack_cd: f32,
}

impl Enemy {
    pub fn new(kind: EnemyKind, health_mul: f32, damage_mul: f32) -> Self {
        let d = kind.def();
        Self {
            kind,
            health: d.health * health_mul,
            speed: d.speed,
            dps: d.contact_dps * damage_mul,
            radius: d.radius,
            xp: d.xp,
            frozen: 0.0,
            slow_left: 0.0,
            slow: 0.0,
            hit_flash: 0.0,
            just_hit: false,
            attack_cd: 0.0,
        }
    }

    /// Takes a hit (death is resolved later in the frame).
    pub fn hurt(&mut self, damage: f32) {
        self.health = rules::apply_damage(self.health, damage).0;
        self.hit_flash = 0.12;
        self.just_hit = true;
    }

    pub fn chill(&mut self, slow: f32, secs: f32) {
        (self.slow_left, self.slow) = rules::merge_slow((self.slow_left, self.slow), (secs, slow));
    }

    pub fn freeze(&mut self, secs: f32) {
        self.frozen = self.frozen.max(secs);
    }

    pub fn alive(&self) -> bool {
        self.health > 0.0
    }
}

#[derive(Resource, Default)]
struct Spawner {
    /// Seconds until the next group.
    next: f32,
}

#[allow(clippy::too_many_arguments)]
fn spawn_waves(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    mut spawner: ResMut<Spawner>,
    mut rng: ResMut<CombatRng>,
    assets: Res<Placeholders>,
    heroes: Query<&Transform, With<Hero>>,
    enemies: Query<(), With<Enemy>>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    spawner.next -= time.delta_secs();
    if spawner.next > 0.0 {
        return;
    }
    let wave = waves::wave_at(run.time);
    spawner.next = wave.interval;
    let alive = enemies.iter().count() as u32;
    if alive >= wave.max_alive {
        return;
    }
    let rng = &mut rng.0;
    let kind = waves::pick_kind(wave.weights, rng.f32());
    // Wolves come as a bigger pack.
    let group = if kind == EnemyKind::Wolf {
        wave.group + 2
    } else {
        wave.group
    };
    let angle = rng.range(0.0, std::f32::consts::TAU);
    let radius = rng.range(SPAWN_RING_MIN, SPAWN_RING_MAX);
    let center = rules::ring_point(hero.translation, angle, radius);
    for i in 0..group.min(wave.max_alive - alive) {
        let jitter = if i == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(rng.range(-2.5, 2.5), 0.0, rng.range(-2.5, 2.5))
        };
        let e = commands
            .spawn((
                Name::new(kind.def().name),
                Enemy::new(kind, wave.health_mul, wave.damage_mul),
                // Height is fixed by `move_enemies` once the terrain under it is known.
                Transform::from_translation(center + jitter),
                Visibility::default(),
            ))
            .id();
        attach_enemy_visual(&mut commands, e, kind, &assets);
    }
}

/// Enemies run at the hero, push apart a little, follow the ground and face where they go.
/// One left far behind is put back on the ring.
fn move_enemies(
    time: Res<Time>,
    mut rng: ResMut<CombatRng>,
    streamer: Option<Res<TerrainStreamer>>,
    adt_tiles: Option<Res<Assets<AdtTile>>>,
    heroes: Query<&Transform, (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(&mut Transform, &mut Enemy)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    let hero_pos = hero.translation;

    // Crowding: a cheap pairwise push on a snapshot (the waves cap the count at a few hundred).
    let snapshot: Vec<(Vec3, f32)> = enemies
        .iter()
        .map(|(xf, e)| (xf.translation, e.radius))
        .collect();

    let mut cache = None;
    for (i, (mut xf, mut e)) in enemies.iter_mut().enumerate() {
        e.frozen = (e.frozen - dt).max(0.0);
        e.slow_left = (e.slow_left - dt).max(0.0);
        e.hit_flash = (e.hit_flash - dt).max(0.0);

        let mut pos = xf.translation;
        if rules::flat_dist(pos, hero_pos) > LEASH {
            let angle = rng.0.range(0.0, std::f32::consts::TAU);
            pos = rules::ring_point(hero_pos, angle, SPAWN_RING_MAX);
        }
        let speed = rules::effective_speed(e.speed, e.frozen, e.slow_left, e.slow);
        let next = rules::chase_step(pos, hero_pos, speed, dt, e.radius + 0.4);

        let mut push = Vec3::ZERO;
        if speed > 0.0 {
            for (j, (other, r)) in snapshot.iter().enumerate() {
                if i == j {
                    continue;
                }
                let d = (next - *other).with_y(0.0);
                let min = e.radius + r;
                let len = d.length();
                if len < min && len > 1e-4 {
                    push += d / len * (min - len);
                }
            }
        }
        let mut new_pos = next + push.clamp_length_max(speed.max(1.0) * dt);

        let ground = match (&streamer, &adt_tiles) {
            (Some(s), Some(a)) => terrain_height_under_cached(s, a, new_pos, &mut cache),
            _ => None,
        };
        new_pos.y = ground.unwrap_or(hero_pos.y);

        let dir = (hero_pos - new_pos).with_y(0.0);
        if let Some(yaw) = crate::motion::facing_yaw(dir) {
            xf.rotation = Quat::from_rotation_y(yaw);
        }
        xf.translation = new_pos;
        xf.scale = Vec3::splat(if e.hit_flash > 0.0 { 1.15 } else { 1.0 });
    }
}

/// Enemy models swing while they touch the hero and flinch when hit; walking and running come
/// from their movement (`AutoLocomotion`).
fn animate_enemies(
    time: Res<Time>,
    heroes: Query<&Transform, (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(&Transform, &mut Enemy, Option<&mut ModelAnim>)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (xf, mut e, anim) in &mut enemies {
        let touching =
            rules::touching(rules::flat_dist(xf.translation, hero.translation), e.radius);
        let busy = anim.as_ref().is_some_and(|a| a.action().is_some());
        let (action, cd) =
            rules::enemy_action(touching, e.frozen > 0.0, e.attack_cd, dt, e.just_hit, busy);
        e.attack_cd = cd;
        e.just_hit = false;
        if let (Some(action), Some(mut anim)) = (action, anim) {
            anim.play(match action {
                rules::EnemyAction::Attack => AnimState::Attack,
                rules::EnemyAction::Hit => AnimState::Hit,
            });
        }
    }
}

/// A dead enemy's body: its model plays its death, lies a moment, then sinks away.
#[derive(Component)]
pub struct Corpse {
    left: f32,
    ground: f32,
}

/// Dead enemies leave an XP orb. One with a model becomes a [`Corpse`]; a placeholder is removed.
#[allow(clippy::type_complexity)]
fn kill_enemies(
    mut commands: Commands,
    mut run: ResMut<Run>,
    assets: Res<Placeholders>,
    mut enemies: Query<(
        Entity,
        &mut Transform,
        &Enemy,
        Option<&ModelStatus>,
        Option<&mut ModelAnim>,
    )>,
) {
    for (entity, mut xf, e, status, anim) in &mut enemies {
        if e.alive() {
            continue;
        }
        spawn_orb(&mut commands, &assets, xf.translation, e.xp);
        run.kills += 1;
        match (status, anim) {
            (Some(ModelStatus::Ready), Some(mut anim)) => {
                anim.state = AnimState::Death;
                xf.scale = Vec3::ONE;
                commands.entity(entity).remove::<Enemy>().insert(Corpse {
                    left: rules::CORPSE_SECS,
                    ground: xf.translation.y,
                });
            }
            _ => {
                commands.entity(entity).try_despawn();
            }
        }
    }
}

fn tick_corpses(
    mut commands: Commands,
    time: Res<Time>,
    mut corpses: Query<(Entity, &mut Corpse, &mut Transform)>,
) {
    for (entity, mut c, mut xf) in &mut corpses {
        c.left -= time.delta_secs();
        if c.left <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        xf.translation.y = c.ground - rules::corpse_depth(c.left);
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
        init(&mut world, spawn_waves);
        init(&mut world, move_enemies);
        init(&mut world, kill_enemies);
        init(&mut world, animate_enemies);
        init(&mut world, tick_corpses);
    }
}
