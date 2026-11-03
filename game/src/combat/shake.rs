//! Screen shake: the camera trembles when the hero is hurt and, lightly, on critical hits. The
//! shake grows with "trauma" (0–1) squared and fades on its own. `sarsinti=0` in the settings
//! file turns it off, any value from 0 to 1 scales it.

use bevy::prelude::*;

use benilla_world::schedule::WorldStage;

use super::hero::HeroHealth;
use super::CombatSet;
use crate::game::FollowCam;

pub struct ShakePlugin;

impl Plugin for ShakePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Shake {
            trauma: 0.0,
            scale: shake_setting(),
        })
        .add_systems(
            Update,
            // After the follow camera placed itself (WorldStage::Input), before the world draws.
            shake_camera
                .after(WorldStage::Input)
                .before(CombatSet::Spawn),
        );
    }
}

/// How shaken the camera is.
#[derive(Resource, Debug)]
pub struct Shake {
    trauma: f32,
    /// The player's setting (0 = off).
    scale: f32,
}

impl Shake {
    /// Adds trauma, kept within 0–1.
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount.max(0.0)).min(1.0);
    }
}

/// Trauma lost per second.
const DECAY: f32 = 1.8;
/// The farthest the camera moves at full trauma (yards).
const MAX_OFFSET: f32 = 0.35;
/// Trauma from a hit on the hero, per share of its health lost.
const HURT_TRAUMA: f32 = 3.0;

fn shake_setting() -> f32 {
    std::fs::read_to_string(crate::settings::settings_path())
        .ok()
        .and_then(|t| crate::settings::setting(&t, "sarsinti"))
        .and_then(|v| v.replace(',', ".").parse::<f32>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(1.0)
}

/// The camera's offset at `t` seconds for `trauma`: smooth wobble on three axes.
pub fn shake_offset(trauma: f32, t: f32) -> Vec3 {
    let k = trauma.clamp(0.0, 1.0).powi(2) * MAX_OFFSET;
    Vec3::new(
        (t * 37.0).sin() + 0.5 * (t * 61.0 + 1.7).sin(),
        (t * 43.0 + 0.9).sin() + 0.5 * (t * 53.0).sin(),
        (t * 29.0 + 2.3).sin() + 0.5 * (t * 71.0 + 0.4).sin(),
    ) / 1.5
        * k
}

fn shake_camera(
    time: Res<Time>,
    health: Res<HeroHealth>,
    mut shake: ResMut<Shake>,
    mut last: Local<Option<f32>>,
    mut cams: Query<&mut Transform, With<FollowCam>>,
) {
    // A hit on the hero (a heal or a new run raises health and shakes nothing).
    if let Some(prev) = last.replace(health.current) {
        let lost = prev - health.current;
        if lost > 0.0 && health.max > 0.0 {
            shake.add(lost / health.max * HURT_TRAUMA);
        }
    }
    let dt = time.delta_secs();
    if dt <= 0.0 || shake.trauma <= 0.0 {
        return;
    }
    shake.trauma = (shake.trauma - DECAY * dt).max(0.0);
    if shake.scale <= 0.0 {
        return;
    }
    if let Ok(mut xf) = cams.single_mut() {
        xf.translation += shake_offset(shake.trauma, time.elapsed_secs()) * shake.scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shake_grows_with_trauma_and_stays_bounded() {
        assert_eq!(shake_offset(0.0, 1.0), Vec3::ZERO);
        for i in 0..200 {
            let t = i as f32 * 0.037;
            assert!(shake_offset(1.0, t).length() <= MAX_OFFSET * 1.8);
        }
        let small = shake_offset(0.3, 0.5).length();
        let big = shake_offset(0.9, 0.5).length();
        assert!(big > small);
        let mut s = Shake {
            trauma: 0.9,
            scale: 1.0,
        };
        s.add(0.5);
        assert_eq!(s.trauma, 1.0);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(shake_camera).initialize(&mut world);
    }
}
