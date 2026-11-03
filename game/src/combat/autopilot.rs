//! `RNS_AUTOPLAY=1`: the game plays itself, for balance runs. The hero runs away from what is
//! close and toward loot and XP orbs, picks cards by a fixed priority, and every 15 s of run time a row
//! goes to `rnsurvivors-denge.csv` (or `RNS_BALANCE_LOG`). `RNS_SPEEDUP=2` runs the clock faster.
//! It drives the game through its own keys, so it plays by the same rules as a player.

use std::io::Write as _;

use bevy::input::InputSystems;
use bevy::prelude::*;

use super::cards::CardScreen;
use super::enemies::{Enemy, EnemyBolt};
use super::hero::HeroHealth;
use super::portal::MapCleared;
use super::rules::Card;
use super::xp::Orb;
use super::{HeroDied, Run};
use crate::game::{FollowCam, Hero};
use crate::motion;

pub const AUTOPLAY_VAR: &str = "RNS_AUTOPLAY";
pub const LOG_VAR: &str = "RNS_BALANCE_LOG";
pub const SPEEDUP_VAR: &str = "RNS_SPEEDUP";

/// Seconds of run time between log rows.
const SAMPLE_EVERY: f32 = 15.0;

pub struct AutopilotPlugin;

impl Plugin for AutopilotPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var(AUTOPLAY_VAR).as_deref() != Ok("1") {
            return;
        }
        let path = std::env::var(LOG_VAR).unwrap_or_else(|_| "rnsurvivors-denge.csv".to_string());
        let speedup = std::env::var(SPEEDUP_VAR)
            .ok()
            .and_then(|v| crate::config::parse_positive(&v))
            .unwrap_or(1.0)
            .min(8.0);
        warn!(
            "rnsurvivors: {AUTOPLAY_VAR}=1 — the game plays itself, log {path}, speed ×{speedup}"
        );
        let fresh = !std::path::Path::new(&path).exists();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
        if fresh {
            if let Some(f) = file.as_mut() {
                let _ = writeln!(f, "{}", LOG_HEADER);
            }
        }
        app.insert_resource(Autopilot {
            file,
            run: 1,
            next_sample: 0.0,
            speedup,
        })
        .add_systems(Startup, set_speed)
        .add_systems(PreUpdate, drive.after(InputSystems))
        .add_systems(
            Update,
            // The death is seen the frame it happens, before the run resets.
            log_run
                .in_set(super::CombatSet::Reset)
                .after(super::hero::check_death)
                .before(super::hero::reset_run),
        );
    }
}

pub const LOG_HEADER: &str =
    "kosu,olay,sure_sn,level,can,can_max,oldurulen,dusman_sayisi,skilller,pasifler";

#[derive(Resource)]
struct Autopilot {
    file: Option<std::fs::File>,
    run: u32,
    next_sample: f32,
    speedup: f32,
}

fn set_speed(pilot: Res<Autopilot>, mut clock: ResMut<Time<Virtual>>) {
    clock.set_relative_speed(pilot.speedup);
    // A slow software-rendered frame must not become a huge step.
    clock.set_max_delta(std::time::Duration::from_millis(100));
}

/// Where to run: away from close enemies and bolts (closer = stronger), toward the nearest orb in
/// reach, with a slight sideways drift so the hero circles instead of backing into a corner.
/// Loot (bags, the chest) pulls from farther and harder than orbs.
pub fn steer(hero: Vec3, threats: &[Vec3], orbs: &[Vec3], loot: &[Vec3]) -> Vec3 {
    let mut dir = Vec3::ZERO;
    for t in threats {
        let d = (hero - *t).with_y(0.0);
        let len = d.length();
        if len < 12.0 && len > 1e-3 {
            let push = d / len * (1.0 / (len * len).max(0.5));
            dir += push;
            // Sideways, always the same way: a circle.
            dir += Vec3::new(-push.z, 0.0, push.x) * 0.6;
        }
    }
    let danger = dir.length();
    if let Some(o) = orbs
        .iter()
        .filter(|o| (**o - hero).with_y(0.0).length() < 14.0)
        .min_by(|a, b| {
            (**a - hero)
                .length_squared()
                .total_cmp(&(**b - hero).length_squared())
        })
    {
        // Orbs pull less the more danger there is.
        let to = (*o - hero).with_y(0.0).normalize_or_zero();
        dir += to * (0.3 / (1.0 + 4.0 * danger));
    }
    if let Some(l) = loot
        .iter()
        .filter(|l| (**l - hero).with_y(0.0).length() < 45.0)
        .min_by(|a, b| {
            (**a - hero)
                .length_squared()
                .total_cmp(&(**b - hero).length_squared())
        })
    {
        let to = (*l - hero).with_y(0.0).normalize_or_zero();
        dir += to * (0.8 / (1.0 + 2.0 * danger));
    }
    dir.with_y(0.0).normalize_or_zero()
}

