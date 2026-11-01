//! Skills, the loadout a run builds, and the pick-1-of-3 offer on level-up.
//!
//! Skill names are WoW spells (CLAUDE.md §2 allows them); every number, shape and upgrade rule
//! here is this project's own design.

use super::rng::Rng;

/// Playable classes. Only the Mage has a pool so far; more come with their own pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    Mage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkillId {
    // Mage actives.
    Frostbolt,
    Fireball,
    ArcaneMissiles,
    FrostNova,
    Blizzard,
    ArcaneExplosion,
    // Mage passives.
    ArcaneIntellect,
    PresenceOfMind,
}

/// How an active skill reaches its targets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// Projectiles flying at the nearest enemies in range.
    Bolt,
    /// A burst centred on the hero.
    Nova,
    /// An area dropped on an enemy in range, hitting every `tick` for `duration`.
    Ground { tick: f32 },
}

/// What a hit leaves on its target besides damage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    None,
    /// Movement slowed by `slow` (fraction) for `secs`.
    Chill {
        slow: f32,
        secs: f32,
    },
    /// `fraction` of the hit dealt again over `secs`.
    Burn {
        fraction: f32,
        secs: f32,
    },
    /// Cannot move for `secs`.
    Root {
        secs: f32,
    },
}

/// An active skill's numbers at one rank, before the loadout's passives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveStats {
    pub damage: f32,
    /// Seconds between casts.
    pub cooldown: f32,
    /// How far it looks for a target, in yards (0 for a nova).
    pub range: f32,
    /// Projectiles per cast (1 for areas).
    pub count: u32,
    /// Area radius in yards (0 for bolts).
    pub radius: f32,
    /// Seconds a ground area lasts (0 otherwise).
    pub duration: f32,
    pub effect: Effect,
}

/// What a passive skill changes, per rank.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Passive {
    /// `+per_rank` damage fraction.
    Damage { per_rank: f32 },
    /// `-per_rank` cooldown fraction.
    Haste { per_rank: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Active(Shape),
    Passive(Passive),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillDef {
    pub id: SkillId,
    pub name: &'static str,
    pub class: Class,
    pub kind: Kind,
    pub max_rank: u8,
}

/// Damage gained per rank past the first, as a fraction of rank 1.
pub const DAMAGE_PER_RANK: f32 = 0.25;
/// Cooldown lost per rank past the first, as a fraction of rank 1.
pub const COOLDOWN_PER_RANK: f32 = 0.06;
/// The fastest any skill may cast, whatever stacks on it.
pub const MIN_COOLDOWN: f32 = 0.2;

/// How many actives and passives a loadout holds.
pub const MAX_ACTIVES: usize = 4;
pub const MAX_PASSIVES: usize = 2;

/// Choices shown on a level-up.
pub const OFFER_SIZE: usize = 3;

pub fn def(id: SkillId) -> SkillDef {
    use Kind::*;
    use SkillId::*;
    let (name, kind) = match id {
        Frostbolt => ("Frostbolt", Active(Shape::Bolt)),
        Fireball => ("Fireball", Active(Shape::Bolt)),
        ArcaneMissiles => ("Arcane Missiles", Active(Shape::Bolt)),
        FrostNova => ("Frost Nova", Active(Shape::Nova)),
        Blizzard => ("Blizzard", Active(Shape::Ground { tick: 0.5 })),
        ArcaneExplosion => ("Arcane Explosion", Active(Shape::Nova)),
        ArcaneIntellect => (
            "Arcane Intellect",
            Passive(self::Passive::Damage { per_rank: 0.08 }),
        ),
        PresenceOfMind => (
            "Presence of Mind",
            Passive(self::Passive::Haste { per_rank: 0.05 }),
        ),
    };
    SkillDef {
        id,
        name,
        class: Class::Mage,
        kind,
        max_rank: 5,
    }
}

/// The skills a class can be offered.
pub fn pool(class: Class) -> &'static [SkillId] {
    use SkillId::*;
    match class {
        Class::Mage => &[
            Frostbolt,
            Fireball,
            ArcaneMissiles,
            FrostNova,
            Blizzard,
            ArcaneExplosion,
            ArcaneIntellect,
            PresenceOfMind,
        ],
    }
}

/// The skill a class starts every run with.
pub fn starting_skill(class: Class) -> SkillId {
    match class {
        Class::Mage => SkillId::Frostbolt,
    }
}

