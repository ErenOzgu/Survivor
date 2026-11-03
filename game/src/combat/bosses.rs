//! Phase 3, "Harita": the elite at its fixed spot, the timed mini bosses and the map boss, their
//! telegraphed special attacks, and what they leave when they die.
//!
//! Bosses are ordinary [`Enemy`] entities (so skills, contact damage, deaths and the run reset
//! treat them like any enemy) with a [`Boss`] component on top. Their look comes from
//! `visuals::attach_enemy_visual` with the kind they borrow (`BossDef::body`), plus a coloured
//! ring under their feet. The rules are in [`super::boss_rules`].

use bevy::prelude::*;

use super::boss_rules::{
    self, aggroed, barrage_spots, boss_health_mul, cap_freeze, charge_hits, charge_path,
    guard_present, in_circle, timed_due, AttackEvent, AttackPhase,
};
use super::enemies::Enemy;
use super::hero::HeroHealth;
use super::portal::{spawn_chest_and_portal, Chest, MapCleared, Portal};
use super::rules::{self, flat_dist};
use super::visuals::{attach_enemy_visual, spawn_flash, Placeholders};
use super::xp::spawn_orb;
use super::{hero_on_ground, CombatRng, CombatSet, DebugStart, HeroDied, RestartRun, Run};
use crate::content::bosses::{Arrival, AttackShape, BossDef, Rank, ELWYNN_BOSSES};
use crate::content::waves::{self, SPAWN_RING_MIN};
use crate::content::SkillId;
use crate::game::Hero;

/// How many bosses the map has.
const BOSSES: usize = ELWYNN_BOSSES.len();

pub struct BossesPlugin;

impl Plugin for BossesPlugin {
    fn build(&self, app: &mut App) {
        // `CombatPlugin` inserts `DebugStart` before adding this plugin.
        let start = app
            .world()
            .get_resource::<DebugStart>()
            .map_or(0.0, |d| d.time);
        app.insert_resource(BossRun::new(start))
            .init_resource::<BossNotice>()
            .configure_sets(
                Update,
                BossSet.after(CombatSet::Act).before(CombatSet::Resolve),
            )
            .add_systems(Startup, build_boss_assets)
            .add_systems(
                Update,
                (spawn_timed_bosses, guard_elites, boss_ai, tick_telegraphs)
                    .chain()
                    .in_set(BossSet)
                    .run_if(hero_on_ground),
            )
            // Deaths are checked even off the ground, like `kill_enemies`.
            .add_systems(Update, boss_deaths.in_set(BossSet).after(tick_telegraphs))
            .add_systems(Update, reset_bosses.in_set(CombatSet::Reset));
    }
}

/// Between the combat's `Act` (moves, casts) and `Resolve` (deaths): boss brains run after the
/// enemies have moved and before dead ones are removed.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BossSet;

/// The map's boss state for the current run.
#[derive(Resource, Debug, Clone)]
pub struct BossRun {
    /// The run clock's value when the run began (0, or the `RNS_TIME` debug jump).
    pub start: f32,
    /// Timed bosses already spawned this run.
    pub spawned: [bool; BOSSES],
    /// Bosses killed this run (an elite is not put back until the next run).
    pub killed: [bool; BOSSES],
    /// Elites and mini bosses killed, in order (for the end-of-map summary).
    pub kill_order: Vec<usize>,
}

impl BossRun {
    pub fn new(start: f32) -> Self {
        Self {
            start,
            spawned: [false; BOSSES],
            killed: [false; BOSSES],
            kill_order: Vec::new(),
        }
    }
}

/// A short call-out ("… seni fark etti!", "… kesildi!"), shown for `left` real seconds.
#[derive(Resource, Default, Debug, Clone)]
pub struct BossNotice {
    pub text: String,
    pub left: f32,
}

impl BossNotice {
    pub fn show(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.left = 3.5;
        info!("rnsurvivors: {}", self.text);
    }
}