/// The WASD combination (x right, y forward) that moves closest to `want` under camera `yaw`.
pub fn keys_for(want: Vec3, yaw: f32) -> Vec2 {
    if want == Vec3::ZERO {
        return Vec2::ZERO;
    }
    let mut best = (Vec2::ZERO, f32::MIN);
    for x in -1..=1 {
        for y in -1..=1 {
            let input = Vec2::new(x as f32, y as f32);
            if input == Vec2::ZERO {
                continue;
            }
            let score = motion::move_dir(input, yaw).dot(want);
            if score > best.1 {
                best = (input, score);
            }
        }
    }
    best.0
}

/// The card the autopilot takes: an evolution, else a skill upgrade, a new skill, a passive
/// upgrade, a new passive, a heal — the first of each kind on screen.
pub fn auto_pick(cards: &[Card]) -> usize {
    let rank = |c: &Card| match c {
        Card::Evolve(..) => 0,
        Card::Upgrade(..) => 1,
        Card::New(_) => 2,
        Card::UpgradePassive(..) => 3,
        Card::NewPassive(_) => 4,
        Card::Heal => 5,
    };
    cards
        .iter()
        .enumerate()
        .min_by_key(|(i, c)| (rank(c), *i))
        .map_or(0, |(i, _)| i)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn drive(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    cards: Res<CardScreen>,
    loot_screen: Res<super::gear_ui::LootScreen>,
    cams: Query<&FollowCam>,
    heroes: Query<&Transform, With<Hero>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Hero>)>,
    bolts: Query<&Transform, (With<EnemyBolt>, Without<Hero>)>,
    orbs: Query<&Transform, (With<Orb>, Without<Hero>)>,
    loot: Query<
        &Transform,
        (
            Or<(With<super::gear_ui::LootBag>, With<super::portal::Chest>)>,
            Without<Hero>,
        ),
    >,
    mut picked: Local<bool>,
    health: Res<super::hero::HeroHealth>,
) {
    for k in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD] {
        keys.release(k);
    }
    for k in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Enter,
        KeyCode::KeyZ,
        KeyCode::KeyX,
        KeyCode::KeyC,
    ] {
        keys.release(k);
    }
    // Take the chest's loot.
    if loot_screen.is_open() {
        keys.press(KeyCode::Enter);
        return;
    }
    if let Some(on_screen) = cards.cards() {
        // One key press per screen; the next screen gets its own.
        if !*picked {
            let key = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3][auto_pick(on_screen)];
            keys.press(key);
            *picked = true;
        }
        return;
    }
    *picked = false;
    let Ok(hero) = heroes.single() else {
        return;
    };
    let threats: Vec<Vec3> = enemies
        .iter()
        .chain(bolts.iter())
        .map(|t| t.translation)
        .collect();
    let loot_spots: Vec<Vec3> = loot.iter().map(|t| t.translation).collect();
    let orb_spots: Vec<Vec3> = orbs.iter().map(|t| t.translation).collect();
    let want = steer(hero.translation, &threats, &orb_spots, &loot_spots);
    let yaw = cams.single().map(|c| c.yaw).unwrap_or(0.0);
    // Cooldowns: the shield when hurt, the damage when crowded, Bloodlust once things get busy.
    let near = threats
        .iter()
        .filter(|t| super::rules::flat_dist(**t, hero.translation) < 15.0)
        .count();
    if health.fraction() < 0.5 {
        keys.press(KeyCode::KeyX);
    }
    if near >= 12 {
        keys.press(KeyCode::KeyC);
    }
    if near >= 20 {
        keys.press(KeyCode::KeyZ);
    }
    let input = keys_for(want, yaw);
    if input.y > 0.0 {
        keys.press(KeyCode::KeyW);
    }
    if input.y < 0.0 {
        keys.press(KeyCode::KeyS);
    }
    if input.x > 0.0 {
        keys.press(KeyCode::KeyD);
    }
    if input.x < 0.0 {
        keys.press(KeyCode::KeyA);
    }
}

fn compact_skills(run: &Run) -> String {
    run.loadout
        .0
        .iter()
        .map(|(s, l)| format!("{}{l}", s.key()))
        .collect::<Vec<_>>()
        .join("+")
}

