//! World collision for the hero and the enemies: their moves are swept against the engine's world
//! (buildings, walls, trees, rocks) with a small capsule, and slide along what they hit. Ground
//! and walkable slopes are not walls, so hills stay climbable; the terrain height still comes from
//! the heightfield.

use avian3d::prelude::Collider;
use bevy::prelude::*;

use benilla_world::collision::WorldCollision;
use benilla_world::schedule::WorldStage;

use super::CombatSet;
use crate::game::Hero;

pub struct CollidePlugin;

impl Plugin for CollidePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            // After the hero moved (WorldStage::Input), before anything reads where it is.
            block_hero.after(WorldStage::Input).before(CombatSet::Spawn),
        );
    }
}

/// The swept body: a capsule this wide, this far above the feet (over low clutter and steps).
const BODY_RADIUS: f32 = 0.45;
const BODY_HALF: f32 = 0.5;
const BODY_LIFT: f32 = 1.3;
/// Space kept from a wall.
const SKIN: f32 = 0.05;
/// A face this flat (normal's up part) is ground, not a wall.
const GROUND_NORMAL_Y: f32 = 0.6;

/// The body shape.
pub fn body() -> Collider {
    Collider::capsule(BODY_RADIUS, BODY_HALF * 2.0)
}

/// What is left of a horizontal `movement` that hit a wall `dist` along it with horizontal normal
/// `normal`: the part up to the wall, and the rest slid along the wall.
pub fn slide(movement: Vec3, dist: f32, normal: Vec3) -> (Vec3, Vec3) {
    let len = movement.length();
    if len < 1e-6 {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let dir = movement / len;
    let reach = (dist - SKIN).clamp(0.0, len);
    let allowed = dir * reach;
    let rest = movement - allowed;
    let n = normal.with_y(0.0).normalize_or_zero();
    let along = rest - n * rest.dot(n);
    (allowed, along)
}

/// Sweeps a horizontal move from `from` (feet) by `movement` against walls, sliding once; returns
/// the move that is allowed.
pub fn swept_move(world: &WorldCollision, shape: &Collider, from: Vec3, movement: Vec3) -> Vec3 {
    let movement = movement.with_y(0.0);
    if movement.length_squared() < 1e-8 {
        return movement;
    }
    let origin = from + Vec3::Y * BODY_LIFT;
    let wall = |at: Vec3, m: Vec3| {
        world
            .cast_body(shape, at, m, SKIN)
            // Ground is not a wall; and a body that starts inside something (distance 0) is let
            // out rather than locked in place.
            .filter(|h| h.face_normal.y < GROUND_NORMAL_Y && h.distance > 1e-4)
    };
    let Some(hit) = wall(origin, movement) else {
        return movement;
    };
    let (allowed, along) = slide(movement, hit.distance, hit.face_normal);
    if along.length_squared() < 1e-8 {
        return allowed;
    }
    match wall(origin + allowed, along) {
        None => allowed + along,
        Some(h2) => allowed + slide(along, h2.distance, h2.face_normal).0,
    }
}

/// The hero's move this frame is checked against walls; a blocked move slides along the wall.
fn block_hero(
    world: WorldCollision,
    mut heroes: Query<&mut Transform, With<Hero>>,
    mut last: Local<Option<Vec3>>,
    mut shape: Local<Option<Collider>>,
) {
    let Ok(mut xf) = heroes.single_mut() else {
        return;
    };
    let now = xf.translation;
    let Some(prev) = last.replace(now) else {
        return;
    };
    let movement = (now - prev).with_y(0.0);
    // A jump (Blink, a run reset) is not a walk: no sweep.
    if movement.length() > 3.0 || movement.length_squared() < 1e-8 {
        return;
    }
    let shape = shape.get_or_insert_with(body);
    let allowed = swept_move(&world, shape, prev, movement);
    if (allowed - movement).length_squared() > 1e-8 {
        let fixed = prev + allowed;
        xf.translation.x = fixed.x;
        xf.translation.z = fixed.z;
        *last = Some(xf.translation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_ahead_stops_and_slides() {
        // Moving +x into a wall facing -x, at an angle: stop at the wall, keep the z part.
        let (allowed, along) = slide(Vec3::new(1.0, 0.0, 1.0), 0.5, Vec3::NEG_X);
        assert!((allowed.length() - 0.45).abs() < 1e-5);
        assert!(along.x.abs() < 1e-5 && along.z > 0.0, "{along:?}");
        // Head on: nothing to slide.
        let (_, along) = slide(Vec3::X, 0.2, Vec3::NEG_X);
        assert!(along.length() < 1e-5);
        // Already touching.
        let (allowed, _) = slide(Vec3::X, 0.0, Vec3::NEG_X);
        assert_eq!(allowed, Vec3::ZERO);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(block_hero).initialize(&mut world);
    }
}
