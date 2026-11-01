//! Combat rules: health, hit rolls, cooldowns, targeting and status effects. Positions are on
//! the ground plane (Bevy `x, z`), in yards.

use bevy::math::Vec2;

use super::rng::Rng;
use super::skills::Effect;

/// Chance for a hit to crit, and how much a crit multiplies.
pub const CRIT_CHANCE: f32 = 0.05;
pub const CRIT_MULT: f32 = 1.5;
/// A cooldown fires at most this many casts in one frame, so a long hitch is not a burst.
pub const MAX_CASTS_PER_TICK: u32 = 2;
/// Seconds the hero cannot be hurt again after a contact hit.
pub const HERO_HIT_GRACE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn full(max: f32) -> Self {
        Self { current: max, max }
    }

    /// Takes `amount` damage; true if this hit killed (it was alive and now is not).
    pub fn damage(&mut self, amount: f32) -> bool {
        if self.is_dead() {
            return false;
        }
        self.current = (self.current - amount.max(0.0)).max(0.0);
        self.is_dead()
    }

    pub fn heal(&mut self, amount: f32) {
        if !self.is_dead() {
            self.current = (self.current + amount.max(0.0)).min(self.max);
        }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            self.current / self.max
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub amount: f32,
    pub crit: bool,
}

/// Rolls one hit of `base` damage.
pub fn roll_hit(base: f32, rng: &mut Rng) -> Hit {
    let crit = rng.chance(CRIT_CHANCE);
    Hit {
        amount: if crit { base * CRIT_MULT } else { base },
        crit,
    }
}

/// A skill's cast timer. Starts ready.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Cooldown {
    pub remaining: f32,
}

impl Cooldown {
    /// Advances by `dt` for a skill of `period` seconds; returns how many casts are due.
    pub fn tick(&mut self, dt: f32, period: f32) -> u32 {
        let period = period.max(1e-3);
        self.remaining -= dt;
        let mut casts = 0;
        while self.remaining <= 0.0 && casts < MAX_CASTS_PER_TICK {
            self.remaining += period;
            casts += 1;
        }
        // Past the cap, drop the backlog rather than carrying it into the next frames.
        if self.remaining <= 0.0 {
            self.remaining = period;
        }
        casts
    }
}

/// Indices of up to `k` targets within `range` of `origin`, nearest first.
pub fn nearest_in_range(origin: Vec2, range: f32, targets: &[Vec2], k: usize) -> Vec<usize> {
    let r2 = range * range;
    let mut hits: Vec<(f32, usize)> = targets
        .iter()
        .enumerate()
        .map(|(i, p)| (p.distance_squared(origin), i))
        .filter(|&(d, _)| d <= r2)
        .collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    hits.into_iter().take(k).map(|(_, i)| i).collect()
}

/// Indices of every target within `radius` of `center`.
pub fn in_radius(center: Vec2, radius: f32, targets: &[Vec2]) -> Vec<usize> {
    let r2 = radius * radius;
    targets
        .iter()
        .enumerate()
        .filter(|(_, p)| p.distance_squared(center) <= r2)
        .map(|(i, _)| i)
        .collect()
}

/// Where a projectile flying at `speed` lands this frame, and whether it reached `target`.
pub fn home_in(pos: Vec2, target: Vec2, speed: f32, dt: f32) -> (Vec2, bool) {
    let to = target - pos;
    let step = speed * dt;
    if to.length() <= step {
        (target, true)
    } else {
        (pos + to.normalize() * step, false)
    }
}

/// An enemy's timed conditions from skill effects.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Statuses {
    pub slow: f32,
    pub slow_secs: f32,
    pub root_secs: f32,
    /// Burn damage per second, and its seconds left.
    pub burn_dps: f32,
    pub burn_secs: f32,
}

impl Statuses {
    /// Applies what a hit of `damage` carrying `effect` leaves. A stronger slow replaces a weaker
    /// one; durations refresh to the longer; burns add their damage per second.
    pub fn apply(&mut self, effect: Effect, damage: f32) {
        match effect {
            Effect::None => {}
            Effect::Chill { slow, secs } => {
                if slow >= self.slow || self.slow_secs <= 0.0 {
                    self.slow = slow;
                }
                self.slow_secs = self.slow_secs.max(secs);
            }
            Effect::Root { secs } => self.root_secs = self.root_secs.max(secs),
            Effect::Burn { fraction, secs } => {
                if secs > 0.0 {
                    let left = if self.burn_secs > 0.0 {
                        self.burn_dps * self.burn_secs
                    } else {
                        0.0
                    };
                    let total = left + damage * fraction;
                    self.burn_secs = self.burn_secs.max(secs);
                    self.burn_dps = total / self.burn_secs;
                }
            }
        }
    }

    /// Advances by `dt`; returns the burn damage dealt in that time.
    pub fn tick(&mut self, dt: f32) -> f32 {
        let burn_time = dt.min(self.burn_secs.max(0.0));
        let burn = self.burn_dps * burn_time;
        self.burn_secs = (self.burn_secs - dt).max(0.0);
        if self.burn_secs == 0.0 {
            self.burn_dps = 0.0;
        }
        self.slow_secs = (self.slow_secs - dt).max(0.0);
        if self.slow_secs == 0.0 {
            self.slow = 0.0;
        }
        self.root_secs = (self.root_secs - dt).max(0.0);
        burn
    }

