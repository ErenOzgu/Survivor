//! What a character looks like: race, sex, the customization dials and what it wears, and the
//! hero's look from the environment.

/// `RNS_HERO_RACE`: the hero's race, by name (`troll`, `human`, `orc`, `dwarf`, `nightelf`,
/// `undead`, `tauren`, `gnome`) or ChrRaces id 1..8.
pub const RACE_VAR: &str = "RNS_HERO_RACE";
/// `RNS_HERO_SEX`: `female`/`f`/`1` or `male`/`m`/`0`.
pub const SEX_VAR: &str = "RNS_HERO_SEX";
/// `RNS_HERO_LOOK`: `skin,face,hair_style,hair_color,facial_hair`, the character-create dials.
pub const LOOK_VAR: &str = "RNS_HERO_LOOK";
/// `RNS_HERO_GEAR`: the worn items as `slot:display_id` pairs (ItemDisplayInfo ids), e.g.
/// `chest:12345,mainhand:472`; `none` for nothing; unset for the default mage gear.
pub const GEAR_VAR: &str = "RNS_HERO_GEAR";

/// A playable race, by its `ChrRaces.dbc` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Race(pub u8);

#[allow(dead_code)] // the named races are API for game code
impl Race {
    pub const HUMAN: Race = Race(1);
    pub const ORC: Race = Race(2);
    pub const DWARF: Race = Race(3);
    pub const NIGHT_ELF: Race = Race(4);
    pub const UNDEAD: Race = Race(5);
    pub const TAUREN: Race = Race(6);
    pub const GNOME: Race = Race(7);
    pub const TROLL: Race = Race(8);

    /// A race by name or id; case and `-`/`_`/space are ignored.
    pub fn parse(s: &str) -> Option<Race> {
        let k: String = s
            .trim()
            .chars()
            .filter(|c| !matches!(c, '-' | '_' | ' '))
            .collect::<String>()
            .to_ascii_lowercase();
        let r = match k.as_str() {
            "human" => 1,
            "orc" => 2,
            "dwarf" => 3,
            "nightelf" | "elf" => 4,
            "undead" | "scourge" | "forsaken" => 5,
            "tauren" => 6,
            "gnome" => 7,
            "troll" => 8,
            n => match n.parse::<u8>() {
                Ok(id @ 1..=8) => id,
                _ => return None,
            },
        };
        Some(Race(r))
    }
}

/// Body sex as the client numbers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sex {
    Male = 0,
    Female = 1,
}

impl Sex {
    pub fn parse(s: &str) -> Option<Sex> {
        match s.trim().to_ascii_lowercase().as_str() {
            "male" | "m" | "0" => Some(Sex::Male),
            "female" | "f" | "1" => Some(Sex::Female),
            _ => None,
        }
    }

    pub fn id(self) -> u8 {
        self as u8
    }
}

/// Where a worn item goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GearSlot {
    Head,
    Shirt,
    Chest,
    Belt,
    Legs,
    Feet,
    Wrist,
    Hands,
    Tabard,
    MainHand,
}

impl GearSlot {
    /// The body slot index the composite and geoset tables use (shirt 0 … tabard 7), for the eight
    /// slots painted onto the body.
    pub fn body_index(self) -> Option<usize> {
        Some(match self {
            GearSlot::Shirt => 0,
            GearSlot::Chest => 1,
            GearSlot::Belt => 2,
            GearSlot::Legs => 3,
            GearSlot::Feet => 4,
            GearSlot::Wrist => 5,
            GearSlot::Hands => 6,
            GearSlot::Tabard => 7,
            GearSlot::Head | GearSlot::MainHand => return None,
        })
    }

    /// The slot an `InventoryType` (Item.dbc / CharStartOutfit) is worn in.
    pub fn from_inventory_type(inv: u8) -> Option<GearSlot> {
        Some(match inv {
            1 => GearSlot::Head,
            4 => GearSlot::Shirt,
            5 | 20 => GearSlot::Chest, // chest, robe
            6 => GearSlot::Belt,
            7 => GearSlot::Legs,
            8 => GearSlot::Feet,
            9 => GearSlot::Wrist,
            10 => GearSlot::Hands,
            19 => GearSlot::Tabard,
            13 | 17 | 21 => GearSlot::MainHand, // one-hand, two-hand, main hand
            _ => return None,
        })
    }

