//! The Mage's eight skills (DESIGN.md "İlk sınıf: Mage"), each with its levels, and their three
//! evolutions: two skills at their last level fuse one of them into a stronger form.
//!
//! A level lists the skill's full stats at that level, not a delta, so the table reads as what the
//! player gets. Fields a skill does not use stay at their [`SkillLevel::BASE`] value.

/// The most skills a hero carries at once.
pub const MAX_SKILLS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkillId {
    Frostbolt,
    Fireball,
    ArcaneMissiles,
    FrostNova,
    Blizzard,
    Flamestrike,
    ArcaneExplosion,
    Blink,
    /// Evolution of Frostbolt with Frost Nova.
    WinterBolts,
    /// Evolution of Fireball with Flamestrike.
    Pyroblast,
    /// Evolution of Arcane Missiles with Arcane Explosion.
    ArcaneStorm,
}

/// The evolutions: `(skill that evolves, partner, what it becomes)`. Both at their last level
/// offer the evolution card; the first skill is replaced, the partner stays.
pub const EVOLUTIONS: [(SkillId, SkillId, SkillId); 3] = [
    (SkillId::Frostbolt, SkillId::FrostNova, SkillId::WinterBolts),
    (SkillId::Fireball, SkillId::Flamestrike, SkillId::Pyroblast),
    (
        SkillId::ArcaneMissiles,
        SkillId::ArcaneExplosion,
        SkillId::ArcaneStorm,
    ),
];

impl SkillId {
    pub const ALL: [SkillId; 8] = [
        SkillId::Frostbolt,
        SkillId::Fireball,
        SkillId::ArcaneMissiles,
        SkillId::FrostNova,
        SkillId::Blizzard,
        SkillId::Flamestrike,
        SkillId::ArcaneExplosion,
        SkillId::Blink,
    ];

    /// The evolved forms, never offered as a new skill.
    pub const EVOLVED: [SkillId; 3] = [
        SkillId::WinterBolts,
        SkillId::Pyroblast,
        SkillId::ArcaneStorm,
    ];

    /// Every skill, base and evolved.
    pub fn every() -> impl Iterator<Item = SkillId> {
        Self::ALL.into_iter().chain(Self::EVOLVED)
    }

    pub fn is_evolved(self) -> bool {
        Self::EVOLVED.contains(&self)
    }

    /// The evolution this base skill can become, with its partner.
    pub fn evolution(self) -> Option<(SkillId, SkillId)> {
        EVOLUTIONS
            .iter()
            .find(|(b, _, _)| *b == self)
            .map(|(_, p, into)| (*p, *into))
    }

    pub fn def(self) -> &'static SkillDef {
        match self {
            SkillId::Frostbolt => &FROSTBOLT,
            SkillId::Fireball => &FIREBALL,
            SkillId::ArcaneMissiles => &ARCANE_MISSILES,
            SkillId::FrostNova => &FROST_NOVA,
            SkillId::Blizzard => &BLIZZARD,
            SkillId::Flamestrike => &FLAMESTRIKE,
            SkillId::ArcaneExplosion => &ARCANE_EXPLOSION,
            SkillId::Blink => &BLINK,
            SkillId::WinterBolts => &WINTER_BOLTS,
            SkillId::Pyroblast => &PYROBLAST,
            SkillId::ArcaneStorm => &ARCANE_STORM,
        }
    }

    /// The highest level of this skill.
    pub fn max_level(self) -> u8 {
        self.def().levels.len() as u8
    }

    /// The stats at `level` (1-based), clamped into the table.
    pub fn level(self, level: u8) -> &'static SkillLevel {
        let levels = self.def().levels;
        let i = (level.max(1) as usize - 1).min(levels.len() - 1);
        &levels[i]
    }

    /// A lowercase ASCII key, for the debug env var (`RNS_SKILLS=frostbolt:3,blink`).
    pub fn key(self) -> &'static str {
        match self {
            SkillId::Frostbolt => "frostbolt",
            SkillId::Fireball => "fireball",
            SkillId::ArcaneMissiles => "missiles",
            SkillId::FrostNova => "nova",
            SkillId::Blizzard => "blizzard",
            SkillId::Flamestrike => "flamestrike",
            SkillId::ArcaneExplosion => "explosion",
            SkillId::Blink => "blink",
            SkillId::WinterBolts => "winterbolts",
            SkillId::Pyroblast => "pyroblast",
            SkillId::ArcaneStorm => "arcanestorm",
        }
    }

    pub fn from_key(key: &str) -> Option<SkillId> {
        let key = key.trim().to_ascii_lowercase();
        SkillId::every().find(|s| s.key() == key)
    }
}

