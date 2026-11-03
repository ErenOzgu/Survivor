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
pub use catalog::ModelData;
#[allow(unused_imports)]
pub use look::{CharacterLook, Gear, GearItem, GearSlot, Race, Sex};
pub use test_spawn::DebugCenter;
#[allow(unused_imports)]
pub use test_spawn::TestCreature;

/// The lowest creature display whose model lives in `folder` (e.g. `Creature\\Kobold`) and has a
/// skin, so it draws textured. `None` without tables or without such a display.
pub fn creature_display_in_folder(data: &ModelData, folder: &str) -> Option<u32> {
    creature_displays_in_folder(data, folder).first().copied()
}

/// Every skinned creature display under `folder`, lowest id first (different skins of one
/// model, such as the kobold's colours).
pub fn creature_displays_in_folder(data: &ModelData, folder: &str) -> Vec<u32> {
    let Some(tables) = data.tables.as_ref() else {
        return Vec::new();
    };
    // `model_path` is owned per row, so collect before filtering.
    let rows: Vec<(u32, String, bool)> = tables
        .creatures
        .display_models()
        .filter_map(|(id, _)| {
            let m = tables.creatures.model(id)?;
            Some((id, m.model_path, m.textures[0].is_some()))
        })
        .collect();
    test_spawn::displays_by_folder(rows.iter().map(|(id, p, s)| (*id, p.as_str(), *s)), folder)
}

/// Creature displays with a skin whose model path holds `needle` (e.g. `spider`).
pub fn creature_displays_matching(data: &ModelData, needle: &str) -> Vec<u32> {
    let Some(tables) = data.tables.as_ref() else {
        return Vec::new();
    };
    let rows: Vec<(u32, String, bool)> = tables
        .creatures
        .display_models()
        .filter_map(|(id, _)| {
            let m = tables.creatures.model(id)?;
            Some((id, m.model_path, m.textures[0].is_some()))
        })
        .collect();
    test_spawn::displays_matching(rows.iter().map(|(id, p, s)| (*id, p.as_str(), *s)), needle)
}

/// Rebuilds an entity's model for a new request (the hero's gear changed): the old model goes and
/// the new one is built as for a fresh request.
pub fn rebuild_model(
    commands: &mut Commands,
    entity: Entity,
    request: ModelRequest,
    old_root: Option<Entity>,
) {
    if let Some(root) = old_root {
        commands.entity(root).try_despawn();
    }
    commands
        .entity(entity)
        .remove::<(ModelStatus, ModelRoot, build::Pending)>()
        .insert(request);
}

/// The client's look for one worn item: `slot`'s candidate the seed picks, as `(display id, icon
/// path)`. `None` without tables or candidates.
pub fn item_look(data: &ModelData, slot: GearSlot, seed: u32) -> Option<(u32, Option<String>)> {
    let cat = data.tables.as_ref()?.items.as_ref()?;
    let ids = gear::slot_displays(cat.iter(), slot);
    let id = *ids.get(seed as usize % ids.len().max(1))?;
    let icon = cat
        .get(id)
        .and_then(|d| d.icon.clone())
        .map(|i| format!("Interface\\Icons\\{i}"));
    Some((id, icon))
}

/// Whether a model path names what we look for: contains one of `include` and none of
/// `exclude`, ignoring case.
pub fn path_matches(path: &str, include: &[&str], exclude: &[&str]) -> bool {
    let p = path.to_ascii_lowercase();
    include.iter().any(|w| p.contains(w)) && !exclude.iter().any(|w| p.contains(w))
}

/// A placed object's model (a chest, a sack) by what its path says: the lowest display id whose
/// model matches. `None` without tables or a match.
pub fn object_model(data: &ModelData, include: &[&str], exclude: &[&str]) -> Option<String> {
    let objects = data.tables.as_ref()?.objects.as_ref()?;
    objects
        .iter()
        .filter(|(_, path)| path_matches(path, include, exclude))
        .min_by_key(|(id, _)| *id)
        .map(|(_, path)| path.to_string())
}

/// A character-model NPC display whose baked look names one of `needles` (a Defias, VanCleef),
/// the lowest id first.
pub fn npc_display(data: &ModelData, needles: &[&str]) -> Option<u32> {
    let creatures = &data.tables.as_ref()?.creatures;
    let mut ids: Vec<u32> = creatures
        .display_models()
        .filter_map(|(id, _)| {
            let m = creatures.model(id)?;
            let bake = m.npc_appearance.as_ref()?.bake_name.as_deref()?;
            path_matches(bake, needles, &[]).then_some(id)
        })
        .collect();
    ids.sort_unstable();
    // The needles are in order of preference.
    for n in needles {
        if let Some(id) = ids.iter().copied().find(|id| {
            creatures
                .model(*id)
                .and_then(|m| m.npc_appearance)
                .and_then(|a| a.bake_name)
                .is_some_and(|b| b.to_ascii_lowercase().contains(n))
        }) {
            return Some(id);
        }
    }
    None
}

/// What model an entity wants.
#[derive(Component, Clone, Debug)]
pub enum ModelRequest {
    /// A player-character body with its look and gear.
    Character(CharacterLook),
    /// A creature by its `CreatureDisplayInfo.dbc` id (character-model NPCs included).
    Creature { display_id: u32 },
    /// A spell effect model (`Spells\\…mdx`), with its particles and ribbons.
    Effect { path: String, scale: f32 },
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
    fn paths_match_words() {
        let chest = "World\\Generic\\ActiveDoodads\\Chest02\\Chest02.mdx";
        assert!(path_matches(chest, &["chest"], &["mail"]));
        assert!(!path_matches(chest, &["sack"], &[]));
        assert!(!path_matches(
            "World\\Mailbox\\MailChest.mdx",
            &["chest"],
            &["mail"]
        ));
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