fn compact_passives(run: &Run) -> String {
    run.passives
        .0
        .iter()
        .map(|(p, l)| format!("{}{l}", p.key()))
        .collect::<Vec<_>>()
        .join("+")
}

/// A log row.
pub fn row(run_no: u32, event: &str, run: &Run, hp: (f32, f32), enemies: usize) -> String {
    format!(
        "{run_no},{event},{:.0},{},{:.0},{:.0},{},{enemies},{},{}",
        run.time,
        run.level,
        hp.0,
        hp.1,
        run.kills,
        compact_skills(run),
        compact_passives(run)
    )
}

#[allow(clippy::too_many_arguments)]
fn log_run(
    mut pilot: ResMut<Autopilot>,
    run: Res<Run>,
    health: Res<HeroHealth>,
    enemies: Query<(), With<Enemy>>,
    mut died: MessageReader<HeroDied>,
    mut cleared: MessageReader<MapCleared>,
) {
    let alive = enemies.iter().count();
    let hp = (health.current, health.max);
    let mut lines = Vec::new();
    if run.time >= pilot.next_sample {
        lines.push(row(pilot.run, "ornek", &run, hp, alive));
        pilot.next_sample = run.time + SAMPLE_EVERY;
    }
    let end = if died.read().count() > 0 {
        Some("olum")
    } else if cleared.read().count() > 0 {
        Some("portal")
    } else {
        None
    };
    if let Some(event) = end {
        lines.push(row(pilot.run, event, &run, hp, alive));
        info!(
            "rnsurvivors: autopilot run {} ended: {}",
            pilot.run,
            lines.last().unwrap()
        );
        pilot.run += 1;
        pilot.next_sample = 0.0;
    }
    if let Some(f) = pilot.file.as_mut() {
        for l in lines {
            let _ = writeln!(f, "{l}");
        }
        let _ = f.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::SkillId;

    #[test]
    fn runs_from_danger_and_to_orbs() {
        let hero = Vec3::ZERO;
        let away = steer(hero, &[Vec3::X * 3.0], &[], &[]);
        assert!(away.x < 0.0, "{away:?}");
        let to_orb = steer(hero, &[], &[Vec3::Z * 5.0], &[]);
        assert!(to_orb.z > 0.9);
        assert_eq!(steer(hero, &[], &[], &[]), Vec3::ZERO);
        // Loot pulls from far away, where orbs do not.
        let far = Vec3::X * 30.0;
        assert_eq!(steer(hero, &[], &[far], &[]), Vec3::ZERO);
        assert!(steer(hero, &[], &[], &[far]).x > 0.9);
        // Danger beats greed.
        let both = steer(hero, &[Vec3::Z * 2.0], &[Vec3::Z * 5.0], &[]);
        assert!(both.z < 0.0);
    }

    #[test]
    fn keys_point_the_right_way() {
        for yaw in [0.0, 0.7, -2.0] {
            for want in [Vec3::X, Vec3::NEG_Z, Vec3::new(1.0, 0.0, 1.0).normalize()] {
                let k = keys_for(want, yaw);
                assert!(
                    motion::move_dir(k, yaw).dot(want) > 0.9,
                    "{yaw} {want:?} {k:?}"
                );
            }
        }
        assert_eq!(keys_for(Vec3::ZERO, 0.0), Vec2::ZERO);
    }

    #[test]
    fn picks_evolutions_then_upgrades() {
        let fb = SkillId::Frostbolt;
        let cards = [
            Card::NewPassive(crate::content::passives::PassiveId::Power),
            Card::Upgrade(fb, 2),
            Card::Evolve(fb, SkillId::WinterBolts),
        ];
        assert_eq!(auto_pick(&cards), 2);
        assert_eq!(auto_pick(&cards[..2]), 1);
        assert_eq!(auto_pick(&[Card::Heal]), 0);
    }

    #[test]
    fn rows_have_every_column() {
        let run = Run {
            time: 61.0,
            level: 4,
            xp: 0,
            loadout: crate::combat::rules::Loadout::starting(),
            passives: Default::default(),
            kills: 30,
            pending_cards: 0,
            xp_total: 0,
            revive_used: false,
            start_applied: false,
            abilities: Default::default(),
            packs_cleared: 0,
        };
        let r = row(2, "ornek", &run, (80.0, 100.0), 12);
        assert_eq!(r, "2,ornek,61,4,80,100,30,12,frostbolt1,");
        assert_eq!(r.split(',').count(), LOG_HEADER.split(',').count());
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(drive).initialize(&mut world);
        IntoSystem::into_system(log_run).initialize(&mut world);
    }
}
