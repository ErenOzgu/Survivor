//! WoW art for the combat UI, read from the player's own client: each Mage skill's spell (by its
//! English name in `Spell.dbc`) with its icon, and the interface textures the HUD, the cards and
//! the enemy health bars are drawn with. Without data everything here stays empty and the UI
//! falls back to plain colours.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{
    load_loading_screens, load_spell_catalog, load_spell_visual_catalog, SpellCatalog,
    SpellVisualCatalog, VisualStages,
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
/// The player frame (the target frame, mirrored), the class icons, the action bar and its
/// gryphons, the minimap's ring, the boss skull.
pub const TARGET_FRAME: &str = "Interface\\TargetingFrame\\UI-TargetingFrame";
pub const CLASS_ICONS: &str = "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Classes";
pub const MAIN_BAR: &str = "Interface\\MainMenuBar\\UI-MainMenuBar-Dwarf";
pub const END_CAP: &str = "Interface\\MainMenuBar\\UI-MainMenuBar-EndCap-Dwarf";
pub const MINIMAP_BORDER: &str = "Interface\\Minimap\\UI-Minimap-Border";
pub const BOSS_SKULL: &str = "Interface\\TargetingFrame\\UI-TargetingFrame-Skull";
/// Every interface texture loaded at start.
pub const UI_TEXTURES: [&str; 14] = [
    STATUS_BAR,
    PLATE_BORDER,
    DIALOG_BACKGROUND,
    TOOLTIP_BACKGROUND,
    SLOT_FRAME,
    TALENT_FROST,
    TALENT_FIRE,
    TALENT_ARCANE,
    TARGET_FRAME,
    CLASS_ICONS,
    MAIN_BAR,
    END_CAP,
    MINIMAP_BORDER,
    BOSS_SKULL,
];
/// The mage talent trees' paintings (their top-left piece), behind the level-up cards' icons.
pub const TALENT_FROST: &str = "Interface\\TalentFrame\\MageFrost-TopLeft";
pub const TALENT_FIRE: &str = "Interface\\TalentFrame\\MageFire-TopLeft";
pub const TALENT_ARCANE: &str = "Interface\\TalentFrame\\MageArcane-TopLeft";

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
    /// Each skill's effect models, resolved from the visual tables.
    pub fx: HashMap<SkillId, SkillFx>,
    /// The XP orb's look: a green missile model from the client.
    pub orb_model: Option<String>,
    /// Loading-screen paintings for the level-up cards, by [`SCENE_NEEDLES`] key.
    pub scenes: HashMap<&'static str, Handle<Image>>,
}

/// Which painting the level-up cards show behind the icon: `kart=agac` (the talent trees) or
/// `kart=yukleme` (the loading screens, the default) in the settings file, or `RNS_CARD_ART`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardArt {
    Talent,
    #[default]
    Loading,
}

pub const CARD_ART_VAR: &str = "RNS_CARD_ART";

impl CardArt {
    pub fn parse(v: &str) -> Option<Self> {
        match v.trim().to_ascii_lowercase().as_str() {
            "agac" | "ağaç" | "talent" => Some(CardArt::Talent),
            "yukleme" | "yükleme" | "loading" => Some(CardArt::Loading),
            _ => None,
        }
    }

    pub fn from_settings() -> Self {
        std::env::var(CARD_ART_VAR)
            .ok()
            .or_else(|| {
                std::fs::read_to_string(crate::settings::settings_path())
                    .ok()
                    .and_then(|t| crate::settings::setting(&t, "kart"))
            })
            .and_then(|v| Self::parse(&v))
            .unwrap_or_default()
    }
}

/// Each card school's loading screen, by words in its file name, in preference order.
pub const SCENE_NEEDLES: [(&str, &[&str]); 5] = [
    ("frost", &["naxxramas", "scholomance", "easternkingdom"]),
    ("fire", &["moltencore", "blackrock", "ragefire"]),
    ("arcane", &["diremaul", "ahnqiraj", "kalimdor"]),
    ("passive", &["easternkingdom"]),
    ("potion", &["kalimdor"]),
];

/// The first path (by needle order) whose file name holds a needle.
pub fn pick_scene<'a>(paths: &'a [String], needles: &[&str]) -> Option<&'a str> {
    needles.iter().find_map(|n| {
        paths
            .iter()
            .find(|p| {
                let file = p.rsplit('\\').next().unwrap_or(p).to_ascii_lowercase();
                file.contains(n)
            })
            .map(String::as_str)
    })
}