/// A boss on top of its [`Enemy`].
#[derive(Component, Debug)]
pub struct Boss {
    /// Index into [`ELWYNN_BOSSES`].
    pub idx: usize,
    /// Health at spawn (after scaling), for bars and the "hurt" aggro.
    pub max_health: f32,
    /// Fighting (a guarding elite sleeps at its spot until woken).
    pub awake: bool,
    /// A guard's spot (Bevy space; only `x`/`z` matter).
    home: Vec3,
    pub attack: boss_rules::AttackState,
    /// The current charge's direction.
    charge_dir: Vec3,
    /// The current attack already hurt the hero.
    hit: bool,
    /// The attack that landed this frame, if any.
    landed: Option<usize>,
    /// Its death was handled (orbs, loot, portal).
    dead: bool,
}

impl Boss {
    pub fn def(&self) -> &'static BossDef {
        ELWYNN_BOSSES[self.idx]
    }
}

/// A ground telegraph of a boss's wind-up; gone when the wind-up is.
#[derive(Component, Debug)]
pub struct Telegraph {
    owner: Entity,
    attack: usize,
    shape: TeleShape,
    /// The growing inner part (the outline does not change).
    fill: bool,
}

#[derive(Debug, Clone, Copy)]
enum TeleShape {
    Circle {
        center: Vec3,
        radius: f32,
    },
    Lane {
        start: Vec3,
        dir: Vec3,
        length: f32,
        width: f32,
    },
}

/// Meshes and materials for rings, telegraphs, the chest and the portal.
#[derive(Resource)]
pub struct BossAssets {
    /// A flat disc of radius 1 (scale to a radius).
    pub disc: Handle<Mesh>,
    /// A flat 1×1 slab (scale to a lane).
    pub slab: Handle<Mesh>,
    pub tele_outline: Handle<StandardMaterial>,
    pub tele_fill: Handle<StandardMaterial>,
    pub rings: Vec<Handle<StandardMaterial>>,
    pub chest: (Handle<Mesh>, Handle<StandardMaterial>),
    pub portal: (Handle<Mesh>, Handle<StandardMaterial>),
    pub portal_core: Handle<StandardMaterial>,
}

fn glass(c: [f32; 3], a: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(c[0], c[1], c[2], a),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

fn build_boss_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let red = [1.0, 0.15, 0.1];
    commands.insert_resource(BossAssets {
        disc: meshes.add(Cylinder::new(1.0, 0.3)),
        slab: meshes.add(Cuboid::new(1.0, 0.3, 1.0)),
        tele_outline: materials.add(glass(red, 0.18)),
        tele_fill: materials.add(glass(red, 0.4)),
        rings: ELWYNN_BOSSES
            .iter()
            .map(|b| materials.add(glass(b.ring_color, 0.45)))
            .collect(),
        chest: (
            meshes.add(Cuboid::new(1.2, 0.8, 0.8)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.75, 0.2),
                unlit: true,
                ..default()
            }),
        ),
        portal: (
            meshes.add(Torus::new(1.2, 1.6)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.45, 0.6, 1.0),
                unlit: true,
                ..default()
            }),
        ),
        portal_core: materials.add(glass([0.55, 0.4, 1.0], 0.55)),
    });
}

