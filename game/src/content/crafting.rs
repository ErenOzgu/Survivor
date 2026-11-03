//! Professions: materials enemies drop, cooking (one meal taken into a run for its buff),
//! blacksmithing (an item's bonuses made stronger) and enchanting (one extra bonus on an item).
//! Own design and numbers (CLAUDE.md §2); the material names are WoW's, their icons the
//! player's client's.

use super::items::{Affix, MAX_FORGE};
use super::EnemyKind;

/// A crafting material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mat {
    LinenCloth,
    CopperOre,
    StrangeDust,
    WolfMeat,
    SpiderLeg,
}

impl Mat {
    pub const ALL: [Mat; 5] = [
        Mat::LinenCloth,
        Mat::CopperOre,
        Mat::StrangeDust,
        Mat::WolfMeat,
        Mat::SpiderLeg,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mat::LinenCloth => "Keten Kumaş",
            Mat::CopperOre => "Bakır Cevheri",
            Mat::StrangeDust => "Garip Toz",
            Mat::WolfMeat => "Kurt Eti",
            Mat::SpiderLeg => "Örümcek Bacağı",
        }
    }

    /// The client icon (`Interface\Icons\…` name).
    pub fn icon(self) -> &'static str {
        match self {
            Mat::LinenCloth => "INV_Fabric_Linen_01",
            Mat::CopperOre => "INV_Ore_Copper_01",
            Mat::StrangeDust => "INV_Enchant_DustStrange",
            Mat::WolfMeat => "INV_Misc_Food_14",
            Mat::SpiderLeg => "INV_Misc_MonsterSpiderCarapace_01",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Mat::LinenCloth => "linen",
            Mat::CopperOre => "ore",
            Mat::StrangeDust => "dust",
            Mat::WolfMeat => "meat",
            Mat::SpiderLeg => "leg",
        }
    }

    pub fn from_key(k: &str) -> Option<Mat> {
        Self::ALL.into_iter().find(|m| m.key() == k)
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// What a common enemy of a kind may drop: `(material, chance 0–1)`.
pub fn drops_for(kind: EnemyKind) -> &'static [(Mat, f32)] {
    match kind {
        EnemyKind::Kobold => &[(Mat::CopperOre, 0.06), (Mat::LinenCloth, 0.05)],
        EnemyKind::Geomancer => &[(Mat::LinenCloth, 0.08), (Mat::StrangeDust, 0.03)],
        EnemyKind::Sapper => &[(Mat::CopperOre, 0.1)],
        EnemyKind::Wolf => &[(Mat::WolfMeat, 0.1)],
        EnemyKind::Spider => &[(Mat::SpiderLeg, 0.1)],
        EnemyKind::GnollChief | EnemyKind::DefiasBoss => &[],
    }
}

/// What a boss leaves on top of its loot.
pub const BOSS_DROPS: [(Mat, u32); 3] = [
    (Mat::StrangeDust, 3),
    (Mat::CopperOre, 2),
    (Mat::LinenCloth, 2),
];

/// A meal: cooked from materials, one taken into a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Food {
    WolfKebab,
    SpiderSoup,
    HuntersStew,
}

/// A meal's recipe and what it gives for the run.
pub struct FoodDef {
    pub name: &'static str,
    pub icon: &'static str,
    pub cost: &'static [(Mat, u32)],
    pub desc: &'static str,
    /// The bonus for the run, as an item bonus.
    pub buff: (Affix, f32),
}

impl Food {
    pub const ALL: [Food; 3] = [Food::WolfKebab, Food::SpiderSoup, Food::HuntersStew];

