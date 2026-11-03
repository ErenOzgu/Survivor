//! Phase 5, progression: the Mage's talent tree, bought with talent points earned from the XP
//! gathered in runs. Own design (CLAUDE.md §2): three columns of three tiers; a tier opens with
//! points spent in its column.

/// Talent columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Column {
    /// Defence.
    Frost,
    /// Offence.
    Fire,
    /// Utility.
    Arcane,
}

impl Column {
    pub const ALL: [Column; 3] = [Column::Frost, Column::Fire, Column::Arcane];

    pub fn name(self) -> &'static str {
        match self {
            Column::Frost => "Buz · Savunma",
            Column::Fire => "Ateş · Saldırı",
            Column::Arcane => "Arkan · Fayda",
        }
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            Column::Frost => [0.45, 0.8, 1.0],
            Column::Fire => [1.0, 0.45, 0.1],
            Column::Arcane => [0.85, 0.45, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TalentId {
    IceShell,
    ColdBlood,
    IceBlock,
    Fervor,
    Ignition,
    Searing,
    ArcaneMind,
    Insight,
    Preparation,
}

/// What a talent does, per rank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TalentEffect {
    /// Max health, hit points.
    Health(f32),
    /// Health regenerated per second.
    Regen(f32),
    /// Once per run, death is refused: back up with this share of health and a moment safe.
    Revive(f32),
    /// Spell damage, a fraction.
    Damage(f32),
    /// Critical chance.
    Crit(f32),
    /// Added to the crit multiplier.
    CritPower(f32),
    /// Cooldown reduction.
    Haste(f32),
    /// XP gained.
    Xp(f32),
    /// Extra card picks at the start of a run.
    StartCards(u32),
}

#[derive(Debug)]
pub struct TalentDef {
    pub name: &'static str,
    pub column: Column,
    /// 0, 1 or 2; tier `t` needs `TIER_GATE[t]` points spent in the column.
    pub tier: usize,
    pub max_rank: u8,
    pub effect: TalentEffect,
    /// Turkish, with `{}` for the per-rank number.
    pub desc: &'static str,
    /// The WoW spell whose icon it shows.
    pub icon_spell: &'static str,
}

/// Points a column needs before each tier opens.
pub const TIER_GATE: [u32; 3] = [0, 3, 5];

impl TalentId {
    pub const ALL: [TalentId; 9] = [
        TalentId::IceShell,
        TalentId::ColdBlood,
        TalentId::IceBlock,
        TalentId::Fervor,
        TalentId::Ignition,
        TalentId::Searing,
        TalentId::ArcaneMind,
        TalentId::Insight,
        TalentId::Preparation,
    ];