/// An active skill's numbers at `rank` (1-based); `None` for a passive.
pub fn active_stats(id: SkillId, rank: u8) -> Option<ActiveStats> {
    use SkillId::*;
    let r = f32::from(rank.max(1) - 1);
    let base = match id {
        Frostbolt => ActiveStats {
            damage: 12.0,
            cooldown: 1.4,
            range: 22.0,
            count: 1,
            radius: 0.0,
            duration: 0.0,
            effect: Effect::Chill {
                slow: 0.3,
                secs: 2.0,
            },
        },
        Fireball => ActiveStats {
            damage: 20.0,
            cooldown: 2.2,
            range: 24.0,
            count: 1,
            radius: 0.0,
            duration: 0.0,
            effect: Effect::Burn {
                fraction: 0.25,
                secs: 3.0,
            },
        },
        ArcaneMissiles => ActiveStats {
            damage: 5.0,
            cooldown: 2.0,
            range: 18.0,
            count: 3,
            radius: 0.0,
            duration: 0.0,
            effect: Effect::None,
        },
        FrostNova => ActiveStats {
            damage: 8.0,
            cooldown: 6.0,
            range: 0.0,
            count: 1,
            radius: 6.0,
            duration: 0.0,
            effect: Effect::Root { secs: 1.5 },
        },
        Blizzard => ActiveStats {
            damage: 4.0,
            cooldown: 7.0,
            range: 20.0,
            count: 1,
            radius: 5.0,
            duration: 4.0,
            effect: Effect::Chill {
                slow: 0.4,
                secs: 1.0,
            },
        },
        ArcaneExplosion => ActiveStats {
            damage: 7.0,
            cooldown: 2.5,
            range: 0.0,
            count: 1,
            radius: 4.5,
            duration: 0.0,
            effect: Effect::None,
        },
        ArcaneIntellect | PresenceOfMind => return None,
    };
    // The shared rank rule, then each skill's own extra.
    let mut s = ActiveStats {
        damage: base.damage * (1.0 + DAMAGE_PER_RANK * r),
        cooldown: base.cooldown * (1.0 - COOLDOWN_PER_RANK * r),
        ..base
    };
    match id {
        // A second bolt at rank 4.
        Frostbolt if rank >= 4 => s.count += 1,
        // A second fireball at rank 5.
        Fireball if rank >= 5 => s.count += 1,
        // One more missile per rank.
        ArcaneMissiles => s.count += u32::from(rank.max(1) - 1),
        // Areas widen instead.
        FrostNova => s.radius += 0.6 * r,
        Blizzard => s.radius += 0.5 * r,
        ArcaneExplosion => s.radius += 0.4 * r,
        _ => {}
    }
    Some(s)
}

/// The skills a run holds and their ranks, in the order they were taken.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Loadout {
    pub skills: Vec<(SkillId, u8)>,
}

/// The loadout's passives folded into multipliers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mods {
    pub damage: f32,
    pub cooldown: f32,
}

impl Loadout {
    /// A fresh run's loadout: the class's starting skill at rank 1.
    pub fn new(class: Class) -> Self {
        Self {
            skills: vec![(starting_skill(class), 1)],
        }
    }

    pub fn rank(&self, id: SkillId) -> Option<u8> {
        self.skills.iter().find(|(s, _)| *s == id).map(|&(_, r)| r)
    }

    fn count(&self, passive: bool) -> usize {
        self.skills
            .iter()
            .filter(|(s, _)| matches!(def(*s).kind, Kind::Passive(_)) == passive)
            .count()
    }

