//! Auto-casting: each owned skill fires on its own cooldown, and the things it leaves behind
//! (projectiles, homing missiles, ground areas, pulses, Blink's jump), evolutions included.
//! Every hit goes through [`strike`]: the passives' damage bonus and critical hits.

use std::collections::HashMap;

use bevy::prelude::*;

use super::art::WowArt;
use super::enemies::Enemy;
use super::fx::{self, BurstKind, FxBudget};
use super::hero::HeroHealth;
use super::rules::{self, Rng, Stats};
use super::visuals::{spawn_flash, Placeholders};
use super::{hero_on_ground, CombatRng, CombatSet, Run};
use crate::content::{SkillId, SkillLevel};
use crate::game::Hero;
use crate::models::ModelPlaceholder;
use crate::models::{AnimState, ModelAnim};

pub struct SkillsPlugin;

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillTimers>().add_systems(
            Update,
            (cast_skills, move_projectiles, tick_zones)
                .chain()
                .in_set(CombatSet::Act)
                .run_if(hero_on_ground),
        );
    }
}

/// Seconds until each owned skill may fire again (missing = ready).
#[derive(Resource, Default)]
pub struct SkillTimers(pub HashMap<SkillId, f32>);

/// Height above the feet that projectiles fly at.
const CAST_HEIGHT: f32 = 1.1;
/// A projectile's hit radius, added to the enemy's.
const PROJ_RADIUS: f32 = 0.35;
/// Pyroblast's burning ground: its tick damage as a share of the blast, and its size.
const BURN_SHARE: f32 = 0.15;
const BURN_RADIUS_SHARE: f32 = 0.8;

#[derive(Component)]
pub struct Projectile {
    pub skill: SkillId,
    pub dir: Vec3,
    pub speed: f32,
    pub damage: f32,
    /// Hits left after the current one.
    pub pierce: u32,
    pub life: f32,
    /// Already hit (a piercing bolt hits each enemy once).
    pub hit: Vec<Entity>,
    /// A homing projectile steers to this enemy (re-picked when it dies).
    pub homing: bool,
    pub target: Option<Entity>,
    pub on_hit: OnHit,
}

/// What a projectile does when it hits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OnHit {
    /// Just damage.
    Plain,
    /// Slows what it hits: fraction, seconds.
    Chill(f32, f32),
    /// Slows and freezes what it hits: fraction, freeze seconds.
    Freeze(f32, f32),
    /// Explodes on its first hit (or when it runs out), and may leave burning ground.
    Explode { radius: f32, burn: Option<Burn> },
}

/// Burning ground left behind: damage per tick, radius, seconds, tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burn {
    pub damage: f32,
    pub radius: f32,
    pub secs: f32,
    pub tick: f32,
}

/// A ground area that ticks damage (and maybe a slow) on what stands in it.
#[derive(Component)]
pub struct Zone {
    /// Whose area it is (its look).
    pub skill: SkillId,
    pub damage: f32,
    pub radius: f32,
    pub tick: f32,
    pub next_tick: f32,
    pub life: f32,
    pub slow: f32,
}

/// What a cast needs to know about one enemy.
#[derive(Clone, Copy)]
struct Target {
    entity: Entity,
    pos: Vec3,
}

/// One hit on an enemy: the passives' damage bonus and a crit roll.
pub fn strike(enemy: &mut Enemy, base: f32, stats: &Stats, rng: &mut Rng) {
    let (dmg, crit) = rules::hit_damage(base, stats, rng.f32());
    enemy.strike(dmg, crit);
}