/// Puts boss `idx` into the world at `at` (feet; the height is fixed by the enemy movement once
/// the terrain under it is known). Health scales with the hero's `level`, contact damage with
/// the run's wave `damage_mul`.
#[allow(clippy::too_many_arguments)]
fn spawn_boss(
    commands: &mut Commands,
    placeholders: &Placeholders,
    assets: &BossAssets,
    idx: usize,
    at: Vec3,
    level: u32,
    damage_mul: f32,
    awake: bool,
) -> Entity {
    let def = ELWYNN_BOSSES[idx];
    let s = def.stats;
    let mut enemy = Enemy::new(def.body, 1.0, 1.0);
    let health = s.health * boss_health_mul(level, def.expected_level);
    enemy.health = health;
    enemy.speed = if awake { s.speed } else { 0.0 };
    enemy.dps = s.contact_dps * damage_mul;
    enemy.radius = s.radius;
    enemy.xp = s.xp;
    let e = commands
        .spawn((
            Name::new(def.name()),
            enemy,
            Boss {
                idx,
                max_health: health,
                awake,
                home: at,
                attack: boss_rules::AttackState::new(def.first_attack),
                charge_dir: Vec3::NEG_Z,
                hit: false,
                landed: None,
                dead: false,
            },
            Transform::from_translation(at),
            Visibility::default(),
        ))
        .id();
    attach_enemy_visual(commands, e, def.body, placeholders);
    let r = s.radius * 1.5;
    commands.entity(e).with_children(|p| {
        p.spawn((
            Name::new("boss ring"),
            Mesh3d(assets.disc.clone()),
            MeshMaterial3d(assets.rings[idx].clone()),
            Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(r, 0.3, r)),
        ));
    });
    info!(
        "rnsurvivors: {} appears ({health:.0} health, level {level})",
        def.name()
    );
    e
}

/// Mini bosses and the map boss arrive on the run clock, on the spawn ring around the hero.
#[allow(clippy::too_many_arguments)]
fn spawn_timed_bosses(
    mut commands: Commands,
    run: Res<Run>,
    mut bosses: ResMut<BossRun>,
    mut rng: ResMut<CombatRng>,
    placeholders: Res<Placeholders>,
    assets: Res<BossAssets>,
    heroes: Query<&Transform, With<Hero>>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    for (idx, def) in ELWYNN_BOSSES.iter().enumerate() {
        let Some(at) = def.arrives_at() else {
            continue;
        };
        if !timed_due(at, bosses.start, run.time, bosses.spawned[idx]) {
            continue;
        }
        bosses.spawned[idx] = true;
        let angle = rng.0.range(0.0, std::f32::consts::TAU);
        let pos = rules::ring_point(hero.translation, angle, SPAWN_RING_MIN);
        let damage_mul = waves::wave_at(run.time).damage_mul;
        spawn_boss(
            &mut commands,
            &placeholders,
            &assets,
            idx,
            pos,
            run.level,
            damage_mul,
            true,
        );
    }
}

/// The spot of a guarding boss in Bevy space, at height `y`.
pub fn guard_spot(def: &BossDef, y: f32) -> Option<Vec3> {
    match def.arrival {
        Arrival::Guard { at, .. } => {
            let mut p = benilla_assets::coords::wow_to_bevy([at.0, at.1, 0.0]);
            p.y = y;
            Some(p)
        }
        Arrival::Timed { .. } => None,
    }
}

/// Guarding elites wait at their spot: put in the world when the hero comes near, taken out
/// again if the hero leaves without waking them. Once killed, not again this run.
#[allow(clippy::too_many_arguments)]
fn guard_elites(
    mut commands: Commands,
    run: Res<Run>,
    state: Res<BossRun>,
    placeholders: Res<Placeholders>,
    assets: Res<BossAssets>,
    heroes: Query<&Transform, With<Hero>>,
    bosses: Query<(Entity, &Boss)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    for (idx, def) in ELWYNN_BOSSES.iter().enumerate() {
        let Some(spot) = guard_spot(def, hero.translation.y) else {
            continue;
        };
        if state.killed[idx] {
            continue;
        }
        let dist = flat_dist(spot, hero.translation);
        let existing = bosses.iter().find(|(_, b)| b.idx == idx);
        let want = guard_present(dist, existing.is_some());
        match existing {
            None if want => {
                let damage_mul = waves::wave_at(run.time).damage_mul;
                spawn_boss(
                    &mut commands,
                    &placeholders,
                    &assets,
                    idx,
                    spot,
                    run.level,
                    damage_mul,
                    false,
                );
            }
            Some((e, b)) if !want && !b.awake => {
                commands.entity(e).try_despawn();
            }
            _ => {}
        }
    }
}

