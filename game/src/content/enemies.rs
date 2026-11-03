//! The first map's enemies (Elwynn, around Goldshire): five normal kinds for the waves, and the
//! bodies the elite and the bosses borrow.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Kobold,
    Wolf,
    Spider,
    /// Keeps its distance and throws slow fire bolts (dodgeable).
    Geomancer,
    /// Runs at the hero, lights a short fuse, and blows up.
    Sapper,
    /// Elite at a fixed spot (phase 3).
    GnollChief,
    /// The map boss, arrives on a timer (phase 3).
    DefiasBoss,
}

impl EnemyKind {
    /// The kinds the wave spawner draws from.
    pub const NORMAL: [EnemyKind; 5] = [
        EnemyKind::Kobold,
        EnemyKind::Wolf,
        EnemyKind::Spider,
        EnemyKind::Geomancer,
        EnemyKind::Sapper,
    ];

    pub fn def(self) -> &'static EnemyDef {
        match self {
            EnemyKind::Kobold => &KOBOLD,
            EnemyKind::Wolf => &WOLF,
            EnemyKind::Spider => &SPIDER,
            EnemyKind::Geomancer => &GEOMANCER,
            EnemyKind::Sapper => &SAPPER,
            EnemyKind::GnollChief => &GNOLL_CHIEF,
            EnemyKind::DefiasBoss => &DEFIAS_BOSS,
        }
    }

    /// How the kind fights (elites and bosses add their own special attacks on top).
    pub fn behavior(self) -> Behavior {
        match self {
            EnemyKind::Geomancer => Behavior::Ranged(GEOMANCER_BOLT),
            EnemyKind::Sapper => Behavior::Exploder(SAPPER_BLAST),
            _ => Behavior::Melee,
        }
    }
}

/// How an enemy attacks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Behavior {
    /// Runs into the hero; contact damage.
    Melee,
    /// Stops at a distance and shoots.
    Ranged(RangedDef),
    /// Runs in, lights a fuse, explodes (and dies).
    Exploder(ExplodeDef),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangedDef {
    /// The distance it keeps from the hero.
    pub keep: f32,
    /// It shoots when the hero is this close.
    pub range: f32,
    /// Seconds between shots.
    pub cooldown: f32,
    /// Damage per bolt (times the wave's damage multiplier).
    pub damage: f32,
    /// Bolt speed, yards per second (slower than the hero: it can be dodged).
    pub speed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExplodeDef {
    /// It lights the fuse this close to the hero.
    pub trigger: f32,
    /// Seconds from the fuse to the blast; the warning circle grows meanwhile.
    pub fuse: f32,
    pub radius: f32,
    /// Blast damage (times the wave's damage multiplier).
    pub damage: f32,
}

pub const GEOMANCER_BOLT: RangedDef = RangedDef {
    keep: 10.0,
    range: 14.0,
    cooldown: 2.6,
    damage: 9.0,
    speed: 6.5,
};

pub const SAPPER_BLAST: ExplodeDef = ExplodeDef {
    trigger: 2.2,
    fuse: 0.9,
    radius: 3.0,
    damage: 22.0,
};

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

/// A kobold caster at the back of the pack.
pub static GEOMANCER: EnemyDef = EnemyDef {
    name: "Kobold Falcı",
    health: 22.0,
    speed: 3.0,
    contact_dps: 4.0,
    xp: 2,
    radius: 0.5,
    color: [0.45, 0.35, 0.85],
    scale: 0.85,
};

/// A kobold with a lit candle and too much powder. Killing it first gives its XP; letting it
/// blow up gives none.
pub static SAPPER: EnemyDef = EnemyDef {
    name: "Fitilci Kobold",
    health: 10.0,
    speed: 5.2,
    contact_dps: 0.0,
    xp: 2,
    radius: 0.5,
    color: [0.95, 0.35, 0.15],
    scale: 0.8,
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
    fn special_kinds_are_fair() {
        let Behavior::Ranged(r) = EnemyKind::Geomancer.behavior() else {
            panic!("geomancer shoots");
        };
        assert!(r.speed < crate::motion::HERO_SPEED, "bolts can be outrun");
        assert!(r.keep < r.range);
        let Behavior::Exploder(x) = EnemyKind::Sapper.behavior() else {
            panic!("sapper explodes");
        };
        assert!(x.fuse >= 0.6, "time to step away");
        assert!(
            crate::motion::HERO_SPEED * x.fuse > x.radius - x.trigger,
            "a running hero can clear the blast"
        );
        assert_eq!(EnemyKind::Kobold.behavior(), Behavior::Melee);
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