    pub fn def(self) -> &'static FoodDef {
        match self {
            Food::WolfKebab => &FoodDef {
                name: "Kurt Kebabı",
                icon: "INV_Misc_Food_14",
                cost: &[(Mat::WolfMeat, 2)],
                desc: "Koşu boyunca +25 en yüksek can.",
                buff: (Affix::Health, 25.0),
            },
            Food::SpiderSoup => &FoodDef {
                name: "Örümcek Çorbası",
                icon: "INV_Misc_Food_09",
                cost: &[(Mat::SpiderLeg, 2), (Mat::WolfMeat, 1)],
                desc: "Koşu boyunca %10 daha hızlı koşarsın.",
                buff: (Affix::Speed, 0.10),
            },
            Food::HuntersStew => &FoodDef {
                name: "Avcı Yahnisi",
                icon: "INV_Misc_Food_15",
                cost: &[(Mat::WolfMeat, 2), (Mat::SpiderLeg, 2)],
                desc: "Koşu boyunca büyüler %10 fazla vurur.",
                buff: (Affix::Power, 0.10),
            },
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Food::WolfKebab => "kebab",
            Food::SpiderSoup => "soup",
            Food::HuntersStew => "stew",
        }
    }

    pub fn from_key(k: &str) -> Option<Food> {
        Self::ALL.into_iter().find(|f| f.key() == k)
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Blacksmithing's cost for the next forge level (after `level` are done), `None` at the top.
pub fn forge_cost(level: u8) -> Option<[(Mat, u32); 2]> {
    (level < MAX_FORGE).then(|| {
        let n = level as u32 + 1;
        [(Mat::CopperOre, 3 * n), (Mat::LinenCloth, n)]
    })
}

/// An enchant: its name, the bonus it puts on an item and what it costs.
pub struct EnchantDef {
    pub name: &'static str,
    pub bonus: (Affix, f32),
    pub cost: &'static [(Mat, u32)],
}

pub const ENCHANTS: [EnchantDef; 5] = [
    EnchantDef {
        name: "Küçük Güç",
        bonus: (Affix::Power, 0.06),
        cost: &[(Mat::StrangeDust, 3)],
    },
    EnchantDef {
        name: "Küçük Keskinlik",
        bonus: (Affix::Crit, 0.04),
        cost: &[(Mat::StrangeDust, 2)],
    },
    EnchantDef {
        name: "Küçük Dayanıklılık",
        bonus: (Affix::Health, 15.0),
        cost: &[(Mat::StrangeDust, 2)],
    },
    EnchantDef {
        name: "Küçük Çeviklik",
        bonus: (Affix::Speed, 0.05),
        cost: &[(Mat::StrangeDust, 2), (Mat::LinenCloth, 1)],
    },
    EnchantDef {
        name: "Küçük Bilgelik",
        bonus: (Affix::Wisdom, 0.08),
        cost: &[(Mat::StrangeDust, 1), (Mat::LinenCloth, 2)],
    },
];

/// Whether `have` (counts by [`Mat::index`]) covers `cost`.
pub fn affords(have: &[u32; 5], cost: &[(Mat, u32)]) -> bool {
    cost.iter().all(|(m, n)| have[m.index()] >= *n)
}

/// Takes `cost` from `have`; false (and nothing taken) if it does not cover it.
pub fn pay(have: &mut [u32; 5], cost: &[(Mat, u32)]) -> bool {
    if !affords(have, cost) {
        return false;
    }
    for (m, n) in cost {
        have[m.index()] -= n;
    }
    true
}

/// A cost as a line: "3 Bakır Cevheri, 1 Keten Kumaş".
pub fn cost_line(cost: &[(Mat, u32)]) -> String {
    cost.iter()
        .map(|(m, n)| format!("{n} {}", m.name()))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_are_paid_only_in_full() {
        let mut have = [0u32; 5];
        have[Mat::WolfMeat.index()] = 3;
        let kebab = Food::WolfKebab.def().cost;
        assert!(pay(&mut have, kebab));
        assert_eq!(have[Mat::WolfMeat.index()], 1);
        assert!(!pay(&mut have, kebab));
        assert_eq!(have[Mat::WolfMeat.index()], 1, "nothing taken");
        assert_eq!(cost_line(kebab), "2 Kurt Eti");
    }

    #[test]
    fn forging_gets_dearer_and_stops() {
        assert_eq!(forge_cost(0).unwrap()[0].1, 3);
        assert_eq!(forge_cost(2).unwrap()[0].1, 9);
        assert!(forge_cost(MAX_FORGE).is_none());
    }

    #[test]
    fn keys_round_trip() {
        for m in Mat::ALL {
            assert_eq!(Mat::from_key(m.key()), Some(m));
        }
        for f in Food::ALL {
            assert_eq!(Food::from_key(f.key()), Some(f));
        }
    }
}
