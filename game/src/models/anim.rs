//! Animation states the game sets on a model, and the driver that plays them on its rig.
//!
//! The game writes [`ModelAnim`] on the entity that carries the [`super::ModelRequest`]; the
//! driver picks the model's clip for the state (by animation id, with fallbacks, since not every
//! model authors every id) and cross-fades to it.

use std::time::Duration;

use benilla_assets::ModelAnimations;
use bevy::animation::transition::AnimationTransitions;
use bevy::prelude::*;

/// Animation ids from the 1.12 client's `AnimationData.dbc` (the engine format's own numbering).
pub mod ids {
    pub const STAND: u16 = 0;
    pub const DEATH: u16 = 1;
    pub const SPELL: u16 = 2;
    pub const WALK: u16 = 4;
    pub const RUN: u16 = 5;
    pub const DEAD: u16 = 6;
    pub const STAND_WOUND: u16 = 8;
    pub const COMBAT_WOUND: u16 = 9;
    pub const ATTACK_UNARMED: u16 = 16;
    pub const ATTACK_1H: u16 = 17;
    pub const ATTACK_2H: u16 = 18;
    pub const ATTACK_2HL: u16 = 19;
    pub const SPELL_CAST: u16 = 32;
    pub const JUMP_START: u16 = 37;
    pub const JUMP: u16 = 38;
    pub const SPELL_CAST_DIRECTED: u16 = 53;
    pub const SPELL_CAST_OMNI: u16 = 54;
}

/// What a model is doing. Idle, Walk, Run and Death are states; Cast, Attack and Hit are actions
/// that play once over the state ([`ModelAnim::play`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AnimState {
    #[default]
    Idle,
    Walk,
    Run,
    Cast,
    Attack,
    Hit,
    Death,
    /// In the air (a jump): held until the feet are down again.
    Jump,
}

impl AnimState {
    /// The animation ids to try, best first.
    pub fn candidates(self) -> &'static [u16] {
        use ids::*;
        match self {
            AnimState::Idle => &[STAND],
            AnimState::Walk => &[WALK, RUN, STAND],
            AnimState::Run => &[RUN, WALK, STAND],
            AnimState::Cast => &[SPELL_CAST_DIRECTED, SPELL_CAST, SPELL_CAST_OMNI, SPELL],
            AnimState::Attack => &[ATTACK_UNARMED, ATTACK_1H, ATTACK_2H, ATTACK_2HL],
            AnimState::Hit => &[COMBAT_WOUND, STAND_WOUND],
            AnimState::Death => &[DEATH, DEAD],
            AnimState::Jump => &[JUMP, JUMP_START, STAND],
        }
    }

    /// Whether the clip loops; otherwise it plays once (Death then holds its last frame).
    pub fn looping(self) -> bool {
        matches!(
            self,
            AnimState::Idle | AnimState::Walk | AnimState::Run | AnimState::Jump
        )
    }

    /// Whether this is an action that plays once over the current state.
    pub fn is_action(self) -> bool {
        matches!(self, AnimState::Cast | AnimState::Attack | AnimState::Hit)
    }

    /// Whether it is a locomotion state, which [`AutoLocomotion`] may overwrite.
    pub fn is_locomotion(self) -> bool {
        matches!(self, AnimState::Idle | AnimState::Walk | AnimState::Run)
    }
}

/// The first of `state`'s candidate ids the model has (`has`); `None` if it has none.
pub fn pick_anim_id(state: AnimState, has: impl Fn(u16) -> bool) -> Option<u16> {
    state.candidates().iter().copied().find(|&id| has(id))
}

/// The playback rate that keeps feet from sliding: the body's speed over the clip's authored gait
/// speed, clamped; `1.0` when either is unknown.
pub fn playback_rate(actual: f32, authored: f32) -> f32 {
    if actual > 0.0 && authored > 0.0 {
        (actual / authored).clamp(0.25, 3.0)
    } else {
        1.0
    }
}

/// Walking speed under which a body counts as standing still (yards per second).
pub const STILL_SPEED: f32 = 0.3;
/// Speed from which a body runs rather than walks.
pub const RUN_SPEED: f32 = 4.0;

