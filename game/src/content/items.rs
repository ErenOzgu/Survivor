//! Phase 4, loot: gear the hero keeps between runs. Own design (CLAUDE.md §2): six slots, four
//! rarities, and bonuses ("affixes") on the same stats the passives use. Items are rolled, not
//! listed: a seed fixes each item's numbers and its look (an `ItemDisplayInfo` row the client
//! offers for that slot).

use crate::combat::rules::Rng;

/// Where an item is worn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemSlot {
    Head,
    Chest,
    Hands,
    Legs,
    Feet,
    Weapon,
}

impl ItemSlot {
    pub const ALL: [ItemSlot; 6] = [
        ItemSlot::Head,
        ItemSlot::Chest,
        ItemSlot::Hands,
        ItemSlot::Legs,
        ItemSlot::Feet,
        ItemSlot::Weapon,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ItemSlot::Head => "Baş",
            ItemSlot::Chest => "Göğüs",
            ItemSlot::Hands => "Eller",
            ItemSlot::Legs => "Bacaklar",
            ItemSlot::Feet => "Ayaklar",
            ItemSlot::Weapon => "Silah",
        }
    }

    /// The item's base name at each rarity tier.
    fn base_names(self) -> [&'static str; 4] {
        match self {
            ItemSlot::Head => ["Bez Başlık", "Büyücü Şapkası", "Arkan Taç", "Yıldız Tacı"],
            ItemSlot::Chest => [
                "Çırak Cübbesi",
                "Büyücü Cübbesi",
                "Arkan Cübbe",
                "Yıldız Cübbesi",
            ],
            ItemSlot::Hands => [
                "Bez Eldiven",
                "Büyücü Eldiveni",
                "Arkan Eldiven",
                "Yıldız Eldiveni",
            ],
            ItemSlot::Legs => ["Bez Pantolon", "Büyücü Eteği", "Arkan Etek", "Yıldız Eteği"],
            ItemSlot::Feet => [
                "Bez Ayakkabı",
                "Büyücü Çizmesi",
                "Arkan Çizme",
                "Yıldız Çizmesi",
            ],
            ItemSlot::Weapon => ["Çırak Asası", "Büyücü Asası", "Arkan Asa", "Yıldız Asası"],
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ItemSlot::Head => "head",
            ItemSlot::Chest => "chest",
            ItemSlot::Hands => "hands",
            ItemSlot::Legs => "legs",
            ItemSlot::Feet => "feet",
            ItemSlot::Weapon => "weapon",
        }
    }

    pub fn from_key(k: &str) -> Option<ItemSlot> {
        Self::ALL.into_iter().find(|s| s.key() == k)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

impl Rarity {
    pub const ALL: [Rarity; 4] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic];

    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Sıradan",
            Rarity::Uncommon => "Kaliteli",
            Rarity::Rare => "Nadir",
            Rarity::Epic => "Destansı",
        }
    }

    /// The genre's colours: white, green, blue, purple.
    pub fn color(self) -> [f32; 3] {
        match self {
            Rarity::Common => [0.95, 0.95, 0.95],
            Rarity::Uncommon => [0.12, 1.0, 0.0],
            Rarity::Rare => [0.0, 0.44, 0.87],
            Rarity::Epic => [0.64, 0.21, 0.93],
        }
    }

    /// How many bonuses an item of this rarity rolls, and how strong they are.
    pub fn affix_count(self) -> usize {
        match self {
            Rarity::Common => 1,
            Rarity::Uncommon => 2,
            Rarity::Rare => 3,
            Rarity::Epic => 3,
        }
    }

    pub fn power(self) -> f32 {
        match self {
            Rarity::Common => 1.0,
            Rarity::Uncommon => 1.4,
            Rarity::Rare => 1.9,
            Rarity::Epic => 2.6,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A bonus an item can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Affix {
    Power,
    Haste,
    Speed,
    Health,
    Magnet,
    Crit,
    Wisdom,
}

impl Affix {
    pub const ALL: [Affix; 7] = [
        Affix::Power,
        Affix::Haste,
        Affix::Speed,
        Affix::Health,
        Affix::Magnet,
        Affix::Crit,
        Affix::Wisdom,
    ];

    /// The full value of a common, level-1 roll (a fraction; hit points for Health).
    pub fn base(self) -> f32 {
        match self {
            Affix::Power => 0.04,
            Affix::Haste => 0.03,
            Affix::Speed => 0.03,
            Affix::Health => 8.0,
            Affix::Magnet => 0.08,
            Affix::Crit => 0.02,
            Affix::Wisdom => 0.04,
        }
    }

    /// The adjective an item with this as its main bonus is named with.
    pub fn adjective(self) -> &'static str {
        match self {
            Affix::Power => "Güçlü",
            Affix::Haste => "Akışkan",
            Affix::Speed => "Hafif",
            Affix::Health => "Sağlam",
            Affix::Magnet => "Çekici",
            Affix::Crit => "Keskin",
            Affix::Wisdom => "Bilge",
        }
    }

    /// The tooltip line for a value.
    pub fn line(self, v: f32) -> String {
        let pct = (v * 100.0 * 10.0).round() / 10.0;
        match self {
            Affix::Power => format!("+%{pct} büyü hasarı"),
            Affix::Haste => format!("-%{pct} bekleme süresi"),
            Affix::Speed => format!("+%{pct} koşu hızı"),
            Affix::Health => format!("+{v:.0} en yüksek can"),
            Affix::Magnet => format!("+%{pct} XP çekim alanı"),
            Affix::Crit => format!("+%{pct} kritik şansı"),
            Affix::Wisdom => format!("+%{pct} XP"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Affix::Power => "power",
            Affix::Haste => "haste",
            Affix::Speed => "speed",
            Affix::Health => "health",
            Affix::Magnet => "magnet",
            Affix::Crit => "crit",
            Affix::Wisdom => "wisdom",
        }
    }

    pub fn from_key(k: &str) -> Option<Affix> {
        Self::ALL.into_iter().find(|a| a.key() == k)
    }
}

/// One item.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub slot: ItemSlot,
    pub rarity: Rarity,
    /// The hero's level when it dropped; the bonuses grow a little with it.
    pub level: u32,
    pub affixes: Vec<(Affix, f32)>,
    /// Picks the item's look among the client's displays for the slot.
    pub look: u32,
}

impl Item {
    /// "Keskin Büyücü Cübbesi": the main bonus's adjective and the slot's base name for the
    /// rarity.
    pub fn name(&self) -> String {
        let base = self.slot.base_names()[self.rarity.index()];
        match self.affixes.first() {
            Some((a, _)) => format!("{} {base}", a.adjective()),
            None => base.to_string(),
        }
    }

    /// Tooltip lines: rarity, slot, level, then each bonus.
    pub fn lines(&self) -> Vec<String> {
        let mut out = vec![format!(
            "{} · {} · seviye {}",
            self.rarity.name(),
            self.slot.name(),
            self.level
        )];
        out.extend(self.affixes.iter().map(|(a, v)| a.line(*v)));
        out
    }

    /// The bonus of one kind this item gives.
    pub fn bonus(&self, affix: Affix) -> f32 {
        self.affixes
            .iter()
            .filter(|(a, _)| *a == affix)
            .map(|(_, v)| v)
            .sum()
    }

    /// One line per field, for the save: `slot rarity level look affix:value …`.
    pub fn to_save(&self) -> String {
        let mut s = format!(
            "{} {} {} {}",
            self.slot.key(),
            self.rarity.index(),
            self.level,
            self.look
        );
        for (a, v) in &self.affixes {
            s.push_str(&format!(" {}:{v}", a.key()));
        }
        s
    }

    pub fn from_save(s: &str) -> Option<Item> {
        let mut parts = s.split_whitespace();
        let slot = ItemSlot::from_key(parts.next()?)?;
        let rarity = *Rarity::ALL.get(parts.next()?.parse::<usize>().ok()?)?;
        let level = parts.next()?.parse().ok()?;
        let look = parts.next()?.parse().ok()?;
        let mut affixes = Vec::new();
        for p in parts {
            let (k, v) = p.split_once(':')?;
            let v: f32 = v.parse().ok().filter(|v: &f32| v.is_finite())?;
            affixes.push((Affix::from_key(k)?, v));
        }
        Some(Item {
            slot,
            rarity,
            level,
            affixes,
            look,
        })
    }
}

/// Who dropped it, which sets the rarity odds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An elite or a mini boss: one item.
    Elite,
    /// The map boss's chest: three items, better odds.
    Chest,
}

