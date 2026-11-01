//! The pure rules of one run: no ECS, no rendering, every number this project's own design
//! (CLAUDE.md §2). The Bevy systems that drive them come next (phase 2 wiring).

pub mod combat;
pub mod director;
pub mod progression;
pub mod rng;
pub mod skills;