    /// The multiplier on movement speed right now.
    pub fn speed_mult(&self) -> f32 {
        if self.root_secs > 0.0 {
            0.0
        } else if self.slow_secs > 0.0 {
            (1.0 - self.slow).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

/// An enemy's step toward the hero this frame, stopping at `stop` yards (melee reach).
pub fn chase(pos: Vec2, hero: Vec2, speed: f32, dt: f32, stop: f32) -> Vec2 {
    let to = hero - pos;
    let dist = to.length();
    if dist <= stop || dist < 1e-6 {
        return pos;
    }
    let step = (speed * dt).min(dist - stop);
    pos + to / dist * step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_kills_once() {
        let mut h = Health::full(10.0);
        assert!(!h.damage(4.0));
        assert!(h.damage(10.0));
        assert_eq!(h.current, 0.0);
        assert!(!h.damage(5.0), "already dead");
        h.heal(5.0);
        assert!(h.is_dead(), "no healing the dead");
    }

    #[test]
    fn heal_caps_at_max() {
        let mut h = Health::full(10.0);
        h.damage(3.0);
        h.heal(100.0);
        assert_eq!(h.current, 10.0);
        assert_eq!(h.fraction(), 1.0);
    }

    #[test]
    fn crits_are_rare_and_bigger() {
        let mut rng = Rng::new(1);
        let mut crits = 0;
        for _ in 0..10_000 {
            let h = roll_hit(10.0, &mut rng);
            if h.crit {
                crits += 1;
                assert_eq!(h.amount, 15.0);
            } else {
                assert_eq!(h.amount, 10.0);
            }
        }
        assert!((300..700).contains(&crits), "{crits}");
    }

    #[test]
    fn cooldown_starts_ready_and_repeats() {
        let mut c = Cooldown::default();
        assert_eq!(c.tick(0.016, 1.0), 1);
        assert_eq!(c.tick(0.5, 1.0), 0);
        assert_eq!(c.tick(0.5, 1.0), 1);
    }

    #[test]
    fn cooldown_caps_a_hitch() {
        let mut c = Cooldown::default();
        assert_eq!(c.tick(10.0, 0.5), MAX_CASTS_PER_TICK);
        assert!(c.remaining > 0.0);
    }

    #[test]
    fn nearest_first_within_range() {
        let t = [
            Vec2::new(10.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(50.0, 0.0),
            Vec2::new(0.0, -5.0),
        ];
        assert_eq!(nearest_in_range(Vec2::ZERO, 20.0, &t, 3), vec![1, 3, 0]);
        assert_eq!(nearest_in_range(Vec2::ZERO, 20.0, &t, 1), vec![1]);
        assert!(nearest_in_range(Vec2::ZERO, 1.0, &t, 3).is_empty());
        assert_eq!(in_radius(Vec2::ZERO, 5.0, &t), vec![1, 3]);
    }

    #[test]
    fn projectile_arrives() {
        let (p, hit) = home_in(Vec2::ZERO, Vec2::new(10.0, 0.0), 20.0, 0.25);
        assert_eq!((p, hit), (Vec2::new(5.0, 0.0), false));
        let (p, hit) = home_in(p, Vec2::new(10.0, 0.0), 20.0, 0.5);
        assert_eq!((p, hit), (Vec2::new(10.0, 0.0), true));
    }

    #[test]
    fn chill_and_root_slow_movement() {
        let mut s = Statuses::default();
        assert_eq!(s.speed_mult(), 1.0);
        s.apply(
            Effect::Chill {
                slow: 0.3,
                secs: 2.0,
            },
            0.0,
        );
        assert!((s.speed_mult() - 0.7).abs() < 1e-6);
        s.apply(Effect::Root { secs: 1.0 }, 0.0);
        assert_eq!(s.speed_mult(), 0.0);
        s.tick(1.0);
        assert!((s.speed_mult() - 0.7).abs() < 1e-6, "root over, chill left");
        s.tick(1.0);
        assert_eq!(s.speed_mult(), 1.0);
    }

    #[test]
    fn burn_deals_its_share_over_time() {
        let mut s = Statuses::default();
        s.apply(
            Effect::Burn {
                fraction: 0.25,
                secs: 2.0,
            },
            40.0,
        );
        let mut total = 0.0;
        for _ in 0..30 {
            total += s.tick(0.1);
        }
        assert!((total - 10.0).abs() < 1e-3, "{total}");
        assert_eq!(s.burn_dps, 0.0);
    }

    #[test]
    fn burns_stack_damage() {
        let mut s = Statuses::default();
        let burn = Effect::Burn {
            fraction: 0.5,
            secs: 2.0,
        };
        s.apply(burn, 10.0);
        s.apply(burn, 10.0);
        let mut total = 0.0;
        for _ in 0..40 {
            total += s.tick(0.1);
        }
        assert!((total - 10.0).abs() < 1e-3, "{total}");
    }

    #[test]
    fn chase_stops_at_reach() {
        let p = chase(Vec2::new(10.0, 0.0), Vec2::ZERO, 4.0, 1.0, 1.0);
        assert_eq!(p, Vec2::new(6.0, 0.0));
        let p = chase(Vec2::new(2.0, 0.0), Vec2::ZERO, 4.0, 1.0, 1.0);
        assert_eq!(p, Vec2::new(1.0, 0.0));
        let p = chase(Vec2::new(0.5, 0.0), Vec2::ZERO, 4.0, 1.0, 1.0);
        assert_eq!(p, Vec2::new(0.5, 0.0));
    }
}
