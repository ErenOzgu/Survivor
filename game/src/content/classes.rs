//! The classes. Every caster plays the same skill roles (a slowing bolt, a bursting bolt, a
//! volley, a freezing nova, two ground areas, a burst around the hero, an escape, and the three
//! evolutions), so the numbers stay balanced; a class gives them its own WoW spells: their
//! names, icons and effects (from the player's client), its own Turkish lines and its schools'
//! names. Own design (CLAUDE.md §2).

use std::sync::atomic::{AtomicUsize, Ordering};

use super::SkillId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClassId {
    #[default]
    Mage,
    Warlock,
    Priest,
}

/// One skill role as a class plays it: the WoW spell (its icon and effects), the shown name and
/// the card line. Empty keeps the Mage's.
#[derive(Debug, Clone, Copy)]
pub struct ClassSkill {
    pub spell: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

pub struct ClassDef {
    pub name: &'static str,
    pub blurb: &'static str,
    /// The class colour (sRGB), as WoW's.
    pub color: [f32; 3],
    /// Its square in the character-creation class icons (texture coordinates).
    pub icon_uv: [f32; 4],
    /// The three schools' names, in the Mage's frost/fire/arcane slots.
    pub schools: [&'static str; 3],
    /// Per [`SkillId::every`] slot.
    pub skills: [ClassSkill; 11],
}

const fn cs(spell: &'static str, name: &'static str, desc: &'static str) -> ClassSkill {
    ClassSkill { spell, name, desc }
}

const KEEP: ClassSkill = cs("", "", "");

static MAGE: ClassDef = ClassDef {
    name: "Büyücü",
    blurb: "Buzla yavaşlatır, ateşle yakar, arkanla her yeri vurur.",
    color: [0.41, 0.8, 0.94],
    icon_uv: [0.25, 0.0, 0.49609375, 0.25],
    schools: ["Buz", "Ateş", "Arkan"],
    skills: [KEEP; 11],
};

static WARLOCK: ClassDef = ClassDef {
    name: "Kara Büyücü",
    blurb: "Gölgeyle sinsice yavaşlatır, cehennem ateşiyle yakar, korkuyla durdurur.",
    color: [0.58, 0.51, 0.79],
    icon_uv: [0.7421875, 0.25, 0.98828125, 0.5],
    schools: ["Gölge", "Ateş", "Lanet"],
    skills: [
        cs(
            "Shadow Bolt",
            "Shadow Bolt",
            "En yakın düşmana gölge oku; vurduğunu yavaşlatır.",
        ),
        cs(
            "Soul Fire",
            "Soul Fire",
            "Yavaş ama güçlü bir ruh ateşi; çarptığı yerde patlar.",
        ),
        cs(
            "Searing Pain",
            "Searing Pain",
            "Hızlı alev okları en yakın düşmanlara saplanır.",
        ),
        cs(
            "Howl of Terror",
            "Howl of Terror",
            "Etrafındakileri dehşetle olduğu yere çakar.",
        ),
        cs(
            "Rain of Fire",
            "Rain of Fire",
            "Ateş yağmuru yağdırır; içindekiler yavaşlar.",
        ),
        cs(
            "Immolate",
            "Immolate",
            "Düşmanların arasına yanan bir alan bırakır.",
        ),
        cs("Hellfire", "Hellfire", "Çevresine cehennem ateşi saçar."),
        cs(
            "Shadow Ward",
            "Gölge Kayması",
            "Tehlikede gölgelerin arasından kayıp kurtulur.",
        ),
        cs(
            "Shadow Bolt",
            "Gölge Fırtınası",
            "Gölge okları sağanağı; yavaşlatıp durdurur.",
        ),
        cs(
            "Soul Fire",
            "Ruh Yangını",
            "Dev bir ruh ateşi; patladığı yer yanmaya devam eder.",
        ),
        cs(
            "Searing Pain",
            "Kavurucu Yağmur",
            "Sayısız alev oku bütün yakın düşmanlara.",
        ),
    ],
};

static PRIEST: ClassDef = ClassDef {
    name: "Rahip",
    blurb: "Kutsal ışıkla çarpar, zihni bükerek durdurur, gölgeyle yavaş yavaş eritir.",
    color: [1.0, 1.0, 1.0],
    icon_uv: [0.49609375, 0.25, 0.7421875, 0.5],
    schools: ["Kutsal", "Kutsal Ateş", "Gölge"],
    skills: [
        cs(
            "Smite",
            "Smite",
            "En yakın düşmanı kutsal ışıkla çarpar; yavaşlatır.",
        ),
        cs(
            "Holy Fire",
            "Holy Fire",
            "Yavaş ama güçlü kutsal ateş; çarptığı yerde patlar.",
        ),
        cs(
            "Mind Flay",
            "Mind Flay",
            "Zihin dalgaları en yakın düşmanları dövüp geçer.",
        ),
        cs(
            "Psychic Scream",
            "Psychic Scream",
            "Bir çığlıkla etrafındakileri yerine dondurur.",
        ),
        cs(
            "Shadow Word: Pain",
            "Shadow Word: Pain",
            "Yere acı bırakır; içindekiler yavaşlar.",
        ),
        cs(
            "Starshards",
            "Starshards",
            "Gökten yıldız parçaları yağar, yeri yakar.",
        ),
        cs(
            "Holy Nova",
            "Holy Nova",
            "Çevresine kutsal bir dalga yayar.",
        ),
        cs("Fade", "Fade", "Tehlikede gözden kaybolup kaçar."),
        cs(
            "Smite",
            "Kutsal Gazap",
            "Işık okları sağanağı; yavaşlatıp durdurur.",
        ),
        cs(
            "Holy Fire",
            "Işık Patlaması",
            "Dev bir kutsal ateş; patladığı yer yanmaya devam eder.",
        ),
        cs(
            "Mind Flay",
            "Zihin Fırtınası",
            "Durmayan zihin dalgaları bütün yakın düşmanlara.",
        ),
    ],
};

impl ClassId {
    pub const ALL: [ClassId; 3] = [ClassId::Mage, ClassId::Warlock, ClassId::Priest];

