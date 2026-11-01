//! Pure movement and camera math, kept free of ECS so it can be unit-tested.
//!
//! Space is Bevy's: `+Y` up, the ground is the XZ plane. A yaw of 0 looks along `-Z`, which is
//! WoW north (see `benilla_assets::coords::wow_to_bevy`).

use bevy::math::{EulerRot, Quat, Vec2, Vec3};

/// Hero run speed in yards per second (a WoW character's base run speed).
pub const HERO_SPEED: f32 = 7.0;

/// Camera defaults: distance from the look-at point, and pitch (negative looks down).
pub const CAM_DISTANCE: f32 = 28.0;
pub const CAM_MIN_DISTANCE: f32 = 8.0;
pub const CAM_MAX_DISTANCE: f32 = 90.0;
pub const CAM_PITCH: f32 = -0.95;
/// How high above the hero's feet the camera looks, in yards.
pub const CAM_LOOK_HEIGHT: f32 = 1.2;
/// Zoom step per wheel line, as a fraction of the current distance.
pub const ZOOM_STEP: f32 = 0.1;

/// Turns a WASD input (`x` right, `y` forward, each in `-1..=1`) into a unit direction on the
/// ground plane, relative to a camera looking along `cam_yaw`. Zero input gives zero.
pub fn move_dir(input: Vec2, cam_yaw: f32) -> Vec3 {
    if input == Vec2::ZERO {
        return Vec3::ZERO;
    }
    let rot = Quat::from_rotation_y(cam_yaw);
    let forward = rot * Vec3::NEG_Z;
    let right = rot * Vec3::X;
    (forward * input.y + right * input.x)
        .with_y(0.0)
        .normalize_or_zero()
}

/// One frame's horizontal step: `pos` moved along `dir` at `speed` for `dt` seconds.
pub fn step(pos: Vec3, dir: Vec3, speed: f32, dt: f32) -> Vec3 {
    pos + dir * speed * dt
}

/// The yaw (around `+Y`) that makes a model whose front is `-Z` face along `dir`; `None` for no
/// horizontal direction.
pub fn facing_yaw(dir: Vec3) -> Option<f32> {
    let flat = Vec2::new(dir.x, dir.z);
    if flat.length_squared() < 1e-8 {
        return None;
    }
    Some((-dir.x).atan2(-dir.z))
}

/// The hero's height after following the ground: on terrain it stands on it; with no terrain
/// under it (not streamed yet, off the map, a hole) it keeps its height. Returns the new `y` and
/// whether it is on the ground.
pub fn follow_ground(current_y: f32, ground: Option<f32>) -> (f32, bool) {
    match ground {
        Some(g) if g.is_finite() => (g, true),
        _ => (current_y, false),
    }
}

/// The camera's offset from its look-at point for a yaw, pitch and distance: it sits behind and
/// above, looking at the point.
pub fn camera_offset(yaw: f32, pitch: f32, distance: f32) -> Vec3 {
    let rot = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    // The camera looks along its `-Z`; it sits `distance` back along `+Z` from the target.
    rot * Vec3::Z * distance
}

/// The new camera distance after `lines` wheel lines (positive = zoom in), clamped.
pub fn zoom(distance: f32, lines: f32) -> f32 {
    (distance * (1.0 - lines * ZOOM_STEP)).clamp(CAM_MIN_DISTANCE, CAM_MAX_DISTANCE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-5
    }

    #[test]
    fn no_input_no_motion() {
        assert_eq!(move_dir(Vec2::ZERO, 1.0), Vec3::ZERO);
    }

    #[test]
    fn forward_is_camera_forward() {
        assert!(close(move_dir(Vec2::Y, 0.0), Vec3::NEG_Z));
        assert!(close(move_dir(Vec2::X, 0.0), Vec3::X));
        // Turned a quarter left, forward points along -X.
        assert!(close(move_dir(Vec2::Y, FRAC_PI_2), Vec3::NEG_X));
    }

    #[test]
    fn diagonals_are_not_faster() {
        let d = move_dir(Vec2::new(1.0, 1.0), 0.3);
        assert!((d.length() - 1.0).abs() < 1e-5);
        assert_eq!(d.y, 0.0);
    }

    #[test]
    fn step_moves_speed_times_dt() {
        let p = step(Vec3::new(1.0, 5.0, 1.0), Vec3::NEG_Z, HERO_SPEED, 0.5);
        assert!(close(p, Vec3::new(1.0, 5.0, 1.0 - 3.5)));
    }

    #[test]
    fn facing_matches_direction() {
        for dir in [Vec3::NEG_Z, Vec3::X, Vec3::new(-1.0, 0.0, 1.0).normalize()] {
            let yaw = facing_yaw(dir).unwrap();
            assert!(close(Quat::from_rotation_y(yaw) * Vec3::NEG_Z, dir));
        }
        assert_eq!(facing_yaw(Vec3::Y), None);
    }

    #[test]
    fn ground_wins_when_present() {
        assert_eq!(follow_ground(100.0, Some(42.5)), (42.5, true));
        assert_eq!(follow_ground(100.0, None), (100.0, false));
        assert_eq!(follow_ground(100.0, Some(f32::NAN)), (100.0, false));
    }

    #[test]
    fn camera_sits_behind_and_above() {
        let off = camera_offset(0.0, CAM_PITCH, CAM_DISTANCE);
        assert!((off.length() - CAM_DISTANCE).abs() < 1e-4);
        assert!(off.y > 0.0, "above");
        assert!(off.z > 0.0, "behind a camera looking north (-Z)");
        assert!(off.x.abs() < 1e-5);
        // Looking from the offset back at the target is the camera's forward.
        let rot = Quat::from_euler(EulerRot::YXZ, 0.0, CAM_PITCH, 0.0);
        assert!(close(-off.normalize(), rot * Vec3::NEG_Z));
    }

    #[test]
    fn zoom_clamps() {
        assert!(zoom(CAM_DISTANCE, 1.0) < CAM_DISTANCE);
        assert!(zoom(CAM_DISTANCE, -1.0) > CAM_DISTANCE);
        assert_eq!(zoom(CAM_MIN_DISTANCE, 100.0), CAM_MIN_DISTANCE);
        assert_eq!(zoom(CAM_MAX_DISTANCE, -100.0), CAM_MAX_DISTANCE);
    }
}
