//! The spawn director: how often and how many enemies come, where, and how tough, as a run's
//! clock runs. Elites at fixed spots, mini bosses and the map boss's placement are phase 3; the
//! tier stats and the boss timer are here so combat can be built against them.

use bevy::math::Vec2;

use super::rng::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    Normal,
    Elite,
    MiniBoss,
    Boss,
}

/// An enemy's numbers at a run's start, before [`health_scale`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyStats {
    pub health: f32,
    /// Yards per second; the hero runs at 7.
    pub speed: f32,
    /// Damage per contact hit on the hero.
    pub contact: f32,
    /// Body radius in yards, for contact and spacing.
    pub radius: f32,
}

pub fn tier_stats(tier: Tier) -> EnemyStats {
    match tier {
        Tier::Normal => EnemyStats {
            health: 20.0,
            speed: 4.5,
            contact: 5.0,
            radius: 0.6,
        },
        Tier::Elite => EnemyStats {
            health: 150.0,
            speed: 4.0,
            contact: 12.0,
            radius: 1.0,
        },
        Tier::MiniBoss => EnemyStats {
            health: 800.0,
            speed: 3.8,
            contact: 20.0,
            radius: 1.6,
        },
        Tier::Boss => EnemyStats {
            health: 4000.0,
            speed: 3.5,
            contact: 35.0,
            radius: 2.5,
        },
    }
}

/// The hero's health at a run's start.
pub const HERO_HEALTH: f32 = 100.0;

/// Seconds between spawn waves at the start, the floor it shrinks to, and the seconds it takes to
/// halve.
pub const WAVE_INTERVAL_START: f32 = 1.5;
pub const WAVE_INTERVAL_MIN: f32 = 0.3;
pub const WAVE_INTERVAL_HALF_LIFE: f32 = 180.0;
/// Enemies per wave at the start, and the cap it grows to (one more per minute).
pub const PACK_MAX: u32 = 6;
/// No new spawns while this many are alive.
pub const MAX_ALIVE: usize = 150;
/// The spawn ring around the hero: just off screen at the default camera distance.
pub const RING_INNER: f32 = 24.0;
pub const RING_OUTER: f32 = 30.0;
/// The map boss arrives on its own after this many seconds (DESIGN.md: on a timer).
pub const BOSS_AT: f32 = 600.0;

/// Seconds between waves `elapsed` seconds into a run.
pub fn wave_interval(elapsed: f32) -> f32 {
    (WAVE_INTERVAL_START * 0.5f32.powf(elapsed.max(0.0) / WAVE_INTERVAL_HALF_LIFE))
        .max(WAVE_INTERVAL_MIN)
}

/// Enemies per wave `elapsed` seconds in.
pub fn pack_size(elapsed: f32) -> u32 {
    (1 + (elapsed.max(0.0) / 60.0) as u32).min(PACK_MAX)
}

/// Enemy health multiplier `elapsed` seconds in: +15% per minute.
pub fn health_scale(elapsed: f32) -> f32 {
    1.0 + 0.15 * elapsed.max(0.0) / 60.0
}

/// A point on the spawn ring around `hero`.
pub fn ring_point(hero: Vec2, rng: &mut Rng) -> Vec2 {
    let angle = rng.range_f32(0.0, std::f32::consts::TAU);
    let dist = rng.range_f32(RING_INNER, RING_OUTER);
    hero + Vec2::from_angle(angle) * dist
}

/// The director's clock.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Director {
    /// Seconds since the run started.
    pub elapsed: f32,
    next_wave: f32,
    boss_called: bool,
}

/// What the director wants spawned this frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Orders {
    /// Normal enemies to spawn on the ring.
    pub normals: u32,
    /// The map boss is due (once per run).
    pub boss: bool,
}

impl Director {
    /// Advances by `dt` with `alive` enemies up; returns what to spawn.
    pub fn tick(&mut self, dt: f32, alive: usize) -> Orders {
        self.elapsed += dt;
        let mut orders = Orders::default();
        if !self.boss_called && self.elapsed >= BOSS_AT {
            self.boss_called = true;
            orders.boss = true;
        }
        if self.elapsed >= self.next_wave {
            self.next_wave = self.elapsed + wave_interval(self.elapsed);
            let room = MAX_ALIVE.saturating_sub(alive) as u32;
            orders.normals = pack_size(self.elapsed).min(room);
        }
        orders
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_get_tougher() {
        let t = [Tier::Normal, Tier::Elite, Tier::MiniBoss, Tier::Boss].map(tier_stats);
        for w in t.windows(2) {
            assert!(w[1].health > w[0].health && w[1].contact > w[0].contact);
        }
        // Nothing outruns the hero.
        assert!(t.iter().all(|s| s.speed < crate::motion::HERO_SPEED));
    }

    #[test]
    fn pressure_rises_over_time() {
        assert_eq!(wave_interval(0.0), WAVE_INTERVAL_START);
        assert!((wave_interval(WAVE_INTERVAL_HALF_LIFE) - WAVE_INTERVAL_START / 2.0).abs() < 1e-5);
        assert_eq!(wave_interval(1e6), WAVE_INTERVAL_MIN);
        assert_eq!(pack_size(0.0), 1);
        assert_eq!(pack_size(125.0), 3);
        assert_eq!(pack_size(1e6), PACK_MAX);
        assert!((health_scale(120.0) - 1.3).abs() < 1e-5);
    }

    #[test]
    fn ring_points_are_on_the_ring() {
        let mut rng = Rng::new(4);
        let hero = Vec2::new(100.0, -50.0);
        for _ in 0..1000 {
            let d = ring_point(hero, &mut rng).distance(hero);
            assert!((RING_INNER - 1e-3..RING_OUTER + 1e-3).contains(&d), "{d}");
        }
    }

    #[test]
    fn first_wave_is_immediate_then_spaced() {
        let mut d = Director::default();
        assert_eq!(d.tick(0.016, 0).normals, 1);
        assert_eq!(d.tick(0.5, 1).normals, 0);
        assert_eq!(d.tick(1.1, 1).normals, 1);
    }

    #[test]
    fn alive_cap_holds_spawns() {
        let mut d = Director {
            elapsed: 300.0,
            ..Default::default()
        };
        assert_eq!(d.tick(0.1, MAX_ALIVE).normals, 0);
        let mut d = Director {
            elapsed: 300.0,
            ..Default::default()
        };
        assert_eq!(d.tick(0.1, MAX_ALIVE - 2).normals, 2);
    }

    #[test]
    fn boss_comes_once_on_time() {
        let mut d = Director::default();
        let mut bosses = 0;
        let mut at = None;
        for _ in 0..(BOSS_AT as usize * 2 * 10) {
            if d.tick(0.1, 0).boss {
                bosses += 1;
                at = Some(d.elapsed);
            }
        }
        assert_eq!(bosses, 1);
        assert!((at.unwrap() - BOSS_AT).abs() < 0.11);
    }
}
