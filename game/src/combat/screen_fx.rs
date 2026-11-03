//! Screen effects: a level-up raises the client's own golden light column (`LevelUp.mdx`) on the
//! hero with its sound, and while health is low the screen's edges pulse red.

use bevy::prelude::*;

use benilla_assets::WorldAssets;

use super::fx::Burst;
use super::hero::HeroHealth;
use super::sound::Sound;
use super::{CombatSet, Run};
use crate::game::Hero;
use crate::models::ModelRequest;

pub struct ScreenFxPlugin;

impl Plugin for ScreenFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_vignette)
            .add_systems(Update, (level_up_fx, pulse_vignette).in_set(CombatSet::Ui));
    }
}

/// The client's level-up effect and sound.
pub const LEVEL_UP_MODEL: &str = "Spells\\LevelUp\\LevelUp.mdx";
pub const LEVEL_UP_SOUND: &str = "Sound\\Interface\\LevelUp.wav";
/// How long the light column stands.
const LEVEL_UP_SECS: f32 = 3.0;

/// When the level goes up (not at a run's start or reset), the column rises on the hero.
fn level_up_fx(
    mut commands: Commands,
    run: Res<Run>,
    heroes: Query<Entity, With<Hero>>,
    sound: Option<ResMut<Sound>>,
    wa: Option<Res<WorldAssets>>,
    mut last: Local<Option<u32>>,
) {
    let prev = last.replace(run.level);
    let Some(prev) = prev else {
        return;
    };
    if run.level <= prev {
        return;
    }
    if let Ok(hero) = heroes.single() {
        commands.entity(hero).with_child((
            Name::new("level up"),
            Burst::lasting(LEVEL_UP_SECS),
            Transform::IDENTITY,
            Visibility::default(),
            ModelRequest::Effect {
                path: LEVEL_UP_MODEL.to_string(),
                scale: 1.0,
            },
        ));
    }
    if let (Some(mut sound), Some(wa)) = (sound, wa) {
        sound.play_file(&wa, LEVEL_UP_SOUND, 1.0);
    }
}

/// The red edge shown at low health.
#[derive(Component)]
struct Vignette {
    shown: f32,
}

/// Health share below which the edges pulse.
const LOW_HEALTH: f32 = 0.35;

/// How red the edges are at `fraction` health, `t` seconds in: none above the threshold, a
/// stronger and faster beat the lower it gets.
pub fn vignette_strength(fraction: f32, t: f32) -> f32 {
    if fraction >= LOW_HEALTH || fraction <= 0.0 {
        return 0.0;
    }
    let danger = 1.0 - fraction / LOW_HEALTH;
    let beat = 0.5 + 0.5 * (t * (4.0 + 4.0 * danger)).sin();
    (0.25 + 0.45 * danger) * (0.55 + 0.45 * beat)
}

fn vignette(alpha: f32) -> BackgroundGradient {
    BackgroundGradient::from(RadialGradient::new(
        UiPosition::CENTER,
        RadialGradientShape::FarthestCorner,
        vec![
            ColorStop::percent(Color::srgba(0.8, 0.0, 0.0, 0.0), 45.0),
            ColorStop::percent(Color::srgba(0.8, 0.0, 0.0, alpha), 100.0),
        ],
    ))
}

fn spawn_vignette(mut commands: Commands) {
    commands.spawn((
        Name::new("low health vignette"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        vignette(0.0),
        Pickable::IGNORE,
        GlobalZIndex(-1),
        Visibility::Hidden,
        Vignette { shown: 0.0 },
    ));
}

fn pulse_vignette(
    real: Res<Time<Real>>,
    health: Res<HeroHealth>,
    mut q: Query<(&mut Vignette, &mut BackgroundGradient, &mut Visibility)>,
) {
    let a = vignette_strength(health.fraction(), real.elapsed_secs());
    for (mut v, mut g, mut vis) in &mut q {
        if (a - v.shown).abs() < 0.01 {
            continue;
        }
        v.shown = a;
        *g = vignette(a);
        *vis = if a > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_pulse_only_at_low_health() {
        assert_eq!(vignette_strength(1.0, 0.0), 0.0);
        assert_eq!(vignette_strength(0.5, 0.0), 0.0);
        let low = (0..50)
            .map(|i| vignette_strength(0.05, i as f32 * 0.1))
            .fold(0.0, f32::max);
        let mid = (0..50)
            .map(|i| vignette_strength(0.3, i as f32 * 0.1))
            .fold(0.0, f32::max);
        assert!(low > mid && mid > 0.0 && low <= 1.0);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(level_up_fx).initialize(&mut world);
        IntoSystem::into_system(pulse_vignette).initialize(&mut world);
    }
}
