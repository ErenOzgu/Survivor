//! Test creatures for checking creature rendering on a machine with the data: `RNS_TEST_CREATURES`
//! or F8 spawns a few around the [`DebugCenter`]; F9 steps their animation state; F10 makes the
//! center entity cast.

use benilla_assets::AdtTile;
use benilla_world::terrain_stream::{terrain_height_under, TerrainStreamer};
use bevy::prelude::*;

use super::catalog::ModelData;
use super::{paths, AnimState, ModelAnim, ModelRequest};

/// `RNS_TEST_CREATURES`: `1` spawns a kobold, a wolf, a spider and a gnoll near the hero once the
/// ground is in; a comma list of CreatureDisplayInfo ids spawns those instead.
pub const TEST_VAR: &str = "RNS_TEST_CREATURES";

/// The model folders the default test set picks a display from, by name.
pub const DEFAULT_FOLDERS: [(&str, &str); 4] = [
    ("kobold", "Creature\\Kobold"),
    ("wolf", "Creature\\Wolf"),
    ("spider", "Creature\\Spider"),
    ("gnoll", "Creature\\Gnoll"),
];

/// Yards from the center the test creatures stand at.
const RING: f32 = 6.0;

/// The entity test creatures spawn around (the hero), and F10's caster.
#[derive(Component)]
pub struct DebugCenter;

/// A spawned test creature.
#[derive(Component)]
pub struct TestCreature;

/// What to spawn and whether it has been.
#[derive(Resource)]
pub(super) struct TestSpawn {
    /// Explicit display ids; `None` uses [`DEFAULT_FOLDERS`].
    ids: Option<Vec<u32>>,
    /// Spawn as soon as the ground is in.
    armed: bool,
    done: bool,
    /// F9's position in the state cycle.
    step: usize,
}

/// What `RNS_TEST_CREATURES` asks for: `None` off, `Some(None)` the default set, `Some(Some(ids))`.
pub fn parse_test_var(v: Option<&str>) -> Option<Option<Vec<u32>>> {
    let v = v?.trim();
    match v.to_ascii_lowercase().as_str() {
        "" | "0" | "off" | "false" | "no" => None,
        "1" | "on" | "true" | "yes" => Some(None),
        _ => {
            let ids: Option<Vec<u32>> = v
                .split(',')
                .map(|p| p.trim().parse::<u32>().ok().filter(|&id| id != 0))
                .collect();
            match ids {
                Some(ids) if !ids.is_empty() => Some(Some(ids)),
                _ => {
                    warn!("rnsurvivors: {TEST_VAR}={v:?} wants `1` or display ids — using the default set");
                    Some(None)
                }
            }
        }
    }
}

impl TestSpawn {
    pub(super) fn from_env() -> Self {
        let parsed = parse_test_var(std::env::var(TEST_VAR).ok().as_deref());
        Self {
            armed: parsed.is_some(),
            ids: parsed.flatten(),
            done: false,
            step: 0,
        }
    }
}

/// The lowest display id whose model lives in `folder` and has a skin variation (so it draws
/// textured), from `(display id, model path, skinned)` rows.
pub fn pick_by_folder<'a>(
    rows: impl Iterator<Item = (u32, &'a str, bool)>,
    folder: &str,
) -> Option<u32> {
    rows.filter(|(_, path, skinned)| *skinned && paths::same_path(paths::model_dir(path), folder))
        .map(|(id, _, _)| id)
        .min()
}

