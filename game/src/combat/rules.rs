//! The combat rules as pure functions: XP curve, level-up cards, cooldowns, damage and status
//! math, Blink's trigger, spawn geometry, orb pickup. Nothing here touches the ECS.

use bevy::math::{Vec2, Vec3};

use crate::content::items::{Affix, Item};
use crate::content::passives::{PassiveId, MAX_PASSIVES};
use crate::content::talents::{TalentEffect, Talents};
use crate::content::{SkillId, MAX_SKILLS};

/// The hero's starting skill.
pub const START_SKILL: SkillId = SkillId::Frostbolt;
/// Hero health and its regeneration per second.
pub const HERO_MAX_HEALTH: f32 = 100.0;
pub const HERO_REGEN: f32 = 1.0;
/// Cards offered per level-up.
pub const CARDS: usize = 3;
/// Health a "can iksiri" card gives back, when there is nothing left to learn.
pub const HEAL_CARD: f32 = 40.0;
/// Orbs within this distance fly to the hero; within [`ORB_COLLECT`] they are taken.
pub const ORB_MAGNET: f32 = 6.5;
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

    /// Applies a skill card. A new skill is ignored when the loadout is full or already has it;
    /// an upgrade is ignored past the skill's max; an evolution needs both parents at their max
    /// and replaces the first. Passive and heal cards are not skill changes.
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
            Card::Evolve(from, into) => {
                if self.evolution_ready(from) == Some(into) {
                    if let Some(entry) = self.0.iter_mut().find(|(s, _)| *s == from) {
                        *entry = (into, 1);
                    }
                }
            }
            Card::NewPassive(_) | Card::UpgradePassive(..) | Card::Heal => {}
        }
    }

    /// What `base` can evolve into now: it and its partner at their last level, the evolved
    /// form not owned yet.
    pub fn evolution_ready(&self, base: SkillId) -> Option<SkillId> {
        let (partner, into) = base.evolution()?;
        let maxed = |s: SkillId| self.level_of(s) == Some(s.max_level());
        (maxed(base) && maxed(partner) && self.level_of(into).is_none()).then_some(into)
    }
}

/// The passives the hero carries, with their levels, in the order learned.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Passives(pub Vec<(PassiveId, u8)>);

impl Passives {
    pub fn level_of(&self, id: PassiveId) -> u8 {
        self.0.iter().find(|(p, _)| *p == id).map_or(0, |(_, l)| *l)
    }

    /// Applies a passive card, under the same rules as skills.
    pub fn apply(&mut self, card: Card) {
        match card {
            Card::NewPassive(id) => {
                if self.0.len() < MAX_PASSIVES && self.level_of(id) == 0 {
                    self.0.push((id, 1));
                }
            }
            Card::UpgradePassive(id, to) => {
                if let Some(entry) = self.0.iter_mut().find(|(p, _)| *p == id) {
                    if to == entry.1 + 1 && to <= id.max_level() {
                        entry.1 = to;
                    }
                }
            }
            _ => {}
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
    /// Fuse a maxed skill (with its maxed partner) into its evolved form.
    Evolve(SkillId, SkillId),
    /// Take a passive at level 1.
    NewPassive(PassiveId),
    /// Raise an owned passive to this level.
    UpgradePassive(PassiveId, u8),
    /// Nothing left to learn: heal instead.
    Heal,
}

/// Evolution cards the loadout earned.
pub fn evolution_cards(loadout: &Loadout) -> Vec<Card> {
    loadout
        .0
        .iter()
        .filter_map(|(s, _)| {
            loadout
                .evolution_ready(*s)
                .map(|into| Card::Evolve(*s, into))
        })
        .collect()
}

/// Whether `base` lives on as its evolved form (then it is not learnable again).
fn evolved_away(loadout: &Loadout, base: SkillId) -> bool {
    base.evolution()
        .is_some_and(|(_, into)| loadout.level_of(into).is_some())
}

/// Every ordinary card the hero could take now: the next level of each owned skill or passive
/// below its max, and each unowned skill or passive while there is room. Evolved forms are never
/// offered as new skills.
pub fn card_pool(loadout: &Loadout, passives: &Passives) -> Vec<Card> {
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
                .filter(|s| loadout.level_of(*s).is_none() && !evolved_away(loadout, *s))
                .map(Card::New),
        );
    }
    pool.extend(
        passives
            .0
            .iter()
            .filter(|(p, l)| *l < p.max_level())
            .map(|(p, l)| Card::UpgradePassive(*p, l + 1)),
    );
    if passives.0.len() < MAX_PASSIVES {
        pool.extend(
            PassiveId::ALL
                .into_iter()
                .filter(|p| passives.level_of(*p) == 0)
                .map(Card::NewPassive),
        );
    }
    pool
}

