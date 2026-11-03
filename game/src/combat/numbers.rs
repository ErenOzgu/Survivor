//! Floating damage numbers: what each enemy took this frame pops out of its head, rises on a
//! little arc and fades, in the colour of the school that hit (frost blue, fire orange, arcane
//! violet). A critical hit is bigger, brighter, ends in "!" and nudges the camera. Many hits in
//! one frame on one enemy show as one number.

use bevy::prelude::*;

use super::cards::School;
use super::enemies::{kill_enemies, Enemy};
use super::hud::UiFont;
use super::plates::bar_height;
use super::rules::Card;
use super::shake::Shake;
use super::CombatSet;
use crate::content::SkillId;
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
const LIFE: f32 = 1.0;
const RISE: f32 = 1.8;
/// Seconds the pop-in takes, and how big a number starts (normal, critical).
const POP: f32 = 0.16;
const POP_FROM: (f32, f32) = (1.35, 2.1);
/// Camera trauma per critical number, and the most in one frame.
const CRIT_SHAKE: f32 = 0.05;
const CRIT_SHAKE_FRAME: f32 = 0.12;
/// The most numbers alive at once; small non-crit numbers are dropped past it.
const MAX_NUMBERS: usize = 120;

/// A floating number: where it started in the world, and its age.
#[derive(Component)]
pub struct FloatNumber {
    world: Vec3,
    age: f32,
    crit: bool,
    /// A sideways drift in pixels over its life, so numbers on one enemy do not stack exactly.
    drift: f32,
}

/// The text of a number: whole hit points, at least 1; a crit ends in "!".
pub fn number_text(amount: f32, crit: bool) -> String {
    let n = amount.round().max(1.0) as u32;
    if crit {
        format!("{n}!")
    } else {
        n.to_string()
    }
}

/// Font size and colour of a number: the hitting skill's school, brighter on a crit.
pub fn number_style(crit: bool, skill: Option<SkillId>) -> (f32, Color) {
    let base = match skill.map(|id| School::of(Card::New(id))) {
        Some(School::Frost) => [0.45, 0.78, 1.0],
        Some(School::Fire) => [1.0, 0.55, 0.12],
        Some(School::Arcane) => [0.88, 0.52, 1.0],
        _ => [1.0, 1.0, 1.0],
    };
    if crit {
        let c = base.map(|v: f32| (v * 0.6 + 0.4).min(1.0));
        (32.0, Color::srgb(c[0], c[1], c[2]))
    } else {
        (20.0, Color::srgb(base[0], base[1], base[2]))
    }
}

/// The pop-in: a number starts big and settles to its size.
pub fn number_scale(age: f32, crit: bool) -> f32 {
    let from = if crit { POP_FROM.1 } else { POP_FROM.0 };
    let t = (age / POP).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    from + (1.0 - from) * ease
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
    mut shake: ResMut<Shake>,
) {
    let mut live = numbers.iter().count();
    let mut shaken = 0.0;
    for (e, gt, mut enemy) in &mut enemies {
        if enemy.took <= 0.0 {
            continue;
        }
        let (amount, crit, skill) = (enemy.took, enemy.took_crit, enemy.took_skill);
        enemy.took = 0.0;
        enemy.took_crit = false;
        enemy.took_skill = None;
        if crit && shaken < CRIT_SHAKE_FRAME {
            shake.add(CRIT_SHAKE);
            shaken += CRIT_SHAKE;
        }
        if live >= MAX_NUMBERS && !crit {
            continue;
        }
        live += 1;
        let (size, color) = number_style(crit, skill);
        // A deterministic spread from the entity id.
        let drift = ((e.to_bits() % 7) as f32 - 3.0) * 9.0;
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
            Text::new(number_text(amount, crit)),
            font.text(size),
            TextColor(color),
            TextShadow {
                offset: Vec2::splat(2.0),
                color: Color::srgba(0.0, 0.0, 0.0, 0.95),
            },
            UiTransform::from_scale(Vec2::splat(number_scale(0.0, crit))),
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
        &mut UiTransform,
    )>,
) {
    // Real time: numbers finish floating even while a card screen pauses the game.
    let dt = real.delta_secs();
    let cam = cams.single().ok();
    for (e, mut n, mut node, mut color, mut vis, mut xf) in &mut numbers {
        n.age += dt;
        if n.age >= LIFE {
            commands.entity(e).try_despawn();
            continue;
        }
        let (rise, alpha) = number_motion(n.age);
        let at = n.world + Vec3::Y * rise;
        match cam.and_then(|(c, cgt)| c.world_to_viewport(cgt, at).ok()) {
            Some(p) => {
                let half = if n.crit { 26.0 } else { 12.0 };
                // A little arc: the drift grows as it rises.
                let arc = n.drift * (0.4 + (n.age / LIFE).min(1.0));
                node.left = Val::Px(p.x - half + arc);
                node.top = Val::Px(p.y);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
        color.0 = color.0.with_alpha(alpha);
        xf.scale = Vec2::splat(number_scale(n.age, n.crit));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_well() {
        assert_eq!(number_text(0.2, false), "1");
        assert_eq!(number_text(12.6, true), "13!");
        let fire = Some(SkillId::Fireball);
        assert!(number_style(true, fire).0 > number_style(false, fire).0);
        assert_ne!(
            number_style(false, fire).1,
            number_style(false, Some(SkillId::Frostbolt)).1
        );
        assert!(number_scale(0.0, true) > number_scale(0.0, false));
        assert!((number_scale(POP, true) - 1.0).abs() < 1e-5);
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
