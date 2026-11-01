//! Experience and levels within one run. Levels reset every run (DESIGN.md); the XP earned is
//! also totalled for the talents unlocked between runs (phase 5).

/// The highest level a run can reach.
pub const MAX_LEVEL: u32 = 60;

/// XP needed to go from `level` to `level + 1`: a gentle quadratic, so early levels come every
/// few kills and later ones stretch out. 10, 17, 26, 37, 50, … ; `0` at [`MAX_LEVEL`].
pub fn xp_to_next(level: u32) -> u32 {
    if level >= MAX_LEVEL {
        return 0;
    }
    let n = level.saturating_sub(1);
    10 + 6 * n + n * n
}

/// One run's level and XP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub level: u32,
    /// XP into the current level.
    pub xp: u32,
    /// All XP gained this run, for the talent currency.
    pub total_xp: u64,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            total_xp: 0,
        }
    }
}

impl Progress {
    /// Adds `amount` XP and returns how many levels were gained (each one is a skill pick).
    /// XP past [`MAX_LEVEL`] still counts toward `total_xp`.
    pub fn gain(&mut self, amount: u32) -> u32 {
        self.total_xp += u64::from(amount);
        if self.level >= MAX_LEVEL {
            return 0;
        }
        self.xp += amount;
        let mut gained = 0;
        while self.level < MAX_LEVEL && self.xp >= xp_to_next(self.level) {
            self.xp -= xp_to_next(self.level);
            self.level += 1;
            gained += 1;
        }
        if self.level >= MAX_LEVEL {
            self.xp = 0;
        }
        gained
    }

    /// Progress toward the next level in `0..=1`, for the XP bar.
    pub fn fraction(&self) -> f32 {
        match xp_to_next(self.level) {
            0 => 1.0,
            need => self.xp as f32 / need as f32,
        }
    }
}

/// XP a kill is worth, by the enemy's tier.
pub fn kill_xp(tier: crate::run::director::Tier) -> u32 {
    use crate::run::director::Tier;
    match tier {
        Tier::Normal => 2,
        Tier::Elite => 15,
        Tier::MiniBoss => 60,
        Tier::Boss => 200,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_grows() {
        assert_eq!(xp_to_next(1), 10);
        assert_eq!(xp_to_next(2), 17);
        assert_eq!(xp_to_next(3), 26);
        for l in 1..MAX_LEVEL - 1 {
            assert!(xp_to_next(l + 1) > xp_to_next(l));
        }
        assert_eq!(xp_to_next(MAX_LEVEL), 0);
    }

    #[test]
    fn gain_carries_over() {
        let mut p = Progress::default();
        assert_eq!(p.gain(9), 0);
        assert_eq!(p.gain(1), 1);
        assert_eq!((p.level, p.xp), (2, 0));
        // 17 + 26 = 43 crosses two levels with 2 left over.
        assert_eq!(p.gain(45), 2);
        assert_eq!((p.level, p.xp), (4, 2));
        assert_eq!(p.total_xp, 55);
    }

    #[test]
    fn capped_at_max_level() {
        let mut p = Progress::default();
        let gained = p.gain(10_000_000);
        assert_eq!(gained, MAX_LEVEL - 1);
        assert_eq!((p.level, p.xp), (MAX_LEVEL, 0));
        assert_eq!(p.gain(50), 0);
        assert_eq!(p.total_xp, 10_000_050);
        assert_eq!(p.fraction(), 1.0);
    }

    #[test]
    fn fraction_tracks_xp() {
        let mut p = Progress::default();
        p.gain(5);
        assert!((p.fraction() - 0.5).abs() < 1e-6);
    }
}