    pub fn parse(s: &str) -> Option<GearSlot> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "head" | "helm" => GearSlot::Head,
            "shirt" => GearSlot::Shirt,
            "chest" | "robe" => GearSlot::Chest,
            "belt" | "waist" => GearSlot::Belt,
            "legs" | "pants" => GearSlot::Legs,
            "feet" | "boots" => GearSlot::Feet,
            "wrist" => GearSlot::Wrist,
            "hands" | "gloves" => GearSlot::Hands,
            "tabard" => GearSlot::Tabard,
            "mainhand" | "weapon" | "staff" => GearSlot::MainHand,
            _ => return None,
        })
    }
}

/// One worn item: its slot and its `ItemDisplayInfo.dbc` id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GearItem {
    pub slot: GearSlot,
    pub display_id: u32,
}

/// What a character wears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gear {
    /// Exactly these items.
    Items(Vec<GearItem>),
    /// A robe and a staff for a mage, chosen from the client's own tables at load
    /// ([`super::gear`]).
    MageDefault,
    /// The mage default with these slots replaced: each `(slot, seed)` wears the client's
    /// item look for that slot the seed picks ([`super::gear::slot_displays`]).
    MageWith(Vec<(GearSlot, u32)>),
}

/// A player-character body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterLook {
    pub race: Race,
    pub sex: Sex,
    pub skin: u8,
    pub face: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub facial_hair: u8,
    pub gear: Gear,
}

impl CharacterLook {
    /// A body with the first of every dial, wearing `gear`.
    pub fn new(race: Race, sex: Sex, gear: Gear) -> Self {
        Self {
            race,
            sex,
            skin: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            facial_hair: 0,
            gear,
        }
    }

    /// The hero: a Troll female mage unless the `RNS_HERO_*` variables say otherwise.
    pub fn hero_from_env() -> Self {
        let get = |k: &str| std::env::var(k).ok();
        hero_look(
            get(RACE_VAR).as_deref(),
            get(SEX_VAR).as_deref(),
            get(LOOK_VAR).as_deref(),
            get(GEAR_VAR).as_deref(),
        )
    }
}

/// [`CharacterLook::hero_from_env`] over the raw values; a malformed value warns and keeps the
/// default.
pub fn hero_look(
    race: Option<&str>,
    sex: Option<&str>,
    look: Option<&str>,
    gear: Option<&str>,
) -> CharacterLook {
    let mut l = CharacterLook::new(Race::TROLL, Sex::Female, Gear::MageDefault);
    if let Some(v) = race {
        match Race::parse(v) {
            Some(r) => l.race = r,
            None => bevy::log::warn!("rnsurvivors: {RACE_VAR}={v:?} is not a race — using troll"),
        }
    }
    if let Some(v) = sex {
        match Sex::parse(v) {
            Some(s) => l.sex = s,
            None => bevy::log::warn!("rnsurvivors: {SEX_VAR}={v:?} is not a sex — using female"),
        }
    }
    if let Some(v) = look {
        match parse_dials(v) {
            Some([skin, face, hair_style, hair_color, facial_hair]) => {
                l.skin = skin;
                l.face = face;
                l.hair_style = hair_style;
                l.hair_color = hair_color;
                l.facial_hair = facial_hair;
            }
            None => bevy::log::warn!(
                "rnsurvivors: {LOOK_VAR}={v:?} wants `skin,face,hair_style,hair_color,facial_hair`"
            ),
        }
    }
    if let Some(v) = gear {
        match parse_gear(v) {
            Some(items) => l.gear = Gear::Items(items),
            None => bevy::log::warn!(
                "rnsurvivors: {GEAR_VAR}={v:?} wants `slot:display_id,…` — using the mage gear"
            ),
        }
    }
    l
}