/// Draws up to `n` distinct cards: an earned evolution always comes first, the rest at random
/// from the pool; with nothing left at all, a single heal card.
pub fn roll_cards(loadout: &Loadout, passives: &Passives, rng: &mut Rng, n: usize) -> Vec<Card> {
    let mut out = evolution_cards(loadout);
    out.truncate(n);
    let mut pool = card_pool(loadout, passives);
    while out.len() < n && !pool.is_empty() {
        out.push(pool.swap_remove(rng.below(pool.len())));
    }
    if out.is_empty() {
        out.push(Card::Heal);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The hero's stats from passives.

/// Base critical-hit chance, and how hard a crit hits.
pub const BASE_CRIT: f32 = 0.05;
pub const CRIT_MULTIPLIER: f32 = 2.0;
/// Cooldowns never go below this fraction of their table value.
pub const MIN_COOLDOWN_MUL: f32 = 0.5;

/// Lasting bonuses from outside the run: worn gear and talents. Fractions, except `health` and
/// `regen` (hit points) and `crit_power` (added to the crit multiplier).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bonus {
    pub damage: f32,
    pub haste: f32,
    pub speed: f32,
    pub health: f32,
    pub magnet: f32,
    pub crit: f32,
    pub xp: f32,
    pub regen: f32,
    pub crit_power: f32,
}

impl Bonus {
    /// What worn items give.
    pub fn from_gear<'a>(items: impl Iterator<Item = &'a Item>) -> Bonus {
        let mut b = Bonus::default();
        for it in items {
            b.damage += it.bonus(Affix::Power);
            b.haste += it.bonus(Affix::Haste);
            b.speed += it.bonus(Affix::Speed);
            b.health += it.bonus(Affix::Health);
            b.magnet += it.bonus(Affix::Magnet);
            b.crit += it.bonus(Affix::Crit);
            b.xp += it.bonus(Affix::Wisdom);
        }
        b
    }

    /// What bought talents give.
    pub fn from_talents(t: &Talents) -> Bonus {
        use TalentEffect as E;
        Bonus {
            damage: t.total(|e| matches!(e, E::Damage(_)).then(|| effect_value(e))),
            haste: t.total(|e| matches!(e, E::Haste(_)).then(|| effect_value(e))),
            health: t.total(|e| matches!(e, E::Health(_)).then(|| effect_value(e))),
            crit: t.total(|e| matches!(e, E::Crit(_)).then(|| effect_value(e))),
            xp: t.total(|e| matches!(e, E::Xp(_)).then(|| effect_value(e))),
            regen: t.total(|e| matches!(e, E::Regen(_)).then(|| effect_value(e))),
            crit_power: t.total(|e| matches!(e, E::CritPower(_)).then(|| effect_value(e))),
            ..Bonus::default()
        }
    }

    pub fn plus(self, o: Bonus) -> Bonus {
        Bonus {
            damage: self.damage + o.damage,
            haste: self.haste + o.haste,
            speed: self.speed + o.speed,
            health: self.health + o.health,
            magnet: self.magnet + o.magnet,
            crit: self.crit + o.crit,
            xp: self.xp + o.xp,
            regen: self.regen + o.regen,
            crit_power: self.crit_power + o.crit_power,
        }
    }
}