/// How a skill's projectile behaves on hit.
pub fn on_hit_for(id: SkillId, s: &SkillLevel) -> OnHit {
    match id {
        SkillId::Frostbolt => OnHit::Chill(s.slow, s.duration),
        SkillId::WinterBolts => OnHit::Freeze(s.slow, s.duration),
        SkillId::Fireball => OnHit::Explode {
            radius: s.radius,
            burn: None,
        },
        SkillId::Pyroblast => OnHit::Explode {
            radius: s.radius,
            burn: Some(Burn {
                damage: s.damage * BURN_SHARE,
                radius: s.radius * BURN_RADIUS_SHARE,
                secs: s.duration,
                tick: s.tick,
            }),
        },
        SkillId::ArcaneStorm => OnHit::Explode {
            radius: s.radius,
            burn: None,
        },
        _ => OnHit::Plain,
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn cast_skills(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    save: Res<super::save::Save>,
    assets: Res<Placeholders>,
    art: Res<WowArt>,
    mut budget: ResMut<FxBudget>,
    mut timers: ResMut<SkillTimers>,
    mut rng: ResMut<CombatRng>,
    mut health: ResMut<HeroHealth>,
    mut heroes: Query<(&mut Transform, Option<&mut ModelAnim>), (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Hero>>,
) {
    let Ok((mut hero_xf, mut hero_anim)) = heroes.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let stats = Stats::from_parts(&run.passives, &save.data.bonus());
    let hero = hero_xf.translation;
    // Alive enemies, nearest first.
    let mut targets: Vec<Target> = enemies
        .iter()
        .filter(|(_, _, e)| e.alive())
        .map(|(entity, xf, _)| Target {
            entity,
            pos: xf.translation,
        })
        .collect();
    targets.sort_by(|a, b| rules::flat_dist(a.pos, hero).total_cmp(&rules::flat_dist(b.pos, hero)));
    let within = |r: f32| -> Vec<Target> {
        targets
            .iter()
            .copied()
            .take_while(|t| rules::flat_dist(t.pos, hero) <= r)
            .collect()
    };
    let rng = &mut rng.0;

    for &(id, level) in &run.loadout.0 {
        let s = id.level(level);
        let prev = timers.0.get(&id).copied().unwrap_or(0.0);
        let (next, ready) = rules::tick_cooldown(prev, dt, s.cooldown * stats.cooldown_mul);
        if !ready {
            timers.0.insert(id, next);
            continue;
        }
        // A skill with nothing to do waits, ready, instead of wasting its cast.
        let cast = match id {
            SkillId::Frostbolt | SkillId::Fireball | SkillId::WinterBolts | SkillId::Pyroblast => {
                let in_range = within(s.range);
                if in_range.is_empty() {
                    false
                } else {
                    for i in 0..s.count as usize {
                        // One bolt per nearest enemy; extras fan out around the nearest.
                        let (t, spread) = match in_range.get(i) {
                            Some(t) => (t, 0.0),
                            None => (&in_range[0], 0.25 * i as f32),
                        };
                        let dir = Quat::from_rotation_y(spread)
                            * (t.pos - hero).with_y(0.0).normalize_or(Vec3::NEG_Z);
                        spawn_projectile(&mut commands, &assets, id, s, hero, dir, None);
                    }
                    true
                }
            }
            SkillId::ArcaneMissiles | SkillId::ArcaneStorm => {
                let in_range = within(s.range);
                if in_range.is_empty() {
                    false
                } else {
                    for i in 0..s.count {
                        let t = in_range[rng.below(in_range.len())];
                        // Missiles leave in a fan, then curve in.
                        let a = std::f32::consts::TAU * i as f32 / s.count as f32;
                        let dir = Vec3::new(a.cos(), 0.0, a.sin());
                        spawn_projectile(&mut commands, &assets, id, s, hero, dir, Some(t.entity));
                    }
                    true
                }
            }
            SkillId::FrostNova | SkillId::ArcaneExplosion => {
                let hit = within(s.radius);
                if hit.is_empty() {
                    false
                } else {
                    for t in hit {
                        if let Ok((_, _, mut e)) = enemies.get_mut(t.entity) {
                            strike(&mut e, s.damage, &stats, rng);
                            if id == SkillId::FrostNova {
                                e.freeze(s.duration);
                            }
                        }
                    }
                    fx::burst(
                        &mut commands,
                        &mut budget,
                        &art,
                        &assets,
                        id,
                        BurstKind::Area,
                        hero,
                        s.radius,
                    );
                    true
                }
            }
            SkillId::Blizzard => {
                let in_range = within(s.range);
                if in_range.is_empty() {
                    false
                } else {
                    for _ in 0..s.count {
                        // On a random enemy in reach, a little off.
                        let t = in_range[rng.below(in_range.len())];
                        let off = Vec3::new(rng.range(-1.5, 1.5), 0.0, rng.range(-1.5, 1.5));
                        spawn_zone(&mut commands, &assets, id, s, t.pos + off);
                    }
                    true
                }
            }
            SkillId::Flamestrike => match within(s.range).first() {
                Some(t) => {
                    spawn_zone(&mut commands, &assets, id, s, t.pos);
                    true
                }
                None => false,
            },
            SkillId::Blink => {
                let near: Vec<Vec3> = within(s.radius).iter().map(|t| t.pos).collect();
                if rules::should_blink(health.fraction(), near.len() as u32, s.count) {
                    let facing = hero_xf.rotation * Vec3::NEG_Z;
                    // Away from what is on top of the hero, or from everything a bit further.
                    let threat: Vec<Vec3> = if near.is_empty() {
                        within(8.0).iter().map(|t| t.pos).collect()
                    } else {
                        near
                    };
                    let to = rules::blink_target(hero, &threat, s.range, facing);
                    fx::burst(
                        &mut commands,
                        &mut budget,
                        &art,
                        &assets,
                        id,
                        BurstKind::Area,
                        hero,
                        2.5,
                    );
                    hero_xf.translation = to.with_y(hero.y);
                    fx::burst(
                        &mut commands,
                        &mut budget,
                        &art,
                        &assets,
                        id,
                        BurstKind::Area,
                        hero_xf.translation,
                        2.5,
                    );
                    health.invulnerable = health.invulnerable.max(s.duration);
                    true
                } else {
                    false
                }
            }
        };
        timers.0.insert(id, if cast { next } else { 0.0 });
        // Missiles and areas sound as they are cast (novas sound in their burst).
        if cast
            && !matches!(
                id,
                SkillId::FrostNova | SkillId::ArcaneExplosion | SkillId::Blink
            )
        {
            budget.sound(art.fx.get(&id).and_then(|f| f.cast_sound));
        }
        // The hero's model casts with the spell, unless it is mid-cast already (Blink is instant).
        if cast && id != SkillId::Blink {
            if let Some(anim) = hero_anim.as_mut() {
                if anim.action().is_none() {
                    anim.play(AnimState::Cast);
                }
            }
        }
    }
}

/// A projectile of skill `id`; `target` makes it a homing missile.
fn spawn_projectile(
    commands: &mut Commands,
    assets: &Placeholders,
    id: SkillId,
    s: &SkillLevel,
    from: Vec3,
    dir: Vec3,
    target: Option<Entity>,
) {
    let (mesh, scale) = match id {
        SkillId::Frostbolt => (assets.bolt.clone(), 1.0),
        SkillId::WinterBolts => (assets.bolt.clone(), 1.4),
        SkillId::Fireball => (assets.ball.clone(), 1.3),
        SkillId::Pyroblast => (assets.ball.clone(), 2.2),
        SkillId::ArcaneStorm => (assets.ball.clone(), 0.65),
        _ => (assets.ball.clone(), 0.5),
    };
    let at = from + Vec3::Y * CAST_HEIGHT;
    let homing = target.is_some();
    let life = if homing { 3.0 } else { s.range / s.speed * 1.4 };
    commands
        .spawn((
            Name::new(id.def().name),
            Projectile {
                skill: id,
                dir,
                speed: s.speed,
                damage: s.damage,
                pierce: s.pierce,
                life,
                hit: Vec::new(),
                homing,
                target,
                on_hit: on_hit_for(id, s),
            },
            Transform::from_translation(at).looking_to(dir, Vec3::Y),
            Visibility::default(),
        ))
        .with_children(|c| {
            // The plain shape until the client's missile model is in (`fx`).
            c.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(assets.solid[&id].clone()),
                Transform::from_scale(Vec3::splat(scale)),
                ModelPlaceholder,
            ));
        });
}

fn spawn_zone(
    commands: &mut Commands,
    assets: &Placeholders,
    id: SkillId,
    s: &SkillLevel,
    at: Vec3,
) {
    spawn_zone_with(
        commands,
        assets,
        id,
        Zone {
            skill: id,
            damage: s.damage,
            radius: s.radius,
            tick: s.tick,
            next_tick: 0.0,
            life: s.duration,
            slow: s.slow,
        },
        at,
    );
}

fn spawn_zone_with(
    commands: &mut Commands,
    assets: &Placeholders,
    id: SkillId,
    zone: Zone,
    at: Vec3,
) {
    let r = zone.radius;
    commands
        .spawn((
            Name::new(id.def().name),
            zone,
            Transform::from_translation(at + Vec3::Y * 0.15),
            Visibility::default(),
        ))
        .with_children(|c| {
            // The plain disc until the client's area model is in (`fx`).
            c.spawn((
                Mesh3d(assets.disc.clone()),
                MeshMaterial3d(assets.glass[&id].clone()),
                Transform::from_scale(Vec3::new(r, 1.0, r)),
                ModelPlaceholder,
            ));
        });
}

/// Projectiles fly, steer (missiles), hit, pierce, slow, freeze or explode.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    save: Res<super::save::Save>,
    assets: Res<Placeholders>,
    art: Res<WowArt>,
    mut budget: ResMut<FxBudget>,
    mut rng: ResMut<CombatRng>,
    mut projs: Query<(Entity, &mut Transform, &mut Projectile), Without<Enemy>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Projectile>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let stats = Stats::from_parts(&run.passives, &save.data.bonus());
    let rng = &mut rng.0;
    for (pe, mut xf, mut p) in &mut projs {
        p.life -= dt;
        let mut pos = xf.translation;

        if p.homing {
            // Lost its target: the nearest live enemy instead.
            let target = p
                .target
                .filter(|t| enemies.get(*t).is_ok_and(|(_, _, e)| e.alive()))
                .or_else(|| {
                    enemies
                        .iter()
                        .filter(|(_, _, e)| e.alive())
                        .min_by(|a, b| {
                            rules::flat_dist(a.1.translation, pos)
                                .total_cmp(&rules::flat_dist(b.1.translation, pos))
                        })
                        .map(|(e, _, _)| e)
                });
            p.target = target;
            if let Some(Ok((_, txf, _))) = target.map(|t| enemies.get(t)) {
                let want = (txf.translation + Vec3::Y * CAST_HEIGHT - pos).normalize_or(p.dir);
                p.dir = p.dir.lerp(want, (6.0 * dt).min(1.0)).normalize_or(want);
            }
        }
        pos += p.dir * p.speed * dt;
        xf.translation = pos;
        xf.look_to(p.dir, Vec3::Y);

        // First enemy touched that it has not hit yet.
        let hit = enemies
            .iter()
            .filter(|(e, _, en)| en.alive() && !p.hit.contains(e))
            .find(|(_, exf, en)| rules::flat_dist(exf.translation, pos) <= en.radius + PROJ_RADIUS)
            .map(|(e, _, _)| e);

        let mut done = p.life <= 0.0;
        if let Some(target) = hit {
            match p.on_hit {
                OnHit::Explode { .. } => done = true,
                on_hit => {
                    if let Ok((_, exf, mut en)) = enemies.get_mut(target) {
                        fx::burst(
                            &mut commands,
                            &mut budget,
                            &art,
                            &assets,
                            p.skill,
                            BurstKind::Impact,
                            exf.translation + Vec3::Y * CAST_HEIGHT,
                            0.0,
                        );
                        strike(&mut en, p.damage, &stats, rng);
                        match on_hit {
                            OnHit::Chill(slow, secs) => en.chill(slow, secs),
                            OnHit::Freeze(slow, secs) => {
                                en.freeze(secs);
                                en.chill(slow, secs + 1.0);
                            }
                            _ => {}
                        }
                    }
                    p.hit.push(target);
                    if p.pierce == 0 {
                        done = true;
                    } else {
                        p.pierce -= 1;
                    }
                }
            }
        }
        if done {
            if let OnHit::Explode { radius, burn } = p.on_hit {
                for (_, exf, mut en) in &mut enemies {
                    let d = rules::flat_dist(exf.translation, pos);
                    let dmg = rules::blast_damage(p.damage, (d - en.radius).max(0.0), radius);
                    if dmg > 0.0 && en.alive() {
                        strike(&mut en, dmg, &stats, rng);
                    }
                }
                let ground = pos - Vec3::Y * CAST_HEIGHT;
                fx::burst(
                    &mut commands,
                    &mut budget,
                    &art,
                    &assets,
                    p.skill,
                    BurstKind::Impact,
                    pos,
                    radius,
                );
                if art
                    .fx
                    .get(&p.skill)
                    .and_then(|f| f.impact.as_ref())
                    .is_none()
                {
                    spawn_flash(&mut commands, &assets, p.skill, ground, radius);
                }
                if let Some(b) = burn {
                    spawn_zone_with(
                        &mut commands,
                        &assets,
                        SkillId::Flamestrike,
                        Zone {
                            skill: p.skill,
                            damage: b.damage,
                            radius: b.radius,
                            tick: b.tick,
                            next_tick: b.tick,
                            life: b.secs,
                            slow: 0.0,
                        },
                        ground,
                    );
                }
            }
            commands.entity(pe).try_despawn();
        }
    }
}

