//! The first map's elite, mini bosses and map boss (phase 3), and their special attacks.
//!
//! Every name and number here is Rnsurvivors' own design (CLAUDE.md §2). The elite and the map
//! boss take their base stats from `enemies.rs` (`GNOLL_CHIEF`, `DEFIAS_BOSS`); the two mini
//! bosses have their own stats below. A boss's `body` is the normal enemy kind whose look it
//! borrows (`combat::visuals::attach_enemy_visual`), so the visuals code needs no new kinds.

use super::enemies::{EnemyDef, EnemyKind, DEFIAS_BOSS, GNOLL_CHIEF};
use super::waves::{BOSS_AT, GNOLL_CHIEF_AT};

/// What a boss is in the map's structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    /// Waits at a fixed spot; once per run.
    Elite,
    /// Arrives at a set time in the run, with a warning.
    Mini,
    /// Arrives on the map timer; killing it opens the portal.
    MapBoss,
}

/// How a boss enters the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arrival {
    /// Stands at WoW world `x, y` until the hero comes within `aggro` yards (or hurts it).
    Guard { at: (f32, f32), aggro: f32 },
    /// Appears on the spawn ring around the hero at `at` seconds into the run.
    Timed { at: f32 },
}

/// A special attack's shape. Every one is telegraphed on the ground during its wind-up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttackShape {
    /// A circle around the boss itself.
    Slam { radius: f32 },
    /// A straight dash toward where the hero stood when the wind-up began.
    Charge { speed: f32, length: f32, width: f32 },
    /// `count` circles of `radius` dropped around the hero's position (one on it).
    Barrage {
        count: u32,
        radius: f32,
        spread: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackDef {
    /// Turkish name, shown as a short call-out when it starts.
    pub name: &'static str,
    pub shape: AttackShape,
    /// The boss uses it only when the hero is between these distances (yards).
    pub min_range: f32,
    pub max_range: f32,
    /// Seconds of telegraph before it lands.
    pub windup: f32,
    /// Seconds after it lands before the next special attack may start.
    pub recover: f32,
    /// Damage to the hero if caught (before the run's wave damage multiplier).
    pub damage: f32,
}

#[derive(Debug)]
pub struct BossDef {
    pub rank: Rank,
    pub stats: &'static EnemyDef,
    /// Whose look it borrows.
    pub body: EnemyKind,
    pub arrival: Arrival,
    /// The hero level this boss is balanced for; health scales around it.
    pub expected_level: u32,
    /// Extra XP orbs (each worth `bonus_xp`) scattered where it dies.
    pub bonus_orbs: u32,
    pub bonus_xp: u32,
    /// Seconds before its first special attack once it is fighting.
    pub first_attack: f32,
    pub attacks: &'static [AttackDef],
    /// The ring under its feet that marks it as special (sRGB).
    pub ring_color: [f32; 3],
}

impl BossDef {
    /// Its name in the zone being played.
    pub fn name(&self) -> &'static str {
        super::zones::boss_name(self).unwrap_or(self.stats.name)
    }

    pub fn arrives_at(&self) -> Option<f32> {
        match self.arrival {
            Arrival::Timed { at } => Some(at),
            Arrival::Guard { .. } => None,
        }
    }
}

// --- Elite: the gnoll chief at the mine mouth -------------------------------------------------

pub static GNOLL_CHIEF_BOSS: BossDef = BossDef {
    rank: Rank::Elite,
    stats: &GNOLL_CHIEF,
    body: EnemyKind::GnollChief,
    arrival: Arrival::Guard {
        at: GNOLL_CHIEF_AT,
        aggro: 16.0,
    },
    expected_level: 7,
    bonus_orbs: 8,
    bonus_xp: 6,
    first_attack: 2.0,
    attacks: &[
        AttackDef {
            name: "Yer Sarsıntısı",
            shape: AttackShape::Slam { radius: 5.5 },
            min_range: 0.0,
            max_range: 6.0,
            windup: 1.2,
            recover: 3.5,
            damage: 30.0,
        },
        AttackDef {
            name: "Hücum",
            shape: AttackShape::Charge {
                speed: 20.0,
                length: 18.0,
                width: 2.4,
            },
            min_range: 7.0,
            max_range: 18.0,
            windup: 1.0,
            recover: 4.0,
            damage: 24.0,
        },
    ],
    ring_color: [1.0, 0.75, 0.2],
};

// --- Mini boss 1: the kobold foreman, 3:00 ---------------------------------------------------

/// A kobold overseer with a sack of mining charges: slow, sturdy, throws bombs.
pub static KOBOLD_FOREMAN: EnemyDef = EnemyDef {
    name: "Kobold Ustabaşı",
    health: 420.0,
    speed: 3.0,
    contact_dps: 14.0,
    xp: 25,
    radius: 1.0,
    color: [0.85, 0.5, 0.15],
    scale: 1.6,
};

pub static KOBOLD_FOREMAN_BOSS: BossDef = BossDef {
    rank: Rank::Mini,
    stats: &KOBOLD_FOREMAN,
    body: EnemyKind::Kobold,
    arrival: Arrival::Timed { at: 180.0 },
    expected_level: 9,
    bonus_orbs: 6,
    bonus_xp: 5,
    first_attack: 2.5,
    attacks: &[AttackDef {
        name: "Maden Patlayıcısı",
        shape: AttackShape::Barrage {
            count: 3,
            radius: 3.0,
            spread: 4.5,
        },
        min_range: 0.0,
        max_range: 20.0,
        windup: 1.5,
        recover: 3.5,
        damage: 22.0,
    }],
    ring_color: [1.0, 0.45, 0.1],
};