/// The animation state F9 steps to after `step` presses.
pub fn cycle_state(step: usize) -> AnimState {
    const CYCLE: [AnimState; 7] = [
        AnimState::Idle,
        AnimState::Walk,
        AnimState::Run,
        AnimState::Cast,
        AnimState::Attack,
        AnimState::Hit,
        AnimState::Death,
    ];
    CYCLE[step % CYCLE.len()]
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_test_creatures(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut spawn: ResMut<TestSpawn>,
    data: Option<Res<ModelData>>,
    centers: Query<&GlobalTransform, With<DebugCenter>>,
    streamer: Option<Res<TerrainStreamer>>,
    adt_tiles: Option<Res<Assets<AdtTile>>>,
) {
    if keys.just_pressed(KeyCode::F8) && !spawn.done {
        spawn.armed = true;
    }
    if !spawn.armed || spawn.done {
        return;
    }
    let Some(data) = data else { return };
    if data.tables.is_none() {
        warn!("rnsurvivors: no WoW data, no test creatures");
        spawn.done = true;
        return;
    }
    let Ok(center) = centers.single().map(|c| c.translation()) else {
        return;
    };
    let ids: Vec<(String, u32)> = match &spawn.ids {
        Some(ids) => ids
            .iter()
            .map(|id| (format!("display {id}"), *id))
            .collect(),
        None => DEFAULT_FOLDERS
            .iter()
            .filter_map(|(name, folder)| {
                let found = super::creature_display_in_folder(&data, folder);
                if found.is_none() {
                    warn!("rnsurvivors: no {name} display found under {folder}");
                }
                found.map(|id| (name.to_string(), id))
            })
            .collect(),
    };
    // Every creature stands on resident ground, or none spawns yet.
    let mut placed = Vec::new();
    for (i, (name, id)) in ids.iter().enumerate() {
        let angle = i as f32 / ids.len().max(1) as f32 * std::f32::consts::TAU;
        let mut pos = center + Vec3::new(angle.sin(), 0.0, angle.cos()) * RING;
        let ground = match (&streamer, &adt_tiles) {
            (Some(s), Some(a)) => terrain_height_under(s, a, pos),
            _ => None,
        };
        let Some(y) = ground else { return };
        pos.y = y;
        placed.push((name.clone(), *id, pos));
    }
    for (name, id, pos) in placed {
        info!("rnsurvivors: test creature {name} (display {id})");
        let face = Vec3::new(center.x, pos.y, center.z);
        commands.spawn((
            Name::new(format!("test {name}")),
            TestCreature,
            Transform::from_translation(pos).looking_at(face, Vec3::Y),
            Visibility::default(),
            ModelRequest::Creature { display_id: id },
            ModelAnim::default(),
        ));
    }
    spawn.done = true;
}

/// F9: every test creature steps to the next animation state. F10: the center entity casts.
pub(super) fn debug_anim_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut spawn: ResMut<TestSpawn>,
    mut creatures: Query<&mut ModelAnim, (With<TestCreature>, Without<DebugCenter>)>,
    mut centers: Query<&mut ModelAnim, With<DebugCenter>>,
) {
    if keys.just_pressed(KeyCode::F9) {
        spawn.step += 1;
        let state = cycle_state(spawn.step);
        info!("rnsurvivors: test creatures → {state:?}");
        for mut a in &mut creatures {
            if state.is_action() {
                if a.state == AnimState::Death {
                    a.state = AnimState::Idle;
                }
                a.play(state);
            } else {
                a.state = state;
            }
        }
    }
    if keys.just_pressed(KeyCode::F10) {
        for mut a in &mut centers {
            a.play(AnimState::Cast);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_var_parses() {
        assert_eq!(parse_test_var(None), None);
        assert_eq!(parse_test_var(Some("0")), None);
        assert_eq!(parse_test_var(Some("1")), Some(None));
        assert_eq!(parse_test_var(Some("12, 34")), Some(Some(vec![12, 34])));
        assert_eq!(parse_test_var(Some("x")), Some(None));
    }

    #[test]
    fn the_lowest_skinned_display_in_the_folder_wins() {
        let rows = [
            (50, "Creature\\Kobold\\Kobold.mdx", true),
            (40, "Creature\\Kobold\\Kobold.mdx", false),
            (45, "creature/kobold/kobold.mdx", true),
            (10, "Creature\\KoboldBoss\\KoboldBoss.mdx", true),
            (5, "Creature\\Wolf\\Wolf.mdx", true),
        ];
        assert_eq!(
            pick_by_folder(rows.into_iter(), "Creature\\Kobold"),
            Some(45)
        );
        assert_eq!(pick_by_folder(rows.into_iter(), "Creature\\Wolf"), Some(5));
        assert_eq!(pick_by_folder(rows.into_iter(), "Creature\\Spider"), None);
    }

    #[test]
    fn the_cycle_walks_every_state_and_wraps() {
        assert_eq!(cycle_state(0), AnimState::Idle);
        assert_eq!(cycle_state(6), AnimState::Death);
        assert_eq!(cycle_state(7), AnimState::Idle);
    }
}
