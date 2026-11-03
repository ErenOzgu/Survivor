//! XP orbs: dropped by enemies, pulled in near the hero, turned into levels.

use bevy::prelude::*;

use super::rules::{self, ORB_MAGNET};
use super::visuals::Placeholders;
use super::{CombatSet, Run};
use crate::game::Hero;

pub struct XpPlugin;

impl Plugin for XpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (collect_orbs, merge_orbs)
                .chain()
                .in_set(CombatSet::Resolve)
                .run_if(super::hero_on_ground),
        );
    }
}

#[derive(Component)]
pub struct Orb {
    pub xp: u32,
}

/// Bigger orbs for bigger XP.
pub fn orb_scale(xp: u32) -> f32 {
    (0.8 + 0.25 * xp as f32).min(2.5)
}

/// The most orbs on the map: past it the farthest from the hero merge into the nearest kept
/// orb (their XP adds up), so every orb keeps its model and the map stays readable.
pub const ORB_CAP: usize = 80;

/// Which orbs merge into which: the `cap` nearest the hero stay, each other one goes into the
/// staying orb nearest to it. Returns `(from, into)` indices.
pub fn plan_merge(orbs: &[Vec3], hero: Vec3, cap: usize) -> Vec<(usize, usize)> {
    if orbs.len() <= cap || cap == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..orbs.len()).collect();
    order.sort_by(|a, b| {
        rules::flat_dist(orbs[*a], hero).total_cmp(&rules::flat_dist(orbs[*b], hero))
    });
    let (kept, extra) = order.split_at(cap);
    extra
        .iter()
        .map(|&from| {
            let into = kept
                .iter()
                .copied()
                .min_by(|a, b| {
                    rules::flat_dist(orbs[*a], orbs[from])
                        .total_cmp(&rules::flat_dist(orbs[*b], orbs[from]))
                })
                .expect("cap > 0");
            (from, into)
        })
        .collect()
}

/// Once a second, merges the orbs past [`ORB_CAP`].
fn merge_orbs(
    mut commands: Commands,
    time: Res<Time>,
    mut every: Local<f32>,
    heroes: Query<&Transform, (With<Hero>, Without<Orb>)>,
    mut orbs: Query<(Entity, &mut Transform, &mut Orb)>,
) {
    *every -= time.delta_secs();
    if *every > 0.0 {
        return;
    }
    *every = 1.0;
    let Ok(hero) = heroes.single() else {
        return;
    };
    let list: Vec<(Entity, Vec3, u32)> = orbs
        .iter()
        .map(|(e, xf, o)| (e, xf.translation, o.xp))
        .collect();
    let at: Vec<Vec3> = list.iter().map(|o| o.1).collect();
    let plan = plan_merge(&at, hero.translation, ORB_CAP);
    if plan.is_empty() {
        return;
    }
    let mut add: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    for (from, into) in &plan {
        *add.entry(*into).or_default() += list[*from].2;
        commands.entity(list[*from].0).try_despawn();
    }
    for (into, xp) in add {
        if let Ok((_, mut xf, mut orb)) = orbs.get_mut(list[into].0) {
            orb.xp += xp;
            xf.scale = Vec3::splat(orb_scale(orb.xp));
        }
    }
}

pub fn spawn_orb(commands: &mut Commands, assets: &Placeholders, at: Vec3, xp: u32) {
    let scale = orb_scale(xp);
    commands
        .spawn((
            Name::new("xp orb"),
            Orb { xp },
            Transform::from_translation(at + Vec3::Y * 0.4).with_scale(Vec3::splat(scale)),
            Visibility::default(),
        ))
        .with_children(|c| {
            // A glowing green sphere that always shows; the client's missile model (`fx`) only
            // adds its sparkle around it (on its own it can be too faint to see).
            c.spawn((
                Mesh3d(assets.orb.0.clone()),
                MeshMaterial3d(assets.orb.1.clone()),
                Transform::default(),
            ));
        });
}

fn collect_orbs(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    save: Res<super::save::Save>,
    heroes: Query<&Transform, (With<Hero>, Without<Orb>)>,
    mut orbs: Query<(Entity, &mut Transform, &Orb)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let dt = time.delta_secs();
    let stats = rules::Stats::from_parts(&run.passives, &save.data.bonus());
    let magnet = ORB_MAGNET * stats.magnet_mul;
    let mut gained = 0;
    for (e, mut xf, orb) in &mut orbs {
        let (pos, taken) = rules::orb_step(xf.translation, hero.translation, magnet, dt);
        if taken {
            gained += rules::orb_xp(orb.xp, &stats);
            commands.entity(e).try_despawn();
        } else {
            // Lift toward the hero's chest as it flies in.
            xf.translation = pos.with_y(if pos != xf.translation {
                xf.translation.y.max(hero.translation.y + 0.6)
            } else {
                xf.translation.y
            });
        }
    }
    if gained > 0 {
        run.xp_total += gained;
        let (level, xp, ups) = rules::add_xp(run.level, run.xp, gained);
        run.level = level;
        run.xp = xp;
        run.pending_cards += ups;
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn far_orbs_merge_into_the_nearest_kept_one() {
        let hero = Vec3::ZERO;
        let orbs = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(60.0, 0.0, 0.0),
        ];
        assert!(plan_merge(&orbs, hero, 4).is_empty());
        let plan = plan_merge(&orbs, hero, 2);
        assert_eq!(plan.len(), 2);
        // Both far ones go into the kept orb at x = 2, the nearer of the two kept to them.
        assert!(plan.iter().all(|(_, into)| *into == 2));
        assert!(orb_scale(1000) <= 2.5);
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
        init(&mut world, collect_orbs);
        init(&mut world, merge_orbs);
    }
}
