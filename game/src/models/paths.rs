//! Archive paths and asset URLs the model layer builds.

use super::look::{Race, Sex};

/// The `Character\` folder name of a race (ChrRaces `ClientFileString`).
pub fn race_folder(race: Race) -> Option<&'static str> {
    Some(match race.0 {
        1 => "Human",
        2 => "Orc",
        3 => "Dwarf",
        4 => "NightElf",
        5 => "Scourge",
        6 => "Tauren",
        7 => "Gnome",
        8 => "Troll",
        _ => return None,
    })
}

/// A body model's path, `Character\Troll\Female\TrollFemale.mdx`: the fallback when the display
/// tables cannot name it.
pub fn character_model_path(race: Race, sex: Sex) -> Option<String> {
    let r = race_folder(race)?;
    let s = match sex {
        Sex::Male => "Male",
        Sex::Female => "Female",
    };
    Some(format!("Character\\{r}\\{s}\\{r}{s}.mdx"))
}

/// `Creature\Kobold\Kobold.mdx` → `Creature\Kobold`, where the skin variations live.
pub fn model_dir(model_path: &str) -> &str {
    model_path
        .rsplit_once(['\\', '/'])
        .map(|(dir, _)| dir)
        .unwrap_or("")
}

/// The folder a held weapon's model and texture live in.
pub const WEAPON_DIR: &str = "Item\\ObjectComponents\\Weapon";

/// A held weapon's model path from its ItemDisplayInfo model name.
pub fn weapon_model_path(model_name: &str) -> String {
    format!("{WEAPON_DIR}\\{model_name}")
}

/// The folder helmet models live in.
pub const HELM_DIR: &str = "Item\\ObjectComponents\\Head";

/// A helmet's model for a race (1-8: Hu Or Dw Ni Sc Ta Gn Tr) and sex (0 male, 1 female): the row
/// names only the stem, the files are per race and sex, as the client builds them.
pub fn helm_model_path(model_name: &str, race: u8, sex: u8) -> String {
    const RACE_PREFIX: [&str; 8] = ["Hu", "Or", "Dw", "Ni", "Sc", "Ta", "Gn", "Tr"];
    let prefix = RACE_PREFIX[(race.clamp(1, 8) - 1) as usize];
    let letter = if sex == 1 { 'F' } else { 'M' };
    let lower = model_name.to_ascii_lowercase();
    let stem = if lower.ends_with(".mdx") || lower.ends_with(".mdl") {
        &model_name[..model_name.len() - 4]
    } else if lower.ends_with(".m2") {
        &model_name[..model_name.len() - 3]
    } else {
        model_name
    };
    format!("{HELM_DIR}\\{stem}_{prefix}{letter}.m2")
}

/// An archive path as the `mpq://` URL the asset server loads, lowercased with forward slashes.
pub fn mpq_url(path: &str) -> String {
    format!("mpq://{}", path.replace('\\', "/").to_ascii_lowercase())
}

/// A character-model NPC's baked body atlas under `Textures\BakedNpcTextures\`.
pub fn baked_npc_url(bake_name: &str) -> String {
    mpq_url(&format!("Textures\\BakedNpcTextures\\{bake_name}"))
}

/// Whether two archive paths name the same file (case and slash direction ignored).
pub fn same_path(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes().zip(b.bytes()).all(|(x, y)| {
            let n = |c: u8| {
                if c == b'/' {
                    b'\\'
                } else {
                    c.to_ascii_lowercase()
                }
            };
            n(x) == n(y)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helmets_are_per_race_and_sex() {
        assert_eq!(
            helm_model_path("Helm_Robe_A_01.mdx", 8, 1),
            "Item\\ObjectComponents\\Head\\Helm_Robe_A_01_TrF.m2"
        );
        assert_eq!(
            helm_model_path("Helm_Plate_B_01", 1, 0),
            "Item\\ObjectComponents\\Head\\Helm_Plate_B_01_HuM.m2"
        );
    }

    #[test]
    fn character_paths() {
        assert_eq!(
            character_model_path(Race::TROLL, Sex::Female).as_deref(),
            Some("Character\\Troll\\Female\\TrollFemale.mdx")
        );
        assert_eq!(
            character_model_path(Race::UNDEAD, Sex::Male).as_deref(),
            Some("Character\\Scourge\\Male\\ScourgeMale.mdx")
        );
        assert_eq!(character_model_path(Race(11), Sex::Male), None);
    }

    #[test]
    fn dirs() {
        assert_eq!(
            model_dir("Creature\\Kobold\\Kobold.mdx"),
            "Creature\\Kobold"
        );
        assert_eq!(model_dir("Kobold.mdx"), "");
    }

    #[test]
    fn urls() {
        assert_eq!(
            weapon_model_path("stave_2h_long_a_01.m2"),
            "Item\\ObjectComponents\\Weapon\\stave_2h_long_a_01.m2"
        );
        assert_eq!(
            mpq_url("Character\\Troll\\Hair00_00.blp"),
            "mpq://character/troll/hair00_00.blp"
        );
        assert_eq!(
            baked_npc_url("HumanMale_Defias.blp"),
            "mpq://textures/bakednpctextures/humanmale_defias.blp"
        );
    }

    #[test]
    fn path_compare_ignores_case_and_slashes() {
        assert!(same_path(
            "Creature\\Wolf\\Wolf.mdx",
            "creature/wolf/WOLF.MDX"
        ));
        assert!(!same_path(
            "Creature\\Wolf\\Wolf.mdx",
            "Creature\\Wolf\\Worg.mdx"
        ));
    }
}
