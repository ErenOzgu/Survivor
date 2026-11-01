//! The HUD: health bar, XP bar, level, run timer, kills, owned skills, and the death banner.

use bevy::prelude::*;

use super::hero::{DeathNotice, HeroHealth};
use super::rules;
use super::{skill_label, CombatSet, Run};
use crate::game::FollowCam;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_font, spawn_hud).chain())
            .add_systems(
                Update,
                (ui_on_follow_camera, update_hud).in_set(CombatSet::Ui),
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

fn load_font(mut commands: Commands, server: Res<AssetServer>) {
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
#[derive(Component)]
struct SkillsText;
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

    // Bottom-left: owned skills.
    commands
        .spawn((
            Name::new("hud skills"),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(16.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|p| {
            p.spawn((Text::new(""), font.text(18.0), white, SkillsText));
        });

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
        Query<&mut Text, With<SkillsText>>,
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
    if let Ok(mut t) = texts.p3().single_mut() {
        let lines: Vec<String> = run
            .loadout
            .0
            .iter()
            .map(|(id, l)| {
                let max = if *l >= id.max_level() {
                    "  (en üst)"
                } else {
                    ""
                };
                format!("{}{max}", skill_label(*id, *l))
            })
            .collect();
        **t = format!(
            "Skill'ler {}/{}\n{}",
            run.loadout.0.len(),
            crate::content::MAX_SKILLS,
            lines.join("\n")
        );
    }
    notice.left = (notice.left - real.delta_secs()).max(0.0);
    if let Ok(mut t) = texts.p4().single_mut() {
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
