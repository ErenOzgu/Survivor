//! Rnsurvivors: a survivors-style game in the WoW 1.12 world, on the benilla engine.
//!
//! - [`game`]: the Bevy plugin (hero, movement, follow camera).
//! - [`motion`], [`config`]: pure movement/camera math and env settings.
//! - [`run`]: pure rules of one run (XP, skills, combat, spawning), not yet wired into the ECS.

pub mod config;
pub mod game;
pub mod motion;
pub mod run;
