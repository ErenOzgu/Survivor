//! Phase 2, "Savaş": waves of enemies, auto-cast skills, XP orbs, level-up cards and the HUD.
//!
//! The rules live in [`rules`] as pure functions; the modules here are thin ECS systems on top.
//! Everything waits until the hero stands on streamed ground, and while a level-up card screen
//! is open the virtual clock is paused, so every system below sees `dt = 0`.

pub mod art;
pub mod cards;
pub mod enemies;
pub mod flat;
pub mod hero;
pub mod hud;
pub mod plates;
pub mod rules;
pub mod skills;
pub mod visuals;
pub mod xp;

use bevy::prelude::*;

use benilla_world::schedule::WorldStage;

use crate::content::SkillId;
use crate::game::Hero;
use rules::{Loadout, Rng};

/// Debug env vars, read once at start-up (see README).
pub const LEVEL_VAR: &str = "RNS_LEVEL";
pub const SKILLS_VAR: &str = "RNS_SKILLS";
pub const GOD_VAR: &str = "RNS_GOD";

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        let debug = DebugStart::from_env();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        app.insert_resource(Run::new(&debug))
            .insert_resource(debug)
            .insert_resource(CombatRng(Rng::new(seed)))
            .add_message::<HeroDied>()
            .configure_sets(
                Update,
                (
                    CombatSet::Spawn,
                    CombatSet::Act,
                    CombatSet::Resolve,
                    CombatSet::Reset,
                    CombatSet::Ui,
                )
                    .chain()
                    .after(WorldStage::Input)
                    .before(WorldStage::Stream),
            )
            .add_systems(
                Update,
                tick_run_clock
                    .in_set(CombatSet::Spawn)
                    .run_if(hero_on_ground),
            )
            .add_plugins((
                visuals::VisualsPlugin,
                hero::HeroPlugin,
                enemies::EnemiesPlugin,
                skills::SkillsPlugin,
                xp::XpPlugin,
                cards::CardsPlugin,
                hud::HudPlugin,
                flat::FlatPlugin,
                art::ArtPlugin,
                plates::PlatesPlugin,
            ));
    }
}

/// The order of the combat frame.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombatSet {
    /// The clock and new enemies.
    Spawn,
    /// Movement, casting, projectiles, contact damage.
    Act,
    /// Enemy deaths, orbs, level-ups.
    Resolve,
    /// The hero's death and the run reset (after `Resolve`, so it also clears this frame's orbs).
    Reset,
    /// Card screen and HUD.
    Ui,
}

/// The current run: time, level, XP and skills. Reset when the hero dies.
#[derive(Resource, Debug, Clone)]
pub struct Run {
    /// Seconds of play (the virtual clock; stops on the card screen).
    pub time: f32,
    pub level: u32,
    pub xp: u32,
    pub loadout: Loadout,
    pub kills: u32,
    /// Level-ups whose card has not been picked yet.
    pub pending_cards: u32,
}

impl Run {
    pub fn new(debug: &DebugStart) -> Self {
        Self {
            time: 0.0,
            level: debug.level,
            xp: 0,
            loadout: debug.loadout.clone(),
            kills: 0,
            pending_cards: debug.level.saturating_sub(1),
        }
    }
}

/// How a run starts: normally level 1 with Frostbolt; the debug env vars can change it.
#[derive(Resource, Debug, Clone)]
pub struct DebugStart {
    pub level: u32,
    pub loadout: Loadout,
    /// The hero takes no damage.
    pub god: bool,
}

impl DebugStart {
    pub fn from_env() -> Self {
        let level = std::env::var(LEVEL_VAR)
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|l| (1..=100).contains(l))
            .unwrap_or(1);
        let loadout = std::env::var(SKILLS_VAR)
            .ok()
            .map(|v| rules::parse_skills(&v))
            .filter(|l| !l.0.is_empty())
            .unwrap_or_else(Loadout::starting);
        let god = std::env::var(GOD_VAR).as_deref() == Ok("1");
        if level > 1 || god || loadout != Loadout::starting() {
            info!("rnsurvivors: debug start — level {level}, skills {loadout:?}, god {god}");
        }
        Self {
            level,
            loadout,
            god,
        }
    }
}

/// The combat's random source.
#[derive(Resource)]
pub struct CombatRng(pub Rng);

/// Sent when the hero's health reaches zero; the run resets on it.
#[derive(Message, Debug, Clone, Copy)]
pub struct HeroDied;

/// Run condition: the hero exists and stands on streamed terrain.
pub fn hero_on_ground(heroes: Query<&Hero>) -> bool {
    heroes.single().is_ok_and(|h| h.grounded)
}

fn tick_run_clock(time: Res<Time>, mut run: ResMut<Run>) {
    run.time += time.delta_secs();
}

/// The Turkish label for a skill at a level, as the HUD and cards show it.
pub fn skill_label(id: SkillId, level: u8) -> String {
    format!("{} {}", id.def().name, roman(level))
}

fn roman(n: u8) -> &'static str {
    match n {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        6 => "VI",
        _ => "?",
    }
}
