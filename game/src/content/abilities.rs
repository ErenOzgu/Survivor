//! The hero's cooldowns: three spells the player casts with Z, X and C (the skills cast
//! themselves). Bloodlust once per run; one defensive and one offensive on five-minute cooldowns.
//! Rnsurvivors' own numbers (CLAUDE.md §2); the names and icons are WoW's spells, read from the
//! player's client.

/// A cooldown spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AbilityId {
    /// Everything faster: skills refill sooner, the hero runs quicker. Once per run.
    Bloodlust,
    /// Defensive: an ice shield soaks damage.
    IceBarrier,
    /// Offensive: spells hit much harder.
    ArcanePower,
}

/// What a cooldown spell does and how often.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilityDef {
    /// The WoW spell's name: its icon and effects come from the client's `Spell.dbc` row.
    pub name: &'static str,
    /// The key shown on its slot.
    pub key_label: &'static str,
    /// Seconds between casts; `None`: once per run.
    pub cooldown: Option<f32>,
    /// Seconds it lasts.
    pub duration: f32,
    /// Turkish, for the slot's tooltip line and the README.
    pub desc: &'static str,
    /// Multipliers while it lasts: damage, skill cooldowns, run speed.
    pub damage_mul: f32,
    pub cooldown_mul: f32,
    pub speed_mul: f32,
    /// A shield soaking this share of maximum health, while it lasts.
    pub shield: f32,
}

impl AbilityId {
    pub const ALL: [AbilityId; 3] = [
        AbilityId::Bloodlust,
        AbilityId::IceBarrier,
        AbilityId::ArcanePower,
    ];

    pub fn def(self) -> &'static AbilityDef {
        match self {
            AbilityId::Bloodlust => &BLOODLUST,
            AbilityId::IceBarrier => &ICE_BARRIER,
            AbilityId::ArcanePower => &ARCANE_POWER,
        }
    }
}

const BLOODLUST: AbilityDef = AbilityDef {
    name: "Bloodlust",
    key_label: "Z",
    cooldown: None,
    duration: 40.0,
    desc: "Koşu başına bir kez: 40 sn skiller %35 hızlı dolar, %25 hızlı koşarsın.",
    damage_mul: 1.0,
    cooldown_mul: 0.65,
    speed_mul: 1.25,
    shield: 0.0,
};

const ICE_BARRIER: AbilityDef = AbilityDef {
    name: "Ice Barrier",
    key_label: "X",
    cooldown: Some(300.0),
    duration: 20.0,
    desc: "5 dk'da bir: 20 sn boyunca en yüksek canının %60'ı kadar hasarı buz kalkanı emer.",
    damage_mul: 1.0,
    cooldown_mul: 1.0,
    speed_mul: 1.0,
    shield: 0.6,
};

const ARCANE_POWER: AbilityDef = AbilityDef {
    name: "Arcane Power",
    key_label: "C",
    cooldown: Some(300.0),
    duration: 15.0,
    desc: "5 dk'da bir: 15 sn büyüler %50 fazla vurur.",
    damage_mul: 1.5,
    cooldown_mul: 1.0,
    speed_mul: 1.0,
    shield: 0.0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_once_per_run_two_on_five_minutes() {
        let once = AbilityId::ALL
            .iter()
            .filter(|a| a.def().cooldown.is_none())
            .count();
        assert_eq!(once, 1);
        for a in AbilityId::ALL {
            let d = a.def();
            assert!(d.duration > 0.0 && d.cooldown.is_none_or(|c| c == 300.0));
            assert!(d.damage_mul >= 1.0 && d.cooldown_mul <= 1.0 && d.speed_mul >= 1.0);
        }
        assert!(AbilityId::IceBarrier.def().shield > 0.0);
    }
}
