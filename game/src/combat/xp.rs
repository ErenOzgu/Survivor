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
            (collect_orbs, merge_orbs, animate_orbs)
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

/// The crystal's colour per value tier: small green, middling blue, big (merged) violet.
pub const ORB_TIER_COLORS: [[f32; 3]; 3] = [[0.3, 1.0, 0.45], [0.3, 0.65, 1.0], [0.85, 0.4, 1.0]];

/// An orb's value tier.
pub fn orb_tier(xp: u32) -> usize {
    match xp {
        0..=2 => 0,
        3..=9 => 1,
        _ => 2,
    }
}

/// Bigger crystals for bigger XP, by tier and a little within it.
pub fn orb_scale(xp: u32) -> f32 {
    let base = [0.9, 1.25, 1.65][orb_tier(xp)];
    (base + 0.02 * xp as f32).min(2.2)
}

/// A crystal: two four-sided pyramids point to point (an octahedron, taller than wide), flat
/// faced so it catches the light.
pub fn gem_mesh() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let (w, h) = (0.28, 0.48);
    let tip = [Vec3::Y * h, Vec3::NEG_Y * h];
    let ring = [
        Vec3::new(w, 0.0, 0.0),
        Vec3::new(0.0, 0.0, w),
        Vec3::new(-w, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -w),
    ];
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    for (t, up) in [(tip[0], true), (tip[1], false)] {
        for i in 0..4 {
            let (a, b) = (ring[i], ring[(i + 1) % 4]);
            let (a, b) = if up { (b, a) } else { (a, b) };
            let n = (a - t).cross(b - t).normalize();
            for p in [t, a, b] {
                pos.push(p.to_array());
                nor.push(n.to_array());
            }
        }
    }
    let n = pos.len() as u32;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
    .with_inserted_indices(Indices::U32((0..n).collect()))
}

/// A soft round glow (white, fading out), tinted per tier for the light under an orb.
pub fn glow_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let dx = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let t = (1.0 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            let a = (t * t * 255.0) as u8;
            data.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// An orb's crystal and its glow, which turn and bob.
#[derive(Component)]
pub struct OrbGem {
    phase: f32,
}
#[derive(Component)]
pub struct OrbGlow;

/// The crystals spin and float; a merged orb's crystal takes its new tier's colour.
#[allow(clippy::type_complexity)]
fn animate_orbs(
    time: Res<Time<Real>>,
    assets: Res<Placeholders>,
    orbs: Query<(Ref<Orb>, &Children)>,
    mut gems: Query<
        (
            &OrbGem,
            &mut Transform,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<OrbGlow>,
    >,
    mut glows: Query<&mut MeshMaterial3d<StandardMaterial>, (With<OrbGlow>, Without<OrbGem>)>,
) {
    let t = time.elapsed_secs();
    for (orb, children) in &orbs {
        let tier = orb_tier(orb.xp);
        for c in children.iter() {
            if let Ok((g, mut xf, mut mat)) = gems.get_mut(c) {
                xf.rotation = Quat::from_rotation_y(t * 1.6 + g.phase);
                xf.translation.y = 0.25 + 0.12 * (t * 2.4 + g.phase).sin();
                if orb.is_changed() {
                    mat.0 = assets.orb_tiers[tier].clone();
                }
            } else if orb.is_changed() {
                if let Ok(mut mat) = glows.get_mut(c) {
                    mat.0 = assets.orb_glow.1[tier].clone();
                }
            }
        }
    }
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
            // A turning crystal in its tier's colour, and a soft light on the ground under it.
            let tier = orb_tier(xp);
            c.spawn((
                Mesh3d(assets.orb.0.clone()),
                MeshMaterial3d(assets.orb_tiers[tier].clone()),
                Transform::default(),
                OrbGem {
                    phase: (at.x * 3.1 + at.z * 1.7).rem_euclid(std::f32::consts::TAU),
                },
            ));
            c.spawn((
                Mesh3d(assets.orb_glow.0.clone()),
                MeshMaterial3d(assets.orb_glow.1[tier].clone()),
                Transform::from_xyz(0.0, -0.33, 0.0).with_scale(Vec3::splat(1.9)),
                OrbGlow,
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
        assert_eq!((orb_tier(1), orb_tier(5), orb_tier(40)), (0, 1, 2));
        assert!(orb_scale(40) > orb_scale(1));
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
        init(&mut world, animate_orbs);
    }
}
