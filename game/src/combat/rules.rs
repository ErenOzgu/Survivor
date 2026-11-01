//! The combat rules as pure functions: XP curve, level-up cards, cooldowns, damage and status
//! math, Blink's trigger, spawn geometry, orb pickup. Nothing here touches the ECS.

use bevy::math::{Vec2, Vec3};

use crate::content::{SkillId, MAX_SKILLS};

/// The hero's starting skill.
pub const START_SKILL: SkillId = SkillId::Frostbolt;
/// Hero health and its regeneration per second.
pub const HERO_MAX_HEALTH: f32 = 100.0;
pub const HERO_REGEN: f32 = 0.6;
/// Cards offered per level-up.
pub const CARDS: usize = 3;
/// Health a "can iksiri" card gives back, when there is nothing left to learn.
pub const HEAL_CARD: f32 = 40.0;
/// Orbs within this distance fly to the hero; within [`ORB_COLLECT`] they are taken.
pub const ORB_MAGNET: f32 = 4.5;
pub const ORB_COLLECT: f32 = 0.9;
pub const ORB_SPEED: f32 = 15.0;

// ---------------------------------------------------------------------------------------------
// Randomness: a tiny xorshift, so the rules stay deterministic under test and need no crate.

#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Zero is xorshift's fixed point.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `0..1`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform in `0..n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// ---------------------------------------------------------------------------------------------
// XP and levels.

/// XP needed to go from `level` to `level + 1`: quick early levels, then a gentle quadratic climb.
pub fn xp_to_next(level: u32) -> u32 {
    let l = level.max(1) as f32;
    (5.0 + 3.0 * l + 0.3 * l * l).round() as u32
}

/// Adds `gain` XP; returns the new level, the XP left toward the next one, and how many levels
/// were gained.
pub fn add_xp(level: u32, xp: u32, gain: u32) -> (u32, u32, u32) {
    let (mut level, mut xp) = (level.max(1), xp + gain);
    let mut ups = 0;
    while xp >= xp_to_next(level) {
        xp -= xp_to_next(level);
        level += 1;
        ups += 1;
    }
    (level, xp, ups)
}

// ---------------------------------------------------------------------------------------------
// Owned skills and level-up cards.

/// The skills the hero carries, with their levels, in the order learned.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Loadout(pub Vec<(SkillId, u8)>);

impl Loadout {
    pub fn starting() -> Self {
        Self(vec![(START_SKILL, 1)])
    }

    pub fn level_of(&self, id: SkillId) -> Option<u8> {
        self.0.iter().find(|(s, _)| *s == id).map(|(_, l)| *l)
    }

    /// Applies a card. A new skill is ignored when the loadout is full or already has it; an
    /// upgrade is ignored past the skill's max. Healing is not a loadout change.
    pub fn apply(&mut self, card: Card) {
        match card {
            Card::New(id) => {
                if self.0.len() < MAX_SKILLS && self.level_of(id).is_none() {
                    self.0.push((id, 1));
                }
            }
            Card::Upgrade(id, to) => {
                if let Some(entry) = self.0.iter_mut().find(|(s, _)| *s == id) {
                    if to == entry.1 + 1 && to <= id.max_level() {
                        entry.1 = to;
                    }
                }
            }
            Card::Heal => {}
        }
    }
}

/// One level-up choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Card {
    /// Learn a skill at level 1.
    New(SkillId),
    /// Raise an owned skill to this level.
    Upgrade(SkillId, u8),
    /// Nothing left to learn: heal instead.
    Heal,
}

/// Every card the loadout could take now: the next level of each owned skill below its max, and
/// each unowned skill while there is room.
pub fn card_pool(loadout: &Loadout) -> Vec<Card> {
    let mut pool: Vec<Card> = loadout
        .0
        .iter()
        .filter(|(s, l)| *l < s.max_level())
        .map(|(s, l)| Card::Upgrade(*s, l + 1))
        .collect();
    if loadout.0.len() < MAX_SKILLS {
        pool.extend(
            SkillId::ALL
                .into_iter()
                .filter(|s| loadout.level_of(*s).is_none())
                .map(Card::New),
        );
    }
    pool
}

