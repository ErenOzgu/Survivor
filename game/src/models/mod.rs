//! WoW 1.12 character and creature models for game entities, independent of game logic.
//!
//! Put a [`ModelRequest`] on any entity with a `Transform` (its feet; the model faces its −Z):
//!
//! ```ignore
//! commands.spawn((
//!     Transform::from_translation(feet),
//!     Visibility::default(),
//!     ModelRequest::Creature { display_id: 123 },
//!     ModelAnim::default(),
//! ));
//! ```
//!
//! The plugin builds the model as a child of that entity and tracks it with [`ModelStatus`]
//! (`Loading`, `Ready`, `Failed`). Set [`ModelAnim::state`] (Idle, Walk, Run, Death) or call
//! [`ModelAnim::play`] (Cast, Attack, Hit) to animate it; [`AutoLocomotion`] sets Idle/Walk/Run
//! from how the entity moves. Children marked [`ModelPlaceholder`] are hidden once the model is
//! ready and stay as the fallback when it fails.

mod anim;
mod build;
mod catalog;
mod gear;
mod look;
mod paths;
mod test_spawn;

use bevy::prelude::*;

#[allow(unused_imports)]
pub use anim::{ids as anim_ids, pick_anim_id, AnimState, AutoLocomotion, ModelAnim};
#[allow(unused_imports)]
pub use look::{CharacterLook, Gear, GearItem, GearSlot, Race, Sex};
pub use catalog::ModelData;
pub use test_spawn::DebugCenter;
#[allow(unused_imports)]
pub use test_spawn::TestCreature;

/// The lowest creature display whose model lives in `folder` (e.g. `Creature\\Kobold`) and has a
/// skin, so it draws textured. `None` without tables or without such a display.
pub fn creature_display_in_folder(data: &ModelData, folder: &str) -> Option<u32> {
    let tables = data.tables.as_ref()?;
    // `model_path` is owned per row, so collect before picking.
    let rows: Vec<(u32, String, bool)> = tables
        .creatures
        .display_models()
        .filter_map(|(id, _)| {
            let m = tables.creatures.model(id)?;
            Some((id, m.model_path, m.textures[0].is_some()))
        })
        .collect();
    test_spawn::pick_by_folder(rows.iter().map(|(id, p, s)| (*id, p.as_str(), *s)), folder)
}

/// What model an entity wants.
#[derive(Component, Clone, Debug)]
pub enum ModelRequest {
    /// A player-character body with its look and gear.
    Character(CharacterLook),
    /// A creature by its `CreatureDisplayInfo.dbc` id (character-model NPCs included).
    Creature { display_id: u32 },
}

/// Where an entity's model stands.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ModelStatus {
    Loading,
    Ready,
    /// The model will not appear; the reason is also logged.
    Failed(String),
}

/// On a requesting entity once its model is built: the model root (scale, rig, animation player).
#[derive(Component, Clone, Copy, Debug)]
pub struct ModelRoot(pub Entity);

/// A child shown until the model is ready (a placeholder mesh), and kept if it fails.
#[derive(Component)]
pub struct ModelPlaceholder;

/// Builds and animates requested models.
pub struct ModelsPlugin;

/// The model systems' set, after the game's input stage moved the entities.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSet;

impl Plugin for ModelsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(test_spawn::TestSpawn::from_env())
            .add_systems(
                Startup,
                catalog::load_model_data.after(benilla_assets::AssetSet::Open),
            )
            .configure_sets(
                Update,
                ModelSet.after(benilla_world::schedule::WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    test_spawn::spawn_test_creatures,
                    build::resolve_requests,
                    build::finish_models,
                    swap_placeholders,
                    test_spawn::debug_anim_keys,
                    anim::auto_locomotion,
                    anim::drive_animations,
                )
                    .chain()
                    .in_set(ModelSet),
            );
    }
}

/// Hide a ready model's placeholders; warn once when one failed and keep them.
fn swap_placeholders(
    changed: Query<(Entity, &ModelStatus, Option<&Children>), Changed<ModelStatus>>,
    mut placeholders: Query<&mut Visibility, With<ModelPlaceholder>>,
) {
    for (entity, status, children) in &changed {
        match status {
            ModelStatus::Ready => {
                for &c in children.into_iter().flatten() {
                    if let Ok(mut v) = placeholders.get_mut(c) {
                        *v = Visibility::Hidden;
                    }
                }
            }
            ModelStatus::Failed(why)
                if children.is_some_and(|c| c.iter().any(|c| placeholders.contains(c))) =>
            {
                warn!("rnsurvivors: {entity} keeps its placeholder: {why}");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(tables_loaded: bool) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        if tables_loaded {
            app.insert_resource(catalog::ModelData { tables: None });
        }
        app.add_systems(Update, (build::resolve_requests, swap_placeholders).chain());
        app
    }

    fn requester(app: &mut App) -> (Entity, Entity) {
        let owner = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::default(),
                ModelRequest::Creature { display_id: 1 },
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((Visibility::default(), ModelPlaceholder, ChildOf(owner)))
            .id();
        (owner, child)
    }

    #[test]
    fn a_request_waits_for_the_tables() {
        let mut app = app(false);
        let (owner, _) = requester(&mut app);
        app.update();
        assert!(app.world().get::<ModelStatus>(owner).is_none());
    }

    #[test]
    fn without_data_the_request_fails_and_the_placeholder_stays() {
        let mut app = app(true);
        let (owner, child) = requester(&mut app);
        app.update();
        app.update();
        assert!(matches!(
            app.world().get::<ModelStatus>(owner),
            Some(ModelStatus::Failed(_))
        ));
        assert_eq!(
            app.world().get::<Visibility>(child),
            Some(&Visibility::Inherited)
        );
    }

    #[test]
    fn a_ready_model_hides_its_placeholders() {
        let mut app = app(true);
        let (owner, child) = requester(&mut app);
        app.world_mut().entity_mut(owner).insert(ModelStatus::Ready);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(child),
            Some(&Visibility::Hidden)
        );
    }
}