/// Hurts the hero by a special attack (respecting god mode and Blink's invulnerability).
fn hurt_hero(health: &mut HeroHealth, debug: &DebugStart, damage: f32) {
    if debug.god || health.invulnerable > 0.0 {
        return;
    }
    health.current = rules::apply_damage(health.current, damage).0;
}

/// Boss brains: guards wake, special attacks wind up (with a telegraph), land and recover.
/// While a special is going the boss does not walk; a charge moves it here instead.
#[allow(clippy::too_many_arguments)]
fn boss_ai(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    debug: Res<DebugStart>,
    mut health: ResMut<HeroHealth>,
    mut rng: ResMut<CombatRng>,
    mut notice: ResMut<BossNotice>,
    placeholders: Res<Placeholders>,
    assets: Res<BossAssets>,
    heroes: Query<&Transform, (With<Hero>, Without<Enemy>)>,
    mut bosses: Query<(Entity, &mut Transform, &mut Enemy, &mut Boss), Without<Hero>>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let hero = hero.translation;
    let dt = time.delta_secs();
    let damage_mul = waves::wave_at(run.time).damage_mul;
    for (entity, mut xf, mut e, mut boss) in &mut bosses {
        boss.landed = None;
        if !e.alive() {
            continue;
        }
        e.frozen = cap_freeze(e.frozen);
        let def = boss.def();
        let pos = xf.translation;
        let dist = flat_dist(pos, hero);

        if !boss.awake {
            let aggro = match def.arrival {
                Arrival::Guard { aggro, .. } => aggro,
                Arrival::Timed { .. } => f32::INFINITY,
            };
            if aggroed(dist, aggro, e.health, boss.max_health) {
                boss.awake = true;
                notice.show(format!("{} seni fark etti!", def.name()));
            } else {
                // Stay put (the enemy movement or its leash may have moved it).
                e.speed = 0.0;
                xf.translation.x = boss.home.x;
                xf.translation.z = boss.home.z;
                continue;
            }
        }
        if dt <= 0.0 || e.frozen > 0.0 {
            // A frozen boss's special waits (its telegraph stays).
            continue;
        }

        match boss.attack.step(dt, def.attacks, dist) {
            AttackEvent::Begin(a) => {
                boss.hit = false;
                let atk = &def.attacks[a];
                let shapes: Vec<TeleShape> = match atk.shape {
                    AttackShape::Slam { radius } => vec![TeleShape::Circle {
                        center: pos,
                        radius,
                    }],
                    AttackShape::Charge { length, width, .. } => {
                        let (dir, _) = charge_path(pos, hero, length, xf.forward().into());
                        boss.charge_dir = dir;
                        vec![TeleShape::Lane {
                            start: pos,
                            dir,
                            length,
                            width,
                        }]
                    }
                    AttackShape::Barrage {
                        count,
                        radius,
                        spread,
                    } => {
                        let a0 = rng.0.range(0.0, std::f32::consts::TAU);
                        barrage_spots(hero, count, spread, a0)
                            .into_iter()
                            .map(|center| TeleShape::Circle { center, radius })
                            .collect()
                    }
                };
                for shape in shapes {
                    for fill in [false, true] {
                        spawn_telegraph(&mut commands, &assets, entity, a, shape, fill);
                    }
                }
            }
            AttackEvent::Land(a) => {
                boss.landed = Some(a);
                let atk = &def.attacks[a];
                // A slam strikes around the boss now; a barrage's circles are resolved by
                // `tick_telegraphs`, which knows where each one is; a charge starts moving.
                if let AttackShape::Slam { radius } = atk.shape {
                    spawn_flash(&mut commands, &placeholders, SkillId::Fireball, pos, radius);
                    if in_circle(pos, radius, hero) {
                        boss.hit = true;
                        hurt_hero(&mut health, &debug, atk.damage * damage_mul);
                    }
                }
            }
            AttackEvent::End(_) | AttackEvent::None => {}
        }

        // A charge in progress: run along the locked lane, hurting the hero once.
        if let AttackPhase::Active { attack, .. } = boss.attack.phase {
            let atk = &def.attacks[attack];
            if let AttackShape::Charge { speed, width, .. } = atk.shape {
                xf.translation += boss.charge_dir * speed * dt;
                if !boss.hit && charge_hits(xf.translation, e.radius, width, hero) {
                    boss.hit = true;
                    hurt_hero(&mut health, &debug, atk.damage * damage_mul);
                }
            }
        }
        e.speed = if boss.attack.busy() {
            0.0
        } else {
            def.stats.speed
        };
    }
}

