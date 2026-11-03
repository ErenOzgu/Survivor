//! The zones a run can be played in. Each keeps the first map's rules (the same enemy roles,
//! waves and boss fights) but its own ground, its own creatures (the client's models for that
//! part of the world, found by words in their paths), its own names and a harder fight. A zone
//! opens when the one before it is cleared. Own design (CLAUDE.md §2); the places and creature
//! models are WoW's, from the player's client.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::bosses::{BossDef, ELWYNN_BOSSES};
use super::EnemyKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ZoneId {
    #[default]
    Elwynn,
    Westfall,
    Redridge,
    Duskwood,
}

/// A zone: where it is, how hard, what its creatures look like and are called.
pub struct ZoneDef {
    pub name: &'static str,
    pub sub: &'static str,
    /// Where a run starts and where the guarding elite waits, WoW world `x, y`.
    pub start: (f32, f32),
    pub guard: (f32, f32),
    /// On enemies' and bosses' health and damage.
    pub level_mul: f32,
    /// Per enemy role (in [`EnemyKind::NORMAL`] order, then the elite's and the map boss's
    /// bodies): words its model path may hold, best first; empty keeps Elwynn's look.
    pub looks: [&'static [&'static str]; 7],
    /// The same roles' names; empty keeps Elwynn's.
    pub names: [&'static str; 7],
    /// The four bosses' names, in [`ELWYNN_BOSSES`] order; empty keeps Elwynn's.
    pub boss_names: [&'static str; 4],
}

impl ZoneId {
    pub const ALL: [ZoneId; 4] = [
        ZoneId::Elwynn,
        ZoneId::Westfall,
        ZoneId::Redridge,
        ZoneId::Duskwood,
    ];

    pub fn def(self) -> &'static ZoneDef {
        match self {
            ZoneId::Elwynn => &ELWYNN,
            ZoneId::Westfall => &WESTFALL,
            ZoneId::Redridge => &REDRIDGE,
            ZoneId::Duskwood => &DUSKWOOD,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ZoneId::Elwynn => "elwynn",
            ZoneId::Westfall => "westfall",
            ZoneId::Redridge => "redridge",
            ZoneId::Duskwood => "duskwood",
        }
    }

    pub fn from_key(k: &str) -> Option<ZoneId> {
        Self::ALL.into_iter().find(|z| z.key() == k.trim())
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

static ELWYNN: ZoneDef = ZoneDef {
    name: "Elwynn Ormanı",
    sub: "Goldshire çevresi",
    start: crate::config::DEFAULT_START,
    guard: super::waves::GNOLL_CHIEF_AT,
    level_mul: 1.0,
    looks: [&[], &[], &[], &[], &[], &[], &[]],
    names: ["", "", "", "", "", "", ""],
    boss_names: ["", "", "", ""],
};

static WESTFALL: ZoneDef = ZoneDef {
    name: "Westfall",
    sub: "Sentinel Tepesi'nin tarlaları",
    start: (-10420.0, 1150.0),
    guard: (-9900.0, 1350.0),
    level_mul: 1.35,
    looks: [
        &["murloc"],
        &["coyote", "jackal", "wolf"],
        &["vulture", "buzzard"],
        &["murloc"],
        &["harvestgolem", "golem"],
        &["harvestgolem", "golem"],
        &[],
    ],
    names: [
        "Murloc Akıncı",
        "Çakal",
        "Leş Akbabası",
        "Murloc Kahini",
        "Hasat Bekçisi",
        "Dev Hasat Golemi",
        "",
    ],
    boss_names: [
        "Paslı Hasat Golemi",
        "Murloc Kabile Reisi",
        "Kızıl Akbaba",
        "Defias Kaptanı",
    ],
};

static REDRIDGE: ZoneDef = ZoneDef {
    name: "Redridge Dağları",
    sub: "Lakeshire'ın kuzeyi",
    start: (-9150.0, -2050.0),
    guard: (-9050.0, -2350.0),
    level_mul: 1.75,
    looks: [
        &["gnoll"],
        &["wolf"],
        &["tarantula", "spider"],
        &["gnoll"],
        &["kobold"],
        &["gnoll"],
        &["dragonwhelp", "whelp"],
    ],
    names: [
        "Redridge Gnollu",
        "Dağ Kurdu",
        "Tarantula",
        "Gnoll Mistiği",
        "Tünel Faresi",
        "Gnoll Savaş Şefi",
        "Kara Ejder Yavrusu",
    ],
    boss_names: [
        "Gnoll Savaş Şefi",
        "Tünel Faresi Ustası",
        "Tarantula Ana",
        "Kara Ejder Yavrusu",
    ],
};

static DUSKWOOD: ZoneDef = ZoneDef {
    name: "Duskwood",
    sub: "Darkshire'ın kıyısı",
    start: (-10400.0, -1100.0),
    guard: (-10700.0, -900.0),
    level_mul: 2.2,
    looks: [
        &["skeleton"],
        &["worgen"],
        &["spider"],
        &["skeleton"],
        &["ghoul", "zombie"],
        &["worgen"],
        &["abomination", "ghoul", "skeleton"],
    ],
    names: [
        "İskelet",
        "Kurtadam",
        "Gece Örümceği",
        "İskelet Büyücü",
        "Hortlak",
        "Kurtadam Sürü Lideri",
        "Mezar Devi",
    ],
    boss_names: [
        "Kurtadam Sürü Lideri",
        "İskelet Lord",
        "Kara Dul",
        "Mezar Devi",
    ],
};

/// The zone the run is in (set when a run starts in it).
static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn current() -> ZoneId {
    ZoneId::ALL[CURRENT.load(Ordering::Relaxed).min(ZoneId::ALL.len() - 1)]
}

pub fn set_current(z: ZoneId) {
    CURRENT.store(z.index(), Ordering::Relaxed);
}

/// A role's slot in [`ZoneDef::looks`] and [`ZoneDef::names`].
fn role(kind: EnemyKind) -> usize {
    match kind {
        EnemyKind::Kobold => 0,
        EnemyKind::Wolf => 1,
        EnemyKind::Spider => 2,
        EnemyKind::Geomancer => 3,
        EnemyKind::Sapper => 4,
        EnemyKind::GnollChief => 5,
        EnemyKind::DefiasBoss => 6,
    }
}

/// An enemy's name in the current zone, if the zone renames it.
pub fn enemy_name(kind: EnemyKind) -> Option<&'static str> {
    enemy_name_in(current(), kind)
}

pub fn enemy_name_in(zone: ZoneId, kind: EnemyKind) -> Option<&'static str> {
    let n = zone.def().names[role(kind)];
    (!n.is_empty()).then_some(n)
}

