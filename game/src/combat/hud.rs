//! The HUD, in the client's own frames: the player frame (portrait, level, health, XP) top-left,
//! the action bar with its gryphons and XP strip at the bottom (owned skills, levels, cooldown
//! shades), the run timer and kills top-centre, and the death banner. Plain panels stand in for a
//! texture the client lacks.

use bevy::prelude::*;

use super::art::{WowArt, CLASS_ICONS, END_CAP, MAIN_BAR, STATUS_BAR, TARGET_FRAME};
use super::hero::{DeathNotice, HeroHealth};
use super::rules;
use super::{CombatSet, Run};
use crate::content::abilities::AbilityId;
use crate::content::SkillId;
use crate::game::FollowCam;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_font, spawn_hud).chain())
            .add_systems(
                Update,
                (
                    ui_on_follow_camera,
                    update_hud,
                    update_skill_bar,
                    update_cooldowns,
                    update_ability_bar,
                    update_ability_slots,
                    skin_bars,
                    apply_skins,
                )
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
/// The XP strip along the top of the action bar.
#[derive(Component)]
struct XpStrip;
#[derive(Component)]
struct XpText;
#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct TimerText;
/// The action bar's row of owned skills.
#[derive(Component)]
struct SkillBar;
#[derive(Component)]
struct NoticeText;

/// A node that wears part of a client interface texture once the art is in: `uv` is the part
/// (left, top, right, bottom, 0–1, as the client's own texture coordinates), `flip_x` mirrors it.
#[derive(Component, Clone, Copy)]
pub struct WowSkin {
    pub path: &'static str,
    pub uv: [f32; 4],
    pub flip_x: bool,
}

impl WowSkin {
    pub const fn new(path: &'static str, uv: [f32; 4]) -> Self {
        Self {
            path,
            uv,
            flip_x: false,
        }
    }

    pub const fn flipped(mut self) -> Self {
        self.flip_x = true;
        self
    }
}

/// A plain stand-in shown only while the texture at this path is missing.
#[derive(Component)]
pub struct Fallback(pub &'static str);

/// The cooldown spells' slots (the bar's 7th–9th buttons), rebuilt when the art arrives.
#[derive(Component)]
struct AbilityBar;
/// A cooldown spell's slot parts: its shade, its countdown, its frame.
#[derive(Component)]
struct AbilityShade(AbilityId);
#[derive(Component)]
struct AbilityTimer(AbilityId);
#[derive(Component)]
struct AbilitySlot(AbilityId);

/// The dark shade over a skill slot while it cools down; `full` is the longest wait seen.
#[derive(Component)]
struct CooldownShade {
    id: SkillId,
    full: f32,
}

const PANEL: Color = Color::srgba(0.05, 0.05, 0.08, 0.75);
const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);

/// The player frame's and the action bar's scale over the client's own pixel sizes.
const FRAME_SCALE: f32 = 1.3;
const BAR_SCALE: f32 = 1.3;

fn fpx(v: f32) -> Val {
    Val::Px(v * FRAME_SCALE)
}

fn bpx(v: f32) -> Val {
    Val::Px(v * BAR_SCALE)
}

/// An absolutely placed box, in frame pixels.
fn at(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: fpx(left),
        top: fpx(top),
        width: fpx(w),
        height: fpx(h),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

/// A status bar: a dark trough with a fill (`marker`) in it.
fn bar_in(p: &mut ChildSpawnerCommands, node: Node, fill: Color, marker: impl Component) {
    p.spawn((node, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65))))
        .with_children(|p| {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(fill),
                marker,
            ));
        });
}

/// The action bar's two client pieces (`UI-MainMenuBar-Dwarf`, 256² each holding a 53-pixel
/// band: the XP strip on top, six button slots under it), in texture coordinates.
const BAR_PIECES: [[f32; 4]; 2] = [
    [0.0, 203.0 / 256.0, 1.0, 1.0],
    [0.0, 139.0 / 256.0, 1.0, 192.0 / 256.0],
];
/// The mage's square in the character-creation class icons.
pub const MAGE_ICON: [f32; 4] = [0.25, 0.0, 0.49609375, 0.25];
/// The player frame: the target frame mirrored, as the client draws it.
const PLAYER_FRAME: [f32; 4] = [0.09375, 0.0, 1.0, 0.78125];

