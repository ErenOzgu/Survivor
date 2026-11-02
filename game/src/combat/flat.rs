//! `RNS_FLAT=1`: a test mode for machines without WoW data. Where no terrain is streamed, the
//! hero stands on a flat plane at the start height, so the whole combat loop runs (with
//! placeholder looks). A green floor with scattered markers is drawn so movement shows.
//! Real terrain, when present, still wins.

use bevy::prelude::*;

use benilla_assets::coords::wow_to_bevy;

use super::CombatSet;
use crate::game::{Hero, Start};

pub const FLAT_VAR: &str = "RNS_FLAT";

pub struct FlatPlugin;

impl Plugin for FlatPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var(FLAT_VAR).as_deref() != Ok("1") {
            return;
        }
        warn!("rnsurvivors: {FLAT_VAR}=1 — flat test ground where there is no terrain");
        app.add_systems(Startup, spawn_floor).add_systems(
            Update,
            // After the hero's own ground check (WorldStage::Input), before any combat system.
            stand_on_flat
                .after(benilla_world::schedule::WorldStage::Input)
                .before(CombatSet::Spawn),
        );
    }
}

/// Half the floor's size, yards.
const FLOOR_HALF: f32 = 400.0;

/// The flat ground height: the start's `z`.
fn flat_y(start: &Start) -> f32 {
    wow_to_bevy(start.0.wow()).y
}

/// A hero the terrain did not ground stands on the flat plane.
fn stand_on_flat(start: Res<Start>, mut heroes: Query<(&mut Transform, &mut Hero)>) {
    let Ok((mut xf, mut hero)) = heroes.single_mut() else {
        return;
    };
    if !hero.grounded {
        xf.translation.y = flat_y(&start);
        hero.grounded = true;
    }
}

/// Marker positions on a grid with a deterministic jitter, so the floor shows motion.
pub fn marker_spots(half: f32, step: f32) -> Vec<Vec2> {
    let n = (2.0 * half / step) as i32;
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            let h = ((i * 73_856_093) ^ (j * 19_349_663)).unsigned_abs();
            let jx = (h % 1000) as f32 / 1000.0 - 0.5;
            let jz = ((h / 1000) % 1000) as f32 / 1000.0 - 0.5;
            out.push(Vec2::new(
                -half + i as f32 * step + jx * step * 0.8,
                -half + j as f32 * step + jz * step * 0.8,
            ));
        }
    }
    out
}

fn spawn_floor(
    mut commands: Commands,
    start: Res<Start>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let center = wow_to_bevy(start.0.wow()).with_y(flat_y(&start));
    let floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.38, 0.18),
        unlit: true,
        ..default()
    });
    let marker = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.42, 0.33),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Name::new("flat floor"),
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(2.0 * FLOOR_HALF, 2.0 * FLOOR_HALF),
            ),
        ),
        MeshMaterial3d(floor),
        Transform::from_translation(center - Vec3::Y * 0.02),
    ));
    let stone = meshes.add(Cuboid::new(0.6, 0.25, 0.6));
    for p in marker_spots(FLOOR_HALF, 8.0) {
        commands.spawn((
            Mesh3d(stone.clone()),
            MeshMaterial3d(marker.clone()),
            Transform::from_translation(center + Vec3::new(p.x, 0.1, p.y)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_cover_the_floor() {
        let spots = marker_spots(40.0, 8.0);
        assert_eq!(spots.len(), 11 * 11);
        assert!(spots
            .iter()
            .all(|p| p.x.abs() <= 40.0 + 3.3 && p.y.abs() <= 40.0 + 3.3));
        assert_eq!(spots, marker_spots(40.0, 8.0), "deterministic");
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(stand_on_flat).initialize(&mut world);
        IntoSystem::into_system(spawn_floor).initialize(&mut world);
    }
}