fn effect_value(e: TalentEffect) -> f32 {
    use TalentEffect as E;
    match e {
        E::Health(v)
        | E::Regen(v)
        | E::Revive(v)
        | E::Damage(v)
        | E::Crit(v)
        | E::CritPower(v)
        | E::Haste(v)
        | E::Xp(v) => v,
        E::StartCards(n) => n as f32,
    }
}

/// What the passives, gear and talents add up to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub damage_mul: f32,
    pub cooldown_mul: f32,
    pub speed_mul: f32,
    pub max_health: f32,
    pub magnet_mul: f32,
    pub crit_chance: f32,
    pub crit_mul: f32,
    pub xp_mul: f32,
    /// Health regenerated per second.
    pub regen: f32,
}

impl Stats {
    pub fn from_passives(p: &Passives) -> Self {
        Self::from_parts(p, &Bonus::default())
    }

    pub fn from_parts(p: &Passives, x: &Bonus) -> Self {
        let b = |id: PassiveId| id.bonus(p.level_of(id));
        Stats {
            damage_mul: 1.0 + b(PassiveId::Power) + x.damage,
            cooldown_mul: (1.0 - b(PassiveId::Haste) - x.haste).max(MIN_COOLDOWN_MUL),
            speed_mul: 1.0 + b(PassiveId::Swiftness) + x.speed,
            max_health: HERO_MAX_HEALTH + b(PassiveId::Vitality) + x.health,
            magnet_mul: 1.0 + b(PassiveId::Attraction) + x.magnet,
            crit_chance: (BASE_CRIT + b(PassiveId::Precision) + x.crit).min(1.0),
            crit_mul: CRIT_MULTIPLIER + x.crit_power,
            xp_mul: 1.0 + b(PassiveId::Wisdom) + x.xp,
            regen: HERO_REGEN + x.regen,
        }
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self::from_passives(&Passives::default())
    }
}

/// One hit's damage: the base times the damage bonus, doubled on a crit (`roll` in `0..1`).
/// Returns the damage and whether it crit.
pub fn hit_damage(base: f32, stats: &Stats, roll: f32) -> (f32, bool) {
    let crit = roll < stats.crit_chance;
    let mul = if crit { stats.crit_mul } else { 1.0 };
    (base * stats.damage_mul * mul, crit)
}

/// XP an orb gives under the XP bonus (never less than its face value).
pub fn orb_xp(face: u32, stats: &Stats) -> u32 {
    ((face as f32 * stats.xp_mul).round() as u32).max(face)
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
// Contact and enemy animation.

/// How far past its body an enemy reaches the hero.
pub const CONTACT_REACH: f32 = 0.6;
/// Seconds between attack swings of an enemy in contact (the damage itself is continuous).
pub const ATTACK_EVERY: f32 = 1.1;
/// A dead enemy's body stays this long, sinking into the ground over the last [`CORPSE_SINK`].
pub const CORPSE_SECS: f32 = 2.2;
pub const CORPSE_SINK: f32 = 0.7;
pub const CORPSE_DEPTH: f32 = 1.5;

/// A ranged enemy's step: close in to `keep` yards, back off when the hero comes within 70% of
/// it, else hold.
pub fn kite_step(pos: Vec3, hero: Vec3, speed: f32, dt: f32, keep: f32) -> Vec3 {
    let d = flat_dist(pos, hero);
    if d > keep {
        chase_step(pos, hero, speed, dt, keep)
    } else if d < keep * 0.7 && d > 1e-4 {
        let away = (pos - hero).with_y(0.0).normalize_or_zero();
        pos + away * (speed * dt).min(keep * 0.7 - d)
    } else {
        pos
    }
}

/// How grown a fuse's warning circle is, `0..=1`, with `left` of `total` seconds to go.
pub fn fuse_progress(left: f32, total: f32) -> f32 {
    if total <= 0.0 {
        1.0
    } else {
        (1.0 - left / total).clamp(0.0, 1.0)
    }
}

/// Whether an enemy `dist` yards from the hero, with body `radius`, touches it.
pub fn touching(dist: f32, radius: f32) -> bool {
    dist <= radius + CONTACT_REACH
}

/// The one-shot animation an enemy plays this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyAction {
    Attack,
    Hit,
}