// --- Mini boss 2: the grey pack leader, 6:30 -------------------------------------------------

/// A big old wolf that leads the packs: fast, lunges across the screen.
pub static PACK_LEADER: EnemyDef = EnemyDef {
    name: "Gri Pençe",
    health: 900.0,
    speed: 5.6,
    contact_dps: 18.0,
    xp: 35,
    radius: 1.1,
    color: [0.75, 0.75, 0.8],
    scale: 1.7,
};

pub static PACK_LEADER_BOSS: BossDef = BossDef {
    rank: Rank::Mini,
    stats: &PACK_LEADER,
    body: EnemyKind::Wolf,
    arrival: Arrival::Timed { at: 390.0 },
    expected_level: 14,
    bonus_orbs: 8,
    bonus_xp: 7,
    first_attack: 2.0,
    attacks: &[AttackDef {
        name: "Atılış",
        shape: AttackShape::Charge {
            speed: 26.0,
            length: 22.0,
            width: 2.2,
        },
        min_range: 5.0,
        max_range: 22.0,
        windup: 0.8,
        recover: 2.6,
        damage: 26.0,
    }],
    ring_color: [0.7, 0.85, 1.0],
};

// --- Map boss: the Defias leader, on the map timer -------------------------------------------

pub static DEFIAS_LEADER_BOSS: BossDef = BossDef {
    rank: Rank::MapBoss,
    stats: &DEFIAS_BOSS,
    body: EnemyKind::DefiasBoss,
    arrival: Arrival::Timed { at: BOSS_AT },
    expected_level: 18,
    bonus_orbs: 12,
    bonus_xp: 10,
    first_attack: 3.0,
    attacks: &[
        AttackDef {
            name: "Kara Barut",
            shape: AttackShape::Barrage {
                count: 5,
                radius: 3.2,
                spread: 6.0,
            },
            min_range: 0.0,
            max_range: 24.0,
            windup: 1.6,
            recover: 2.5,
            damage: 30.0,
        },
        AttackDef {
            name: "Kılıç Hücumu",
            shape: AttackShape::Charge {
                speed: 24.0,
                length: 20.0,
                width: 2.6,
            },
            min_range: 6.0,
            max_range: 20.0,
            windup: 0.9,
            recover: 2.5,
            damage: 34.0,
        },
        AttackDef {
            name: "Savuruş",
            shape: AttackShape::Slam { radius: 6.0 },
            min_range: 0.0,
            max_range: 5.0,
            windup: 1.0,
            recover: 2.0,
            damage: 36.0,
        },
    ],
    ring_color: [0.95, 0.15, 0.15],
};

/// Every boss of the first map, in a fixed order (their index is their id in a run).
pub static ELWYNN_BOSSES: [&BossDef; 4] = [
    &GNOLL_CHIEF_BOSS,
    &KOBOLD_FOREMAN_BOSS,
    &PACK_LEADER_BOSS,
    &DEFIAS_LEADER_BOSS,
];

/// Seconds of warning before a timed boss arrives.
pub const WARN_LEAD: f32 = 8.0;
/// Seconds the "has arrived" banner stays after it does.
pub const WARN_LINGER: f32 = 3.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_elite_two_minis_one_map_boss() {
        let count = |r| ELWYNN_BOSSES.iter().filter(|b| b.rank == r).count();
        assert_eq!(count(Rank::Elite), 1);
        assert_eq!(count(Rank::Mini), 2);
        assert_eq!(count(Rank::MapBoss), 1);
    }

    #[test]
    fn timed_bosses_arrive_in_order_and_before_nothing_overlaps() {
        let times: Vec<f32> = ELWYNN_BOSSES
            .iter()
            .filter_map(|b| b.arrives_at())
            .collect();
        assert_eq!(times.len(), 3);
        for w in times.windows(2) {
            // Warnings never overlap.
            assert!(w[1] - w[0] > WARN_LEAD + WARN_LINGER, "{times:?}");
        }
        assert_eq!(*times.last().unwrap(), BOSS_AT);
        assert_eq!(DEFIAS_LEADER_BOSS.rank, Rank::MapBoss);
    }

    #[test]
    fn minis_sit_between_normals_and_the_map_boss() {
        for b in [&KOBOLD_FOREMAN_BOSS, &PACK_LEADER_BOSS] {
            assert!(b.stats.health > 10.0 * super::super::enemies::SPIDER.health);
            assert!(b.stats.health < DEFIAS_BOSS.health);
            assert!(EnemyKind::NORMAL.contains(&b.body));
        }
    }

    #[test]
    fn every_boss_has_a_sane_special_attack() {
        for b in ELWYNN_BOSSES {
            assert!(!b.attacks.is_empty(), "{}", b.name());
            for a in b.attacks {
                assert!(a.windup >= 0.6, "{}: telegraph too short to dodge", a.name);
                assert!(a.min_range <= a.max_range && a.damage > 0.0 && a.recover > 0.0);
                match a.shape {
                    AttackShape::Slam { radius } => assert!(radius > b.stats.radius),
                    AttackShape::Charge { speed, length, .. } => {
                        assert!(speed > crate::motion::HERO_SPEED && length > 0.0)
                    }
                    AttackShape::Barrage { count, radius, .. } => {
                        assert!(count >= 1 && radius > 0.0)
                    }
                }
            }
        }
    }
}