/// The part of a `w`×`h` loading screen a card's oval shows: a centred cut at `aspect`
/// (width / height), `zoom` of the width. A square file holds a 4:3 picture in its top.
pub fn scene_crop(w: f32, h: f32, aspect: f32, zoom: f32) -> Rect {
    let content_h = if (w - h).abs() < 1.0 { w * 0.75 } else { h };
    let mut cw = w * zoom;
    let mut ch = cw / aspect;
    if ch > content_h {
        ch = content_h;
        cw = ch * aspect;
    }
    let (cx, cy) = (w / 2.0, content_h / 2.0);
    Rect::new(cx - cw / 2.0, cy - ch / 2.0, cx + cw / 2.0, cy + ch / 2.0)
}

/// The effect models a skill shows (paths of `Spells\\…mdx`, as the client names them).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillFx {
    /// What flies.
    pub missile: Option<String>,
    /// Where a hit lands (on the enemy).
    pub impact: Option<String>,
    /// On the ground: the area a skill covers, or its burst at the hero's feet.
    pub area: Option<String>,
    /// On what it leaves frozen.
    pub state: Option<String>,
    /// `SoundEntries` kits: when it is cast, and where it hits.
    pub cast_sound: Option<u32>,
    pub impact_sound: Option<u32>,
}

/// Attachment tags as the visual kits use them.
const TAG_FEET: u16 = 0x13;
const TAG_CHEST: u16 = 0x22;

/// One visual's effect models, by role: `effect_path` gives an effect id's model, `kit` a kit's
/// `(tag, effect id)` slots.
pub fn visual_fx(
    stages: &VisualStages,
    effect_path: &dyn Fn(u32) -> Option<String>,
    kit: &dyn Fn(u32) -> Vec<(u16, u32)>,
    kit_sound: &dyn Fn(u32) -> Option<u32>,
) -> SkillFx {
    let sound = |k: u32| (k != 0).then(|| kit_sound(k)).flatten();
    let pick = |kit_id: u32, prefer: &[u16]| -> Option<String> {
        if kit_id == 0 {
            return None;
        }
        let slots = kit(kit_id);
        prefer
            .iter()
            .find_map(|t| slots.iter().find(|(tag, _)| tag == t))
            .or(slots.first())
            .and_then(|(_, id)| effect_path(*id))
    };
    SkillFx {
        missile: (stages.missile_model != 0)
            .then(|| effect_path(stages.missile_model))
            .flatten(),
        impact: pick(stages.impact, &[TAG_CHEST, u16::MAX]),
        area: (stages.area_effect != 0)
            .then(|| effect_path(stages.area_effect))
            .flatten()
            .or_else(|| {
                // A burst on the ground: the cast kit's feet or world slot only (not the hands).
                let slots = if stages.cast == 0 {
                    Vec::new()
                } else {
                    kit(stages.cast)
                };
                [TAG_FEET, u16::MAX]
                    .iter()
                    .find_map(|t| slots.iter().find(|(tag, _)| tag == t))
                    .and_then(|(_, id)| effect_path(*id))
            }),
        state: pick(stages.state, &[u16::MAX, TAG_FEET]),
        cast_sound: sound(stages.cast).or_else(|| sound(stages.precast)),
        impact_sound: sound(stages.impact),
    }
}

/// Where each skill borrows its effects from besides its own spell (Arcane Missiles' missile is
/// the triggered "Arcane Missile"; an evolution takes its parent's).
pub fn fx_sources(id: SkillId) -> &'static [&'static str] {
    match id {
        SkillId::ArcaneMissiles | SkillId::ArcaneStorm => &["Arcane Missile", "Arcane Missiles"],
        SkillId::WinterBolts => &["Frostbolt", "Frost Nova"],
        _ => &[],
    }
}