impl Source {
    /// Relative odds of common, uncommon, rare, epic.
    pub fn odds(self) -> [f32; 4] {
        match self {
            Source::Elite => [50.0, 35.0, 13.0, 2.0],
            Source::Chest => [25.0, 42.0, 25.0, 8.0],
        }
    }

    pub fn count(self) -> usize {
        match self {
            Source::Elite => 1,
            Source::Chest => 3,
        }
    }
}

/// The highest item level.
pub const MAX_ITEM_LEVEL: u32 = 60;

/// Rolls one item at `level` from `source`.
pub fn roll_item(rng: &mut Rng, source: Source, level: u32) -> Item {
    let level = level.clamp(1, MAX_ITEM_LEVEL);
    let slot = ItemSlot::ALL[rng.below(ItemSlot::ALL.len())];
    let odds = source.odds();
    let total: f32 = odds.iter().sum();
    let mut r = rng.f32() * total;
    let mut rarity = Rarity::Common;
    for (w, rar) in odds.iter().zip(Rarity::ALL) {
        if r < *w {
            rarity = rar;
            break;
        }
        r -= w;
        rarity = rar;
    }
    // A weapon always carries spell power first; the rest is drawn without repeats.
    let mut pool: Vec<Affix> = Affix::ALL.to_vec();
    let mut picks = Vec::new();
    if slot == ItemSlot::Weapon {
        pool.retain(|a| *a != Affix::Power);
        picks.push(Affix::Power);
    }
    while picks.len() < rarity.affix_count() && !pool.is_empty() {
        picks.push(pool.swap_remove(rng.below(pool.len())));
    }
    let level_mul = 1.0 + 0.04 * (level - 1) as f32;
    let affixes = picks
        .into_iter()
        .map(|a| {
            let roll = rng.range(0.7, 1.0);
            let v = a.base() * rarity.power() * level_mul * roll;
            // Keep the save short and the tooltip clean.
            let v = if a == Affix::Health {
                v.round()
            } else {
                (v * 1000.0).round() / 1000.0
            };
            (a, v)
        })
        .collect();
    Item {
        slot,
        rarity,
        level,
        affixes,
        look: (rng.next_u64() % 100_000) as u32,
    }
}

