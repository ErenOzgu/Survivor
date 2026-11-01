//! The first map's enemies (Elwynn, around Goldshire). The elite and the boss are data for phase 3;
//! phase 2 spawns only the three normal kinds.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Kobold,
    Wolf,
    Spider,
    /// Elite at a fixed spot (phase 3).
    GnollChief,
    /// The map boss, arrives on a timer (phase 3).
    DefiasBoss,
}

impl EnemyKind {
    /// The kinds the wave spawner draws from.
    pub const NORMAL: [EnemyKind; 3] = [EnemyKind::Kobold, EnemyKind::Wolf, EnemyKind::Spider];

    pub fn def(self) -> &'static EnemyDef {
        match self {
            EnemyKind::Kobold => &KOBOLD,
            EnemyKind::Wolf => &WOLF,
            EnemyKind::Spider => &SPIDER,
            EnemyKind::GnollChief => &GNOLL_CHIEF,
            EnemyKind::DefiasBoss => &DEFIAS_BOSS,
        }
    }
}

#[derive(Debug)]
pub struct EnemyDef {
    /// Turkish display name.
    pub name: &'static str,
    pub health: f32,
    /// Yards per second (the hero runs at 7).
    pub speed: f32,
    /// Damage per second while touching the hero.
    pub contact_dps: f32,
    /// XP its orb is worth.
    pub xp: u32,
    /// Body radius in yards, for hits and crowding.
    pub radius: f32,
    /// Placeholder look: colour (sRGB) and size multiplier.
    pub color: [f32; 3],
    pub scale: f32,
}

/// Slow and many; the bulk of the early waves.
pub static KOBOLD: EnemyDef = EnemyDef {
    name: "Kobold",
    health: 16.0,
    speed: 3.4,
    contact_dps: 6.0,
    xp: 1,
    radius: 0.5,
    color: [0.75, 0.55, 0.3],
    scale: 0.85,
};

/// Fragile but nearly as fast as the hero; comes in packs.
pub static WOLF: EnemyDef = EnemyDef {
    name: "Kurt",
    health: 12.0,
    speed: 6.0,
    contact_dps: 7.0,
    xp: 1,
    radius: 0.6,
    color: [0.55, 0.55, 0.6],
    scale: 0.9,
};

/// Tough and slow; hurts on contact.
pub static SPIDER: EnemyDef = EnemyDef {
    name: "Örümcek",
    health: 36.0,
    speed: 2.9,
    contact_dps: 11.0,
    xp: 3,
    radius: 0.75,
    color: [0.3, 0.15, 0.35],
    scale: 1.1,
};

pub static GNOLL_CHIEF: EnemyDef = EnemyDef {
    name: "Gnoll Şefi",
    health: 650.0,
    speed: 4.2,
    contact_dps: 20.0,
    xp: 40,
    radius: 1.2,
    color: [0.85, 0.65, 0.2],
    scale: 1.9,
};

pub static DEFIAS_BOSS: EnemyDef = EnemyDef {
    name: "Defias Lideri",
    health: 2600.0,
    speed: 4.6,
    contact_dps: 28.0,
    xp: 150,
    radius: 1.3,
    color: [0.7, 0.1, 0.1],
    scale: 2.1,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_enemies_are_slower_than_the_hero() {
        for k in EnemyKind::NORMAL {
            assert!(k.def().speed < crate::motion::HERO_SPEED, "{k:?}");
        }
    }

    #[test]
    fn elites_outclass_normals() {
        let strongest = EnemyKind::NORMAL
            .iter()
            .map(|k| k.def().health)
            .fold(0.0, f32::max);
        assert!(GNOLL_CHIEF.health > 10.0 * strongest);
        assert!(DEFIAS_BOSS.health > GNOLL_CHIEF.health);
    }
}
