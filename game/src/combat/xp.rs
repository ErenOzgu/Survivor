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
            collect_orbs
                .in_set(CombatSet::Resolve)
                .run_if(super::hero_on_ground),
        );
    }
}

#[derive(Component)]
pub struct Orb {
    pub xp: u32,
}

pub fn spawn_orb(commands: &mut Commands, assets: &Placeholders, at: Vec3, xp: u32) {
    // Bigger orbs for bigger XP.
    let scale = (0.8 + 0.25 * xp as f32).min(2.5);
    commands.spawn((
        Name::new("xp orb"),
        Orb { xp },
        Mesh3d(assets.orb.0.clone()),
        MeshMaterial3d(assets.orb.1.clone()),
        Transform::from_translation(at + Vec3::Y * 0.4).with_scale(Vec3::splat(scale)),
    ));
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
    }
}