fn spawn_telegraph(
    commands: &mut Commands,
    assets: &BossAssets,
    owner: Entity,
    attack: usize,
    shape: TeleShape,
    fill: bool,
) {
    let (mesh, xf) = match shape {
        TeleShape::Circle { center, .. } => (
            assets.disc.clone(),
            Transform::from_translation(center + Vec3::Y * 0.1),
        ),
        TeleShape::Lane { start, .. } => (
            assets.slab.clone(),
            Transform::from_translation(start + Vec3::Y * 0.1),
        ),
    };
    let mat = if fill {
        assets.tele_fill.clone()
    } else {
        assets.tele_outline.clone()
    };
    let mut xf = xf;
    place_telegraph(&mut xf, shape, if fill { 0.0 } else { 1.0 });
    commands.spawn((
        Name::new("telegraph"),
        Telegraph {
            owner,
            attack,
            shape,
            fill,
        },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        xf,
    ));
}

/// Sets a telegraph piece's transform for a fill `progress` (`0..=1`; the outline is always 1).
fn place_telegraph(xf: &mut Transform, shape: TeleShape, progress: f32) {
    let p = progress.clamp(0.02, 1.0);
    match shape {
        TeleShape::Circle { center, radius } => {
            let r = radius * p;
            xf.translation = center + Vec3::Y * 0.1;
            xf.scale = Vec3::new(r, 1.0, r);
        }
        TeleShape::Lane {
            start,
            dir,
            length,
            width,
        } => {
            let len = length * p;
            xf.translation = start + dir * (len * 0.5) + Vec3::Y * 0.1;
            if let Some(yaw) = crate::motion::facing_yaw(dir) {
                xf.rotation = Quat::from_rotation_y(yaw);
            }
            xf.scale = Vec3::new(width, 1.0, len);
        }
    }
}

/// Telegraphs fill up with their boss's wind-up and go when it ends. A barrage's damage is
/// resolved here, where each impact circle is known, on the frame its boss lands it.
#[allow(clippy::too_many_arguments)]
fn tick_telegraphs(
    mut commands: Commands,
    run: Res<Run>,
    debug: Res<DebugStart>,
    mut health: ResMut<HeroHealth>,
    placeholders: Res<Placeholders>,
    heroes: Query<&Transform, (With<Hero>, Without<Telegraph>)>,
    mut bosses: Query<&mut Boss>,
    mut telegraphs: Query<(Entity, &Telegraph, &mut Transform)>,
) {
    let hero = heroes.single().map(|h| h.translation).ok();
    let damage_mul = waves::wave_at(run.time).damage_mul;
    for (e, t, mut xf) in &mut telegraphs {
        let Ok(mut boss) = bosses.get_mut(t.owner) else {
            commands.entity(e).try_despawn();
            continue;
        };
        let def = boss.def();
        let winding = matches!(
            boss.attack.phase,
            AttackPhase::Windup { attack, .. } if attack == t.attack
        );
        if winding {
            if t.fill {
                place_telegraph(&mut xf, t.shape, boss.attack.windup_progress(def.attacks));
            }
            continue;
        }
        commands.entity(e).try_despawn();
        // Landed this frame (not cancelled by death): a barrage circle strikes.
        let atk = &def.attacks[t.attack];
        let just_landed = boss.landed == Some(t.attack);
        if let (true, true, AttackShape::Barrage { .. }, TeleShape::Circle { center, radius }) =
            (t.fill, just_landed, atk.shape, t.shape)
        {
            spawn_flash(
                &mut commands,
                &placeholders,
                SkillId::Fireball,
                center,
                radius,
            );
            if let Some(hero) = hero {
                if !boss.hit && in_circle(center, radius, hero) {
                    boss.hit = true;
                    hurt_hero(&mut health, &debug, atk.damage * damage_mul);
                }
            }
        }
    }
}