/// One skill: its name, what it does, and its levels.
#[derive(Debug)]
pub struct SkillDef {
    /// The name shown: the WoW spell name for a base skill (kept in English, as in the game),
    /// our own for an evolution.
    pub name: &'static str,
    /// The WoW spell whose icon (and later effects) this skill borrows, by name in `Spell.dbc`.
    pub icon_spell: &'static str,
    /// What the skill does, in Turkish, for the card of a new skill.
    pub desc: &'static str,
    /// Card text for reaching each level, in Turkish; `[0]` is level 1 (same as `desc`).
    pub level_text: &'static [&'static str],
    pub levels: &'static [SkillLevel],
    /// Placeholder colour (sRGB) for the skill's projectiles and areas, and its HUD tag.
    pub color: [f32; 3],
}

/// A skill's stats at one level. Units: yards, seconds, hit points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillLevel {
    /// Damage per hit (projectiles, pulses) or per tick (ground areas).
    pub damage: f32,
    /// Seconds between casts.
    pub cooldown: f32,
    /// Projectiles, missiles or areas per cast.
    pub count: u32,
    /// Area radius: explosion, nova, ground area.
    pub radius: f32,
    /// Extra enemies a projectile passes through after its first hit.
    pub pierce: u32,
    /// Seconds a ground area lasts, or a freeze/slow lasts.
    pub duration: f32,
    /// Fraction of speed taken by a slow (0..1).
    pub slow: f32,
    /// Seconds between a ground area's damage ticks.
    pub tick: f32,
    /// How far the skill reaches: targeting range, or Blink's jump.
    pub range: f32,
    /// Projectile speed, yards per second.
    pub speed: f32,
}

impl SkillLevel {
    pub const BASE: SkillLevel = SkillLevel {
        damage: 0.0,
        cooldown: 1.0,
        count: 1,
        radius: 0.0,
        pierce: 0,
        duration: 0.0,
        slow: 0.0,
        tick: 0.5,
        range: 20.0,
        speed: 22.0,
    };
}

const B: SkillLevel = SkillLevel::BASE;

// Frostbolt: the starting skill. A fast bolt at the nearest enemy that slows it; more bolts, then
// bolts that pass through.
const FB: SkillLevel = SkillLevel {
    damage: 18.0,
    cooldown: 0.95,
    duration: 1.6,
    slow: 0.35,
    range: 18.0,
    speed: 24.0,
    ..B
};
pub static FROSTBOLT: SkillDef = SkillDef {
    name: "Frostbolt",
    icon_spell: "Frostbolt",
    desc: "En yakın düşmana buz oku atar ve onu yavaşlatır.",
    level_text: &[
        "En yakın düşmana buz oku atar ve onu yavaşlatır.",
        "+1 ok, hasar 22.",
        "Oklar 1 düşmanı delip geçer, hasar 26.",
        "+1 ok (toplam 3), bekleme 0,85 sn.",
        "Oklar 3 düşmanı delip geçer, hasar 34, yavaşlatma %50.",
    ],
    levels: &[
        FB,
        SkillLevel {
            damage: 22.0,
            count: 2,
            ..FB
        },
        SkillLevel {
            damage: 26.0,
            count: 2,
            pierce: 1,
            ..FB
        },
        SkillLevel {
            damage: 26.0,
            count: 3,
            pierce: 1,
            cooldown: 0.85,
            ..FB
        },
        SkillLevel {
            damage: 34.0,
            count: 3,
            pierce: 3,
            cooldown: 0.85,
            slow: 0.5,
            ..FB
        },
    ],
    color: [0.45, 0.8, 1.0],
};

// Fireball: slow and heavy; explodes where it lands, the blast grows.
const FI: SkillLevel = SkillLevel {
    damage: 28.0,
    cooldown: 2.6,
    radius: 2.5,
    range: 20.0,
    speed: 13.0,
    ..B
};
pub static FIREBALL: SkillDef = SkillDef {
    name: "Fireball",
    icon_spell: "Fireball",
    desc: "Yavaş ama güçlü bir ateş topu; çarptığı yerde patlar.",
    level_text: &[
        "Yavaş ama güçlü bir ateş topu; çarptığı yerde patlar.",
        "Patlama alanı büyür, hasar 34.",
        "Patlama alanı büyür, bekleme 2,3 sn.",
        "Patlama alanı büyür, hasar 44.",
        "İki ateş topu birden, en büyük patlama, hasar 55.",
    ],
    levels: &[
        FI,
        SkillLevel {
            damage: 34.0,
            radius: 3.2,
            ..FI
        },
        SkillLevel {
            damage: 34.0,
            radius: 4.0,
            cooldown: 2.3,
            ..FI
        },
        SkillLevel {
            damage: 44.0,
            radius: 4.8,
            cooldown: 2.3,
            ..FI
        },
        SkillLevel {
            damage: 55.0,
            radius: 5.8,
            cooldown: 2.3,
            count: 2,
            ..FI
        },
    ],
    color: [1.0, 0.45, 0.1],
};