/// Ground areas tick their damage and slow, then fade.
fn tick_zones(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    save: Res<super::save::Save>,
    mut rng: ResMut<CombatRng>,
    mut zones: Query<(Entity, &Transform, &mut Zone), Without<Enemy>>,
    mut enemies: Query<(&Transform, &mut Enemy), Without<Zone>>,
) {
    let dt = time.delta_secs();
    let stats = Stats::from_parts(&run.passives, &save.data.bonus());
    let rng = &mut rng.0;
    for (ze, zxf, mut z) in &mut zones {
        z.life -= dt;
        z.next_tick -= dt;
        if z.next_tick <= 0.0 {
            z.next_tick += z.tick;
            for (exf, mut e) in &mut enemies {
                if e.alive()
                    && rules::flat_dist(exf.translation, zxf.translation) <= z.radius + e.radius
                {
                    strike(&mut e, z.damage, &stats, rng);
                    if z.slow > 0.0 {
                        e.chill(z.slow, z.tick + 0.3);
                    }
                }
            }
        }
        if z.life <= 0.0 {
            commands.entity(ze).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evolutions_hit_harder_than_their_parents() {
        assert_eq!(
            on_hit_for(SkillId::WinterBolts, SkillId::WinterBolts.level(1)),
            OnHit::Freeze(0.5, 0.9)
        );
        match on_hit_for(SkillId::Pyroblast, SkillId::Pyroblast.level(1)) {
            OnHit::Explode { burn: Some(b), .. } => assert!(b.damage > 0.0 && b.secs > 0.0),
            other => panic!("{other:?}"),
        }
        for (base, _, into) in crate::content::skills::EVOLUTIONS {
            let top = base.level(base.max_level());
            let evo = into.level(1);
            assert!(
                evo.damage * evo.count as f32 > top.damage * top.count as f32,
                "{into:?}"
            );
        }
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
        init(&mut world, cast_skills);
        init(&mut world, move_projectiles);
        init(&mut world, tick_zones);
    }
}
