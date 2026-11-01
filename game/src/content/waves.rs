//! The first map's wave schedule: how dense and strong the spawns are at a time into the run.

use super::EnemyKind;

/// Spawns happen on a ring this far from the hero, just off screen at the default zoom.
pub const SPAWN_RING_MIN: f32 = 24.0;
pub const SPAWN_RING_MAX: f32 = 30.0;
/// An enemy that falls this far behind is moved back onto the ring ahead of the hero.
pub const LEASH: f32 = 45.0;

/// When the Defias boss arrives, seconds into the run (phase 3).
#[allow(dead_code)] // phase 3
pub const BOSS_AT: f32 = 600.0;
/// Where the gnoll chief waits, WoW world `x, y` (phase 3). Roughly the Fargodeep Mine mouth,
/// south-west of Goldshire; to be checked in game.
#[allow(dead_code)] // phase 3
pub const GNOLL_CHIEF_AT: (f32, f32) = (-9800.0, 140.0);

/// The spawner's settings at one moment of the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveParams {
    /// Seconds between spawn groups.
    pub interval: f32,
    /// Enemies per group (they appear together, at one spot on the ring).
    pub group: u32,
    /// Most enemies alive at once; the spawner waits above it.
    pub max_alive: u32,
    /// Multipliers on the enemy table's health and contact damage.
    pub health_mul: f32,
    pub damage_mul: f32,
    /// Relative odds of each kind in [`EnemyKind::NORMAL`] order (kobold, wolf, spider).
    pub weights: [f32; 3],
}

/// The wave at `t` seconds into the run: steadily denser, then stronger; wolves join at 0:45,
/// spiders at 2:00.
pub fn wave_at(t: f32) -> WaveParams {
    let t = t.max(0.0);
    let ramp = |start: f32, over: f32| ((t - start) / over).clamp(0.0, 1.0);
    WaveParams {
        interval: (1.2 * 0.93f32.powf(t / 20.0)).max(0.25),
        group: 1 + (t / 75.0) as u32,
        max_alive: (50.0 + t / 4.0).min(220.0) as u32,
        health_mul: 1.0 + t / 200.0,
        damage_mul: 1.0 + t / 400.0,
        weights: [1.0, 0.9 * ramp(45.0, 60.0), 0.7 * ramp(120.0, 90.0)],
    }
}

/// The kind for a roll `r` in `0..1` under `weights` (in [`EnemyKind::NORMAL`] order).
pub fn pick_kind(weights: [f32; 3], r: f32) -> EnemyKind {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return EnemyKind::Kobold;
    }
    let mut acc = 0.0;
    for (w, k) in weights.iter().zip(EnemyKind::NORMAL) {
        acc += w / total;
        if r < acc {
            return k;
        }
    }
    EnemyKind::NORMAL[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_only_get_harder() {
        let mut prev = wave_at(0.0);
        for s in 1..=900 {
            let w = wave_at(s as f32);
            assert!(w.interval <= prev.interval);
            assert!(w.group >= prev.group);
            assert!(w.max_alive >= prev.max_alive);
            assert!(w.health_mul >= prev.health_mul);
            prev = w;
        }
        assert!(prev.interval >= 0.25 && prev.max_alive <= 220);
    }

    #[test]
    fn the_start_is_kobolds_only() {
        let w = wave_at(10.0);
        assert_eq!(w.weights[1], 0.0);
        assert_eq!(w.weights[2], 0.0);
        for i in 0..10 {
            assert_eq!(pick_kind(w.weights, i as f32 / 10.0), EnemyKind::Kobold);
        }
    }

    #[test]
    fn later_all_kinds_appear() {
        let w = wave_at(300.0);
        let mut seen = [false; 3];
        for i in 0..100 {
            let k = pick_kind(w.weights, i as f32 / 100.0);
            seen[EnemyKind::NORMAL.iter().position(|n| *n == k).unwrap()] = true;
        }
        assert_eq!(seen, [true; 3]);
    }

    #[test]
    fn zero_weights_fall_back() {
        assert_eq!(pick_kind([0.0; 3], 0.5), EnemyKind::Kobold);
        assert_eq!(pick_kind([1.0, 1.0, 1.0], 0.9999), EnemyKind::Spider);
    }
}
