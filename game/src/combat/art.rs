//! WoW art for the combat UI, read from the player's own client: each Mage skill's spell (by its
//! English name in `Spell.dbc`) with its icon, and the interface textures the HUD, the cards and
//! the enemy health bars are drawn with. Without data everything here stays empty and the UI
//! falls back to plain colours.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{
    load_spell_catalog, load_spell_visual_catalog, SpellCatalog, SpellVisualCatalog,
};

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
    /// The spell visual tables (missile models, cast and impact kits), for the spell effects.
    pub visuals: Option<SpellVisualCatalog>,
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
    let visuals = {
        let mut chain = wa.chain.lock_recover();
        load_spell_visual_catalog(&mut chain)
    };
    match visuals {
        Ok(v) => art.visuals = Some(v),
        Err(e) => warn!("rnsurvivors: spell visuals unavailable ({e:#})"),
    }
    art.spells = Some(catalog);
    write_report(&art);
}

/// The report file: what was read from the client, for sharing without the console.
pub const REPORT_FILE: &str = "rnsurvivors-rapor.txt";

/// One stage of a spell's visual: its name and the effect models it fires.
pub struct StageRow {
    pub stage: &'static str,
    pub kit: u32,
    pub models: Vec<String>,
}

/// Every stage of a `SpellVisual` row with the model paths its kits name (attachment: path).
pub fn visual_rows(
    visuals: &SpellVisualCatalog,
    visual_id: u32,
) -> (Option<String>, Vec<StageRow>) {
    let Some(st) = visuals.stages(visual_id) else {
        return (None, Vec::new());
    };
    let path = |id: u32| visuals.effect_path(id).map(str::to_string);
    let missile = (st.missile_model != 0).then(|| {
        path(st.missile_model).unwrap_or_else(|| format!("(effect {} yolu yok)", st.missile_model))
    });
    let mut rows = Vec::new();
    for (stage, kit) in [
        ("precast", st.precast),
        ("cast", st.cast),
        ("impact", st.impact),
        ("state", st.state),
        ("channel", st.channel),
        ("area kit", st.area_kit),
    ] {
        if kit == 0 {
            continue;
        }
        let models = visuals
            .kit(kit)
            .map(|k| {
                k.effects()
                    .map(|(tag, id)| {
                        format!(
                            "{}: {}",
                            attach_name(tag),
                            path(id).unwrap_or_else(|| format!("effect {id}"))
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        rows.push(StageRow { stage, kit, models });
    }
    if st.area_effect != 0 {
        rows.push(StageRow {
            stage: "area model",
            kit: 0,
            models: path(st.area_effect).into_iter().collect(),
        });
    }
    (missile, rows)
}

/// A kit slot's attachment, as a readable name.
pub fn attach_name(tag: u16) -> &'static str {
    match tag {
        0x14 => "baş",
        0x22 => "göğüs",
        0x13 => "ayak",
        0x15 => "sol el",
        0x16 => "sağ el",
        0x11 => "nefes",
        0x17..=0x19 => "özel",
        u16::MAX => "dünya",
        _ => "?",
    }
}

fn write_report(art: &WowArt) {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "Rnsurvivors veri raporu");
    let _ = writeln!(
        out,
        "(Oyunun senin WoW istemcinden okudukları: numaralar ve dosya yolları.)"
    );
    let _ = writeln!(out, "Data: {:?}\n", benilla_formats::wow_data());
    let _ = writeln!(out, "== Skill'ler ==");
    for id in SkillId::every() {
        let def = id.def();
        let Some(sk) = art.skills.get(&id) else {
            let _ = writeln!(out, "[{}] büyü bulunamadı ({:?})", def.name, def.icon_spell);
            continue;
        };
        let _ = writeln!(
            out,
            "[{}] spell {} ({}), visual {}, ikon {} {}",
            def.name,
            sk.spell_id,
            def.icon_spell,
            sk.visual,
            sk.icon_path.as_deref().unwrap_or("-"),
            if sk.icon.is_some() {
                "(yüklendi)"
            } else {
                "(YÜKLENEMEDİ)"
            }
        );
        if let Some(v) = &art.visuals {
            let (missile, rows) = visual_rows(v, sk.visual);
            if let Some(m) = missile {
                let _ = writeln!(out, "    füze: {m}");
            }
            for r in rows {
                let _ = writeln!(
                    out,
                    "    {} (kit {}): {}",
                    r.stage,
                    r.kit,
                    r.models.join(" | ")
                );
            }
        }
    }
    let _ = writeln!(out, "\n== Pasifler ==");
    for p in PassiveId::ALL {
        let d = p.def();
        let ok = if art.passives.contains_key(&p) {
            "ikon yüklendi"
        } else {
            "İKON YOK"
        };
        let _ = writeln!(out, "[{}] {:?}: {ok}", d.name, d.icon_spell);
    }
    let _ = writeln!(out, "\n== Arayüz dokuları ==");
    for path in [
        STATUS_BAR,
        PLATE_BORDER,
        DIALOG_BACKGROUND,
        TOOLTIP_BACKGROUND,
        SLOT_FRAME,
    ] {
        let ok = if art.ui.contains_key(path) {
            "var"
        } else {
            "YOK"
        };
        let _ = writeln!(out, "{path}: {ok}");
    }
    match std::fs::write(REPORT_FILE, &out) {
        Ok(()) => {
            let at = std::fs::canonicalize(REPORT_FILE)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| REPORT_FILE.to_string());
            info!("rnsurvivors: rapor yazıldı: {at}");
        }
        Err(e) => warn!("rnsurvivors: rapor yazılamadı ({e})"),
    }
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
    fn attachments_read() {
        assert_eq!(attach_name(0x16), "sağ el");
        assert_eq!(attach_name(u16::MAX), "dünya");
        assert_eq!(attach_name(1), "?");
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(load_art).initialize(&mut world);
    }
}