/// Merges effect sets: the first that has each role wins.
pub fn merge_fx(sets: impl Iterator<Item = SkillFx>) -> SkillFx {
    let mut out = SkillFx::default();
    for s in sets {
        out.missile = out.missile.or(s.missile);
        out.impact = out.impact.or(s.impact);
        out.area = out.area.or(s.area);
        out.state = out.state.or(s.state);
        out.cast_sound = out.cast_sound.or(s.cast_sound);
        out.impact_sound = out.impact_sound.or(s.impact_sound);
    }
    out
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

/// The spell a skill is drawn as, from `(id, name, rank, icon)` rows: among the rows named
/// `name`, the player's own spell first — a real icon before a placeholder (`Temp`: the client's
/// trainer and test rows), then "Rank 1" before unranked, then the lowest id.
pub fn pick_spell<'a>(
    rows: impl Iterator<Item = (u32, &'a str, Option<&'a str>, Option<&'a str>)>,
    name: &str,
) -> Option<u32> {
    rows.filter(|(_, n, _, _)| n.eq_ignore_ascii_case(name))
        .filter_map(|(id, _, rank, icon)| {
            let rank_class = match rank {
                Some("Rank 1") => 0,
                None | Some("") => 1,
                _ => return None,
            };
            let placeholder = icon.is_none_or(|i| {
                let i = i.to_ascii_lowercase();
                i.ends_with("\\temp") || i == "temp"
            });
            Some(((placeholder, rank_class, id), id))
        })
        .min_by_key(|(key, _)| *key)
        .map(|(_, id)| id)
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
    for path in UI_TEXTURES {
        match wa.sprite_texture(path, &mut images) {
            Some(h) => {
                art.ui.insert(path, h);
            }
            None => warn!("rnsurvivors: interface texture {path} not found"),
        }
    }
    let screens = {
        let mut chain = wa.chain.lock_recover();
        load_loading_screens(&mut chain)
    };
    match screens {
        Ok(screens) => {
            let paths: Vec<String> = (0..400)
                .filter_map(|id| screens.path(id).map(str::to_string))
                .collect();
            for (key, needles) in SCENE_NEEDLES {
                let Some(path) = pick_scene(&paths, needles) else {
                    continue;
                };
                let stem = path
                    .strip_suffix(".blp")
                    .or_else(|| path.strip_suffix(".BLP"));
                match wa.sprite_texture(stem.unwrap_or(path), &mut images) {
                    Some(h) => {
                        art.scenes.insert(key, h);
                    }
                    None => warn!("rnsurvivors: loading screen {path} not found"),
                }
            }
        }
        Err(e) => warn!("rnsurvivors: loading screens unavailable ({e:#})"),
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
            .map(|(sid, d)| (sid, d.name.as_str(), d.rank.as_deref(), d.icon.as_deref()));
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
            .map(|(sid, d)| (sid, d.name.as_str(), d.rank.as_deref(), d.icon.as_deref()));
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
    // Effect models per skill: borrowed sources lead (the missile), then its own visual.
    if let Some(v) = &art.visuals {
        let path = |id: u32| v.effect_path(id).map(str::to_string);
        let kit = |k: u32| v.kit(k).map(|k| k.effects().collect()).unwrap_or_default();
        let kit_sound = |k: u32| v.kit(k).and_then(|k| k.sound);
        let fx_of = |spell_name: &str| -> SkillFx {
            let rows = catalog
                .iter()
                .map(|(sid, d)| (sid, d.name.as_str(), d.rank.as_deref(), d.icon.as_deref()));
            pick_spell(rows, spell_name)
                .and_then(|sid| catalog.get(sid))
                .and_then(|d| v.stages(d.visual))
                .map(|st| visual_fx(st, &path, &kit, &kit_sound))
                .unwrap_or_default()
        };
        let mut fx = HashMap::new();
        for id in SkillId::every() {
            let own = fx_of(id.def().icon_spell);
            let borrowed: Vec<SkillFx> = fx_sources(id).iter().map(|n| fx_of(n)).collect();
            fx.insert(
                id,
                merge_fx(borrowed.into_iter().chain(std::iter::once(own))),
            );
        }
        let orb = fx_of("Wrath").missile;
        art.fx = fx;
        art.orb_model = orb;
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
    let _ = writeln!(out, "\n== Kullanılan efektler ==");
    for id in SkillId::every() {
        let fx = art.fx.get(&id).cloned().unwrap_or_default();
        let _ = writeln!(
            out,
            "[{}] füze {:?} · isabet {:?} · alan {:?} · durum {:?}",
            id.def().name,
            fx.missile,
            fx.impact,
            fx.area,
            fx.state
        );
    }
    let _ = writeln!(out, "XP küresi: {:?}", art.orb_model);
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
    for path in UI_TEXTURES {
        let ok = if art.ui.contains_key(path) {
            "var"
        } else {
            "YOK"
        };
        let _ = writeln!(out, "{path}: {ok}");
    }
    let _ = writeln!(out, "\n== Kart resimleri (yükleme ekranları) ==");
    for (key, _) in SCENE_NEEDLES {
        let ok = if art.scenes.contains_key(key) {
            "var"
        } else {
            "YOK"
        };
        let _ = writeln!(out, "{key}: {ok}");
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
    fn scenes_are_picked_by_name_and_cropped() {
        let paths = vec![
            "Interface\\Glues\\LoadingScreens\\LoadScreenKalimdor.blp".to_string(),
            "Interface\\Glues\\LoadingScreens\\LoadScreenMoltenCore.blp".to_string(),
        ];
        assert_eq!(
            pick_scene(&paths, &["blackrock", "moltencore"]),
            Some(paths[1].as_str())
        );
        assert_eq!(pick_scene(&paths, &["naxxramas"]), None);
        // A square file: the cut stays in its 4:3 top and keeps the aspect.
        let r = scene_crop(1024.0, 1024.0, 1.2, 0.7);
        assert!(r.min.y >= 0.0 && r.max.y <= 768.0, "{r:?}");
        assert!((r.width() / r.height() - 1.2).abs() < 1e-3);
        assert_eq!(CardArt::parse("Ağaç"), Some(CardArt::Talent));
        assert_eq!(CardArt::parse("yukleme"), Some(CardArt::Loading));
        assert_eq!(CardArt::parse("x"), None);
    }

    #[test]
    fn rank_one_lowest_id_wins() {
        let icon = Some("Interface\\Icons\\Spell_Frost_FrostBolt02");
        let rows = [
            (205, "Frostbolt", Some("Rank 2"), icon),
            (116, "Frostbolt", Some("Rank 1"), icon),
            (9999, "Frostbolt", Some("Rank 1"), icon),
            (5, "Fireball", Some("Rank 1"), icon),
            (30, "frostbolt", None, icon),
        ];
        assert_eq!(
            pick_spell(rows.into_iter(), "Frostbolt"),
            Some(116),
            "rank 1 beats unranked"
        );
        let rows = [(30, "Blink", None, icon), (1953, "Blink", Some(""), icon)];
        assert_eq!(pick_spell(rows.into_iter(), "Blink"), Some(30));
        assert_eq!(pick_spell(rows.into_iter(), "Frostbolt"), None);
    }

    #[test]
    fn trainer_rows_with_a_placeholder_icon_lose() {
        // As in the 1.12 client: a "Rank 1" trainer row with the Temp icon below the real spell.
        let rows = [
            (
                846,
                "Flamestrike",
                Some("Rank 1"),
                Some("Interface\\Icons\\Temp"),
            ),
            (
                2120,
                "Flamestrike",
                Some("Rank 1"),
                Some("Interface\\Icons\\Spell_Fire_SelfDestruct"),
            ),
            (517, "Blink", Some("Rank 1"), Some("Interface\\Icons\\Temp")),
            (
                1953,
                "Blink",
                None,
                Some("Interface\\Icons\\Spell_Arcane_Blink"),
            ),
        ];
        assert_eq!(pick_spell(rows.into_iter(), "Flamestrike"), Some(2120));
        assert_eq!(
            pick_spell(rows.into_iter(), "Blink"),
            Some(1953),
            "a real icon beats the rank"
        );
        let only_temp = [(
            846,
            "Flamestrike",
            Some("Rank 1"),
            Some("Interface\\Icons\\Temp"),
        )];
        assert_eq!(
            pick_spell(only_temp.into_iter(), "Flamestrike"),
            Some(846),
            "still a pick"
        );
    }

    #[test]
    fn effects_are_picked_by_role() {
        let paths: HashMap<u32, &str> = [
            (1, "Spells\\Frostbolt.mdx"),
            (2, "Spells\\Ice_Precast_Low_Hand.mdx"),
            (3, "Spells\\Ice_ImpactDD_Med_Chest.mdx"),
            (4, "Spells\\Frost_Nova_area.mdx"),
            (5, "Spells\\Frost_Nova_state.mdx"),
            (6, "Spells\\Ice_Precast_Uber_Head.mdx"),
        ]
        .into_iter()
        .collect();
        let kits: HashMap<u32, Vec<(u16, u32)>> = [
            (10, vec![(0x15, 2), (0x16, 2)]),
            (11, vec![(TAG_CHEST, 3)]),
            (12, vec![(TAG_FEET, 4)]),
            (13, vec![(0x14, 6), (u16::MAX, 5)]),
        ]
        .into_iter()
        .collect();
        let path = |id: u32| paths.get(&id).map(|s| s.to_string());
        let kit = |k: u32| kits.get(&k).cloned().unwrap_or_default();
        let bolt = VisualStages {
            precast: 10,
            cast: 10,
            impact: 11,
            missile_model: 1,
            ..Default::default()
        };
        let no_sound = |_: u32| None;
        let fx = visual_fx(&bolt, &path, &kit, &no_sound);
        assert_eq!(fx.missile.as_deref(), Some("Spells\\Frostbolt.mdx"));
        assert_eq!(
            fx.impact.as_deref(),
            Some("Spells\\Ice_ImpactDD_Med_Chest.mdx")
        );
        assert_eq!(fx.area, None, "hand effects are not a ground burst");
        let nova = VisualStages {
            cast: 12,
            state: 13,
            ..Default::default()
        };
        let fx = visual_fx(&nova, &path, &kit, &|k| (k == 12).then_some(77));
        assert_eq!(fx.cast_sound, Some(77));
        assert_eq!(fx.area.as_deref(), Some("Spells\\Frost_Nova_area.mdx"));
        assert_eq!(
            fx.state.as_deref(),
            Some("Spells\\Frost_Nova_state.mdx"),
            "world slot first"
        );
        let merged = merge_fx([SkillFx::default(), fx.clone()].into_iter());
        assert_eq!(merged, fx);
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