    pub fn def(self) -> &'static ClassDef {
        match self {
            ClassId::Mage => &MAGE,
            ClassId::Warlock => &WARLOCK,
            ClassId::Priest => &PRIEST,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ClassId::Mage => "mage",
            ClassId::Warlock => "warlock",
            ClassId::Priest => "priest",
        }
    }

    pub fn from_key(k: &str) -> Option<ClassId> {
        Self::ALL.into_iter().find(|c| c.key() == k.trim())
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn current() -> ClassId {
    ClassId::ALL[CURRENT.load(Ordering::Relaxed).min(ClassId::ALL.len() - 1)]
}

pub fn set_current(c: ClassId) {
    CURRENT.store(c.index(), Ordering::Relaxed);
}

/// A skill as `class` plays it (the Mage's own where the class keeps it).
pub fn skill_in(class: ClassId, id: SkillId) -> Option<&'static ClassSkill> {
    let slot = SkillId::every().position(|s| s == id)?;
    let s = &class.def().skills[slot];
    (!s.spell.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_names_every_role() {
        assert_eq!(SkillId::every().count(), 11);
        for c in [ClassId::Warlock, ClassId::Priest] {
            for id in SkillId::every() {
                let s = skill_in(c, id).expect("named");
                assert!(!s.name.is_empty() && !s.desc.is_empty(), "{c:?} {id:?}");
            }
        }
        assert!(skill_in(ClassId::Mage, SkillId::Frostbolt).is_none());
        for c in ClassId::ALL {
            assert_eq!(ClassId::from_key(c.key()), Some(c));
        }
    }
}