fn spawn_hud(mut commands: Commands, font: Res<UiFont>) {
    let white = TextColor(Color::WHITE);
    let shadow = TextShadow {
        offset: Vec2::splat(1.5),
        color: Color::srgba(0.0, 0.0, 0.0, 0.9),
    };

    // Top-left: the player frame (portrait, name, level, health, XP), 232×100 client pixels.
    commands
        .spawn((
            Name::new("hud player frame"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(4.0),
                left: Val::Px(4.0),
                width: fpx(232.0),
                height: fpx(100.0),
                ..default()
            },
        ))
        .with_children(|p| {
            // Without the frame texture: a plain panel behind the bars.
            p.spawn((
                Node {
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..at(96.0, 16.0, 134.0, 54.0)
                },
                BackgroundColor(PANEL),
                BorderColor::all(GOLD.with_alpha(0.5)),
                Fallback(TARGET_FRAME),
            ));
            p.spawn((
                Node {
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..at(42.0, 12.0, 64.0, 64.0)
                },
                BackgroundColor(Color::srgb(0.25, 0.3, 0.6)),
                WowSkin::new(CLASS_ICONS, MAGE_ICON),
            ));
            bar_in(
                p,
                at(106.0, 41.0, 119.0, 12.0),
                Color::srgb(0.1, 0.85, 0.1),
                HpFill,
            );
            bar_in(
                p,
                at(106.0, 52.0, 119.0, 12.0),
                Color::srgb(0.45, 0.3, 0.95),
                XpFill,
            );
            p.spawn((
                at(0.0, 0.0, 232.0, 100.0),
                WowSkin::new(TARGET_FRAME, PLAYER_FRAME).flipped(),
            ));
            p.spawn(at(106.0, 22.0, 119.0, 17.0)).with_children(|n| {
                n.spawn((
                    Text::new("Büyücü"),
                    font.text(12.0 * FRAME_SCALE),
                    TextColor(GOLD),
                    shadow,
                ));
            });
            p.spawn(at(41.0, 56.0, 24.0, 20.0)).with_children(|n| {
                n.spawn((
                    Text::new("1"),
                    font.text(11.0 * FRAME_SCALE),
                    TextColor(GOLD),
                    shadow,
                    LevelText,
                ));
            });
            p.spawn(at(106.0, 41.0, 119.0, 12.0)).with_children(|n| {
                n.spawn((
                    Text::new(""),
                    font.text(9.5 * FRAME_SCALE),
                    white,
                    shadow,
                    HpText,
                ));
            });
            p.spawn(at(106.0, 52.0, 119.0, 12.0)).with_children(|n| {
                n.spawn((
                    Text::new(""),
                    font.text(9.5 * FRAME_SCALE),
                    white,
                    shadow,
                    XpText,
                ));
            });
        });

    // Top-centre: run timer and kills on a small plate.
    commands
        .spawn((
            Name::new("hud timer"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundGradient::from(LinearGradient::to_bottom(vec![
                    ColorStop::auto(Color::srgba(0.12, 0.1, 0.08, 0.9)),
                    ColorStop::auto(Color::srgba(0.03, 0.03, 0.04, 0.9)),
                ])),
                BorderColor::all(Color::srgb(0.6, 0.48, 0.22)),
            ))
            .with_children(|t| {
                t.spawn((
                    Text::new("00:00"),
                    font.text(26.0),
                    TextColor(GOLD),
                    shadow,
                    TimerText,
                ));
            });
        });

    // Bottom-centre: the action bar, 512×53 client pixels, gryphons on its ends.
    commands
        .spawn((
            Name::new("hud action bar"),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(Node {
                width: bpx(512.0),
                height: bpx(53.0),
                ..default()
            })
            .with_children(|b| {
                let piece = |left: f32, w: f32, top: f32, h: f32| Node {
                    position_type: PositionType::Absolute,
                    left: bpx(left),
                    top: bpx(top),
                    width: bpx(w),
                    height: bpx(h),
                    ..default()
                };
                b.spawn((
                    Node {
                        border_radius: BorderRadius::top(Val::Px(8.0)),
                        ..piece(0.0, 512.0, 0.0, 53.0)
                    },
                    BackgroundColor(PANEL),
                    Fallback(MAIN_BAR),
                ));
                // The XP strip sits in the bars' top band.
                bar_in(
                    b,
                    piece(0.0, 512.0, 1.0, 9.0),
                    Color::srgb(0.45, 0.3, 0.95),
                    XpStrip,
                );
                for (i, uv) in BAR_PIECES.iter().enumerate() {
                    b.spawn((
                        piece(256.0 * i as f32, 256.0, 0.0, 53.0),
                        WowSkin::new(MAIN_BAR, *uv),
                    ));
                }
                b.spawn((
                    Node {
                        left: bpx(-96.0),
                        top: bpx(-75.0),
                        ..piece(0.0, 128.0, 0.0, 128.0)
                    },
                    WowSkin::new(END_CAP, [0.0, 0.0, 1.0, 1.0]),
                ));
                b.spawn((
                    Node {
                        left: bpx(512.0 - 32.0),
                        top: bpx(-75.0),
                        ..piece(0.0, 128.0, 0.0, 128.0)
                    },
                    WowSkin::new(END_CAP, [0.0, 0.0, 1.0, 1.0]).flipped(),
                ));
                // The cooldown spells in the 7th to 9th slots (filled by `update_ability_bar`).
                b.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: bpx(8.0 + 42.0 * 6.0),
                        bottom: bpx(4.0),
                        column_gap: bpx(6.0),
                        ..default()
                    },
                    AbilityBar,
                ));
                // Owned skills in the button slots (filled by `update_skill_bar`).
                b.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: bpx(8.0),
                        bottom: bpx(4.0),
                        column_gap: bpx(6.0),
                        ..default()
                    },
                    SkillBar,
                ));
            });
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