/// A dead boss scatters bonus XP orbs; the map boss leaves the loot chest and the portal.
fn boss_deaths(
    mut commands: Commands,
    mut state: ResMut<BossRun>,
    mut rng: ResMut<CombatRng>,
    mut notice: ResMut<BossNotice>,
    placeholders: Res<Placeholders>,
    assets: Res<BossAssets>,
    mut bosses: Query<(&Transform, &Enemy, &mut Boss)>,
) {
    for (xf, e, mut boss) in &mut bosses {
        if e.alive() || boss.dead {
            continue;
        }
        boss.dead = true;
        let def = boss.def();
        state.killed[boss.idx] = true;
        let pos = xf.translation;
        for _ in 0..def.bonus_orbs {
            let a = rng.0.range(0.0, std::f32::consts::TAU);
            let r = rng.0.range(1.5, 4.0);
            spawn_orb(
                &mut commands,
                &placeholders,
                rules::ring_point(pos, a, r),
                def.bonus_xp,
            );
        }
        if def.rank == Rank::MapBoss {
            let a = rng.0.range(0.0, std::f32::consts::TAU);
            spawn_chest_and_portal(&mut commands, &assets, pos, rules::ring_point(pos, a, 5.0));
            notice.show(format!(
                "{} kesildi! Ganimet sandığı ve portal açıldı.",
                def.name()
            ));
        } else {
            state.kill_order.push(boss.idx);
            notice.show(format!("{} kesildi!", def.name()));
        }
    }
}

/// A new run (death or a cleared map) starts the map's bosses over: everything boss-made goes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reset_bosses(
    mut commands: Commands,
    mut died: MessageReader<HeroDied>,
    mut cleared: MessageReader<MapCleared>,
    mut restarted: MessageReader<RestartRun>,
    debug: Res<DebugStart>,
    mut state: ResMut<BossRun>,
    mut notice: ResMut<BossNotice>,
    stuff: Query<Entity, Or<(With<Boss>, With<Telegraph>, With<Chest>, With<Portal>)>>,
) {
    let died = died.read().count() > 0;
    let cleared = cleared.read().count() > 0 || restarted.read().count() > 0;
    if !died && !cleared {
        return;
    }
    for e in &stuff {
        commands.entity(e).try_despawn();
    }
    *state = BossRun::new(debug.time);
    *notice = BossNotice::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::bosses::GNOLL_CHIEF_BOSS;

    #[test]
    fn the_gnoll_spot_maps_to_bevy_space() {
        let p = guard_spot(&GNOLL_CHIEF_BOSS, 12.0).unwrap();
        let (x, y) = crate::content::waves::GNOLL_CHIEF_AT;
        assert_eq!(p, Vec3::new(-y, 12.0, -x));
        assert!(guard_spot(&crate::content::bosses::DEFIAS_LEADER_BOSS, 0.0).is_none());
    }

    #[test]
    fn lane_telegraph_grows_from_the_boss() {
        let shape = TeleShape::Lane {
            start: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            length: 20.0,
            width: 2.0,
        };
        let mut xf = Transform::default();
        place_telegraph(&mut xf, shape, 0.5);
        assert_eq!(xf.scale, Vec3::new(2.0, 1.0, 10.0));
        assert!((xf.translation - Vec3::new(0.0, 0.1, -5.0)).length() < 1e-5);
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
        init(&mut world, build_boss_assets);
        init(&mut world, spawn_timed_bosses);
        init(&mut world, guard_elites);
        init(&mut world, boss_ai);
        init(&mut world, tick_telegraphs);
        init(&mut world, boss_deaths);
        init(&mut world, reset_bosses);
    }
}