/// Picks an enemy's animation for this frame and its next swing timer. Touching and not frozen,
/// it swings every [`ATTACK_EVERY`]; just hit and not already playing something, it flinches.
/// A swing always wins over a flinch.
pub fn enemy_action(
    touching: bool,
    frozen: bool,
    attack_cd: f32,
    dt: f32,
    just_hit: bool,
    busy: bool,
) -> (Option<EnemyAction>, f32) {
    if touching && !frozen {
        let cd = attack_cd - dt;
        if cd <= 0.0 {
            return (Some(EnemyAction::Attack), ATTACK_EVERY);
        }
        let flinch = (just_hit && !busy).then_some(EnemyAction::Hit);
        return (flinch, cd);
    }
    // Out of reach the first swing comes quickly once it arrives.
    let cd = attack_cd.min(ATTACK_EVERY * 0.3);
    (
        (just_hit && !busy && !frozen).then_some(EnemyAction::Hit),
        cd,
    )
}

/// How far below the ground a corpse with `left` seconds to go has sunk.
pub fn corpse_depth(left: f32) -> f32 {
    if left >= CORPSE_SINK {
        0.0
    } else {
        CORPSE_DEPTH * (1.0 - left.max(0.0) / CORPSE_SINK)
    }
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

/// Parses `RNS_PASSIVES`: `all` (every passive at max, past the limit) or a comma list of
/// `key[:level]`, under the same rules as [`parse_skills`].
pub fn parse_passives(s: &str) -> Passives {
    if s.trim().eq_ignore_ascii_case("all") {
        return Passives(PassiveId::ALL.map(|p| (p, p.max_level())).to_vec());
    }
    let mut out = Passives::default();
    for part in s.split(',') {
        let (key, lvl) = match part.split_once(':') {
            Some((k, l)) => (k, l.trim().parse::<u8>().unwrap_or(1)),
            None => (part, 1),
        };
        let Some(id) = PassiveId::from_key(key) else {
            continue;
        };
        if out.level_of(id) == 0 && out.0.len() < MAX_PASSIVES {
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

    fn maxed_passives() -> Passives {
        Passives(
            PassiveId::ALL[..MAX_PASSIVES]
                .iter()
                .map(|p| (*p, p.max_level()))
                .collect(),
        )
    }

    fn check_legal(lo: &Loadout, pa: &Passives, cards: &[Card]) {
        for (i, c) in cards.iter().enumerate() {
            assert!(!cards[..i].contains(c), "duplicate {c:?}");
            match *c {
                Card::New(id) => {
                    assert!(lo.0.len() < MAX_SKILLS && lo.level_of(id).is_none());
                    assert!(!id.is_evolved(), "evolved forms are never new cards");
                }
                Card::Upgrade(id, to) => assert_eq!(lo.level_of(id), Some(to - 1)),
                Card::Evolve(from, into) => assert_eq!(lo.evolution_ready(from), Some(into)),
                Card::NewPassive(p) => {
                    assert!(pa.0.len() < MAX_PASSIVES && pa.level_of(p) == 0)
                }
                Card::UpgradePassive(p, to) => assert_eq!(pa.level_of(p), to - 1),
                Card::Heal => panic!("heal with cards left"),
            }
        }
    }

    #[test]
    fn cards_are_distinct_and_legal() {
        let mut rng = Rng::new(1);
        let pa = Passives(vec![(PassiveId::Power, 2)]);
        for round in 0..200 {
            let lo = if round % 2 == 0 {
                Loadout::starting()
            } else {
                full_loadout(2)
            };
            let cards = roll_cards(&lo, &pa, &mut rng, CARDS);
            assert_eq!(cards.len(), CARDS);
            check_legal(&lo, &pa, &cards);
        }
    }

    #[test]
    fn a_full_loadout_offers_no_new_skills() {
        let lo = full_loadout(1);
        assert!(!card_pool(&lo, &Passives::default())
            .iter()
            .any(|c| matches!(c, Card::New(_))));
    }

    #[test]
    fn full_passives_offer_no_new_passives() {
        let pa = Passives(
            PassiveId::ALL[..MAX_PASSIVES]
                .iter()
                .map(|p| (*p, 1))
                .collect(),
        );
        assert!(!card_pool(&Loadout::starting(), &pa)
            .iter()
            .any(|c| matches!(c, Card::NewPassive(_))));
    }

    #[test]
    fn maxed_skills_get_no_card() {
        let mut lo = full_loadout(1);
        lo.0[0].1 = lo.0[0].0.max_level();
        let maxed = lo.0[0].0;
        assert!(!card_pool(&lo, &Passives::default())
            .iter()
            .any(|c| match c {
                Card::Upgrade(id, _) | Card::New(id) => *id == maxed,
                _ => false,
            }));
    }

    fn all_maxed() -> Loadout {
        Loadout(
            SkillId::ALL[..MAX_SKILLS]
                .iter()
                .map(|s| (*s, s.max_level()))
                .collect(),
        )
    }

    #[test]
    fn everything_maxed_and_evolved_gives_a_heal_card() {
        let mut lo = all_maxed();
        for c in evolution_cards(&lo.clone()) {
            lo.apply(c);
        }
        assert!(evolution_cards(&lo).is_empty());
        assert_eq!(
            roll_cards(&lo, &maxed_passives(), &mut Rng::new(3), CARDS),
            vec![Card::Heal]
        );
    }

    #[test]
    fn evolutions_need_both_parents_maxed_and_come_first() {
        let fb = SkillId::Frostbolt;
        let nova = SkillId::FrostNova;
        let mut lo = Loadout(vec![(fb, fb.max_level()), (nova, nova.max_level() - 1)]);
        assert_eq!(lo.evolution_ready(fb), None, "partner not maxed");
        lo.0[1].1 = nova.max_level();
        assert_eq!(lo.evolution_ready(fb), Some(SkillId::WinterBolts));
        let cards = roll_cards(&lo, &Passives::default(), &mut Rng::new(9), CARDS);
        assert_eq!(cards[0], Card::Evolve(fb, SkillId::WinterBolts));
        lo.apply(cards[0]);
        assert_eq!(
            lo.0,
            vec![(SkillId::WinterBolts, 1), (nova, nova.max_level())]
        );
        assert!(evolution_cards(&lo).is_empty(), "evolves once");
        assert!(
            !card_pool(&lo, &Passives::default()).contains(&Card::New(fb)),
            "an evolved skill does not come back as new"
        );
        // An evolution card without its conditions changes nothing.
        let mut early = Loadout(vec![(fb, 1)]);
        early.apply(Card::Evolve(fb, SkillId::WinterBolts));
        assert_eq!(early.0, vec![(fb, 1)]);
    }

    #[test]
    fn few_options_give_fewer_cards() {
        // Six skills, only Blink can still grow, no evolution pending, every passive maxed.
        let lo = Loadout(vec![
            (SkillId::Blink, 1),
            (SkillId::Blizzard, SkillId::Blizzard.max_level()),
            (SkillId::FrostNova, SkillId::FrostNova.max_level()),
            (SkillId::WinterBolts, 1),
            (SkillId::Pyroblast, 1),
            (SkillId::ArcaneStorm, 1),
        ]);
        let cards = roll_cards(&lo, &maxed_passives(), &mut Rng::new(5), CARDS);
        assert_eq!(cards, vec![Card::Upgrade(SkillId::Blink, 2)]);
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

        let mut pa = Passives::default();
        pa.apply(Card::NewPassive(PassiveId::Haste));
        pa.apply(Card::UpgradePassive(PassiveId::Haste, 3)); // skip refused
        assert_eq!(pa.level_of(PassiveId::Haste), 1);
        pa.apply(Card::UpgradePassive(PassiveId::Haste, 2));
        assert_eq!(pa.level_of(PassiveId::Haste), 2);
        for p in PassiveId::ALL {
            pa.apply(Card::NewPassive(p));
        }
        assert_eq!(pa.0.len(), MAX_PASSIVES);
    }

    #[test]
    fn a_whole_run_of_picks_never_breaks_the_rules() {
        let mut rng = Rng::new(42);
        let mut lo = Loadout::starting();
        let mut pa = Passives::default();
        let mut evolved = 0;
        for _ in 0..150 {
            let cards = roll_cards(&lo, &pa, &mut rng, CARDS);
            if cards == [Card::Heal] {
                break;
            }
            check_legal(&lo, &pa, &cards);
            let pick = cards[rng.below(cards.len())];
            evolved += matches!(pick, Card::Evolve(..)) as u32;
            lo.apply(pick);
            pa.apply(pick);
            assert!(lo.0.len() <= MAX_SKILLS && pa.0.len() <= MAX_PASSIVES);
            for (s, l) in &lo.0 {
                assert!(*l >= 1 && *l <= s.max_level());
            }
        }
        assert_eq!(roll_cards(&lo, &pa, &mut rng, CARDS), vec![Card::Heal]);
        assert!(evolved <= 3);
    }

    #[test]
    fn stats_from_passives() {
        let base = Stats::default();
        assert_eq!(base.damage_mul, 1.0);
        assert_eq!(base.max_health, HERO_MAX_HEALTH);
        assert_eq!(base.crit_chance, BASE_CRIT);
        let s = Stats::from_passives(&maxed_passives());
        assert!(s.damage_mul > 1.4 && s.cooldown_mul < 0.7 && s.cooldown_mul >= MIN_COOLDOWN_MUL);
        assert!(s.max_health > HERO_MAX_HEALTH);
    }

    #[test]
    fn gear_and_talents_add_in() {
        use crate::content::items::{ItemSlot, Rarity};
        use crate::content::talents::TalentId;
        let staff = Item {
            slot: ItemSlot::Weapon,
            rarity: Rarity::Rare,
            level: 5,
            affixes: vec![(Affix::Power, 0.1), (Affix::Health, 12.0)],
            look: 0,
        };
        let mut t = Talents::default();
        t.buy(TalentId::IceShell, 9);
        for _ in 0..3 {
            t.buy(TalentId::Fervor, 9);
        }
        t.buy(TalentId::Ignition, 9);
        let b = Bonus::from_gear([&staff].into_iter()).plus(Bonus::from_talents(&t));
        let s = Stats::from_parts(&Passives::default(), &b);
        assert!((s.damage_mul - 1.22).abs() < 1e-5, "{}", s.damage_mul);
        assert_eq!(s.max_health, HERO_MAX_HEALTH + 22.0);
        assert!((s.crit_chance - (BASE_CRIT + 0.03)).abs() < 1e-6);
        assert_eq!(s.crit_mul, CRIT_MULTIPLIER);
        t.buy(TalentId::Searing, 99); // gate: needs 5 in fire, has 4
        assert_eq!(Bonus::from_talents(&t).crit_power, 0.0);
        t.buy(TalentId::Ignition, 99);
        t.buy(TalentId::Searing, 99);
        assert_eq!(Bonus::from_talents(&t).crit_power, 0.5);
    }

    #[test]
    fn hits_and_crits() {
        let s = Stats {
            damage_mul: 1.5,
            crit_chance: 0.25,
            ..Stats::default()
        };
        assert_eq!(hit_damage(10.0, &s, 0.9), (15.0, false));
        assert_eq!(hit_damage(10.0, &s, 0.1), (30.0, true));
        assert_eq!(orb_xp(3, &Stats::default()), 3);
        let wise = Stats {
            xp_mul: 1.5,
            ..Stats::default()
        };
        assert_eq!(orb_xp(3, &wise), 5);
        assert_eq!(orb_xp(1, &wise), 2);
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
    fn ranged_enemies_hold_their_distance() {
        let hero = Vec3::ZERO;
        let far = kite_step(Vec3::X * 20.0, hero, 3.0, 1.0, 10.0);
        assert!((far.x - 17.0).abs() < 1e-5, "closes in");
        let near = kite_step(Vec3::X * 4.0, hero, 3.0, 1.0, 10.0);
        assert!(near.x > 4.0, "backs off");
        let held = kite_step(Vec3::X * 9.0, hero, 3.0, 1.0, 10.0);
        assert_eq!(held, Vec3::X * 9.0);
        assert!(kite_step(Vec3::X * 12.0, hero, 30.0, 1.0, 10.0).x >= 10.0 - 1e-5);
    }

    #[test]
    fn fuses_grow() {
        assert_eq!(fuse_progress(1.0, 1.0), 0.0);
        assert_eq!(fuse_progress(0.0, 1.0), 1.0);
        assert_eq!(fuse_progress(0.25, 1.0), 0.75);
        assert_eq!(fuse_progress(0.0, 0.0), 1.0);
    }

    #[test]
    fn contact_reach() {
        assert!(touching(1.0, 0.5));
        assert!(!touching(1.2, 0.5));
    }

    #[test]
    fn enemies_swing_on_a_rhythm() {
        let (a, cd) = enemy_action(true, false, 0.05, 0.1, false, false);
        assert_eq!((a, cd), (Some(EnemyAction::Attack), ATTACK_EVERY));
        let (a, cd) = enemy_action(true, false, 1.0, 0.1, false, false);
        assert_eq!(a, None);
        assert!((cd - 0.9).abs() < 1e-6);
        // A swing beats a flinch.
        assert_eq!(
            enemy_action(true, false, 0.0, 0.1, true, false).0,
            Some(EnemyAction::Attack)
        );
    }

    #[test]
    fn frozen_enemies_do_not_swing_or_flinch() {
        assert_eq!(enemy_action(true, true, 0.0, 0.1, true, false).0, None);
    }

    #[test]
    fn flinches_wait_for_the_current_animation() {
        assert_eq!(
            enemy_action(false, false, 1.0, 0.1, true, false).0,
            Some(EnemyAction::Hit)
        );
        assert_eq!(enemy_action(false, false, 1.0, 0.1, true, true).0, None);
        assert_eq!(
            enemy_action(true, false, 1.0, 0.1, true, false).0,
            Some(EnemyAction::Hit)
        );
    }

    #[test]
    fn arriving_enemies_swing_soon() {
        let (_, cd) = enemy_action(false, false, ATTACK_EVERY, 0.1, false, false);
        assert!(cd <= ATTACK_EVERY * 0.3);
    }

    #[test]
    fn corpses_sink_at_the_end() {
        assert_eq!(corpse_depth(CORPSE_SECS), 0.0);
        assert_eq!(corpse_depth(CORPSE_SINK), 0.0);
        assert!((corpse_depth(0.0) - CORPSE_DEPTH).abs() < 1e-6);
        assert!(corpse_depth(CORPSE_SINK / 2.0) > 0.0);
        assert!((corpse_depth(-1.0) - CORPSE_DEPTH).abs() < 1e-6);
    }

    #[test]
    fn debug_passives_parse() {
        assert_eq!(parse_passives("all").0.len(), PassiveId::ALL.len());
        assert_eq!(
            parse_passives("power:3, haste, x, power:5").0,
            vec![(PassiveId::Power, 3), (PassiveId::Haste, 1)]
        );
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
