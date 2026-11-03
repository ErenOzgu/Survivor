//! The cooldown spells in play: Z, X and C cast Bloodlust, Ice Barrier and Arcane Power
//! (`content::abilities`). A cast shows the spell's own WoW effects on the hero (its state model
//! for as long as it lasts, its cast burst and sound); Ice Barrier puts up its shield. The
//! clocks stop with the game (card screens, pause).

use bevy::prelude::*;

use super::art::WowArt;
use super::fx::{Burst, FxBudget};
use super::hero::HeroHealth;
use super::{hero_on_ground, CombatSet, Run};
use crate::content::abilities::AbilityId;
use crate::game::Hero;
use crate::models::ModelRequest;

pub struct AbilitiesPlugin;

impl Plugin for AbilitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            cast_abilities.in_set(CombatSet::Act).run_if(hero_on_ground),
        );
    }
}

/// The key for each cooldown spell, in [`AbilityId::ALL`] order.
pub const ABILITY_KEYS: [KeyCode; 3] = [KeyCode::KeyZ, KeyCode::KeyX, KeyCode::KeyC];

/// A cooldown spell's effect on the hero; removed when the spell ends.
#[derive(Component)]
struct AbilityAura(AbilityId);

#[allow(clippy::too_many_arguments)]
fn cast_abilities(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    mut health: ResMut<HeroHealth>,
    art: Res<WowArt>,
    mut budget: ResMut<FxBudget>,
    heroes: Query<Entity, With<Hero>>,
    auras: Query<(Entity, &AbilityAura)>,
) {
    let dt = time.delta_secs();
    // Paused (a card screen, the pause menu): no casting, the clocks wait.
    if dt <= 0.0 {
        return;
    }
    run.abilities.tick(dt);
    // What is no longer running (it ran out, or a new run started) loses its shield and look.
    let shielded = run.abilities.active().any(|a| a.def().shield > 0.0);
    if !shielded {
        health.shield = 0.0;
    }
    for (e, aura) in &auras {
        if run.abilities.left(aura.0) <= 0.0 {
            commands.entity(e).try_despawn();
        }
    }
    let Ok(hero) = heroes.single() else {
        return;
    };
    for (id, key) in AbilityId::ALL.into_iter().zip(ABILITY_KEYS) {
        if !keys.just_pressed(key) || !run.abilities.cast(id) {
            continue;
        }
        let d = id.def();
        info!("rnsurvivors: cast {}", d.name);
        if d.shield > 0.0 {
            health.shield = health.max * d.shield;
        }
        let fx = art.abilities.get(&id).map(|(_, fx)| fx);
        budget.sound(fx.and_then(|f| f.cast_sound.or(f.impact_sound)));
        // Its lasting look (the state model, else its impact) for the duration, on the hero.
        if let Some(path) = fx.and_then(|f| f.state.clone().or_else(|| f.impact.clone())) {
            commands.entity(hero).with_child((
                Name::new("cooldown aura"),
                AbilityAura(id),
                Transform::IDENTITY,
                Visibility::default(),
                ModelRequest::Effect { path, scale: 1.0 },
            ));
        }
        // Its cast burst at the feet, briefly.
        if let Some(path) = fx.and_then(|f| f.area.clone()) {
            commands.entity(hero).with_child((
                Name::new("cooldown cast"),
                Burst::lasting(2.0),
                Transform::IDENTITY,
                Visibility::default(),
                ModelRequest::Effect { path, scale: 1.0 },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_params() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = bevy::prelude::World::new();
        IntoSystem::into_system(super::cast_abilities).initialize(&mut world);
    }
}