/// The locomotion state for a horizontal speed in yards per second.
pub fn locomotion_for(speed: f32) -> AnimState {
    if speed < STILL_SPEED {
        AnimState::Idle
    } else if speed < RUN_SPEED {
        AnimState::Walk
    } else {
        AnimState::Run
    }
}

/// The game's handle on a model's animation, on the entity carrying the model request.
#[derive(Component, Clone, Debug, Default)]
pub struct ModelAnim {
    /// The current state: Idle, Walk, Run or Death (an action set here plays like [`Self::play`]).
    pub state: AnimState,
    /// How fast the body moves, yards per second, so a gait plays at a matching rate; 0 = 1×.
    pub move_speed: f32,
    action: Option<AnimState>,
    /// Bumped by every [`Self::play`], so the same action played twice restarts.
    action_seq: u32,
}

impl ModelAnim {
    /// Play `action` once over the current state (Cast, Attack, Hit), restarting it if it is
    /// already playing. A non-action is set as the state. A dead body plays no action.
    pub fn play(&mut self, action: AnimState) {
        if !action.is_action() {
            self.state = action;
            return;
        }
        if self.state == AnimState::Death {
            return;
        }
        self.action = Some(action);
        self.action_seq = self.action_seq.wrapping_add(1);
    }

    /// The action currently playing, if any.
    pub fn action(&self) -> Option<AnimState> {
        self.action
    }

    /// What should be playing now: the pending action, else the state.
    pub fn wanted(&self) -> (AnimState, u32) {
        match self.action {
            Some(a) if self.state != AnimState::Death => (a, self.action_seq),
            _ => (self.state, 0),
        }
    }
}

/// Sets Idle/Walk/Run on its entity's [`ModelAnim`] from how fast the entity's `Transform` moves.
/// It never overrides Death, and an action plays over it as usual.
#[derive(Component, Default)]
pub struct AutoLocomotion {
    last: Option<Vec3>,
    /// The speed, smoothed over a few frames, so one stalled frame does not flash Idle.
    smoothed: f32,
}

pub(super) fn auto_locomotion(
    time: Res<Time>,
    mut q: Query<(&GlobalTransform, &mut AutoLocomotion, &mut ModelAnim)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (xf, mut loco, mut anim) in &mut q {
        let p = xf.translation();
        let speed = match loco.last {
            Some(last) => Vec2::new(p.x - last.x, p.z - last.z).length() / dt,
            None => 0.0,
        };
        loco.last = Some(p);
        // Exponential smoothing with a ~0.1 s time constant.
        let k = (dt / 0.1).min(1.0);
        loco.smoothed += (speed - loco.smoothed) * k;
        if anim.state.is_locomotion() {
            let next = locomotion_for(loco.smoothed);
            if anim.state != next {
                anim.state = next;
            }
            anim.move_speed = loco.smoothed;
        }
    }
}

/// On a model root: what the driver last started.
#[derive(Component)]
pub(super) struct Playing {
    state: AnimState,
    seq: u32,
    node: AnimationNodeIndex,
    authored_speed: f32,
}