/// The words a kind's model is found by in the current zone (empty: Elwynn's look).
pub fn enemy_looks(kind: EnemyKind) -> &'static [&'static str] {
    current().def().looks[role(kind)]
}

/// A boss's name in the current zone, if the zone renames it.
pub fn boss_name(def: &BossDef) -> Option<&'static str> {
    boss_name_in(current(), def)
}

pub fn boss_name_in(zone: ZoneId, def: &BossDef) -> Option<&'static str> {
    let idx = ELWYNN_BOSSES.iter().position(|b| std::ptr::eq(*b, def))?;
    let n = zone.def().boss_names[idx];
    (!n.is_empty()).then_some(n)
}

/// Whether a zone is open: the first always; the next once the one before it was cleared.
pub fn unlocked(z: ZoneId, cleared: &[ZoneId]) -> bool {
    z.index() == 0 || cleared.iter().any(|c| c.index() + 1 == z.index())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_open_in_order_and_rename() {
        assert!(unlocked(ZoneId::Elwynn, &[]));
        assert!(!unlocked(ZoneId::Westfall, &[]));
        assert!(unlocked(ZoneId::Westfall, &[ZoneId::Elwynn]));
        assert!(!unlocked(ZoneId::Duskwood, &[ZoneId::Elwynn]));
        for z in ZoneId::ALL {
            assert_eq!(ZoneId::from_key(z.key()), Some(z));
            assert!(z.def().level_mul >= 1.0);
        }
        set_current(ZoneId::Westfall);
        assert_eq!(enemy_name(EnemyKind::Kobold), Some("Murloc Akıncı"));
        assert_eq!(boss_name(ELWYNN_BOSSES[0]), Some(WESTFALL.boss_names[0]));
        set_current(ZoneId::Elwynn);
        assert_eq!(enemy_name(EnemyKind::Kobold), None);
    }
}