/// Rebuilds the skill bar when the loadout changes (or the art arrives): each owned skill as its
/// WoW icon in a button slot, its level in the corner, a shade while it cools down.
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
    let skinned = art.ui(MAIN_BAR).is_some();
    commands.entity(bar).despawn_related::<Children>();
    commands.entity(bar).with_children(|b| {
        for &(id, level) in &run.loadout.0 {
            let c = id.def().color;
            let color = Color::srgb(c[0], c[1], c[2]);
            let maxed = level >= id.max_level();
            b.spawn((
                Node {
                    width: bpx(36.0),
                    height: bpx(36.0),
                    border: UiRect::all(Val::Px(if skinned { 0.0 } else { 1.0 })),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(color.with_alpha(0.35)),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.9)),
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
                            font.text(24.0),
                            TextColor(Color::WHITE),
                        ));
                    }
                }
                slot.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
                    CooldownShade { id, full: 0.0 },
                ));
                // A maxed skill's level is gold, an evolution's pink.
                let level_color = if id.is_evolved() {
                    Color::srgb(1.0, 0.55, 1.0)
                } else if maxed {
                    GOLD
                } else {
                    Color::WHITE
                };
                slot.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(3.0),
                        bottom: Val::Px(1.0),
                        ..default()
                    },
                    Text::new(level.to_string()),
                    font.text(15.0),
                    TextColor(level_color),
                    TextShadow::default(),
                ));
            });
        }
    });
}

/// Builds the cooldown spells' slots (again when the art arrives): icon, key, shade, countdown.
fn update_ability_bar(
    mut commands: Commands,
    art: Res<WowArt>,
    font: Res<UiFont>,
    bars: Query<Entity, With<AbilityBar>>,
    mut built: Local<bool>,
) {
    let Ok(bar) = bars.single() else {
        return;
    };
    if *built && !art.is_changed() {
        return;
    }
    *built = true;
    commands.entity(bar).despawn_related::<Children>();
    commands.entity(bar).with_children(|b| {
        for id in AbilityId::ALL {
            let d = id.def();
            b.spawn((
                Node {
                    width: bpx(36.0),
                    height: bpx(36.0),
                    border: UiRect::all(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.1, 0.3, 0.6)),
                BorderColor::all(Color::NONE),
                AbilitySlot(id),
            ))
            .with_children(|slot| {
                match art.abilities.get(&id).and_then(|(icon, _)| icon.clone()) {
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
                            Text::new(d.name.chars().next().unwrap_or('?').to_string()),
                            font.text(22.0),
                            TextColor(Color::WHITE),
                        ));
                    }
                }
                slot.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(0.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
                    AbilityShade(id),
                ));
                slot.spawn((
                    Text::new(""),
                    font.text(15.0),
                    TextColor(Color::WHITE),
                    TextShadow::default(),
                    AbilityTimer(id),
                ));
                // The key, top-left, as WoW shows a binding.
                slot.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(3.0),
                        top: Val::Px(1.0),
                        ..default()
                    },
                    Text::new(d.key_label),
                    font.text(12.0),
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    TextShadow::default(),
                ));
            });
        }
    });
}

/// `m:ss` above a minute, whole seconds under.
pub fn countdown(secs: f32) -> String {
    let s = secs.ceil() as u32;
    if s >= 60 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        s.to_string()
    }
}

/// The cooldown spells' slots follow their state: dark and counting down while cooling, grey
/// once spent, a gold frame while running.
#[allow(clippy::type_complexity)]
fn update_ability_slots(
    run: Res<Run>,
    mut shades: Query<(&AbilityShade, &mut Node)>,
    mut timers: Query<(&AbilityTimer, &mut Text)>,
    mut slots: Query<(&AbilitySlot, &mut BorderColor)>,
) {
    let a = &run.abilities;
    for (s, mut node) in &mut shades {
        let f = if a.is_spent(s.0) {
            1.0
        } else {
            s.0.def()
                .cooldown
                .map_or(0.0, |cd| shade_fraction(a.ready_in(s.0), cd))
        };
        node.height = Val::Percent(100.0 * f);
    }
    for (t, mut text) in &mut timers {
        let want = if a.left(t.0) > 0.0 {
            String::new()
        } else if a.is_spent(t.0) {
            "—".to_string()
        } else if a.ready_in(t.0) > 0.0 {
            countdown(a.ready_in(t.0))
        } else {
            String::new()
        };
        if text.0 != want {
            text.0 = want;
        }
    }
    for (s, mut border) in &mut slots {
        *border = BorderColor::all(if a.left(s.0) > 0.0 { GOLD } else { Color::NONE });
    }
}

