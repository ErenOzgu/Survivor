//! WoW art for the combat UI, read from the player's own client: each Mage skill's spell (by its
//! English name in `Spell.dbc`) with its icon, and the interface textures the HUD, the cards and
//! the enemy health bars are drawn with. Without data everything here stays empty and the UI
//! falls back to plain colours.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{load_spell_catalog, SpellCatalog};

use crate::content::passives::PassiveId;
use crate::content::SkillId;

pub struct ArtPlugin;

impl Plugin for ArtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WowArt>()
            .add_systems(Startup, load_art.after(benilla_assets::AssetSet::Open));
    }
}

/// Interface textures by role (WoW 1.12 client paths, without extension).
pub const STATUS_BAR: &str = "Interface\\TargetingFrame\\UI-StatusBar";
pub const PLATE_BORDER: &str = "Interface\\Tooltips\\Nameplate-Border";
pub const DIALOG_BACKGROUND: &str = "Interface\\DialogFrame\\UI-DialogBox-Background";
pub const TOOLTIP_BACKGROUND: &str = "Interface\\Tooltips\\UI-Tooltip-Background";
pub const SLOT_FRAME: &str = "Interface\\Buttons\\UI-Quickslot2";

/// What a skill is in the client. `spell_id`, `visual` and `icon_path` feed the spell effects
/// (next) and the log.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SkillSpell {
    /// The rank-1 `Spell.dbc` id the visuals and icon come from.
    pub spell_id: u32,
    /// `SpellVisual.dbc` id (0 = none).
    pub visual: u32,
    /// The icon path, `Interface\Icons\…`.
    pub icon_path: Option<String>,
    pub icon: Option<Handle<Image>>,
}

/// The loaded art; empty without WoW data.
#[derive(Resource, Default)]
pub struct WowArt {
    pub skills: HashMap<SkillId, SkillSpell>,
    /// Interface textures by their path constant above.
    pub ui: HashMap<&'static str, Handle<Image>>,
    /// Passive icons, from the spells named in `content::passives`.
    pub passives: HashMap<PassiveId, Handle<Image>>,
    /// The spell table, kept for the spell effects.
    pub spells: Option<SpellCatalog>,
}

impl WowArt {
    pub fn icon(&self, id: SkillId) -> Option<Handle<Image>> {
        self.skills.get(&id).and_then(|s| s.icon.clone())
    }

    pub fn passive_icon(&self, id: PassiveId) -> Option<Handle<Image>> {
        self.passives.get(&id).cloned()
    }

    pub fn ui(&self, path: &str) -> Option<Handle<Image>> {
        self.ui.get(path).cloned()
    }
}

/// The spell a skill is drawn as, from `(id, name, rank)` rows: the lowest id named `name` at
/// "Rank 1" (the player's spell), else the lowest unranked one.
pub fn pick_spell<'a>(
    rows: impl Iterator<Item = (u32, &'a str, Option<&'a str>)>,
    name: &str,
) -> Option<u32> {
    let mut rank_one = None::<u32>;
    let mut unranked = None::<u32>;
    for (id, n, r) in rows {
        if !n.eq_ignore_ascii_case(name) {
            continue;
        }
        let slot = match r {
            Some("Rank 1") => &mut rank_one,
            None | Some("") => &mut unranked,
            _ => continue,
        };
        *slot = Some(slot.map_or(id, |s| s.min(id)));
    }
    rank_one.or(unranked)
}

fn load_art(
    mut art: ResMut<WowArt>,
    world_assets: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut wa) = world_assets else {
        info!("rnsurvivors: no WoW data, plain UI");
        return;
    };
    for path in [
        STATUS_BAR,
        PLATE_BORDER,
        DIALOG_BACKGROUND,
        TOOLTIP_BACKGROUND,
        SLOT_FRAME,
    ] {
        match wa.sprite_texture(path, &mut images) {
            Some(h) => {
                art.ui.insert(path, h);
            }
            None => warn!("rnsurvivors: interface texture {path} not found"),
        }
    }
    let catalog = {
        let mut chain = wa.chain.lock_recover();
        load_spell_catalog(&mut chain)
    };
    let catalog = match catalog {
        Ok(c) => c,
        Err(e) => {
            warn!("rnsurvivors: spell table unavailable ({e:#}), no spell icons or effects");
            return;
        }
    };
    for id in SkillId::every() {
        let name = id.def().icon_spell;
        let rows = catalog
            .iter()
            .map(|(sid, d)| (sid, d.name.as_str(), d.rank.as_deref()));
        let Some(spell_id) = pick_spell(rows, name) else {
            warn!("rnsurvivors: no spell named {name:?} in Spell.dbc");
            continue;
        };
        let d = catalog.get(spell_id).expect("picked from the catalog");
        let icon = d
            .icon
            .as_deref()
            .and_then(|p| wa.sprite_texture(p, &mut images));
        info!(
            "rnsurvivors: {name} = spell {spell_id}, visual {}, icon {:?}",
            d.visual, d.icon
        );
        art.skills.insert(
            id,
            SkillSpell {
                spell_id,
                visual: d.visual,
                icon_path: d.icon.clone(),
                icon,
            },
        );
    }
    for p in PassiveId::ALL {
        let name = p.def().icon_spell;
        let rows = catalog
            .iter()
            .map(|(sid, d)| (sid, d.name.as_str(), d.rank.as_deref()));
        let icon = pick_spell(rows, name)
            .and_then(|sid| catalog.get(sid))
            .and_then(|d| d.icon.as_deref())
            .and_then(|path| wa.sprite_texture(path, &mut images));
        match icon {
            Some(h) => {
                art.passives.insert(p, h);
            }
            None => warn!("rnsurvivors: no icon for passive {:?} (spell {name:?})", p),
        }
    }
    art.spells = Some(catalog);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_one_lowest_id_wins() {
        let rows = [
            (205, "Frostbolt", Some("Rank 2")),
            (116, "Frostbolt", Some("Rank 1")),
            (9999, "Frostbolt", Some("Rank 1")),
            (5, "Fireball", Some("Rank 1")),
            (30, "frostbolt", None),
        ];
        assert_eq!(
            pick_spell(rows.into_iter(), "Frostbolt"),
            Some(116),
            "rank 1 beats unranked"
        );
        let rows = [(30, "Blink", None), (1953, "Blink", Some(""))];
        assert_eq!(pick_spell(rows.into_iter(), "Blink"), Some(30));
        let rows = [
            (205, "Frostbolt", Some("Rank 2")),
            (116, "Frostbolt", Some("Rank 1")),
        ];
        assert_eq!(pick_spell(rows.into_iter(), "Frostbolt"), Some(116));
        assert_eq!(pick_spell(rows.into_iter(), "Blink"), None);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(load_art).initialize(&mut world);
    }
}
