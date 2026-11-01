//! Auto-casting: each owned skill fires on its own cooldown, and the things it leaves behind
//! (projectiles, homing missiles, ground areas, pulses, Blink's jump).

use std::collections::HashMap;

use bevy::prelude::*;

use super::enemies::Enemy;
use super::hero::HeroHealth;
use super::rules;
use super::visuals::{spawn_flash, Placeholders};
use super::{hero_on_ground, CombatRng, CombatSet, Run};
use crate::content::{SkillId, SkillLevel};
use crate::game::Hero;

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
    pub effect: ProjEffect,
}

#[derive(Clone, Copy)]
pub enum ProjEffect {
    /// Slows what it hits: fraction, seconds.
    Chill(f32, f32),
    /// Explodes on its first hit (or when it runs out), radius.
    Explode(f32),
    /// Steers toward this enemy.
    Home(Option<Entity>),
}

/// A ground area that ticks damage (and maybe a slow) on what stands in it.
#[derive(Component)]
pub struct Zone {
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

#[allow(clippy::too_many_arguments)]
fn cast_skills(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    assets: Res<Placeholders>,
    mut timers: ResMut<SkillTimers>,
    mut rng: ResMut<CombatRng>,
    mut health: ResMut<HeroHealth>,
    mut heroes: Query<&mut Transform, (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Hero>>,
) {
    let Ok(mut hero_xf) = heroes.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
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
        let (next, ready) = rules::tick_cooldown(prev, dt, s.cooldown);
        if !ready {
            timers.0.insert(id, next);
            continue;
        }
        // A skill with nothing to do waits, ready, instead of wasting its cast.
        let cast = match id {
            SkillId::Frostbolt | SkillId::Fireball => {
                let in_range = within(s.range);
                if in_range.is_empty() {
                    false
                } else {
                    let effect = if id == SkillId::Frostbolt {
                        ProjEffect::Chill(s.slow, s.duration)
                    } else {
                        ProjEffect::Explode(s.radius)
                    };
                    for i in 0..s.count as usize {
                        // One bolt per nearest enemy; extras fan out around the nearest.
                        let (t, spread) = match in_range.get(i) {
                            Some(t) => (t, 0.0),
                            None => (&in_range[0], 0.25 * i as f32),
                        };
                        let dir = Quat::from_rotation_y(spread)
                            * (t.pos - hero).with_y(0.0).normalize_or(Vec3::NEG_Z);
                        spawn_projectile(&mut commands, &assets, id, s, hero, dir, effect);
                    }
                    true
                }
            }
            SkillId::ArcaneMissiles => {
                let in_range = within(s.range);
                if in_range.is_empty() {
                    false
                } else {
                    for i in 0..s.count {
                        let t = in_range[rng.below(in_range.len())];
                        // Missiles leave in a fan, then curve in.
                        let a = std::f32::consts::TAU * i as f32 / s.count as f32;
                        let dir = Vec3::new(a.cos(), 0.0, a.sin());
                        spawn_projectile(
                            &mut commands,
                            &assets,
                            id,
                            s,
                            hero,
                            dir,
                            ProjEffect::Home(Some(t.entity)),
                        );
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
                            e.hurt(s.damage);
                            if id == SkillId::FrostNova {
                                e.freeze(s.duration);
                            }
                        }
                    }
                    spawn_flash(&mut commands, &assets, id, hero, s.radius);
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
                    spawn_flash(&mut commands, &assets, id, hero, 1.5);
                    hero_xf.translation = to.with_y(hero.y);
                    spawn_flash(&mut commands, &assets, id, hero_xf.translation, 1.5);
                    health.invulnerable = health.invulnerable.max(s.duration);
                    true
                } else {
                    false
                }
            }
        };
        timers.0.insert(id, if cast { next } else { 0.0 });
    }
}

