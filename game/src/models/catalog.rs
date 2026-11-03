//! The client tables the model layer reads, loaded once from the open patch chain.

use std::sync::{Arc, Mutex};

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{
    load_creature_catalog, load_gameobject_catalog, load_item_display_catalog, Chain,
    CharCreateCatalog, CharSections, CharacterGeosets, CreatureCatalog, GameObjectCatalog,
    ItemDisplayCatalog,
};
use bevy::prelude::*;

/// The tables, or why there are none; inserted at startup, so a request can tell "not yet" (no
/// resource) from "never" (`tables == None`).
#[derive(Resource)]
pub struct ModelData {
    pub tables: Option<Tables>,
}

/// Every table but `creatures` is optional: a missing one degrades that piece (an untextured
/// body, every geoset shown) instead of the whole model.
pub struct Tables {
    /// `CreatureDisplayInfo` + `CreatureModelData` + `CreatureDisplayInfoExtra`.
    pub creatures: CreatureCatalog,
    /// `ChrRaces` (body displays) and `CharStartOutfit`.
    pub races: Option<CharCreateCatalog>,
    pub geosets: Option<CharacterGeosets>,
    pub sections: Option<CharSections>,
    pub items: Option<ItemDisplayCatalog>,
    /// `GameObjectDisplayInfo`: chests, sacks and other placed objects by display id.
    pub objects: Option<GameObjectCatalog>,
    /// The chain the body composite reads its textures from.
    pub chain: Arc<Mutex<Chain>>,
}

pub(super) fn load_model_data(mut commands: Commands, world_assets: Option<Res<WorldAssets>>) {
    let Some(world_assets) = world_assets else {
        warn!("rnsurvivors: no WoW data, so no character or creature models");
        commands.insert_resource(ModelData { tables: None });
        return;
    };
    let mut chain = world_assets.chain.lock_recover();
    let creatures = match load_creature_catalog(&mut chain) {
        Ok(c) => c,
        Err(e) => {
            warn!("rnsurvivors: creature displays unavailable ({e:#}), no models");
            drop(chain);
            commands.insert_resource(ModelData { tables: None });
            return;
        }
    };
    let races = ok("character races", CharCreateCatalog::load(&mut chain));
    let geosets = ok("character geosets", CharacterGeosets::load(&mut chain));
    let sections = ok("character skins", CharSections::load(&mut chain));
    let items = ok("item displays", load_item_display_catalog(&mut chain));
    let objects = ok("object displays", load_gameobject_catalog(&mut chain));
    drop(chain);
    info!(
        "rnsurvivors: model tables: {} creature displays, {} item displays",
        creatures.len(),
        items.as_ref().map_or(0, |i| i.len())
    );
    commands.insert_resource(ModelData {
        tables: Some(Tables {
            creatures,
            races,
            geosets,
            sections,
            items,
            objects,
            chain: world_assets.chain.clone(),
        }),
    });
}

/// A table that failed to load warns and is left out.
fn ok<T>(what: &str, r: anyhow::Result<T>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            warn!("rnsurvivors: {what} unavailable: {e:#}");
            None
        }
    }
}