// Arcane Missiles: small homing missiles at random enemies; more of them.
const AM: SkillLevel = SkillLevel {
    damage: 6.0,
    cooldown: 1.7,
    count: 3,
    range: 16.0,
    speed: 16.0,
    ..B
};
pub static ARCANE_MISSILES: SkillDef = SkillDef {
    name: "Arcane Missiles",
    icon_spell: "Arcane Missiles",
    desc: "Rastgele düşmanlara güdümlü küçük füzeler fırlatır.",
    level_text: &[
        "Rastgele düşmanlara güdümlü küçük füzeler fırlatır.",
        "+1 füze (toplam 4).",
        "+1 füze (toplam 5), hasar 7.",
        "+1 füze (toplam 6), bekleme 1,4 sn.",
        "+2 füze (toplam 8), hasar 8.",
    ],
    levels: &[
        AM,
        SkillLevel { count: 4, ..AM },
        SkillLevel {
            count: 5,
            damage: 7.0,
            ..AM
        },
        SkillLevel {
            count: 6,
            damage: 7.0,
            cooldown: 1.4,
            ..AM
        },
        SkillLevel {
            count: 8,
            damage: 8.0,
            cooldown: 1.4,
            ..AM
        },
    ],
    color: [0.85, 0.4, 1.0],
};

// Frost Nova: freezes everyone around the hero; more often, wider.
const FN: SkillLevel = SkillLevel {
    damage: 4.0,
    cooldown: 9.0,
    radius: 5.0,
    duration: 1.5,
    ..B
};
pub static FROST_NOVA: SkillDef = SkillDef {
    name: "Frost Nova",
    icon_spell: "Frost Nova",
    desc: "Etraftaki bütün düşmanları kısa süre dondurur.",
    level_text: &[
        "Etraftaki bütün düşmanları kısa süre dondurur.",
        "Daha geniş, bekleme 8 sn.",
        "Daha geniş, bekleme 7 sn, donma 2 sn.",
        "Daha geniş, bekleme 6 sn.",
        "En geniş halka, bekleme 5 sn, donma 2,5 sn, hasar 10.",
    ],
    levels: &[
        FN,
        SkillLevel {
            radius: 6.0,
            cooldown: 8.0,
            ..FN
        },
        SkillLevel {
            radius: 7.0,
            cooldown: 7.0,
            duration: 2.0,
            ..FN
        },
        SkillLevel {
            radius: 8.0,
            cooldown: 6.0,
            duration: 2.0,
            ..FN
        },
        SkillLevel {
            radius: 9.5,
            cooldown: 5.0,
            duration: 2.5,
            damage: 10.0,
            ..FN
        },
    ],
    color: [0.7, 0.95, 1.0],
};

// Blizzard: ice rain on a random spot near the hero; damages and slows; lasts longer, grows.
const BZ: SkillLevel = SkillLevel {
    damage: 5.0,
    cooldown: 5.5,
    radius: 3.5,
    duration: 3.0,
    slow: 0.4,
    tick: 0.5,
    range: 11.0,
    ..B
};
pub static BLIZZARD: SkillDef = SkillDef {
    name: "Blizzard",
    icon_spell: "Blizzard",
    desc: "Yakında rastgele bir yere buz yağdırır; içindekileri yavaşlatır.",
    level_text: &[
        "Yakında rastgele bir yere buz yağdırır; içindekileri yavaşlatır.",
        "Süre 4 sn, alan büyür.",
        "Süre 5 sn, alan büyür, hasar 6.",
        "Aynı anda iki buz yağmuru.",
        "Süre 6 sn, en geniş alan, hasar 8.",
    ],
    levels: &[
        BZ,
        SkillLevel {
            duration: 4.0,
            radius: 4.0,
            ..BZ
        },
        SkillLevel {
            duration: 5.0,
            radius: 4.6,
            damage: 6.0,
            ..BZ
        },
        SkillLevel {
            duration: 5.0,
            radius: 4.6,
            damage: 6.0,
            count: 2,
            ..BZ
        },
        SkillLevel {
            duration: 6.0,
            radius: 5.5,
            damage: 8.0,
            count: 2,
            ..BZ
        },
    ],
    color: [0.55, 0.75, 1.0],
};

