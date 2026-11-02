//! Phase 3 HUD: the boss bar at the top of the screen, small health bars over every elite and
//! boss, the arrival warnings, boss call-outs, and a pointer to the elite / the portal.

use bevy::prelude::*;

use super::boss_rules::{arrival_banner, screen_direction};
use super::bosses::{guard_spot, Boss, BossNotice, BossRun};
use super::cards::CardScreen;
use super::enemies::Enemy;
use super::hud::UiFont;
use super::portal::Portal;
use super::rules::flat_dist;
use super::{CombatSet, Run};
use crate::content::bosses::{Rank, ELWYNN_BOSSES};
use crate::game::{FollowCam, Hero};

pub struct BossHudPlugin;

impl Plugin for BossHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn_boss_hud).add_systems(
            Update,
            (
                add_world_bars,
                update_world_bars,
                update_boss_bar,
                update_boss_texts,
            )
                .chain()
                .in_set(CombatSet::Ui),
        );
    }
}

#[derive(Component)]
struct BossBar;
#[derive(Component)]
struct BossBarFill;
#[derive(Component)]
struct BossBarName;
#[derive(Component)]
struct BannerText;
#[derive(Component)]
struct CalloutText;
#[derive(Component)]
struct PointerText;

/// A health bar floating over a boss.
#[derive(Component)]
struct WorldBar {
    boss: Entity,
    fill: Entity,
    label: Entity,
}
#[derive(Component)]
struct WorldBarFill;
#[derive(Component)]
struct WorldBarLabel;

const PANEL: Color = Color::srgba(0.05, 0.05, 0.08, 0.8);
const WORLD_BAR_W: f32 = 120.0;

fn rank_color(rank: Rank) -> Color {
    match rank {
        Rank::Elite => Color::srgb(1.0, 0.75, 0.2),
        Rank::Mini => Color::srgb(1.0, 0.5, 0.15),
        Rank::MapBoss => Color::srgb(0.9, 0.12, 0.12),
    }
}

fn spawn_boss_hud(mut commands: Commands, font: Res<UiFont>) {
    // Top: the boss bar, under the XP bar and the timer.
    commands
        .spawn((
            Name::new("hud boss bar"),
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(58.0),
                left: Val::Percent(25.0),
                width: Val::Percent(50.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font.text(22.0),
                TextColor(Color::WHITE),
                BossBarName,
            ));
            p.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(16.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderColor::all(Color::srgba(1.0, 0.85, 0.4, 0.6)),
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.85, 0.1, 0.1)),
                    BossBarFill,
                ));
            });
        });

    // Centre-top: arrival warnings and call-outs (above the death banner's line).
    commands
        .spawn((
            Name::new("hud boss texts"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(18.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font.text(32.0),
                TextColor(Color::srgb(1.0, 0.55, 0.15)),
                BannerText,
            ));
            p.spawn((
                Text::new(""),
                font.text(24.0),
                TextColor(Color::srgb(1.0, 0.9, 0.5)),
                CalloutText,
            ));
        });

    // Top-right: where the elite / the portal is.
    commands
        .spawn((
            Name::new("hud pointer"),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(18.0),
                right: Val::Px(16.0),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                font.text(18.0),
                TextColor(Color::srgb(0.9, 0.9, 0.75)),
                PointerText,
            ));
        });
}

/// Every new boss gets a floating bar.
fn add_world_bars(
    mut commands: Commands,
    font: Res<UiFont>,
    bosses: Query<(Entity, &Boss), Added<Boss>>,
) {
    for (boss, b) in &bosses {
        let mut fill = Entity::PLACEHOLDER;
        let mut label = Entity::PLACEHOLDER;
        let root = commands
            .spawn((
                Name::new("boss world bar"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(WORLD_BAR_W),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|p| {
                label = p
                    .spawn((
                        Text::new(b.def().name()),
                        font.text(15.0),
                        TextColor(rank_color(b.def().rank)),
                        WorldBarLabel,
                    ))
                    .id();
                p.spawn((
                    Node {
                        width: Val::Px(WORLD_BAR_W),
                        height: Val::Px(7.0),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL),
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
                ))
                .with_children(|p| {
                    fill = p
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(rank_color(b.def().rank)),
                            WorldBarFill,
                        ))
                        .id();
                });
            })
            .id();
        commands.entity(root).insert(WorldBar { boss, fill, label });
    }
}

