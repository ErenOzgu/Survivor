//! Game content as data: the Mage's skills, the first map's enemies and its wave schedule.
//!
//! Every number here is Rnsurvivors' own design (CLAUDE.md §2), tuned for a ~10 minute first map.
//! The systems in `crate::combat` read these tables; nothing in here touches the ECS.

pub mod bosses;
pub mod enemies;
pub mod skills;
pub mod waves;

pub use enemies::EnemyKind;
pub use skills::{SkillId, SkillLevel, MAX_SKILLS};
