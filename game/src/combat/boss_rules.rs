//! Pure rules for phase 3, "Harita": when bosses appear, when the elite wakes, how special
//! attacks are timed and land, boss health scaling, and the texts built from them. No ECS here;
//! `bosses.rs`, `boss_hud.rs` and `portal.rs` are thin systems on top.

use bevy::math::{Quat, Vec3};

use super::rules::flat_dist;
use crate::content::bosses::{AttackDef, AttackShape, BossDef, Rank, WARN_LEAD, WARN_LINGER};

// ---------------------------------------------------------------------------------------------
// Appearing.

/// A guarding elite is put in the world when the hero comes this close to its spot, and taken
/// out again (if untouched) beyond `GUARD_HIDE`. The gap stops it flickering at the edge, and
/// both stay inside the enemy leash (45 yd) so the leash never drags a sleeping elite away.
pub const GUARD_SHOW: f32 = 40.0;
pub const GUARD_HIDE: f32 = 44.0;

/// Whether a guarding elite should be in the world, given the hero's distance to its spot and
/// whether it is in the world now.
pub fn guard_present(dist: f32, present: bool) -> bool {
    if present {
        dist <= GUARD_HIDE
    } else {
        dist <= GUARD_SHOW
    }
}

/// A guarding elite starts fighting when the hero comes within `aggro` yards or hurts it.
pub fn aggroed(dist: f32, aggro: f32, health: f32, max_health: f32) -> bool {
    dist <= aggro || health < max_health
}

/// Whether a timed boss arriving at `at` should spawn now. A run started late (the `RNS_TIME`
/// debug jump starts the clock at `start`) skips the arrivals before its start instead of
/// dropping them all at once.
pub fn timed_due(at: f32, start: f32, now: f32, done: bool) -> bool {
    !done && at >= start && now >= at
}

