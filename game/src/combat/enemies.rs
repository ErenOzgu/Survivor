//! Enemies: the wave spawner around the hero, chasing (or keeping their distance) on the terrain,
//! ranged bolts and sapper blasts, status effects, their model's animations, deaths and corpses.

use bevy::prelude::*;

use benilla_assets::AdtTile;
use benilla_world::terrain_stream::{terrain_height_under_cached, TerrainStreamer};

use super::hero::HeroHealth;
use super::rules;
use super::visuals::{attach_enemy_visual, spawn_flash, Placeholders};
use super::xp::spawn_orb;
use super::{hero_on_ground, CombatRng, CombatSet, DebugStart, Run};
use crate::content::enemies::Behavior;
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
                (
                    move_enemies,
                    enemy_attacks,
                    move_enemy_bolts,
                    animate_enemies,
                )
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

/// Enemies closer than this to the hero collide with the world.
const COLLIDE_WITHIN: f32 = 32.0;

/// One enemy. Its `Transform` is its feet on the terrain.
#[derive(Component, Debug)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub health: f32,
    /// Health at spawn, for the health bar.
    pub max_health: f32,
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
    /// Seconds left of a critical hit's knock-back.
    pub knock: f32,
    /// Hit since its animation last looked (a flinch may play).
    pub just_hit: bool,
    /// Seconds to its next attack swing while touching the hero.
    pub attack_cd: f32,
    /// Damage taken this frame (for the damage numbers), and whether any of it was a crit.
    pub took: f32,
    pub took_crit: bool,
    /// The skill that hit it last this frame (the number's colour).
    pub took_skill: Option<crate::content::SkillId>,
    /// Multiplier on its attacks' damage (the wave's), for bolts and blasts.
    pub damage_mul: f32,
    /// Seconds to its next shot (ranged kinds).
    pub shoot_cd: f32,
    /// Seconds left on a lit fuse (exploders); `None` until lit.
    pub fuse: Option<f32>,
}

impl Enemy {
    pub fn new(kind: EnemyKind, health_mul: f32, damage_mul: f32) -> Self {
        let d = kind.def();
        Self {
            kind,
            health: d.health * health_mul,
            max_health: d.health * health_mul,
            speed: d.speed,
            dps: d.contact_dps * damage_mul,
            radius: d.radius,
            xp: d.xp,
            frozen: 0.0,
            slow_left: 0.0,
            slow: 0.0,
            hit_flash: 0.0,
            knock: 0.0,
            took_skill: None,
            just_hit: false,
            attack_cd: 0.0,
            took: 0.0,
            took_crit: false,
            damage_mul,
            shoot_cd: 1.0,
            fuse: None,
        }
    }

