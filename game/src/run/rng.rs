//! A small seeded random number generator (SplitMix64), so a run's rolls are reproducible from
//! its seed and tests are deterministic. Not for anything cryptographic.

#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // The top 24 bits: exactly representable, never 1.0.
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `[lo, hi)`; `lo` when the range is empty.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        if hi <= lo {
            return lo;
        }
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniform in `0..n`; `n` must be non-zero.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "Rng::below(0)");
        (self.next_u64() % n as u64) as usize
    }

    /// True with probability `p` (clamped to `0..=1`).
    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p.clamp(0.0, 1.0)
    }

    /// An index into `weights` picked in proportion to its weight; `None` if all are zero.
    pub fn weighted(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = self.next_f32() * total;
        let mut last = None;
        for (i, w) in weights.iter().enumerate() {
            let w = w.max(0.0);
            if w <= 0.0 {
                continue;
            }
            if roll < w {
                return Some(i);
            }
            roll -= w;
            last = Some(i);
        }
        // Float rounding can leave a sliver past the end: the last positive weight takes it.
        last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn floats_stay_in_range() {
        let mut r = Rng::new(3);
        for _ in 0..10_000 {
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
            let g = r.range_f32(-2.0, 5.0);
            assert!((-2.0..5.0).contains(&g));
            assert!(r.below(3) < 3);
        }
        assert_eq!(r.range_f32(4.0, 4.0), 4.0);
    }

    #[test]
    fn weighted_skips_zero_weights() {
        let mut r = Rng::new(11);
        assert_eq!(r.weighted(&[0.0, 0.0]), None);
        assert_eq!(r.weighted(&[]), None);
        for _ in 0..1000 {
            assert_eq!(r.weighted(&[0.0, 2.0, 0.0]), Some(1));
        }
    }

    #[test]
    fn weighted_follows_proportions() {
        let mut r = Rng::new(5);
        let mut hits = [0u32; 2];
        for _ in 0..20_000 {
            hits[r.weighted(&[1.0, 3.0]).unwrap()] += 1;
        }
        let share = hits[1] as f32 / 20_000.0;
        assert!((share - 0.75).abs() < 0.02, "share {share}");
    }
}