// Flamestrike: a burning patch under the nearest enemy; the burn grows hotter.
const FS: SkillLevel = SkillLevel {
    damage: 4.0,
    cooldown: 6.0,
    radius: 3.0,
    duration: 4.0,
    tick: 0.5,
    range: 14.0,
    ..B
};
pub static FLAMESTRIKE: SkillDef = SkillDef {
    name: "Flamestrike",
    icon_spell: "Flamestrike",
    desc: "En yakın düşmanın altında yanan bir alan bırakır.",
    level_text: &[
        "En yakın düşmanın altında yanan bir alan bırakır.",
        "Yanma hasarı 6.",
        "Yanma hasarı 8, alan büyür.",
        "Yanma hasarı 11, bekleme 5 sn.",
        "Yanma hasarı 15, süre 5 sn.",
    ],
    levels: &[
        FS,
        SkillLevel { damage: 6.0, ..FS },
        SkillLevel {
            damage: 8.0,
            radius: 3.5,
            ..FS
        },
        SkillLevel {
            damage: 11.0,
            radius: 3.5,
            cooldown: 5.0,
            ..FS
        },
        SkillLevel {
            damage: 15.0,
            radius: 3.5,
            cooldown: 5.0,
            duration: 5.0,
            ..FS
        },
    ],
    color: [1.0, 0.3, 0.05],
};

// Arcane Explosion: a blast wave around the hero; hits harder.
const AE: SkillLevel = SkillLevel {
    damage: 8.0,
    cooldown: 2.5,
    radius: 4.5,
    ..B
};
pub static ARCANE_EXPLOSION: SkillDef = SkillDef {
    name: "Arcane Explosion",
    icon_spell: "Arcane Explosion",
    desc: "Etrafa bir patlama dalgası yayar.",
    level_text: &[
        "Etrafa bir patlama dalgası yayar.",
        "Hasar 12.",
        "Hasar 16, alan büyür.",
        "Hasar 21.",
        "Hasar 28, bekleme 2 sn.",
    ],
    levels: &[
        AE,
        SkillLevel { damage: 12.0, ..AE },
        SkillLevel {
            damage: 16.0,
            radius: 5.0,
            ..AE
        },
        SkillLevel {
            damage: 21.0,
            radius: 5.0,
            ..AE
        },
        SkillLevel {
            damage: 28.0,
            radius: 5.0,
            cooldown: 2.0,
            ..AE
        },
    ],
    color: [0.95, 0.6, 1.0],
};

// Blink: an emergency teleport away from danger; the wait gets shorter. `radius` is how close an
// enemy counts as "on top of you", `count` how many of them make it an emergency, `duration` the
// brief invulnerability after the jump.
const BL: SkillLevel = SkillLevel {
    cooldown: 20.0,
    radius: 2.5,
    count: 4,
    duration: 0.6,
    range: 9.0,
    ..B
};
pub static BLINK: SkillDef = SkillDef {
    name: "Blink",
    icon_spell: "Blink",
    desc: "Tehlikede (az can ya da etrafın sarılınca) kendiliğinden ışınlanır.",
    level_text: &[
        "Tehlikede (az can ya da etrafın sarılınca) kendiliğinden ışınlanır.",
        "Bekleme 16 sn.",
        "Bekleme 13 sn, daha uzağa ışınlanır.",
        "Bekleme 10 sn.",
        "Bekleme 7 sn, ışınlanınca 1 sn hasar almaz.",
    ],
    levels: &[
        BL,
        SkillLevel {
            cooldown: 16.0,
            ..BL
        },
        SkillLevel {
            cooldown: 13.0,
            range: 11.0,
            ..BL
        },
        SkillLevel {
            cooldown: 10.0,
            range: 11.0,
            ..BL
        },
        SkillLevel {
            cooldown: 7.0,
            range: 11.0,
            duration: 1.0,
            ..BL
        },
    ],
    color: [0.8, 0.5, 1.0],
};

// --- Evolutions (one level each; the card says what they keep from both parents). ---