/// How much of a slot the cooldown shade covers: the wait left over the longest wait seen.
pub fn shade_fraction(left: f32, full: f32) -> f32 {
    if full <= 0.0 {
        0.0
    } else {
        (left / full).clamp(0.0, 1.0)
    }
}

fn update_cooldowns(
    timers: Res<super::skills::SkillTimers>,
    mut shades: Query<(&mut CooldownShade, &mut Node)>,
) {
    for (mut shade, mut node) in &mut shades {
        let left = timers.0.get(&shade.id).copied().unwrap_or(0.0);
        if left > shade.full {
            shade.full = left;
        }
        let f = shade_fraction(left, shade.full);
        if f <= 0.0 {
            shade.full = 0.0;
        }
        node.height = Val::Percent(100.0 * f);
    }
}

/// Once the art is in, the bars wear WoW's status-bar texture, tinted.
#[allow(clippy::type_complexity)]
fn skin_bars(
    mut commands: Commands,
    art: Res<WowArt>,
    bars: Query<
        (Entity, &BackgroundColor),
        (
            Or<(With<HpFill>, With<XpFill>, With<XpStrip>)>,
            Without<ImageNode>,
        ),
    >,
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

/// The client pixels of a texture part.
pub fn uv_rect(size: Vec2, uv: [f32; 4]) -> Rect {
    Rect::new(
        uv[0] * size.x,
        uv[1] * size.y,
        uv[2] * size.x,
        uv[3] * size.y,
    )
}

/// Dresses every [`WowSkin`] node in its texture, and hides the stand-ins of those found.
fn apply_skins(
    mut commands: Commands,
    art: Res<WowArt>,
    images: Res<Assets<Image>>,
    skins: Query<(Entity, &WowSkin), Without<ImageNode>>,
    mut fallbacks: Query<(Ref<Fallback>, &mut Visibility)>,
) {
    for (e, skin) in &skins {
        let Some(tex) = art.ui(skin.path) else {
            continue;
        };
        let Some(img) = images.get(&tex) else {
            continue;
        };
        commands.entity(e).insert((
            ImageNode {
                image: tex.clone(),
                rect: Some(uv_rect(img.size_f32(), skin.uv)),
                flip_x: skin.flip_x,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ));
    }
    for (f, mut vis) in &mut fallbacks {
        // When the art arrives, or a window with stand-ins opens.
        if art.is_changed() || f.is_added() {
            *vis = if art.ui(f.0).is_some() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
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
        Query<&mut Node, Or<(With<XpFill>, With<XpStrip>)>>,
    )>,
    mut texts: ParamSet<(
        Query<&mut Text, With<HpText>>,
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<TimerText>>,
        Query<&mut Text, With<NoticeText>>,
        Query<&mut Text, With<XpText>>,
    )>,
) {
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent(100.0 * health.fraction());
    }
    let need = rules::xp_to_next(run.level);
    for mut n in &mut fills.p1() {
        n.width = Val::Percent(100.0 * (run.xp as f32 / need as f32).min(1.0));
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        **t = format!("{:.0} / {:.0}", health.current.ceil(), health.max);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        **t = run.level.to_string();
    }
    if let Ok(mut t) = texts.p4().single_mut() {
        **t = format!("XP {} / {}", run.xp, need);
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

    #[test]
    fn shades_and_texture_parts() {
        assert_eq!(super::shade_fraction(1.0, 4.0), 0.25);
        assert_eq!(super::shade_fraction(1.0, 0.0), 0.0);
        assert_eq!(super::countdown(299.2), "5:00");
        assert_eq!(super::countdown(42.5), "43");
        let r = super::uv_rect(bevy::math::Vec2::new(256.0, 128.0), super::PLAYER_FRAME);
        assert_eq!((r.min.x, r.max.x, r.max.y), (24.0, 256.0, 100.0));
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
        init(&mut world, apply_skins);
        init(&mut world, update_cooldowns);
        init(&mut world, update_ability_bar);
        init(&mut world, update_ability_slots);
        init(&mut world, ui_on_follow_camera);
        init(&mut world, spawn_hud);
        init(&mut world, load_font);
    }
}