/// Rolls a source's drop.
pub fn roll_drop(rng: &mut Rng, source: Source, level: u32) -> Vec<Item> {
    (0..source.count())
        .map(|_| roll_item(rng, source, level))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_follow_the_rules() {
        let mut rng = Rng::new(11);
        let mut seen = std::collections::HashSet::new();
        for i in 0..2000 {
            let src = if i % 2 == 0 {
                Source::Elite
            } else {
                Source::Chest
            };
            let it = roll_item(&mut rng, src, 1 + i % 70);
            assert_eq!(it.affixes.len(), it.rarity.affix_count());
            let kinds: std::collections::HashSet<_> = it.affixes.iter().map(|(a, _)| *a).collect();
            assert_eq!(kinds.len(), it.affixes.len(), "no repeated bonus");
            if it.slot == ItemSlot::Weapon {
                assert_eq!(it.affixes[0].0, Affix::Power);
            }
            assert!(it.level <= MAX_ITEM_LEVEL);
            for (a, v) in &it.affixes {
                assert!(*v > 0.0 && *v <= a.base() * 2.6 * 3.4, "{a:?} {v}");
            }
            seen.insert(it.rarity);
        }
        assert_eq!(seen.len(), 4, "every rarity drops");
    }

    #[test]
    fn chests_beat_elites() {
        let mut rng = Rng::new(5);
        let mut good = |src| {
            (0..3000)
                .filter(|_| roll_item(&mut rng, src, 10).rarity >= Rarity::Rare)
                .count()
        };
        let elite = good(Source::Elite);
        let chest = good(Source::Chest);
        assert!(chest > elite * 3 / 2, "{chest} vs {elite}");
        assert_eq!(roll_drop(&mut rng, Source::Chest, 5).len(), 3);
    }

    #[test]
    fn names_and_lines_read() {
        let it = Item {
            slot: ItemSlot::Chest,
            rarity: Rarity::Rare,
            level: 12,
            affixes: vec![(Affix::Crit, 0.031), (Affix::Health, 14.0)],
            look: 3,
        };
        assert_eq!(it.name(), "Keskin Arkan Cübbe");
        assert_eq!(it.lines()[0], "Nadir · Göğüs · seviye 12");
        assert_eq!(it.lines()[1], "+%3.1 kritik şansı");
        assert_eq!(it.lines()[2], "+14 en yüksek can");
        assert_eq!(it.bonus(Affix::Health), 14.0);
        assert_eq!(it.bonus(Affix::Power), 0.0);
    }

    #[test]
    fn items_round_trip_through_the_save() {
        let mut rng = Rng::new(77);
        for _ in 0..200 {
            let it = roll_item(&mut rng, Source::Chest, 20);
            assert_eq!(Item::from_save(&it.to_save()), Some(it));
        }
        assert_eq!(Item::from_save("hat 1 2 3"), None);
        assert_eq!(Item::from_save("head 9 2 3"), None);
        assert_eq!(Item::from_save("head 1 2 3 power:NaN"), None);
    }
}
