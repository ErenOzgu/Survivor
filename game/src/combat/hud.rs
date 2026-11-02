//! The HUD: health bar, XP bar, level, run timer, kills, owned skills, and the death banner.

use bevy::prelude::*;

use super::art::{WowArt, STATUS_BAR};
use super::hero::{DeathNotice, HeroHealth};
use super::rules;
use super::{CombatSet, Run};
use crate::content::SkillId;
use crate::game::FollowCam;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_font, spawn_hud).chain())
            .add_systems(
                Update,
                (ui_on_follow_camera, update_hud, update_skill_bar, skin_bars)
                    .in_set(CombatSet::Ui),
            );
    }
}

/// The UI font: WoW's own Arial Narrow from the player's Data (it carries Turkish letters).
/// Without WoW data there is no font and the UI shows its boxes but no text.
#[derive(Resource, Clone)]
pub struct UiFont(pub Handle<Font>);

impl UiFont {
    pub fn text(&self, size: f32) -> TextFont {
        TextFont {
            font: self.0.clone(),
            font_size: size,
            ..default()
        }
    }
}

/// `RNS_FONT`: a font file to use instead of WoW's (e.g. `C:\\Windows\\Fonts\\arial.ttf`), for
/// testing without WoW data.
pub const FONT_VAR: &str = "RNS_FONT";

fn load_font(mut commands: Commands, server: Res<AssetServer>, mut fonts: ResMut<Assets<Font>>) {
    if let Ok(path) = std::env::var(FONT_VAR) {
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| Font::try_from_bytes(b).map_err(|e| e.to_string()))
        {
            Ok(font) => {
                commands.insert_resource(UiFont(fonts.add(font)));
                return;
            }
            Err(e) => warn!("rnsurvivors: {FONT_VAR}={path:?} unusable ({e}), using WoW's font"),
        }
    }
    commands.insert_resource(UiFont(server.load("mpq://fonts/arialn.ttf")));
}

/// The UI draws on the follow camera (the engine may add cameras of its own).
fn ui_on_follow_camera(mut commands: Commands, cams: Query<Entity, Added<FollowCam>>) {
    for cam in &cams {
        commands.entity(cam).insert(IsDefaultUiCamera);
    }
}

#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpText;
#[derive(Component)]
struct XpFill;
#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct TimerText;
/// The bottom action-bar-like row of owned skills.
#[derive(Component)]
struct SkillBar;
#[derive(Component)]
struct NoticeText;

const PANEL: Color = Color::srgba(0.05, 0.05, 0.08, 0.75);

fn bar(p: &mut ChildSpawnerCommands, width: Val, height: f32, fill: Color, marker: impl Component) {
    p.spawn((
        Node {
            width,
            height: Val::Px(height),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.3)),
    ))
    .with_children(|p| {
        p.spawn((
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(fill),
            marker,
        ));
    });
}