    pub fn def(self) -> &'static TalentDef {
        match self {
            TalentId::IceShell => &TalentDef {
                name: "Buz Kabuğu",
                column: Column::Frost,
                tier: 0,
                max_rank: 3,
                effect: TalentEffect::Health(10.0),
                desc: "En yüksek can +{} (her puan).",
                icon_spell: "Frost Armor",
            },
            TalentId::ColdBlood => &TalentDef {
                name: "Soğuk Kan",
                column: Column::Frost,
                tier: 1,
                max_rank: 2,
                effect: TalentEffect::Regen(0.3),
                desc: "Saniyede +{} can yenilenir (her puan).",
                icon_spell: "Ice Barrier",
            },
            TalentId::IceBlock => &TalentDef {
                name: "Buz Bloğu",
                column: Column::Frost,
                tier: 2,
                max_rank: 1,
                effect: TalentEffect::Revive(0.4),
                desc: "Koşu başına bir kez ölmezsin: canının %{}'ı ile kalkarsın, 3 sn hasar almazsın.",
                icon_spell: "Ice Block",
            },
            TalentId::Fervor => &TalentDef {
                name: "Kızgınlık",
                column: Column::Fire,
                tier: 0,
                max_rank: 3,
                effect: TalentEffect::Damage(0.04),
                desc: "Büyü hasarı +%{} (her puan).",
                icon_spell: "Fire Power",
            },
            TalentId::Ignition => &TalentDef {
                name: "Tutuşma",
                column: Column::Fire,
                tier: 1,
                max_rank: 2,
                effect: TalentEffect::Crit(0.03),
                desc: "Kritik şansı +%{} (her puan).",
                icon_spell: "Critical Mass",
            },
            TalentId::Searing => &TalentDef {
                name: "Yakıcı Güç",
                column: Column::Fire,
                tier: 2,
                max_rank: 1,
                effect: TalentEffect::CritPower(0.5),
                desc: "Kritik vuruşlar 2 yerine 2,5 kat vurur.",
                icon_spell: "Combustion",
            },
            TalentId::ArcaneMind => &TalentDef {
                name: "Arkan Zihin",
                column: Column::Arcane,
                tier: 0,
                max_rank: 3,
                effect: TalentEffect::Haste(0.03),
                desc: "Bekleme süreleri -%{} (her puan).",
                icon_spell: "Arcane Mind",
            },
            TalentId::Insight => &TalentDef {
                name: "Bilgelik Pınarı",
                column: Column::Arcane,
                tier: 1,
                max_rank: 2,
                effect: TalentEffect::Xp(0.08),
                desc: "Kürelerden +%{} XP (her puan).",
                icon_spell: "Arcane Intellect",
            },
            TalentId::Preparation => &TalentDef {
                name: "Hazırlık",
                column: Column::Arcane,
                tier: 2,
                max_rank: 1,
                effect: TalentEffect::StartCards(1),
                desc: "Her koşu fazladan {} kart seçimiyle başlar.",
                icon_spell: "Evocation",
            },
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            TalentId::IceShell => "iceshell",
            TalentId::ColdBlood => "coldblood",
            TalentId::IceBlock => "iceblock",
            TalentId::Fervor => "fervor",
            TalentId::Ignition => "ignition",
            TalentId::Searing => "searing",
            TalentId::ArcaneMind => "arcanemind",
            TalentId::Insight => "insight",
            TalentId::Preparation => "preparation",
        }
    }

    pub fn from_key(k: &str) -> Option<TalentId> {
        Self::ALL.into_iter().find(|t| t.key() == k)
    }
}

impl TalentDef {
    /// The card line with the per-rank number filled in.
    pub fn line(&self) -> String {
        let n = match self.effect {
            TalentEffect::Health(v) => format!("{v:.0}"),
            TalentEffect::Regen(v) => format!("{v}"),
            TalentEffect::Revive(v) => format!("{:.0}", v * 100.0),
            TalentEffect::Damage(v)
            | TalentEffect::Crit(v)
            | TalentEffect::Haste(v)
            | TalentEffect::Xp(v) => format!("{:.0}", v * 100.0),
            TalentEffect::CritPower(v) => format!("{v}"),
            TalentEffect::StartCards(n) => n.to_string(),
        };
        self.desc.replace("{}", &n)
    }
}

/// XP a talent point costs: the `n`th point (0-based) costs a little more than the one before.
pub fn point_cost(n: u32) -> u64 {
    150 + 60 * n as u64
}

/// Points earned with `xp` talent XP in total.
pub fn points_for_xp(mut xp: u64) -> u32 {
    let mut n = 0;
    while xp >= point_cost(n) {
        xp -= point_cost(n);
        n += 1;
    }
    n
}

/// Talent XP still needed for the next point.
pub fn xp_to_next_point(xp: u64) -> u64 {
    let mut spent = 0;
    let mut n = 0;
    while spent + point_cost(n) <= xp {
        spent += point_cost(n);
        n += 1;
    }
    spent + point_cost(n) - xp
}

/// Bought ranks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Talents(pub Vec<(TalentId, u8)>);

impl Talents {
    pub fn rank(&self, t: TalentId) -> u8 {
        self.0
            .iter()
            .find(|(id, _)| *id == t)
            .map_or(0, |(_, r)| *r)
    }

    pub fn spent(&self) -> u32 {
        self.0.iter().map(|(_, r)| *r as u32).sum()
    }

    pub fn spent_in(&self, c: Column) -> u32 {
        self.0
            .iter()
            .filter(|(t, _)| t.def().column == c)
            .map(|(_, r)| *r as u32)
            .sum()
    }

    /// Whether one more rank of `t` can be bought with `points` earned in all.
    pub fn can_buy(&self, t: TalentId, points: u32) -> bool {
        let d = t.def();
        self.spent() < points
            && self.rank(t) < d.max_rank
            && self.spent_in(d.column) >= TIER_GATE[d.tier]
    }