/// The centre-screen warning for timed arrivals at run time `now`, if one is showing.
pub fn arrival_banner(now: f32, start: f32, bosses: &[&BossDef]) -> Option<String> {
    for b in bosses {
        let Some(at) = b.arrives_at() else {
            continue;
        };
        if at < start {
            continue;
        }
        let what = match b.rank {
            Rank::MapBoss => "BOSS",
            Rank::Mini => "Mini boss",
            Rank::Elite => "Elit",
        };
        if now >= at - WARN_LEAD && now < at {
            let secs = (at - now).ceil() as u32;
            return Some(format!("Dikkat! {what}: {} yaklaşıyor… {secs}", b.name()));
        }
        if now >= at && now < at + WARN_LINGER {
            let tail = if b.rank == Rank::MapBoss {
                " Onu kes, portal açılsın."
            } else {
                ""
            };
            return Some(format!("{what}: {} geldi!{tail}", b.name()));
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Strength.

/// Health multiplier for a boss met at hero level `level` when it was balanced for `expected`:
/// 5% per level either way, kept within 0.75–1.6 so an under-levelled run (or a debug jump) can
/// still win and an over-levelled one still has a fight.
pub fn boss_health_mul(level: u32, expected: u32) -> f32 {
    (1.0 + 0.05 * (level as f32 - expected as f32)).clamp(0.75, 1.6)
}

/// Bosses shrug off long freezes: Frost Nova holds them at most this long.
pub const BOSS_FREEZE_CAP: f32 = 0.8;

pub fn cap_freeze(frozen: f32) -> f32 {
    frozen.min(BOSS_FREEZE_CAP)
}

// ---------------------------------------------------------------------------------------------
// Special attacks.

/// Where a boss is in its special-attack cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttackPhase {
    /// Normal chasing; the next special may start when `left` reaches 0.
    Waiting { left: f32 },
    /// Telegraph on the ground; the boss stands still.
    Windup { attack: usize, left: f32 },
    /// A charge in progress (other shapes land instantly and have no active part).
    Active { attack: usize, left: f32 },
}

/// What happened in one step of the cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackEvent {
    None,
    /// The wind-up of this attack began: lay its telegraph.
    Begin(usize),
    /// The telegraph ran out: the attack lands (a charge starts moving).
    Land(usize),
    /// A charge finished.
    End(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttackState {
    pub phase: AttackPhase,
    /// Where the rotation looks first next time, so a boss with several attacks varies them.
    pub next: usize,
}

/// Seconds a charge runs; zero for shapes that land instantly.
pub fn active_secs(attack: &AttackDef) -> f32 {
    match attack.shape {
        AttackShape::Charge { speed, length, .. } => length / speed.max(0.1),
        _ => 0.0,
    }
}

/// The first attack, starting at `from` and wrapping, whose range holds `dist`.
pub fn pick_attack(attacks: &[AttackDef], dist: f32, from: usize) -> Option<usize> {
    let n = attacks.len();
    (0..n)
        .map(|i| (from + i) % n)
        .find(|&i| dist >= attacks[i].min_range && dist <= attacks[i].max_range)
}

impl AttackState {
    /// A fresh boss: its first special comes after `first` seconds.
    pub fn new(first: f32) -> Self {
        Self {
            phase: AttackPhase::Waiting { left: first },
            next: 0,
        }
    }

    /// Whether the boss is busy with a special (and so does not walk).
    pub fn busy(&self) -> bool {
        !matches!(self.phase, AttackPhase::Waiting { .. })
    }

    /// The attack being wound up or run, if any.
    pub fn current(&self) -> Option<usize> {
        match self.phase {
            AttackPhase::Waiting { .. } => None,
            AttackPhase::Windup { attack, .. } | AttackPhase::Active { attack, .. } => Some(attack),
        }
    }

    /// Advances the cycle by `dt` with the hero `dist` yards away. At most one event per step.
    pub fn step(&mut self, dt: f32, attacks: &[AttackDef], dist: f32) -> AttackEvent {
        match self.phase {
            AttackPhase::Waiting { left } => {
                let left = (left - dt).max(0.0);
                self.phase = AttackPhase::Waiting { left };
                if left > 0.0 || attacks.is_empty() {
                    return AttackEvent::None;
                }
                match pick_attack(attacks, dist, self.next) {
                    Some(a) => {
                        self.next = (a + 1) % attacks.len();
                        self.phase = AttackPhase::Windup {
                            attack: a,
                            left: attacks[a].windup,
                        };
                        AttackEvent::Begin(a)
                    }
                    None => AttackEvent::None,
                }
            }
            AttackPhase::Windup { attack, left } => {
                let left = left - dt;
                if left > 0.0 {
                    self.phase = AttackPhase::Windup { attack, left };
                    return AttackEvent::None;
                }
                let run = active_secs(&attacks[attack]);
                self.phase = if run > 0.0 {
                    AttackPhase::Active { attack, left: run }
                } else {
                    AttackPhase::Waiting {
                        left: attacks[attack].recover,
                    }
                };
                AttackEvent::Land(attack)
            }
            AttackPhase::Active { attack, left } => {
                let left = left - dt;
                if left > 0.0 {
                    self.phase = AttackPhase::Active { attack, left };
                    return AttackEvent::None;
                }
                self.phase = AttackPhase::Waiting {
                    left: attacks[attack].recover,
                };
                AttackEvent::End(attack)
            }
        }
    }

    /// How far through its wind-up the current attack is, `0..=1` (1 outside a wind-up).
    pub fn windup_progress(&self, attacks: &[AttackDef]) -> f32 {
        match self.phase {
            AttackPhase::Windup { attack, left } => {
                let total = attacks[attack].windup.max(1e-3);
                (1.0 - left / total).clamp(0.0, 1.0)
            }
            _ => 1.0,
        }
    }
}

/// The hero's body radius for "caught by an attack" checks.
pub const HERO_RADIUS: f32 = 0.5;

/// Whether the hero at `hero` is caught by a circle.
pub fn in_circle(center: Vec3, radius: f32, hero: Vec3) -> bool {
    flat_dist(center, hero) <= radius + HERO_RADIUS
}

/// A charge's flat direction and end point: from `from` toward `target`, `length` yards long.
/// With the target on top of the boss it goes along `fallback`.
pub fn charge_path(from: Vec3, target: Vec3, length: f32, fallback: Vec3) -> (Vec3, Vec3) {
    let mut dir = (target - from).with_y(0.0).normalize_or_zero();
    if dir == Vec3::ZERO {
        dir = fallback.with_y(0.0).normalize_or_zero();
    }
    if dir == Vec3::ZERO {
        dir = Vec3::NEG_Z;
    }
    (dir, from + dir * length)
}

/// Whether a charging boss of body `radius` with a lane `width` wide touches the hero.
pub fn charge_hits(boss: Vec3, radius: f32, width: f32, hero: Vec3) -> bool {
    flat_dist(boss, hero) <= radius.max(width * 0.5) + HERO_RADIUS
}

/// The impact points of a barrage: one on `center`, the rest evenly on a ring of `spread`
/// yards, turned by `angle0`.
pub fn barrage_spots(center: Vec3, count: u32, spread: f32, angle0: f32) -> Vec<Vec3> {
    let mut out = vec![center];
    let ring = count.saturating_sub(1);
    for i in 0..ring {
        let a = angle0 + std::f32::consts::TAU * i as f32 / ring as f32;
        out.push(center + Quat::from_rotation_y(a) * Vec3::NEG_Z * spread);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Texts.

/// Where something lies from the hero, as seen on screen with the camera turned to `cam_yaw`
/// (`yaw` 0 looks along `-Z`, as in `motion`): one of eight Turkish directions.
pub fn screen_direction(offset: Vec3, cam_yaw: f32) -> &'static str {
    let rot = Quat::from_rotation_y(cam_yaw);
    let forward = rot * Vec3::NEG_Z;
    let right = rot * Vec3::X;
    let (f, r) = (offset.dot(forward), offset.dot(right));
    let angle = r.atan2(f); // 0 ahead, +π/2 right
    let sector = ((angle / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8);
    [
        "önde",
        "sağ önde",
        "sağda",
        "sağ arkada",
        "arkada",
        "sol arkada",
        "solda",
        "sol önde",
    ][sector as usize]
}

/// What the run achieved, for the end-of-map screen.
#[derive(Debug, Clone, PartialEq)]
pub struct MapSummary {
    pub time: f32,
    pub level: u32,
    pub kills: u32,
    /// Names of the elites and mini bosses killed, in kill order.
    pub bosses: Vec<&'static str>,
    pub skills: Vec<String>,
}

/// The lines of the "Harita tamamlandı" screen.
pub fn summary_lines(s: &MapSummary) -> Vec<String> {
    let t = s.time.max(0.0) as u32;
    let mut lines = vec![
        format!("Süre: {:02}:{:02}", t / 60, t % 60),
        format!("Level: {}", s.level),
        format!("Öldürülen düşman: {}", s.kills),
    ];
    if s.bosses.is_empty() {
        lines.push("Kesilen elit / mini boss: yok".to_string());
    } else {
        lines.push(format!("Kesilen elit / mini boss: {}", s.bosses.join(", ")));
    }
    if !s.skills.is_empty() {
        lines.push(format!("Skill'ler: {}", s.skills.join(", ")));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::bosses::*;

    #[test]
    fn guard_shows_and_hides_with_a_gap() {
        assert!(!guard_present(41.0, false));
        assert!(guard_present(39.0, false));
        assert!(guard_present(42.0, true), "no flicker between the two radii");
        assert!(!guard_present(45.0, true));
        assert!(GUARD_HIDE < crate::content::waves::LEASH);
    }

    #[test]
    fn elite_wakes_on_approach_or_on_hurt() {
        assert!(!aggroed(20.0, 16.0, 650.0, 650.0));
        assert!(aggroed(15.9, 16.0, 650.0, 650.0));
        assert!(aggroed(39.0, 16.0, 640.0, 650.0));
    }

    #[test]
    fn timed_arrivals_fire_once_and_respect_a_late_start() {
        assert!(!timed_due(180.0, 0.0, 179.9, false));
        assert!(timed_due(180.0, 0.0, 180.0, false));
        assert!(!timed_due(180.0, 0.0, 200.0, true), "once");
        // RNS_TIME=590: the minis at 180 and 390 are skipped, the boss at 600 still comes.
        assert!(!timed_due(180.0, 590.0, 600.0, false));
        assert!(timed_due(600.0, 590.0, 600.0, false));
    }

    #[test]
    fn banner_counts_down_then_announces() {
        let all: Vec<&BossDef> = ELWYNN_BOSSES.to_vec();
        assert_eq!(arrival_banner(100.0, 0.0, &all), None);
        let b = arrival_banner(175.2, 0.0, &all).unwrap();
        assert!(b.contains("Kobold Ustabaşı") && b.ends_with('5'), "{b}");
        let b = arrival_banner(181.0, 0.0, &all).unwrap();
        assert!(b.contains("geldi"), "{b}");
        assert_eq!(arrival_banner(185.0, 0.0, &all), None);
        let b = arrival_banner(595.0, 590.0, &all).unwrap();
        assert!(b.starts_with("Dikkat! BOSS") && b.contains("Defias"), "{b}");
        let b = arrival_banner(601.0, 590.0, &all).unwrap();
        assert!(b.contains("portal"), "{b}");
        // Skipped arrivals are not announced either.
        assert_eq!(arrival_banner(178.0, 300.0, &all), None);
    }

    #[test]
    fn boss_health_scales_with_level_within_bounds() {
        assert_eq!(boss_health_mul(18, 18), 1.0);
        assert!((boss_health_mul(20, 18) - 1.1).abs() < 1e-5);
        assert!((boss_health_mul(16, 18) - 0.9).abs() < 1e-5);
        assert_eq!(boss_health_mul(1, 18), 0.75);
        assert_eq!(boss_health_mul(60, 18), 1.6);
        let mut prev = 0.0;
        for l in 1..40 {
            let m = boss_health_mul(l, 10);
            assert!(m >= prev);
            prev = m;
        }
    }

    #[test]
    fn freezes_are_capped_on_bosses() {
        assert_eq!(cap_freeze(3.0), BOSS_FREEZE_CAP);
        assert_eq!(cap_freeze(0.3), 0.3);
    }

    #[test]
    fn attack_rotation_respects_range() {
        let a = DEFIAS_LEADER_BOSS.attacks; // barrage 0–24, charge 6–20, slam 0–5
        assert_eq!(pick_attack(a, 3.0, 0), Some(0));
        assert_eq!(pick_attack(a, 3.0, 1), Some(2), "charge skipped up close");
        assert_eq!(pick_attack(a, 10.0, 1), Some(1));
        assert_eq!(pick_attack(a, 10.0, 2), Some(0), "wraps around");
        assert_eq!(pick_attack(a, 30.0, 0), None);
        assert_eq!(pick_attack(&[], 1.0, 0), None);
    }

    #[test]
    fn slam_cycle_waits_winds_up_lands_recovers() {
        let attacks = GNOLL_CHIEF_BOSS.attacks; // slam 0–6 (1.2 s), charge 7–18
        let mut s = AttackState::new(1.0);
        assert_eq!(s.step(0.5, attacks, 3.0), AttackEvent::None);
        assert!(!s.busy());
        assert_eq!(s.step(0.5, attacks, 3.0), AttackEvent::Begin(0));
        assert!(s.busy() && s.current() == Some(0));
        assert_eq!(s.step(0.6, attacks, 3.0), AttackEvent::None);
        assert!((s.windup_progress(attacks) - 0.5).abs() < 1e-4);
        assert_eq!(s.step(0.6, attacks, 3.0), AttackEvent::Land(0));
        assert_eq!(
            s.phase,
            AttackPhase::Waiting {
                left: attacks[0].recover
            }
        );
        // Out of every range: it keeps waiting, ready.
        assert_eq!(s.step(10.0, attacks, 40.0), AttackEvent::None);
        assert_eq!(s.phase, AttackPhase::Waiting { left: 0.0 });
        // The rotation moved on: at mid range the charge comes next.
        assert_eq!(s.step(0.0, attacks, 10.0), AttackEvent::Begin(1));
    }

    #[test]
    fn charge_cycle_has_an_active_part() {
        let attacks = PACK_LEADER_BOSS.attacks;
        let mut s = AttackState::new(0.0);
        assert_eq!(s.step(0.016, attacks, 10.0), AttackEvent::Begin(0));
        let mut t = 0.0;
        let mut landed = None;
        let mut ended = None;
        while t < 5.0 {
            t += 0.01;
            match s.step(0.01, attacks, 10.0) {
                AttackEvent::Land(_) => landed = Some(t),
                AttackEvent::End(_) => {
                    ended = Some(t);
                    break;
                }
                _ => {}
            }
        }
        let (landed, ended) = (landed.unwrap(), ended.unwrap());
        assert!((landed - attacks[0].windup).abs() < 0.03);
        assert!((ended - landed - active_secs(&attacks[0])).abs() < 0.03);
        assert!(!s.busy());
    }

    #[test]
    fn circles_and_charges_hit_what_they_should() {
        let c = Vec3::new(0.0, 5.0, 0.0);
        assert!(in_circle(c, 3.0, Vec3::new(3.4, 0.0, 0.0)), "height ignored");
        assert!(!in_circle(c, 3.0, Vec3::new(3.6, 0.0, 0.0)));
        let (dir, end) = charge_path(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0), 20.0, Vec3::X);
        assert_eq!(dir, Vec3::Z);
        assert_eq!(end, Vec3::new(0.0, 0.0, 20.0));
        let (dir, _) = charge_path(Vec3::ZERO, Vec3::ZERO, 20.0, Vec3::X);
        assert_eq!(dir, Vec3::X);
        assert!(charge_hits(Vec3::ZERO, 1.0, 2.4, Vec3::new(1.6, 0.0, 0.0)));
        assert!(!charge_hits(Vec3::ZERO, 1.0, 2.4, Vec3::new(1.8, 0.0, 0.0)));
    }

    #[test]
    fn barrage_spreads_around_the_target() {
        let c = Vec3::new(10.0, 0.0, -4.0);
        let spots = barrage_spots(c, 5, 6.0, 0.3);
        assert_eq!(spots.len(), 5);
        assert_eq!(spots[0], c);
        for s in &spots[1..] {
            assert!((flat_dist(*s, c) - 6.0).abs() < 1e-3);
        }
        assert_eq!(barrage_spots(c, 1, 6.0, 0.0), vec![c]);
        assert_eq!(barrage_spots(c, 0, 6.0, 0.0), vec![c]);
    }

    #[test]
    fn directions_follow_the_camera() {
        // Camera at yaw 0 looks along -Z.
        assert_eq!(screen_direction(Vec3::NEG_Z, 0.0), "önde");
        assert_eq!(screen_direction(Vec3::X, 0.0), "sağda");
        assert_eq!(screen_direction(Vec3::Z, 0.0), "arkada");
        assert_eq!(screen_direction(Vec3::new(-1.0, 0.0, -1.0), 0.0), "sol önde");
        // Turned a quarter to the left, what was on the left is now ahead.
        let quarter = std::f32::consts::FRAC_PI_2;
        assert_eq!(screen_direction(Vec3::NEG_X, quarter), "önde");
    }

    #[test]
    fn summary_has_the_essentials() {
        let s = MapSummary {
            time: 655.0,
            level: 19,
            kills: 812,
            bosses: vec!["Gnoll Şefi", "Gri Pençe"],
            skills: vec!["Frostbolt III".into()],
        };
        let l = summary_lines(&s).join("\n");
        assert!(l.contains("10:55") && l.contains("19") && l.contains("812"), "{l}");
        assert!(l.contains("Gnoll Şefi, Gri Pençe") && l.contains("Frostbolt III"));
        let empty = MapSummary {
            bosses: vec![],
            skills: vec![],
            ..s
        };
        assert!(summary_lines(&empty).join("\n").contains("yok"));
    }
}
