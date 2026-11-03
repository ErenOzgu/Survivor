//! Passive cards: lasting bonuses picked on level-up next to the skills. Own design and numbers
//! (CLAUDE.md §2). A hero carries at most [`MAX_PASSIVES`], each up to its last level.

/// The most passives a hero carries at once.
pub const MAX_PASSIVES: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PassiveId {
    /// More spell damage.
    Power,
    /// Shorter cooldowns.
    Haste,
    /// Faster running.
    Swiftness,
    /// More maximum health.
    Vitality,
    /// Wider XP pickup.
    Attraction,
    /// Critical hits.
    Precision,
    /// More XP from orbs.
    Wisdom,
    /// Dying enemies burst, hurting those around them.
    Volatile,
    /// Dying enemies leave a bleed on those around them.
    Bloodletting,
}

impl PassiveId {
    pub const ALL: [PassiveId; 9] = [
        PassiveId::Power,
        PassiveId::Haste,
        PassiveId::Swiftness,
        PassiveId::Vitality,
        PassiveId::Attraction,
        PassiveId::Precision,
        PassiveId::Wisdom,
        PassiveId::Volatile,
        PassiveId::Bloodletting,
    ];

    pub fn def(self) -> &'static PassiveDef {
        match self {
            PassiveId::Power => &POWER,
            PassiveId::Haste => &HASTE,
            PassiveId::Swiftness => &SWIFTNESS,
            PassiveId::Vitality => &VITALITY,
            PassiveId::Attraction => &ATTRACTION,
            PassiveId::Precision => &PRECISION,
            PassiveId::Wisdom => &WISDOM,
            PassiveId::Volatile => &VOLATILE,
            PassiveId::Bloodletting => &BLOODLETTING,
        }
    }

    /// A lowercase ASCII key, for the debug env var (`RNS_PASSIVES=power:3,haste`).
    pub fn key(self) -> &'static str {
        match self {
            PassiveId::Power => "power",
            PassiveId::Haste => "haste",
            PassiveId::Swiftness => "speed",
            PassiveId::Vitality => "vitality",
            PassiveId::Attraction => "magnet",
            PassiveId::Precision => "crit",
            PassiveId::Wisdom => "wisdom",
            PassiveId::Volatile => "explode",
            PassiveId::Bloodletting => "bleed",
        }
    }

    pub fn from_key(key: &str) -> Option<PassiveId> {
        let key = key.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    pub fn max_level(self) -> u8 {
        self.def().max_level
    }

    /// The bonus at `level` (0 = not owned): `per_level × level`.
    pub fn bonus(self, level: u8) -> f32 {
        self.def().per_level * level.min(self.max_level()) as f32
    }
}

#[derive(Debug)]
pub struct PassiveDef {
    /// Turkish name.
    pub name: &'static str,
    /// What one level gives, Turkish, with `{}` for the per-level number as the card shows it.
    pub desc: &'static str,
    /// The bonus each level adds (a fraction, or hit points for Vitality).
    pub per_level: f32,
    pub max_level: u8,
    /// The WoW spell whose icon the card shows (a name in the player's `Spell.dbc`).
    pub icon_spell: &'static str,
    pub color: [f32; 3],
}

impl PassiveDef {
    /// The card line, the per-level number filled in.
    pub fn line(&self) -> String {
        let n = if self.per_level >= 1.0 {
            format!("{:.0}", self.per_level)
        } else {
            format!("%{:.0}", self.per_level * 100.0)
        };
        self.desc.replace("{}", &n)
    }
}

pub static POWER: PassiveDef = PassiveDef {
    name: "Büyü Gücü",
    desc: "Bütün büyüler {} daha fazla hasar verir.",
    per_level: 0.10,
    max_level: 5,
    icon_spell: "Arcane Power",
    color: [0.85, 0.45, 1.0],
};

pub static HASTE: PassiveDef = PassiveDef {
    name: "Akış",
    desc: "Bekleme süreleri {} kısalır.",
    per_level: 0.07,
    max_level: 5,
    icon_spell: "Presence of Mind",
    color: [0.5, 0.8, 1.0],
};

pub static SWIFTNESS: PassiveDef = PassiveDef {
    name: "Çeviklik",
    desc: "{} daha hızlı koşarsın.",
    per_level: 0.08,
    max_level: 5,
    icon_spell: "Aspect of the Cheetah",
    color: [0.95, 0.85, 0.35],
};

pub static VITALITY: PassiveDef = PassiveDef {
    name: "Dayanıklılık",
    desc: "En yüksek can +{}.",
    per_level: 20.0,
    max_level: 5,
    icon_spell: "Power Word: Fortitude",
    color: [0.95, 0.95, 0.95],
};

pub static ATTRACTION: PassiveDef = PassiveDef {
    name: "Çekim",
    desc: "XP küreleri {} daha uzaktan gelir.",
    per_level: 0.30,
    max_level: 5,
    icon_spell: "Arcane Intellect",
    color: [0.35, 1.0, 0.5],
};

pub static PRECISION: PassiveDef = PassiveDef {
    name: "Keskinlik",
    desc: "Kritik vuruş şansı +{} (kritik 2 kat vurur).",
    per_level: 0.05,
    max_level: 5,
    icon_spell: "Critical Mass",
    color: [1.0, 0.75, 0.2],
};

pub static WISDOM: PassiveDef = PassiveDef {
    name: "Bilgelik",
    desc: "Kürelerden {} daha fazla XP.",
    per_level: 0.10,
    max_level: 5,
    icon_spell: "Arcane Brilliance",
    color: [0.55, 0.7, 1.0],
};

pub static VOLATILE: PassiveDef = PassiveDef {
    name: "Patlayıcı Ölüm",
    desc: "Ölen düşman patlar: en yüksek canının {}'i kadar hasarı yanındakilere verir.",
    per_level: 0.15,
    max_level: 5,
    icon_spell: "Explosive Trap",
    color: [1.0, 0.45, 0.1],
};

pub static BLOODLETTING: PassiveDef = PassiveDef {
    name: "Kanatan Ölüm",
    desc: "Ölen düşman yanındakileri kanatır: 5 sn'de en yüksek canlarının {}'i kadar hasar.",
    per_level: 0.08,
    max_level: 5,
    icon_spell: "Rend",
    color: [0.85, 0.1, 0.15],
};

/// The burst's reach (yards, past the dying enemy's own size) and the bleed's: its reach and how
/// long it runs.
pub const VOLATILE_RADIUS: f32 = 4.0;
pub const BLEED_RADIUS: f32 = 5.0;
pub const BLEED_SECS: f32 = 5.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_fill_the_number() {
        assert_eq!(POWER.line(), "Bütün büyüler %10 daha fazla hasar verir.");
        assert_eq!(VITALITY.line(), "En yüksek can +20.");
        for p in PassiveId::ALL {
            assert!(!p.def().line().contains("{}"), "{p:?}");
            assert!(p.max_level() >= 1);
        }
    }

    #[test]
    fn bonus_grows_and_caps() {
        assert_eq!(PassiveId::Power.bonus(0), 0.0);
        assert!((PassiveId::Power.bonus(3) - 0.3).abs() < 1e-6);
        assert_eq!(PassiveId::Power.bonus(99), PassiveId::Power.bonus(5));
    }
}