    pub fn actives(&self) -> impl Iterator<Item = (SkillId, u8)> + '_ {
        self.skills
            .iter()
            .copied()
            .filter(|(s, _)| matches!(def(*s).kind, Kind::Active(_)))
    }

    /// Whether `id` could be taken as a new skill: not held, and a free slot of its kind.
    fn can_take(&self, id: SkillId) -> bool {
        if self.rank(id).is_some() {
            return false;
        }
        match def(id).kind {
            Kind::Active(_) => self.count(false) < MAX_ACTIVES,
            Kind::Passive(_) => self.count(true) < MAX_PASSIVES,
        }
    }

    pub fn mods(&self) -> Mods {
        let mut m = Mods {
            damage: 1.0,
            cooldown: 1.0,
        };
        for &(id, rank) in &self.skills {
            match def(id).kind {
                Kind::Passive(Passive::Damage { per_rank }) => {
                    m.damage += per_rank * f32::from(rank)
                }
                Kind::Passive(Passive::Haste { per_rank }) => {
                    m.cooldown -= per_rank * f32::from(rank)
                }
                Kind::Active(_) => {}
            }
        }
        m
    }

    /// An owned active's numbers with the passives applied.
    pub fn effective(&self, id: SkillId) -> Option<ActiveStats> {
        let s = active_stats(id, self.rank(id)?)?;
        let m = self.mods();
        Some(ActiveStats {
            damage: s.damage * m.damage,
            cooldown: (s.cooldown * m.cooldown).max(MIN_COOLDOWN),
            ..s
        })
    }

    /// Applies a picked choice.
    pub fn apply(&mut self, choice: Choice) {
        match choice {
            Choice::New(id) => {
                if self.can_take(id) {
                    self.skills.push((id, 1));
                }
            }
            Choice::Upgrade(id) => {
                let max = def(id).max_rank;
                if let Some((_, r)) = self.skills.iter_mut().find(|(s, _)| *s == id) {
                    *r = (*r + 1).min(max);
                }
            }
            // Restore is the hero's health, not the loadout's.
            Choice::Restore => {}
        }
    }
}

/// One card of a level-up offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Learn a new skill at rank 1.
    New(SkillId),
    /// Raise an owned skill one rank.
    Upgrade(SkillId),
    /// Nothing left to learn or raise: heal instead ([`RESTORE_FRACTION`] of max health).
    Restore,
}

/// The health fraction the [`Choice::Restore`] card heals.
pub const RESTORE_FRACTION: f32 = 0.3;

/// Weights of the two kinds of card: upgrades come a little more often than new skills once the
/// loadout has something worth raising.
const UPGRADE_WEIGHT: f32 = 1.2;
const NEW_WEIGHT: f32 = 1.0;