/// Floating bars follow their boss on screen; a bar whose boss is gone goes too.
#[allow(clippy::type_complexity)]
fn update_world_bars(
    mut commands: Commands,
    cams: Query<(&Camera, &GlobalTransform), With<FollowCam>>,
    bosses: Query<(&GlobalTransform, &Enemy, &Boss)>,
    mut roots: Query<(Entity, &WorldBar, &mut Node), Without<WorldBarFill>>,
    mut fills: Query<&mut Node, (With<WorldBarFill>, Without<WorldBar>)>,
    mut labels: Query<&mut Text, With<WorldBarLabel>>,
) {
    let cam = cams.single().ok();
    for (root, bar, mut node) in &mut roots {
        let Ok((xf, e, b)) = bosses.get(bar.boss) else {
            commands.entity(root).try_despawn();
            continue;
        };
        let def = b.def();
        let head = xf.translation() + Vec3::Y * (2.0 * def.stats.scale + 0.6);
        let screen = cam.and_then(|(c, cx)| c.world_to_viewport(cx, head).ok());
        match screen {
            Some(p) if e.alive() => {
                node.display = Display::Flex;
                node.left = Val::Px(p.x - WORLD_BAR_W * 0.5);
                node.top = Val::Px(p.y - 26.0);
            }
            _ => node.display = Display::None,
        }
        if let Ok(mut f) = fills.get_mut(bar.fill) {
            f.width = Val::Percent(100.0 * (e.health / b.max_health).clamp(0.0, 1.0));
        }
        if let Ok(mut t) = labels.get_mut(bar.label) {
            let want = match b.attack.current() {
                Some(a) if b.awake => format!("{} — {}!", def.name(), def.attacks[a].name),
                _ => def.name().to_string(),
            };
            if t.0 != want {
                t.0 = want;
            }
        }
    }
}

/// The top bar shows the most important boss that is fighting: the map boss over a mini boss
/// over the elite.
#[allow(clippy::type_complexity)]
fn update_boss_bar(
    bosses: Query<(&Enemy, &Boss)>,
    mut nodes: ParamSet<(
        Query<&mut Node, With<BossBar>>,
        Query<&mut Node, With<BossBarFill>>,
    )>,
    mut names: Query<(&mut Text, &mut TextColor), With<BossBarName>>,
) {
    let top = bosses
        .iter()
        .filter(|(e, b)| b.awake && e.alive())
        .max_by_key(|(_, b)| b.def().rank);
    if let Ok(mut n) = nodes.p0().single_mut() {
        let want = if top.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if n.display != want {
            n.display = want;
        }
    }
    let Some((e, b)) = top else {
        return;
    };
    let frac = (e.health / b.max_health).clamp(0.0, 1.0);
    if let Ok(mut n) = nodes.p1().single_mut() {
        n.width = Val::Percent(100.0 * frac);
    }
    if let Ok((mut t, mut c)) = names.single_mut() {
        let def = b.def();
        let kind = match def.rank {
            Rank::Elite => "Elit",
            Rank::Mini => "Mini boss",
            Rank::MapBoss => "BOSS",
        };
        let want = format!("{kind}: {}   {:.0}%", def.name(), (frac * 100.0).ceil());
        if t.0 != want {
            t.0 = want;
        }
        c.0 = rank_color(def.rank);
    }
}

/// Arrival warning, call-out, and the pointer to the elite (or, once open, the portal).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_boss_texts(
    real: Res<Time<Real>>,
    run: Res<Run>,
    state: Res<BossRun>,
    mut notice: ResMut<BossNotice>,
    cards: Res<CardScreen>,
    heroes: Query<&Transform, With<Hero>>,
    cams: Query<&FollowCam>,
    portals: Query<&GlobalTransform, With<Portal>>,
    bosses: Query<&Boss>,
    mut texts: ParamSet<(
        Query<&mut Text, With<BannerText>>,
        Query<&mut Text, With<CalloutText>>,
        Query<&mut Text, With<PointerText>>,
    )>,
) {
    let set = |t: &mut Text, want: &str| {
        if t.0 != want {
            t.0 = want.to_string();
        }
    };
    // The card screen covers the middle of the screen: the banner waits behind it.
    let banner = if cards.is_open() {
        String::new()
    } else {
        arrival_banner(run.time, state.start, &ELWYNN_BOSSES).unwrap_or_default()
    };
    if let Ok(mut t) = texts.p0().single_mut() {
        set(&mut t, &banner);
    }
    notice.left = (notice.left - real.delta_secs()).max(0.0);
    if let Ok(mut t) = texts.p1().single_mut() {
        set(&mut t, if notice.left > 0.0 { &notice.text } else { "" });
    }

    let mut pointer = String::new();
    if let (Ok(hero), Ok(cam)) = (heroes.single(), cams.single()) {
        let hero = hero.translation;
        let mut line = |name: &str, at: Vec3| {
            let d = flat_dist(at, hero);
            let dir = screen_direction((at - hero).with_y(0.0), cam.yaw);
            pointer.push_str(&format!("{name}: {d:.0} yard, {dir}\n"));
        };
        for p in &portals {
            line("Portal", p.translation());
        }
        for (idx, def) in ELWYNN_BOSSES.iter().enumerate() {
            let awake = bosses.iter().any(|b| b.idx == idx && b.awake);
            if state.killed[idx] || awake {
                continue;
            }
            if let Some(spot) = guard_spot(def, hero.y) {
                line(&format!("Elit {}", def.name()), spot);
            }
        }
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        set(&mut t, pointer.trim_end());
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
        init(&mut world, spawn_boss_hud);
        init(&mut world, add_world_bars);
        init(&mut world, update_world_bars);
        init(&mut world, update_boss_bar);
        init(&mut world, update_boss_texts);
    }
}