    /// Buys one rank if allowed; returns whether it did.
    pub fn buy(&mut self, t: TalentId, points: u32) -> bool {
        if !self.can_buy(t, points) {
            return false;
        }
        match self.0.iter_mut().find(|(id, _)| *id == t) {
            Some(e) => e.1 += 1,
            None => self.0.push((t, 1)),
        }
        true
    }

    /// The summed effect of every bought rank of one kind.
    pub fn total(&self, pick: impl Fn(TalentEffect) -> Option<f32>) -> f32 {
        self.0
            .iter()
            .filter_map(|(t, r)| pick(t.def().effect).map(|v| v * *r as f32))
            .sum()
    }

    pub fn to_save(&self) -> String {
        self.0
            .iter()
            .map(|(t, r)| format!("{}:{r}", t.key()))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// Reads [`Self::to_save`]; ranks are clamped and gates re-checked, so a hand-edited file
    /// cannot buy what the rules would not.
    pub fn from_save(s: &str) -> Talents {
        let mut want: Vec<(TalentId, u8)> = s
            .split(',')
            .filter_map(|p| {
                let (k, r) = p.split_once(':')?;
                Some((TalentId::from_key(k.trim())?, r.trim().parse().ok()?))
            })
            .collect();
        want.sort_by_key(|(t, _)| t.def().tier);
        let mut out = Talents::default();
        for (t, r) in want {
            for _ in 0..r {
                out.buy(t, u32::MAX);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_open_with_points_in_the_column() {
        let mut t = Talents::default();
        assert!(!t.can_buy(TalentId::ColdBlood, 10), "tier 2 is closed");
        for _ in 0..3 {
            assert!(t.buy(TalentId::IceShell, 10));
        }
        assert!(!t.buy(TalentId::IceShell, 10), "max rank");
        assert!(t.can_buy(TalentId::ColdBlood, 10));
        assert!(!t.can_buy(TalentId::IceBlock, 10), "needs 5");
        t.buy(TalentId::ColdBlood, 10);
        t.buy(TalentId::ColdBlood, 10);
        assert!(t.buy(TalentId::IceBlock, 10));
        assert!(!t.can_buy(TalentId::Fervor, 6), "out of points");
        assert_eq!(t.spent(), 6);
        assert_eq!(t.spent_in(Column::Frost), 6);
    }

    #[test]
    fn every_talent_reads_and_saves() {
        for t in TalentId::ALL {
            let l = t.def().line();
            assert!(!l.contains("{}"), "{t:?}: {l}");
            assert_eq!(TalentId::from_key(t.key()), Some(t));
        }
        assert_eq!(TalentId::Fervor.def().line(), "Büyü hasarı +%4 (her puan).");
        let mut t = Talents::default();
        for id in [
            TalentId::Fervor,
            TalentId::Fervor,
            TalentId::Fervor,
            TalentId::Ignition,
        ] {
            t.buy(id, 99);
        }
        assert_eq!(Talents::from_save(&t.to_save()), t);
    }

    #[test]
    fn a_hand_edited_save_cannot_skip_gates() {
        let t = Talents::from_save("iceblock:1,searing:9,fervor:7,x:1");
        assert_eq!(t.rank(TalentId::IceBlock), 0, "gate not met");
        assert_eq!(t.rank(TalentId::Fervor), 3, "clamped");
        assert_eq!(t.rank(TalentId::Searing), 0);
    }

    #[test]
    fn points_come_from_xp() {
        assert_eq!(points_for_xp(0), 0);
        assert_eq!(points_for_xp(149), 0);
        assert_eq!(points_for_xp(150), 1);
        assert_eq!(points_for_xp(150 + 210), 2);
        assert_eq!(xp_to_next_point(0), 150);
        assert_eq!(xp_to_next_point(160), 200);
        let all: u32 = TalentId::ALL.iter().map(|t| t.def().max_rank as u32).sum();
        let xp_for_all: u64 = (0..all).map(point_cost).sum();
        assert!(
            xp_for_all > 5_000,
            "the tree takes a few full runs: {xp_for_all}"
        );
    }

    #[test]
    fn totals_add_up() {
        let mut t = Talents::default();
        t.buy(TalentId::IceShell, 9);
        t.buy(TalentId::IceShell, 9);
        let hp = t.total(|e| match e {
            TalentEffect::Health(v) => Some(v),
            _ => None,
        });
        assert_eq!(hp, 20.0);
    }
}