/// Five comma-separated numbers 0..=255; fewer leave the rest at 0.
pub fn parse_dials(s: &str) -> Option<[u8; 5]> {
    let mut out = [0u8; 5];
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    if parts.is_empty() || parts.len() > 5 {
        return None;
    }
    for (i, p) in parts.iter().enumerate() {
        out[i] = p.parse().ok()?;
    }
    Some(out)
}

/// `slot:id` pairs separated by commas, or `none` for an empty list.
pub fn parse_gear(s: &str) -> Option<Vec<GearItem>> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    s.split(',')
        .map(|pair| {
            let (slot, id) = pair.split_once(':')?;
            Some(GearItem {
                slot: GearSlot::parse(slot)?,
                display_id: id.trim().parse().ok().filter(|&id| id != 0)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn races_parse_by_name_and_id() {
        assert_eq!(Race::parse("Troll"), Some(Race::TROLL));
        assert_eq!(Race::parse("night-elf"), Some(Race::NIGHT_ELF));
        assert_eq!(Race::parse(" undead "), Some(Race::UNDEAD));
        assert_eq!(Race::parse("3"), Some(Race::DWARF));
        assert_eq!(Race::parse("9"), None);
        assert_eq!(Race::parse("murloc"), None);
    }

    #[test]
    fn sexes_parse() {
        assert_eq!(Sex::parse("F"), Some(Sex::Female));
        assert_eq!(Sex::parse("male"), Some(Sex::Male));
        assert_eq!(Sex::parse("0"), Some(Sex::Male));
        assert_eq!(Sex::parse("x"), None);
    }

    #[test]
    fn default_hero_is_a_troll_female_mage() {
        let l = hero_look(None, None, None, None);
        assert_eq!((l.race, l.sex), (Race::TROLL, Sex::Female));
        assert_eq!(l.gear, Gear::MageDefault);
    }

    #[test]
    fn env_values_override_and_bad_ones_keep_the_default() {
        let l = hero_look(Some("gnome"), Some("m"), Some("2,1,3"), Some("chest:100"));
        assert_eq!((l.race, l.sex), (Race::GNOME, Sex::Male));
        assert_eq!((l.skin, l.face, l.hair_style, l.hair_color), (2, 1, 3, 0));
        assert_eq!(
            l.gear,
            Gear::Items(vec![GearItem {
                slot: GearSlot::Chest,
                display_id: 100
            }])
        );
        let bad = hero_look(Some("murloc"), Some("?"), Some("a"), Some("x"));
        assert_eq!(bad, hero_look(None, None, None, None));
    }

    #[test]
    fn gear_lists_parse() {
        assert_eq!(
            parse_gear("robe:5, staff:7"),
            Some(vec![
                GearItem {
                    slot: GearSlot::Chest,
                    display_id: 5
                },
                GearItem {
                    slot: GearSlot::MainHand,
                    display_id: 7
                },
            ])
        );
        assert_eq!(parse_gear("none"), Some(vec![]));
        assert_eq!(parse_gear("chest:0"), None);
        assert_eq!(parse_gear("chest"), None);
        assert_eq!(parse_gear("ring:5"), None);
    }

    #[test]
    fn dials_parse() {
        assert_eq!(parse_dials("1,2,3,4,5"), Some([1, 2, 3, 4, 5]));
        assert_eq!(parse_dials("7"), Some([7, 0, 0, 0, 0]));
        assert_eq!(parse_dials("1,2,3,4,5,6"), None);
        assert_eq!(parse_dials("300"), None);
    }

    #[test]
    fn inventory_types_map_to_slots() {
        assert_eq!(GearSlot::from_inventory_type(20), Some(GearSlot::Chest));
        assert_eq!(GearSlot::from_inventory_type(17), Some(GearSlot::MainHand));
        assert_eq!(GearSlot::from_inventory_type(11), None); // a ring is not drawn
        assert_eq!(GearSlot::Chest.body_index(), Some(1));
        assert_eq!(GearSlot::MainHand.body_index(), None);
    }
}