/// Up to [`OFFER_SIZE`] distinct choices for a level-up: upgrades of owned skills below max rank
/// and new skills that fit a free slot. When nothing qualifies, a single [`Choice::Restore`].
pub fn offer(loadout: &Loadout, class: Class, rng: &mut Rng) -> Vec<Choice> {
    let mut candidates: Vec<(Choice, f32)> = Vec::new();
    for &(id, rank) in &loadout.skills {
        if rank < def(id).max_rank {
            candidates.push((Choice::Upgrade(id), UPGRADE_WEIGHT));
        }
    }
    for &id in pool(class) {
        if loadout.can_take(id) {
            candidates.push((Choice::New(id), NEW_WEIGHT));
        }
    }
    let mut out = Vec::with_capacity(OFFER_SIZE);
    while out.len() < OFFER_SIZE {
        let weights: Vec<f32> = candidates.iter().map(|&(_, w)| w).collect();
        let Some(i) = rng.weighted(&weights) else {
            break;
        };
        out.push(candidates.swap_remove(i).0);
    }
    if out.is_empty() {
        out.push(Choice::Restore);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mage_starts_with_frostbolt() {
        let l = Loadout::new(Class::Mage);
        assert_eq!(l.skills, vec![(SkillId::Frostbolt, 1)]);
        assert!(pool(Class::Mage).contains(&starting_skill(Class::Mage)));
    }

    #[test]
    fn every_pool_skill_is_well_formed() {
        for &id in pool(Class::Mage) {
            let d = def(id);
            assert_eq!(d.id, id);
            assert!(d.max_rank >= 1);
            match d.kind {
                Kind::Active(_) => {
                    for r in 1..=d.max_rank {
                        let s = active_stats(id, r).unwrap();
                        assert!(s.damage > 0.0 && s.cooldown > 0.0 && s.count >= 1);
                    }
                }
                Kind::Passive(_) => assert!(active_stats(id, 1).is_none()),
            }
        }
    }

    #[test]
    fn ranks_make_skills_stronger() {
        for &id in pool(Class::Mage) {
            if let (Some(a), Some(b)) = (active_stats(id, 1), active_stats(id, 5)) {
                assert!(b.damage > a.damage, "{id:?}");
                assert!(b.cooldown < a.cooldown, "{id:?}");
                assert!(b.count >= a.count && b.radius >= a.radius, "{id:?}");
            }
        }
        assert_eq!(active_stats(SkillId::Frostbolt, 3).unwrap().count, 1);
        assert_eq!(active_stats(SkillId::Frostbolt, 4).unwrap().count, 2);
        assert_eq!(active_stats(SkillId::ArcaneMissiles, 5).unwrap().count, 7);
    }

    #[test]
    fn passives_fold_into_effective_stats() {
        let mut l = Loadout::new(Class::Mage);
        let base = l.effective(SkillId::Frostbolt).unwrap();
        l.apply(Choice::New(SkillId::ArcaneIntellect));
        l.apply(Choice::Upgrade(SkillId::ArcaneIntellect));
        l.apply(Choice::New(SkillId::PresenceOfMind));
        let m = l.mods();
        assert!((m.damage - 1.16).abs() < 1e-5);
        assert!((m.cooldown - 0.95).abs() < 1e-5);
        let e = l.effective(SkillId::Frostbolt).unwrap();
        assert!((e.damage - base.damage * 1.16).abs() < 1e-4);
        assert!((e.cooldown - base.cooldown * 0.95).abs() < 1e-5);
        assert!(l.effective(SkillId::Fireball).is_none(), "not owned");
    }

    #[test]
    fn upgrades_stop_at_max_rank() {
        let mut l = Loadout::new(Class::Mage);
        for _ in 0..10 {
            l.apply(Choice::Upgrade(SkillId::Frostbolt));
        }
        assert_eq!(l.rank(SkillId::Frostbolt), Some(5));
    }

    #[test]
    fn slots_are_respected() {
        let mut l = Loadout::new(Class::Mage);
        for id in [
            SkillId::Fireball,
            SkillId::ArcaneMissiles,
            SkillId::FrostNova,
            SkillId::Blizzard, // a fifth active: refused
        ] {
            l.apply(Choice::New(id));
        }
        assert_eq!(l.actives().count(), MAX_ACTIVES);
        assert_eq!(l.rank(SkillId::Blizzard), None);
        // Taking a held skill again does nothing.
        l.apply(Choice::New(SkillId::Fireball));
        assert_eq!(l.skills.len(), MAX_ACTIVES);
    }

    #[test]
    fn offers_are_distinct_and_valid() {
        let mut rng = Rng::new(42);
        let l = Loadout::new(Class::Mage);
        for _ in 0..200 {
            let o = offer(&l, Class::Mage, &mut rng);
            assert_eq!(o.len(), OFFER_SIZE);
            for (i, c) in o.iter().enumerate() {
                assert!(!o[..i].contains(c), "duplicate {c:?}");
                match *c {
                    Choice::New(id) => assert!(l.rank(id).is_none()),
                    Choice::Upgrade(id) => assert_eq!(id, SkillId::Frostbolt),
                    Choice::Restore => panic!("restore while choices remain"),
                }
            }
        }
    }

    #[test]
    fn full_loadout_offers_only_upgrades_then_restore() {
        let mut l = Loadout::new(Class::Mage);
        for id in [
            SkillId::Fireball,
            SkillId::ArcaneMissiles,
            SkillId::FrostNova,
            SkillId::ArcaneIntellect,
            SkillId::PresenceOfMind,
        ] {
            l.apply(Choice::New(id));
        }
        let mut rng = Rng::new(1);
        for c in offer(&l, Class::Mage, &mut rng) {
            assert!(matches!(c, Choice::Upgrade(_)), "{c:?}");
        }
        // Everything at max rank: the only card left is Restore.
        for (_, r) in &mut l.skills {
            *r = 5;
        }
        assert_eq!(offer(&l, Class::Mage, &mut rng), vec![Choice::Restore]);
    }

    #[test]
    fn short_offer_when_few_candidates() {
        let mut l = Loadout::new(Class::Mage);
        for id in [
            SkillId::Fireball,
            SkillId::ArcaneMissiles,
            SkillId::FrostNova,
            SkillId::ArcaneIntellect,
            SkillId::PresenceOfMind,
        ] {
            l.apply(Choice::New(id));
        }
        for (id, r) in &mut l.skills {
            if *id != SkillId::Fireball {
                *r = 5;
            }
        }
        assert_eq!(
            offer(&l, Class::Mage, &mut Rng::new(9)),
            vec![Choice::Upgrade(SkillId::Fireball)]
        );
    }
}