fn spawn_hud(mut commands: Commands, font: Res<UiFont>) {
    let white = TextColor(Color::WHITE);
    // XP bar along the top edge.
    commands
        .spawn((
            Name::new("hud xp"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|p| {
            bar(
                p,
                Val::Percent(100.0),
                10.0,
                Color::srgb(0.3, 0.6, 1.0),
                XpFill,
            );
        });

    // Top-left: level, health.
    commands
        .spawn((
            Name::new("hud status"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(18.0),
                left: Val::Px(16.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((Text::new("Level 1"), font.text(26.0), white, LevelText));
            bar(
                p,
                Val::Px(260.0),
                18.0,
                Color::srgb(0.85, 0.15, 0.15),
                HpFill,
            );
            p.spawn((Text::new(""), font.text(16.0), white, HpText));
        });

    // Top-centre: run timer and kills.
    commands
        .spawn((
            Name::new("hud timer"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(18.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((Text::new("00:00"), font.text(28.0), white, TimerText));
        });

    // Bottom-centre: owned skills as an action bar (filled by `update_skill_bar`).
    commands.spawn((
        Name::new("hud skills"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(6.0),
            ..default()
        },
        SkillBar,
    ));

    // Centre: the death banner.
    commands
        .spawn((
            Name::new("hud notice"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(30.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font.text(30.0),
                TextColor(Color::srgb(1.0, 0.35, 0.3)),
                NoticeText,
            ));
        });
}

/// Rebuilds the skill bar when the loadout changes (or the art arrives): each owned skill as its
/// WoW icon in a slot, with its level, gold when maxed.
fn update_skill_bar(
    mut commands: Commands,
    run: Res<Run>,
    art: Res<WowArt>,
    font: Res<UiFont>,
    bars: Query<Entity, With<SkillBar>>,
    mut shown: Local<Option<Vec<(SkillId, u8)>>>,
) {
    let Ok(bar) = bars.single() else {
        return;
    };
    if shown.as_ref() == Some(&run.loadout.0) && !art.is_changed() {
        return;
    }
    *shown = Some(run.loadout.0.clone());
    commands.entity(bar).despawn_related::<Children>();
    commands.entity(bar).with_children(|b| {
        for &(id, level) in &run.loadout.0 {
            let c = id.def().color;
            let color = Color::srgb(c[0], c[1], c[2]);
            let maxed = level >= id.max_level();
            b.spawn((
                Node {
                    width: Val::Px(52.0),
                    height: Val::Px(52.0),
                    border: UiRect::all(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(color.with_alpha(0.35)),
                BorderColor::all(if maxed {
                    Color::srgb(1.0, 0.82, 0.0)
                } else {
                    Color::srgba(0.0, 0.0, 0.0, 0.9)
                }),
            ))
            .with_children(|slot| {
                match art.icon(id) {
                    Some(img) => {
                        slot.spawn((
                            ImageNode::new(img),
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                    }
                    None => {
                        slot.spawn((
                            Text::new(id.def().name.chars().next().unwrap_or('?').to_string()),
                            font.text(26.0),
                            TextColor(Color::WHITE),
                        ));
                    }
                }
                // The level in the corner, as WoW shows a stack count.
                slot.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(2.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    },
                    Text::new(level.to_string()),
                    font.text(16.0),
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                ));
            });
        }
    });
}

/// Once the art is in, the health and XP bars wear WoW's status-bar texture, tinted.
#[allow(clippy::type_complexity)]
fn skin_bars(
    mut commands: Commands,
    art: Res<WowArt>,
    bars: Query<(Entity, &BackgroundColor), (Or<(With<HpFill>, With<XpFill>)>, Without<ImageNode>)>,
) {
    let Some(tex) = art.ui(STATUS_BAR) else {
        return;
    };
    for (e, bg) in &bars {
        commands.entity(e).insert(
            ImageNode::new(tex.clone())
                .with_color(bg.0)
                .with_mode(NodeImageMode::Stretch),
        );
    }
}

/// `mm:ss`.
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_hud(
    real: Res<Time<Real>>,
    run: Res<Run>,
    health: Res<HeroHealth>,
    mut notice: ResMut<DeathNotice>,
    mut fills: ParamSet<(
        Query<&mut Node, With<HpFill>>,
        Query<&mut Node, With<XpFill>>,
    )>,
    mut texts: ParamSet<(
        Query<&mut Text, With<HpText>>,
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<TimerText>>,
        Query<&mut Text, With<NoticeText>>,
    )>,
) {
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent(100.0 * health.fraction());
    }
    let need = rules::xp_to_next(run.level);
    if let Ok(mut n) = fills.p1().single_mut() {
        n.width = Val::Percent(100.0 * (run.xp as f32 / need as f32).min(1.0));
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        **t = format!("Can {:.0} / {:.0}", health.current.ceil(), health.max);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        **t = format!("Level {}   XP {}/{}", run.level, run.xp, need);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        **t = format!("{}   ·   {} düşman", clock(run.time), run.kills);
    }
    notice.left = (notice.left - real.delta_secs()).max(0.0);
    if let Ok(mut t) = texts.p3().single_mut() {
        let want = if notice.left > 0.0 {
            notice.text.as_str()
        } else {
            ""
        };
        if t.0 != want {
            **t = want.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn clock_formats() {
        assert_eq!(super::clock(0.0), "00:00");
        assert_eq!(super::clock(61.9), "01:01");
        assert_eq!(super::clock(-3.0), "00:00");
        assert_eq!(super::clock(3600.0), "60:00");
    }
}

#[cfg(test)]
mod system_params {
    use super::*;
    use bevy::ecs::system::IntoSystem;

    fn init<M>(world: &mut World, s: impl IntoSystem<(), (), M>) {
        let mut s = IntoSystem::into_system(s);
        s.initialize(world);
    }

    /// Each system's parameters are compatible (no conflicting queries); Bevy checks it here.
    #[test]
    fn systems_initialize() {
        let mut world = World::new();
        init(&mut world, update_hud);
        init(&mut world, update_skill_bar);
        init(&mut world, skin_bars);
        init(&mut world, ui_on_follow_camera);
        init(&mut world, spawn_hud);
        init(&mut world, load_font);
    }
}