pub static WINTER_BOLTS: SkillDef = SkillDef {
    name: "Kış Okları",
    icon_spell: "Cone of Cold",
    desc: "Frostbolt ile Frost Nova birleşir: her düşmanı delip geçen, vurduğunu donduran buz okları.",
    level_text: &["Frostbolt ile Frost Nova birleşir: her düşmanı delip geçen, vurduğunu donduran buz okları."],
    levels: &[SkillLevel {
        damage: 30.0,
        cooldown: 0.9,
        count: 4,
        pierce: 99,
        duration: 0.9,
        slow: 0.5,
        range: 20.0,
        speed: 28.0,
        ..B
    }],
    color: [0.6, 0.95, 1.0],
};

pub static PYROBLAST: SkillDef = SkillDef {
    name: "Pyroblast",
    icon_spell: "Pyroblast",
    desc: "Fireball ile Flamestrike birleşir: dev ateş topları patlar ve yeri yakar.",
    level_text: &["Fireball ile Flamestrike birleşir: dev ateş topları patlar ve yeri yakar."],
    levels: &[SkillLevel {
        damage: 90.0,
        cooldown: 2.2,
        count: 2,
        radius: 6.5,
        duration: 4.0,
        tick: 0.5,
        range: 22.0,
        speed: 16.0,
        ..B
    }],
    color: [1.0, 0.35, 0.0],
};

pub static ARCANE_STORM: SkillDef = SkillDef {
    name: "Arkan Fırtına",
    icon_spell: "Evocation",
    desc:
        "Arcane Missiles ile Arcane Explosion birleşir: çarptığı yerde patlayan bir füze sağanağı.",
    level_text: &[
        "Arcane Missiles ile Arcane Explosion birleşir: çarptığı yerde patlayan bir füze sağanağı.",
    ],
    levels: &[SkillLevel {
        damage: 12.0,
        cooldown: 1.2,
        count: 10,
        radius: 2.5,
        range: 18.0,
        speed: 18.0,
        ..B
    }],
    color: [1.0, 0.5, 1.0],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evolutions_pair_base_skills_once() {
        let mut used = Vec::new();
        for (b, p, into) in EVOLUTIONS {
            assert!(
                SkillId::ALL.contains(&b) && SkillId::ALL.contains(&p),
                "{b:?}"
            );
            assert!(into.is_evolved() && !b.is_evolved());
            assert!(
                !used.contains(&b) && !used.contains(&p),
                "a skill in two evolutions"
            );
            used.extend([b, p]);
            assert_eq!(b.evolution(), Some((p, into)));
            assert_eq!(into.max_level(), 1);
        }
        assert_eq!(SkillId::Blink.evolution(), None);
    }

    #[test]
    fn every_skill_has_text_for_every_level() {
        for s in SkillId::every() {
            let d = s.def();
            assert!(!d.levels.is_empty(), "{s:?}");
            assert_eq!(d.levels.len(), d.level_text.len(), "{s:?}");
            assert_eq!(d.level_text[0], d.desc, "{s:?}");
        }
    }

    #[test]
    fn levels_never_get_worse() {
        for s in SkillId::ALL {
            for w in s.def().levels.windows(2) {
                let (a, b) = (&w[0], &w[1]);
                assert!(b.damage >= a.damage, "{s:?} damage");
                assert!(b.cooldown <= a.cooldown, "{s:?} cooldown");
                assert!(b.count >= a.count, "{s:?} count");
                assert!(b.radius >= a.radius, "{s:?} radius");
                assert!(b.pierce >= a.pierce, "{s:?} pierce");
                assert!(b.duration >= a.duration, "{s:?} duration");
                assert!(b != a, "{s:?}: a level must change something");
            }
        }
    }

    #[test]
    fn stats_are_sane() {
        for s in SkillId::ALL {
            for l in s.def().levels {
                assert!(l.cooldown > 0.1, "{s:?}");
                assert!(l.tick > 0.05, "{s:?}");
                assert!((0.0..1.0).contains(&l.slow), "{s:?}");
                assert!(l.count >= 1, "{s:?}");
            }
        }
    }

    #[test]
    fn level_lookup_clamps() {
        let s = SkillId::Frostbolt;
        assert_eq!(s.level(0), s.level(1));
        assert_eq!(s.level(99), s.level(s.max_level()));
    }

    #[test]
    fn keys_round_trip() {
        for s in SkillId::every() {
            assert_eq!(SkillId::from_key(s.key()), Some(s));
        }
        assert_eq!(SkillId::from_key(" Blink "), Some(SkillId::Blink));
        assert_eq!(SkillId::from_key("polymorph"), None);
    }
}
