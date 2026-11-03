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
#[cfg(test)]
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

/// The hero's acceleration toward the speed and direction asked (yards per second²): a turn of
/// the keys blends into the new direction instead of snapping between the eight.
pub const HERO_ACCEL: f32 = 45.0;
/// How fast the hero's body turns toward where it goes (radians per second).
pub const HERO_TURN: f32 = 11.0;

/// Walking backwards (S without W) is this much of the run speed, as in WoW.
pub const BACKPEDAL_MUL: f32 = 0.6;

/// Whether the keys walk backwards: back held, forward not.
pub fn backpedals(input: Vec2) -> bool {
    input.y < 0.0
}

/// A jump's take-off speed (yards per second) and the pull back down: about 1.5 yards high,
/// some 0.85 seconds in the air, a soft arc the take-off and landing clips fit.
pub const JUMP_SPEED: f32 = 7.4;
pub const GRAVITY: f32 = 18.0;

/// One frame of a jump: the height over the ground and the upward speed after `dt`; both end at 0
/// on landing.
pub fn jump_step(air: f32, rise: f32, dt: f32) -> (f32, f32) {
    if air <= 0.0 && rise <= 0.0 {
        return (0.0, 0.0);
    }
    let rise = rise - GRAVITY * dt;
    let air = air + rise * dt;
    if air <= 0.0 {
        (0.0, 0.0)
    } else {
        (air, rise)
    }
}

/// `vel` moved toward `target` by at most `accel * dt`.
pub fn approach(vel: Vec3, target: Vec3, accel: f32, dt: f32) -> Vec3 {
    let d = target - vel;
    let step = accel * dt;
    if d.length() <= step {
        target
    } else {
        vel + d.normalize() * step
    }
}

/// `from` turned toward `to` (radians) by at most `max`, the short way round.
pub fn turn_toward(from: f32, to: f32, max: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut d = (to - from) % tau;
    if d > std::f32::consts::PI {
        d -= tau;
    } else if d < -std::f32::consts::PI {
        d += tau;
    }
    from + d.clamp(-max, max)
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

    #[test]
    fn a_jump_goes_up_and_lands() {
        let (mut air, mut rise) = (0.0, JUMP_SPEED);
        let mut peak: f32 = 0.0;
        let mut frames = 0;
        loop {
            (air, rise) = jump_step(air, rise, 1.0 / 60.0);
            peak = peak.max(air);
            frames += 1;
            if air == 0.0 || frames > 600 {
                break;
            }
        }
        assert!(peak > 1.2 && peak < 1.8, "{peak}");
        assert!((40..65).contains(&frames), "{frames}");
        assert_eq!(jump_step(0.0, 0.0, 0.1), (0.0, 0.0));
    }

    #[test]
    fn moves_ease_and_turns_take_the_short_way() {
        let v = approach(Vec3::ZERO, Vec3::X * 7.0, HERO_ACCEL, 0.05);
        assert!(v.x > 0.0 && v.x < 7.0);
        assert_eq!(approach(v, Vec3::X * 7.0, HERO_ACCEL, 10.0), Vec3::X * 7.0);
        // From just under +pi to just over -pi is a small step, not a full turn back.
        let pi = std::f32::consts::PI;
        let t = turn_toward(pi - 0.1, -pi + 0.1, 1.0);
        assert!((t - (pi + 0.1)).abs() < 1e-4, "{t}");
        assert!((turn_toward(0.0, 2.0, 0.5) - 0.5).abs() < 1e-6);
        assert_eq!(turn_toward(0.0, 0.3, 0.5), 0.3);
    }
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