fn spawn_projectile(
    commands: &mut Commands,
    assets: &Placeholders,
    id: SkillId,
    s: &SkillLevel,
    from: Vec3,
    dir: Vec3,
    effect: ProjEffect,
) {
    let (mesh, scale) = match id {
        SkillId::Frostbolt => (assets.bolt.clone(), 1.0),
        SkillId::Fireball => (assets.ball.clone(), 1.3),
        _ => (assets.ball.clone(), 0.5),
    };
    let at = from + Vec3::Y * CAST_HEIGHT;
    let life = match effect {
        ProjEffect::Home(_) => 3.0,
        _ => s.range / s.speed * 1.4,
    };
    commands.spawn((
        Name::new(id.def().name),
        Projectile {
            skill: id,
            dir,
            speed: s.speed,
            damage: s.damage,
            pierce: s.pierce,
            life,
            hit: Vec::new(),
            effect,
        },
        Mesh3d(mesh),
        MeshMaterial3d(assets.solid[&id].clone()),
        Transform::from_translation(at)
            .looking_to(dir, Vec3::Y)
            .with_scale(Vec3::splat(scale)),
    ));
}

fn spawn_zone(
    commands: &mut Commands,
    assets: &Placeholders,
    id: SkillId,
    s: &SkillLevel,
    at: Vec3,
) {
    commands.spawn((
        Name::new(id.def().name),
        Zone {
            damage: s.damage,
            radius: s.radius,
            tick: s.tick,
            next_tick: 0.0,
            life: s.duration,
            slow: s.slow,
        },
        Mesh3d(assets.disc.clone()),
        MeshMaterial3d(assets.glass[&id].clone()),
        Transform::from_translation(at + Vec3::Y * 0.15)
            .with_scale(Vec3::new(s.radius, 1.0, s.radius)),
    ));
}

/// Projectiles fly, steer (missiles), hit, pierce, slow or explode.
fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<Placeholders>,
    mut projs: Query<(Entity, &mut Transform, &mut Projectile), Without<Enemy>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Projectile>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (pe, mut xf, mut p) in &mut projs {
        p.life -= dt;
        let mut pos = xf.translation;

        if let ProjEffect::Home(target) = p.effect {
            // Lost its target: the nearest live enemy instead.
            let target = target
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
            p.effect = ProjEffect::Home(target);
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
            match p.effect {
                ProjEffect::Explode(_) => done = true,
                _ => {
                    if let Ok((_, _, mut en)) = enemies.get_mut(target) {
                        en.hurt(p.damage);
                        if let ProjEffect::Chill(slow, secs) = p.effect {
                            en.chill(slow, secs);
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
            if let ProjEffect::Explode(radius) = p.effect {
                for (_, exf, mut en) in &mut enemies {
                    let d = rules::flat_dist(exf.translation, pos);
                    let dmg = rules::blast_damage(p.damage, (d - en.radius).max(0.0), radius);
                    if dmg > 0.0 {
                        en.hurt(dmg);
                    }
                }
                spawn_flash(
                    &mut commands,
                    &assets,
                    p.skill,
                    pos - Vec3::Y * CAST_HEIGHT,
                    radius,
                );
            }
            commands.entity(pe).try_despawn();
        }
    }
}

/// Ground areas tick their damage and slow, then fade.
fn tick_zones(
    mut commands: Commands,
    time: Res<Time>,
    mut zones: Query<(Entity, &Transform, &mut Zone), Without<Enemy>>,
    mut enemies: Query<(&Transform, &mut Enemy), Without<Zone>>,
) {
    let dt = time.delta_secs();
    for (ze, zxf, mut z) in &mut zones {
        z.life -= dt;
        z.next_tick -= dt;
        if z.next_tick <= 0.0 {
            z.next_tick += z.tick;
            for (exf, mut e) in &mut enemies {
                if e.alive()
                    && rules::flat_dist(exf.translation, zxf.translation) <= z.radius + e.radius
                {
                    e.hurt(z.damage);
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