/// Draws up to `n` distinct cards from the pool; a maxed-out loadout gets a single heal card.
pub fn roll_cards(loadout: &Loadout, rng: &mut Rng, n: usize) -> Vec<Card> {
    let mut pool = card_pool(loadout);
    if pool.is_empty() {
        return vec![Card::Heal];
    }
    let mut out = Vec::with_capacity(n);
    while out.len() < n && !pool.is_empty() {
        out.push(pool.swap_remove(rng.below(pool.len())));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Cooldowns, damage, status.

/// Advances a cooldown by `dt`. Returns the new remaining time and whether the skill fires this
/// frame (it then restarts at `cooldown`, keeping any overshoot so the rate does not drift).
pub fn tick_cooldown(remaining: f32, dt: f32, cooldown: f32) -> (f32, bool) {
    let r = remaining - dt;
    if r <= 0.0 {
        ((r + cooldown).max(0.0), true)
    } else {
        (r, false)
    }
}

/// Health after a hit; and whether that hit killed.
pub fn apply_damage(health: f32, damage: f32) -> (f32, bool) {
    let h = (health - damage.max(0.0)).max(0.0);
    (h, health > 0.0 && h <= 0.0)
}

/// Linear falloff for blast damage: full at the centre, half at the edge, none outside.
pub fn blast_damage(damage: f32, dist: f32, radius: f32) -> f32 {
    if radius <= 0.0 || dist > radius {
        return 0.0;
    }
    damage * (1.0 - 0.5 * dist / radius)
}

/// An enemy's speed under its status: frozen stands still, a slow takes its fraction.
pub fn effective_speed(base: f32, frozen: f32, slow_left: f32, slow: f32) -> f32 {
    if frozen > 0.0 {
        0.0
    } else if slow_left > 0.0 {
        base * (1.0 - slow.clamp(0.0, 1.0))
    } else {
        base
    }
}

/// A new slow against an existing one: the stronger fraction and the longer time win.
pub fn merge_slow(cur: (f32, f32), new: (f32, f32)) -> (f32, f32) {
    let (cur_left, cur_frac) = cur;
    let (new_left, new_frac) = new;
    if cur_left <= 0.0 {
        return new;
    }
    (cur_left.max(new_left), cur_frac.max(new_frac))
}

// ---------------------------------------------------------------------------------------------
// Blink.

/// Health below this fraction is an emergency.
pub const BLINK_HEALTH: f32 = 0.35;

/// Blink fires when health is low with an enemy close, or when `crowd` enemies are on top of
/// the hero.
pub fn should_blink(health_frac: f32, near: u32, crowd: u32) -> bool {
    (health_frac < BLINK_HEALTH && near > 0) || near >= crowd.max(1)
}

/// Where to Blink: `range` yards from `hero` (on the ground plane), away from the centre of the
/// nearby enemies; straight ahead along `fallback` when they surround it evenly.
pub fn blink_target(hero: Vec3, enemies: &[Vec3], range: f32, fallback: Vec3) -> Vec3 {
    let away = enemies
        .iter()
        .map(|e| (hero - *e).with_y(0.0).normalize_or_zero())
        .sum::<Vec3>();
    let dir = if away.length_squared() > 1e-4 {
        away.normalize()
    } else {
        let f = fallback.with_y(0.0).normalize_or_zero();
        if f == Vec3::ZERO {
            Vec3::NEG_Z
        } else {
            f
        }
    };
    hero + dir * range
}

// ---------------------------------------------------------------------------------------------
// Geometry.

/// A point on the ground ring around `center` at `angle` (radians) and `radius`.
pub fn ring_point(center: Vec3, angle: f32, radius: f32) -> Vec3 {
    center + Vec3::new(angle.cos(), 0.0, angle.sin()) * radius
}

/// Horizontal distance (ignores height, which the terrain owns).
pub fn flat_dist(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// A chaser's step toward `target`, stopping at `stop` yards.
pub fn chase_step(pos: Vec3, target: Vec3, speed: f32, dt: f32, stop: f32) -> Vec3 {
    let to = (target - pos).with_y(0.0);
    let d = to.length();
    if d <= stop || d < 1e-5 {
        return pos;
    }
    pos + to / d * (speed * dt).min(d - stop)
}

/// An XP orb's position after one frame: it flies to the hero inside `magnet`, else stays.
/// Returns the new position and whether it is collected.
pub fn orb_step(orb: Vec3, hero: Vec3, magnet: f32, dt: f32) -> (Vec3, bool) {
    let d = flat_dist(orb, hero);
    if d <= ORB_COLLECT {
        return (orb, true);
    }
    if d > magnet {
        return (orb, false);
    }
    let next = chase_step(orb, hero, ORB_SPEED, dt, 0.0);
    (next, flat_dist(next, hero) <= ORB_COLLECT)
}

// ---------------------------------------------------------------------------------------------
// Debug start (env vars).

/// Parses `RNS_SKILLS`: `all` (every skill at max, past the 6-skill limit, for testing) or a
/// comma list of `key[:level]`. Unknown keys are skipped; levels clamp into the table; the
/// 6-skill limit holds for a list.
pub fn parse_skills(s: &str) -> Loadout {
    if s.trim().eq_ignore_ascii_case("all") {
        return Loadout(SkillId::ALL.map(|id| (id, id.max_level())).to_vec());
    }
    let mut out = Loadout::default();
    for part in s.split(',') {
        let (key, lvl) = match part.split_once(':') {
            Some((k, l)) => (k, l.trim().parse::<u8>().unwrap_or(1)),
            None => (part, 1),
        };
        let Some(id) = SkillId::from_key(key) else {
            continue;
        };
        if out.level_of(id).is_none() && out.0.len() < MAX_SKILLS {
            out.0.push((id, lvl.clamp(1, id.max_level())));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_curve_grows() {
        assert_eq!(xp_to_next(1), 8);
        for l in 1..60 {
            assert!(xp_to_next(l + 1) > xp_to_next(l));
        }
        assert_eq!(xp_to_next(0), xp_to_next(1));
    }

    #[test]
    fn add_xp_carries_over_several_levels() {
        assert_eq!(add_xp(1, 0, 7), (1, 7, 0));
        assert_eq!(add_xp(1, 7, 1), (2, 0, 1));
        let need = xp_to_next(1) + xp_to_next(2) + xp_to_next(3);
        assert_eq!(add_xp(1, 0, need + 2), (4, 2, 3));
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..1000 {
            let x = a.f32();
            assert_eq!(x, b.f32());
            assert!((0.0..1.0).contains(&x));
            assert!(a.below(3) < 3);
            b.below(3);
        }
        let mut z = Rng::new(0);
        assert_ne!(z.next_u64(), 0);
    }

    fn full_loadout(level: u8) -> Loadout {
        Loadout(
            SkillId::ALL[..MAX_SKILLS]
                .iter()
                .map(|s| (*s, level))
                .collect(),
        )
    }

    #[test]
    fn cards_are_distinct_and_legal() {
        let mut rng = Rng::new(1);
        for seed_round in 0..200 {
            let lo = if seed_round % 2 == 0 {
                Loadout::starting()
            } else {
                full_loadout(2)
            };
            let cards = roll_cards(&lo, &mut rng, CARDS);
            assert_eq!(cards.len(), CARDS);
            for (i, c) in cards.iter().enumerate() {
                assert!(!cards[..i].contains(c), "duplicate {c:?}");
                match *c {
                    Card::New(id) => {
                        assert!(lo.0.len() < MAX_SKILLS);
                        assert!(lo.level_of(id).is_none());
                    }
                    Card::Upgrade(id, to) => assert_eq!(lo.level_of(id), Some(to - 1)),
                    Card::Heal => panic!("heal with cards left"),
                }
            }
        }
    }

    #[test]
    fn a_full_loadout_offers_no_new_skills() {
        let lo = full_loadout(1);
        assert!(card_pool(&lo)
            .iter()
            .all(|c| matches!(c, Card::Upgrade(..))));
    }

    #[test]
    fn maxed_skills_get_no_card() {
        let mut lo = full_loadout(1);
        lo.0[0].1 = lo.0[0].0.max_level();
        let maxed = lo.0[0].0;
        assert!(!card_pool(&lo).iter().any(|c| match c {
            Card::Upgrade(id, _) | Card::New(id) => *id == maxed,
            Card::Heal => false,
        }));
    }

    #[test]
    fn everything_maxed_gives_a_heal_card() {
        let lo = Loadout(
            SkillId::ALL[..MAX_SKILLS]
                .iter()
                .map(|s| (*s, s.max_level()))
                .collect(),
        );
        assert_eq!(roll_cards(&lo, &mut Rng::new(3), CARDS), vec![Card::Heal]);
    }

    #[test]
    fn few_options_give_fewer_cards() {
        let mut lo = full_loadout(1);
        for e in lo.0.iter_mut().skip(1) {
            e.1 = e.0.max_level();
        }
        let cards = roll_cards(&lo, &mut Rng::new(5), CARDS);
        assert_eq!(cards, vec![Card::Upgrade(lo.0[0].0, 2)]);
    }

    #[test]
    fn applying_cards_respects_limits() {
        let mut lo = Loadout::starting();
        lo.apply(Card::Upgrade(START_SKILL, 2));
        assert_eq!(lo.level_of(START_SKILL), Some(2));
        lo.apply(Card::Upgrade(START_SKILL, 4)); // skipping a level is refused
        assert_eq!(lo.level_of(START_SKILL), Some(2));
        lo.apply(Card::New(START_SKILL)); // already owned
        assert_eq!(lo.0.len(), 1);
        for s in SkillId::ALL {
            lo.apply(Card::New(s));
        }
        assert_eq!(lo.0.len(), MAX_SKILLS);
        let mut maxed = Loadout(vec![(SkillId::Blink, SkillId::Blink.max_level())]);
        maxed.apply(Card::Upgrade(
            SkillId::Blink,
            SkillId::Blink.max_level() + 1,
        ));
        assert_eq!(
            maxed.level_of(SkillId::Blink),
            Some(SkillId::Blink.max_level())
        );
    }

    #[test]
    fn a_whole_run_of_picks_never_breaks_the_rules() {
        let mut rng = Rng::new(42);
        let mut lo = Loadout::starting();
        for _ in 0..80 {
            let cards = roll_cards(&lo, &mut rng, CARDS);
            let pick = cards[rng.below(cards.len())];
            lo.apply(pick);
            assert!(lo.0.len() <= MAX_SKILLS);
            for (s, l) in &lo.0 {
                assert!(*l >= 1 && *l <= s.max_level());
            }
        }
        assert_eq!(roll_cards(&lo, &mut rng, CARDS), vec![Card::Heal]);
    }

    #[test]
    fn cooldowns_fire_at_the_right_rate() {
        let (mut r, mut fires) = (0.0, 0);
        for _ in 0..600 {
            let (nr, f) = tick_cooldown(r, 1.0 / 60.0, 1.0);
            r = nr;
            fires += f as u32;
        }
        // Fires at t=0 then once a second: 10 s → 10 fires (the 11th lands on the boundary).
        assert!((10..=11).contains(&fires), "{fires}");
        assert_eq!(tick_cooldown(0.5, 0.2, 1.0), (0.3, false));
    }

    #[test]
    fn damage_and_kills() {
        assert_eq!(apply_damage(10.0, 4.0), (6.0, false));
        assert_eq!(apply_damage(10.0, 12.0), (0.0, true));
        assert_eq!(apply_damage(0.0, 5.0), (0.0, false), "already dead");
        assert_eq!(apply_damage(10.0, -5.0), (10.0, false));
    }

    #[test]
    fn blast_falls_off() {
        assert_eq!(blast_damage(20.0, 0.0, 4.0), 20.0);
        assert_eq!(blast_damage(20.0, 4.0, 4.0), 10.0);
        assert_eq!(blast_damage(20.0, 4.1, 4.0), 0.0);
        assert_eq!(blast_damage(20.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn freeze_beats_slow() {
        assert_eq!(effective_speed(4.0, 1.0, 1.0, 0.5), 0.0);
        assert_eq!(effective_speed(4.0, 0.0, 1.0, 0.5), 2.0);
        assert_eq!(effective_speed(4.0, 0.0, 0.0, 0.5), 4.0);
        assert_eq!(merge_slow((0.0, 0.9), (1.0, 0.3)), (1.0, 0.3));
        assert_eq!(merge_slow((2.0, 0.3), (1.0, 0.5)), (2.0, 0.5));
    }

    #[test]
    fn blink_triggers() {
        assert!(!should_blink(1.0, 3, 4));
        assert!(should_blink(1.0, 4, 4));
        assert!(should_blink(0.2, 1, 4));
        assert!(!should_blink(0.2, 0, 4), "low health but nobody near");
    }

    #[test]
    fn blink_goes_away_from_enemies() {
        let hero = Vec3::ZERO;
        let t = blink_target(hero, &[Vec3::new(1.0, 0.0, 0.0)], 9.0, Vec3::Z);
        assert!((t - Vec3::new(-9.0, 0.0, 0.0)).length() < 1e-4);
        // Evenly surrounded: along the fallback.
        let ring = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];
        let t = blink_target(hero, &ring, 5.0, Vec3::Z);
        assert!((t - Vec3::new(0.0, 0.0, 5.0)).length() < 1e-4);
        let t = blink_target(hero, &ring, 5.0, Vec3::ZERO);
        assert!((t - Vec3::new(0.0, 0.0, -5.0)).length() < 1e-4);
    }

    #[test]
    fn chasing_stops_short() {
        let p = chase_step(Vec3::ZERO, Vec3::new(10.0, 3.0, 0.0), 5.0, 1.0, 1.0);
        assert!(
            (p - Vec3::new(5.0, 0.0, 0.0)).length() < 1e-5,
            "height is not chased"
        );
        let p = chase_step(Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), 5.0, 1.0, 1.0);
        assert!((p - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-5);
        assert_eq!(
            chase_step(Vec3::ZERO, Vec3::X * 0.5, 5.0, 1.0, 1.0),
            Vec3::ZERO
        );
    }

    #[test]
    fn ring_points_are_on_the_ring() {
        let c = Vec3::new(3.0, 7.0, -2.0);
        for i in 0..16 {
            let p = ring_point(c, i as f32, 25.0);
            assert!((flat_dist(p, c) - 25.0).abs() < 1e-3);
            assert_eq!(p.y, c.y);
        }
    }

    #[test]
    fn orbs_fly_in_and_get_collected() {
        let hero = Vec3::ZERO;
        assert_eq!(
            orb_step(Vec3::X * 10.0, hero, ORB_MAGNET, 0.1),
            (Vec3::X * 10.0, false)
        );
        let (p, got) = orb_step(Vec3::X * 4.0, hero, ORB_MAGNET, 0.1);
        assert!(!got && p.x < 4.0);
        assert!(orb_step(Vec3::X * 1.5, hero, ORB_MAGNET, 0.1).1);
        assert!(orb_step(Vec3::X * 0.5, hero, ORB_MAGNET, 0.0).1);
    }

    #[test]
    fn debug_skills_parse() {
        assert_eq!(parse_skills("all").0.len(), SkillId::ALL.len());
        let lo = parse_skills("frostbolt:3, blink, nope:2, fireball:99, blink:4");
        assert_eq!(
            lo.0,
            vec![
                (SkillId::Frostbolt, 3),
                (SkillId::Blink, 1),
                (SkillId::Fireball, SkillId::Fireball.max_level()),
            ]
        );
        assert!(parse_skills("").0.is_empty());
        let many = SkillId::ALL.map(|s| s.key()).join(",");
        assert_eq!(parse_skills(&many).0.len(), MAX_SKILLS);
    }
}