    /// Takes a hit, maybe a critical one (death is resolved later in the frame); the damage
    /// shows as a number.
    pub fn strike(&mut self, damage: f32, crit: bool) {
        if !self.alive() {
            return;
        }
        let before = self.health;
        self.health = rules::apply_damage(self.health, damage).0;
        self.took += before - self.health;
        self.took_crit |= crit;
        self.hit_flash = HIT_FLASH;
        if crit {
            self.knock = KNOCK_SECS;
        }
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

/// The "just hit" pulse: seconds, and how much bigger the model swells at its start.
const HIT_FLASH: f32 = 0.14;
const HIT_SWELL: f32 = 0.2;
/// A critical hit pushes a common enemy back this long, this fast (yards per second); bosses
/// stand.
const KNOCK_SECS: f32 = 0.14;
const KNOCK_SPEED: f32 = 7.0;

/// The model's size while the hit pulse runs out: a quick swell that eases back.
pub fn hit_scale(flash_left: f32) -> f32 {
    1.0 + HIT_SWELL * (flash_left / HIT_FLASH).clamp(0.0, 1.0)
}

/// Enemies run at the hero, push apart a little, follow the ground and face where they go.
/// One left far behind is put back on the ring.
#[allow(clippy::too_many_arguments)]
fn move_enemies(
    time: Res<Time>,
    mut rng: ResMut<CombatRng>,
    streamer: Option<Res<TerrainStreamer>>,
    adt_tiles: Option<Res<Assets<AdtTile>>>,
    heroes: Query<&Transform, (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(&mut Transform, &mut Enemy, Has<super::bosses::Boss>)>,
    world: benilla_world::collision::WorldCollision,
    mut shape: Local<Option<avian3d::prelude::Collider>>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    let hero_pos = hero.translation;

    // Crowding: a cheap pairwise push on a snapshot (the waves cap the count at a few hundred).
    let snapshot: Vec<(Vec3, f32)> = enemies
        .iter()
        .map(|(xf, e, _)| (xf.translation, e.radius))
        .collect();

    let mut cache = None;
    for (i, (mut xf, mut e, boss)) in enemies.iter_mut().enumerate() {
        e.frozen = (e.frozen - dt).max(0.0);
        e.slow_left = (e.slow_left - dt).max(0.0);
        e.hit_flash = (e.hit_flash - dt).max(0.0);
        let knocked = e.knock > 0.0 && !boss;
        e.knock = (e.knock - dt).max(0.0);

        let mut pos = xf.translation;
        if rules::flat_dist(pos, hero_pos) > LEASH {
            let angle = rng.0.range(0.0, std::f32::consts::TAU);
            pos = rules::ring_point(hero_pos, angle, SPAWN_RING_MAX);
        }
        let mut speed = rules::effective_speed(e.speed, e.frozen, e.slow_left, e.slow);
        if e.fuse.is_some() {
            speed = 0.0; // a lit sapper stands and fizzes
        }
        let next = match e.kind.behavior() {
            Behavior::Ranged(r) => rules::kite_step(pos, hero_pos, speed, dt, r.keep),
            _ => rules::chase_step(pos, hero_pos, speed, dt, e.radius + 0.4),
        };

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
        if knocked {
            let away = (pos - hero_pos).with_y(0.0).normalize_or_zero();
            new_pos = pos + away * KNOCK_SPEED * dt;
        }
        // Walls stop enemies near the hero too (far ones are off screen and only cost).
        if rules::flat_dist(pos, hero_pos) < COLLIDE_WITHIN {
            let shape = shape.get_or_insert_with(super::collide::body);
            new_pos = pos + super::collide::swept_move(&world, shape, pos, new_pos - pos);
        }

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
        xf.scale = Vec3::splat(hit_scale(e.hit_flash));
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

/// A bolt thrown by a ranged enemy: flies straight, hurts the hero on touch.
#[derive(Component)]
pub struct EnemyBolt {
    dir: Vec3,
    speed: f32,
    damage: f32,
    life: f32,
}

/// The growing red circle under a lit sapper.
#[derive(Component)]
struct FuseMark;

/// How close a bolt must pass to hit the hero (yards).
const BOLT_HIT: f32 = 0.9;

/// Ranged enemies shoot when in range; exploders light their fuse close to the hero and blow up.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn enemy_attacks(
    mut commands: Commands,
    time: Res<Time>,
    debug: Res<DebugStart>,
    assets: Res<Placeholders>,
    mut health: ResMut<HeroHealth>,
    heroes: Query<&Transform, (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy, Option<&mut ModelAnim>)>,
    mut marks: Query<(&ChildOf, &mut Transform), (With<FuseMark>, Without<Enemy>, Without<Hero>)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let hero = hero.translation;
    for (entity, xf, mut e, anim) in &mut enemies {
        if !e.alive() || e.frozen > 0.0 {
            continue;
        }
        let dist = rules::flat_dist(xf.translation, hero);
        match e.kind.behavior() {
            Behavior::Melee => {}
            Behavior::Ranged(r) => {
                e.shoot_cd -= dt;
                if e.shoot_cd <= 0.0 && dist <= r.range {
                    e.shoot_cd = r.cooldown;
                    let from = xf.translation + Vec3::Y * 1.0;
                    let dir = (hero + Vec3::Y * 1.0 - from)
                        .with_y(0.0)
                        .normalize_or(Vec3::NEG_Z);
                    commands.spawn((
                        Name::new("enemy bolt"),
                        EnemyBolt {
                            dir,
                            speed: r.speed,
                            damage: r.damage * e.damage_mul,
                            life: r.range * 1.5 / r.speed,
                        },
                        Mesh3d(assets.enemy_bolt.0.clone()),
                        MeshMaterial3d(assets.enemy_bolt.1.clone()),
                        Transform::from_translation(from),
                    ));
                    if let Some(mut a) = anim {
                        a.play(AnimState::Attack);
                    }
                }
            }
            Behavior::Exploder(x) => match e.fuse {
                None if dist <= x.trigger => {
                    e.fuse = Some(x.fuse);
                    commands.entity(entity).with_children(|p| {
                        p.spawn((
                            FuseMark,
                            Mesh3d(assets.disc.clone()),
                            MeshMaterial3d(assets.warn.clone()),
                            Transform::from_xyz(0.0, 0.12, 0.0)
                                .with_scale(Vec3::new(0.1, 1.0, 0.1)),
                        ));
                    });
                    if let Some(mut a) = anim {
                        a.play(AnimState::Attack);
                    }
                }
                Some(left) => {
                    let left = left - dt;
                    if left > 0.0 {
                        e.fuse = Some(left);
                        continue;
                    }
                    // Boom: the hero takes it if still inside; the sapper is gone, no XP.
                    if dist <= x.radius {
                        health.take(x.damage * e.damage_mul, debug.god);
                    }
                    spawn_flash(
                        &mut commands,
                        &assets,
                        crate::content::SkillId::Fireball,
                        xf.translation,
                        x.radius,
                    );
                    e.fuse = None;
                    e.xp = 0;
                    e.health = 0.0;
                }
                None => {}
            },
        }
    }
    // The warning circles grow to the blast's size as the fuse burns.
    for (parent, mut mxf) in &mut marks {
        if let Ok((_, _, e, _)) = enemies.get(parent.parent()) {
            if let (Some(left), Behavior::Exploder(x)) = (e.fuse, e.kind.behavior()) {
                let r = x.radius * rules::fuse_progress(left, x.fuse).max(0.1);
                mxf.scale = Vec3::new(r, 1.0, r);
            }
        }
    }
}

/// Enemy bolts fly and hit the hero.
fn move_enemy_bolts(
    mut commands: Commands,
    time: Res<Time>,
    debug: Res<DebugStart>,
    mut health: ResMut<HeroHealth>,
    heroes: Query<&Transform, (With<Hero>, Without<EnemyBolt>)>,
    mut bolts: Query<(Entity, &mut Transform, &mut EnemyBolt)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    for (e, mut xf, mut b) in &mut bolts {
        b.life -= dt;
        xf.translation += b.dir * b.speed * dt;
        if rules::flat_dist(xf.translation, hero.translation) <= BOLT_HIT {
            health.take(b.damage, debug.god);
            commands.entity(e).try_despawn();
        } else if b.life <= 0.0 {
            commands.entity(e).try_despawn();
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
pub(super) fn kill_enemies(
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
        // A sapper that blew itself up gives nothing.
        if e.xp > 0 {
            spawn_orb(&mut commands, &assets, xf.translation, e.xp);
            run.kills += 1;
        }
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
        init(&mut world, enemy_attacks);
        init(&mut world, move_enemy_bolts);
    }
}
