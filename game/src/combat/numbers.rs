//! Floating damage numbers: what each enemy took this frame rises from its head and fades. A
//! critical hit is bigger and yellow. Many hits in one frame on one enemy show as one number.

use bevy::prelude::*;

use super::enemies::{kill_enemies, Enemy};
use super::hud::UiFont;
use super::plates::bar_height;
use super::CombatSet;
use crate::game::FollowCam;

pub struct NumbersPlugin;

impl Plugin for NumbersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            // Before deaths turn enemies into corpses, so the killing blow shows too.
            spawn_numbers
                .in_set(CombatSet::Resolve)
                .before(kill_enemies),
        )
        .add_systems(Update, float_numbers.in_set(CombatSet::Ui));
    }
}

/// Seconds a number lives, and how high it rises (yards).
const LIFE: f32 = 0.9;
const RISE: f32 = 1.6;
/// The most numbers alive at once; small non-crit numbers are dropped past it.
const MAX_NUMBERS: usize = 120;

/// A floating number: where it started in the world, and its age.
#[derive(Component)]
pub struct FloatNumber {
    world: Vec3,
    age: f32,
    crit: bool,
    /// A sideways drift in pixels, so numbers on one enemy do not stack exactly.
    drift: f32,
}

/// The text of a number: whole hit points, at least 1.
pub fn number_text(amount: f32) -> String {
    format!("{}", amount.round().max(1.0) as u32)
}

/// Font size and colour of a number.
pub fn number_style(crit: bool) -> (f32, Color) {
    if crit {
        (26.0, Color::srgb(1.0, 0.85, 0.1))
    } else {
        (17.0, Color::srgb(1.0, 1.0, 1.0))
    }
}

/// Height (yards above the start) and opacity at `age`.
pub fn number_motion(age: f32) -> (f32, f32) {
    let t = (age / LIFE).clamp(0.0, 1.0);
    // Rise fast, then slow; fade over the last half.
    let rise = RISE * (1.0 - (1.0 - t) * (1.0 - t));
    let alpha = if t < 0.5 { 1.0 } else { 1.0 - (t - 0.5) * 2.0 };
    (rise, alpha)
}

fn spawn_numbers(
    mut commands: Commands,
    font: Res<UiFont>,
    mut enemies: Query<(Entity, &GlobalTransform, &mut Enemy)>,
    numbers: Query<(), With<FloatNumber>>,
) {
    let mut live = numbers.iter().count();
    for (e, gt, mut enemy) in &mut enemies {
        if enemy.took <= 0.0 {
            continue;
        }
        let (amount, crit) = (enemy.took, enemy.took_crit);
        enemy.took = 0.0;
        enemy.took_crit = false;
        if live >= MAX_NUMBERS && !crit {
            continue;
        }
        live += 1;
        let (size, color) = number_style(crit);
        // A deterministic spread from the entity id.
        let drift = ((e.to_bits() % 7) as f32 - 3.0) * 6.0;
        commands.spawn((
            Name::new("damage number"),
            FloatNumber {
                world: gt.translation() + Vec3::Y * (bar_height(enemy.kind) + 0.3),
                age: 0.0,
                crit,
                drift,
            },
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Text::new(number_text(amount)),
            font.text(size),
            TextColor(color),
            TextShadow::default(),
            Visibility::Hidden,
            GlobalZIndex(5),
        ));
    }
}

#[allow(clippy::type_complexity)]
fn float_numbers(
    mut commands: Commands,
    real: Res<Time<Real>>,
    cams: Query<(&Camera, &GlobalTransform), With<FollowCam>>,
    mut numbers: Query<(
        Entity,
        &mut FloatNumber,
        &mut Node,
        &mut TextColor,
        &mut Visibility,
    )>,
) {
    // Real time: numbers finish floating even while a card screen pauses the game.
    let dt = real.delta_secs();
    let cam = cams.single().ok();
    for (e, mut n, mut node, mut color, mut vis) in &mut numbers {
        n.age += dt;
        if n.age >= LIFE {
            commands.entity(e).try_despawn();
            continue;
        }
        let (rise, alpha) = number_motion(n.age);
        let at = n.world + Vec3::Y * rise;
        match cam.and_then(|(c, cgt)| c.world_to_viewport(cgt, at).ok()) {
            Some(p) => {
                let half = if n.crit { 18.0 } else { 10.0 };
                node.left = Val::Px(p.x - half + n.drift);
                node.top = Val::Px(p.y);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
        color.0 = color.0.with_alpha(alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_well() {
        assert_eq!(number_text(0.2), "1");
        assert_eq!(number_text(12.6), "13");
        assert!(number_style(true).0 > number_style(false).0);
    }

    #[test]
    fn numbers_rise_then_fade() {
        let (r0, a0) = number_motion(0.0);
        let (r1, a1) = number_motion(LIFE * 0.4);
        let (r2, a2) = number_motion(LIFE);
        assert_eq!((r0, a0), (0.0, 1.0));
        assert!(r1 > 0.0 && a1 == 1.0);
        assert!((r2 - RISE).abs() < 1e-5 && a2.abs() < 1e-5);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(spawn_numbers).initialize(&mut world);
        IntoSystem::into_system(float_numbers).initialize(&mut world);
    }
}