/// Plays each owner's wanted state on its model root.
pub(super) fn drive_animations(
    mut owners: Query<(&mut ModelAnim, &super::ModelRoot)>,
    mut roots: Query<(
        Entity,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &ModelAnimations,
        Option<&mut Playing>,
    )>,
    mut commands: Commands,
) {
    for (mut anim, root) in &mut owners {
        let Ok((entity, mut player, mut transitions, anims, playing)) = roots.get_mut(root.0)
        else {
            continue;
        };
        // An action that ran to its end hands back to the state.
        if let Some(p) = playing.as_deref() {
            if p.state.is_action()
                && anim.action.is_some()
                && anim.wanted() == (p.state, p.seq)
                && player.animation(p.node).is_none_or(|a| a.is_finished())
            {
                anim.action = None;
            }
        }
        let (state, seq) = anim.wanted();
        let same = playing
            .as_deref()
            .is_some_and(|p| p.state == state && p.seq == seq);
        if same {
            // Keep the gait in step with the body.
            if let Some(p) = playing.as_deref() {
                if p.state.is_locomotion() {
                    let rate = playback_rate(anim.move_speed, p.authored_speed);
                    if let Some(active) = player.animation_mut(p.node) {
                        active.set_speed(rate);
                    }
                }
            }
            continue;
        }
        let Some(id) = pick_anim_id(state, |id| anims.find(id).is_some()) else {
            // Nothing to play for this state: an action is dropped, a state keeps the last clip.
            if state.is_action() {
                anim.action = None;
            }
            continue;
        };
        let clip = anims.find(id).expect("picked from the model's own clips");
        let fade = Duration::from_secs_f32(clip.blend_time.clamp(0.1, 0.3));
        let active = transitions.play(&mut player, clip.node, fade);
        // A replayed action starts from its beginning.
        active.replay();
        if state.looping() {
            active.repeat();
        }
        let next = Playing {
            state,
            seq,
            node: clip.node,
            authored_speed: clip.move_speed,
        };
        match playing {
            Some(mut p) => *p = next,
            None => {
                commands.entity(entity).insert(next);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_authored_candidate_wins() {
        // A model with Run but no Walk walks with its run.
        let has = |id| id == ids::RUN || id == ids::STAND;
        assert_eq!(pick_anim_id(AnimState::Walk, has), Some(ids::RUN));
        assert_eq!(pick_anim_id(AnimState::Run, has), Some(ids::RUN));
        assert_eq!(pick_anim_id(AnimState::Idle, has), Some(ids::STAND));
        // No attack authored at all: nothing to play.
        assert_eq!(pick_anim_id(AnimState::Attack, has), None);
    }

    #[test]
    fn cast_prefers_the_directed_cast() {
        let has = |id| [ids::SPELL, ids::SPELL_CAST_DIRECTED, ids::SPELL_CAST].contains(&id);
        assert_eq!(
            pick_anim_id(AnimState::Cast, has),
            Some(ids::SPELL_CAST_DIRECTED)
        );
        let only_spell = |id| id == ids::SPELL;
        assert_eq!(pick_anim_id(AnimState::Cast, only_spell), Some(ids::SPELL));
    }

    #[test]
    fn death_falls_back_to_dead() {
        assert_eq!(
            pick_anim_id(AnimState::Death, |id| id == ids::DEAD),
            Some(ids::DEAD)
        );
    }

    #[test]
    fn every_state_has_candidates() {
        for s in [
            AnimState::Idle,
            AnimState::Walk,
            AnimState::Run,
            AnimState::Cast,
            AnimState::Attack,
            AnimState::Hit,
            AnimState::Death,
        ] {
            assert!(!s.candidates().is_empty(), "{s:?}");
        }
    }

    #[test]
    fn only_gaits_loop() {
        assert!(AnimState::Run.looping());
        assert!(!AnimState::Attack.looping());
        assert!(!AnimState::Death.looping());
    }

    #[test]
    fn rate_matches_the_authored_gait() {
        assert_eq!(playback_rate(7.0, 7.0), 1.0);
        assert_eq!(playback_rate(3.5, 7.0), 0.5);
        assert_eq!(playback_rate(100.0, 7.0), 3.0);
        assert_eq!(playback_rate(0.0, 7.0), 1.0);
        assert_eq!(playback_rate(7.0, 0.0), 1.0);
        // A backwards gait is authored negative: play it at 1×.
        assert_eq!(playback_rate(7.0, -3.0), 1.0);
    }

    #[test]
    fn speed_picks_the_gait() {
        assert_eq!(locomotion_for(0.0), AnimState::Idle);
        assert_eq!(locomotion_for(2.5), AnimState::Walk);
        assert_eq!(locomotion_for(7.0), AnimState::Run);
    }

    #[test]
    fn actions_play_over_the_state_and_restart() {
        let mut a = ModelAnim {
            state: AnimState::Run,
            ..default()
        };
        a.play(AnimState::Cast);
        let first = a.wanted();
        assert_eq!(first.0, AnimState::Cast);
        a.play(AnimState::Cast);
        assert_ne!(a.wanted(), first, "a second cast restarts");
        assert_eq!(a.state, AnimState::Run, "the state is kept underneath");
    }

    #[test]
    fn the_dead_play_no_action() {
        let mut a = ModelAnim::default();
        a.play(AnimState::Death);
        assert_eq!(a.state, AnimState::Death);
        a.play(AnimState::Attack);
        assert_eq!(a.wanted(), (AnimState::Death, 0));
    }
}
