//! Health bars over enemies as WoW 1.12 nameplates: the client's `Nameplate-Border` frame
//! (128×32), the red `UI-StatusBar` in its slot (4, 4 from the bottom-left, 103×9), the name
//! above, the level in the frame's round seat on the right, at the client's own proportions.
//! A plate appears once an enemy is hurt, so a fresh wave does not bury the screen in plates.

use bevy::prelude::*;

use super::art::{WowArt, PLATE_BORDER, STATUS_BAR};
use super::bosses::Boss;
use super::enemies::Enemy;
use super::hud::UiFont;
use super::CombatSet;
use super::Run;
use crate::content::EnemyKind;
use crate::game::FollowCam;

pub struct PlatesPlugin;

impl Plugin for PlatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_plates.in_set(CombatSet::Ui));
    }
}

/// The plate in the client's units, and its scale on screen.
const PLATE_W: f32 = 128.0;
const PLATE_H: f32 = 32.0;
const PLATE_SCALE: f32 = 0.72;
/// The health bar's slot, from the plate's bottom-left.
const BAR_X: f32 = 4.0;
const BAR_Y: f32 = 4.0;
const BAR_W: f32 = 103.0;
const BAR_H: f32 = 9.0;
/// The level's seat, from the plate's bottom-right.
const LEVEL_X: f32 = 11.8;
const LEVEL_Y: f32 = 9.1;
/// The hostile bar's red and the level's colour (an even fight is yellow).
const PLATE_RED: Color = Color::srgb(1.0, 0.0, 0.0);
const LEVEL_YELLOW: Color = Color::srgb(1.0, 1.0, 0.0);

fn ppx(v: f32) -> Val {
    Val::Px(v * PLATE_SCALE)
}

/// How far above an enemy's feet its bar floats, yards.
pub fn bar_height(kind: EnemyKind) -> f32 {
    match kind {
        EnemyKind::Kobold | EnemyKind::Geomancer | EnemyKind::Sapper => 2.1,
        EnemyKind::Wolf => 1.7,
        EnemyKind::Spider => 1.6,
        EnemyKind::GnollChief => 3.6,
        EnemyKind::DefiasBoss => 3.6,
    }
}

/// Whether an enemy shows its bar: alive and hurt.
pub fn shows_plate(health: f32, max: f32) -> bool {
    health > 0.0 && health < max
}

/// The bar's fill, percent.
pub fn fill_percent(health: f32, max: f32) -> f32 {
    if max <= 0.0 {
        0.0
    } else {
        (100.0 * health / max).clamp(0.0, 100.0)
    }
}

/// A health bar's root UI node, and the enemy it follows.
#[derive(Component)]
struct Plate {
    owner: Entity,
}

#[derive(Component)]
struct PlateFill;

/// On an enemy that has its bar.
#[derive(Component)]
struct HasPlate;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_plates(
    mut commands: Commands,
    art: Res<WowArt>,
    font: Res<UiFont>,
    run: Res<Run>,
    cams: Query<(&Camera, &GlobalTransform), With<FollowCam>>,
    // Elites and bosses have their own named bar (`boss_hud`).
    enemies: Query<(Entity, &GlobalTransform, &Enemy, Has<HasPlate>), Without<Boss>>,
    mut plates: Query<(Entity, &Plate, &mut Node, &mut Visibility)>,
    mut fills: Query<(&mut Node, &ChildOf), (With<PlateFill>, Without<Plate>)>,
    troughs: Query<&ChildOf, (Without<PlateFill>, Without<Plate>)>,
) {
    // New bars for enemies just hurt.
    for (e, _, enemy, has) in &enemies {
        if has || !shows_plate(enemy.health, enemy.max_health) {
            continue;
        }
        commands.entity(e).insert(HasPlate);
        let fill_img = art.ui(STATUS_BAR);
        let border = art.ui(PLATE_BORDER);
        let shadow = TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        };
        commands
            .spawn((
                Name::new("nameplate"),
                Plate { owner: e },
                // Under the HUD and the minimap.
                GlobalZIndex(-2),
                Node {
                    position_type: PositionType::Absolute,
                    width: ppx(PLATE_W),
                    height: ppx(PLATE_H),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|p| {
                // The bar's trough and fill, under the frame.
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: ppx(BAR_X),
                        top: ppx(PLATE_H - BAR_Y - BAR_H),
                        width: ppx(BAR_W),
                        height: ppx(BAR_H),
                        border: UiRect::all(Val::Px(if border.is_some() { 0.0 } else { 1.0 })),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    BorderColor::all(Color::BLACK),
                ))
                .with_children(|t| {
                    let mut fill = t.spawn((
                        PlateFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                    match &fill_img {
                        Some(img) => {
                            fill.insert(
                                ImageNode::new(img.clone())
                                    .with_color(PLATE_RED)
                                    .with_mode(NodeImageMode::Stretch),
                            );
                        }
                        None => {
                            fill.insert(BackgroundColor(PLATE_RED));
                        }
                    }
                });
                if let Some(img) = &border {
                    p.spawn((
                        ImageNode::new(img.clone()),
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                }
                // The name, over the bar.
                p.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: ppx(0.0),
                    top: ppx(PLATE_H - BAR_Y - BAR_H - 14.0),
                    width: ppx(BAR_W + 2.0 * BAR_X),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|n| {
                    n.spawn((
                        Text::new(enemy.kind.def().name),
                        font.text(12.0 * PLATE_SCALE * 1.1),
                        TextColor(Color::WHITE),
                        shadow,
                    ));
                });
                // The level, in its round seat.
                p.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: ppx(PLATE_W - LEVEL_X - 10.0),
                    top: ppx(PLATE_H - LEVEL_Y - 7.0),
                    width: ppx(20.0),
                    height: ppx(14.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|l| {
                    l.spawn((
                        Text::new(run.level.to_string()),
                        font.text(11.0 * PLATE_SCALE * 1.1),
                        TextColor(LEVEL_YELLOW),
                        shadow,
                    ));
                });
            });
    }

    let cam = cams.single().ok();
    // Each fill's plate (fill → trough → plate), for its health.
    let mut fill_of = std::collections::HashMap::new();
    for (_, child_of) in &fills {
        if let Ok(trough) = troughs.get(child_of.parent()) {
            fill_of.insert(trough.parent(), child_of.parent());
        }
    }
    let mut widths = std::collections::HashMap::new();
    for (pe, plate, mut node, mut vis) in &mut plates {
        let Ok((_, gt, enemy, _)) = enemies.get(plate.owner) else {
            // Dead (now a corpse) or gone.
            commands.entity(pe).try_despawn();
            continue;
        };
        if enemy.health <= 0.0 {
            commands.entity(pe).try_despawn();
            continue;
        }
        let head = gt.translation() + Vec3::Y * bar_height(enemy.kind);
        let screen = cam.and_then(|(c, cgt)| c.world_to_viewport(cgt, head).ok());
        match screen {
            Some(p) => {
                node.left = Val::Px(p.x - PLATE_W * PLATE_SCALE / 2.0);
                node.top = Val::Px(p.y - PLATE_H * PLATE_SCALE);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
        if let Some(trough) = fill_of.get(&pe) {
            widths.insert(*trough, fill_percent(enemy.health, enemy.max_health));
        }
    }
    for (mut f, child_of) in &mut fills {
        if let Some(w) = widths.get(&child_of.parent()) {
            f.width = Val::Percent(*w);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hurt_enemies_show_a_bar() {
        assert!(!shows_plate(10.0, 10.0));
        assert!(shows_plate(9.0, 10.0));
        assert!(!shows_plate(0.0, 10.0));
    }

    #[test]
    fn fill_is_clamped() {
        assert_eq!(fill_percent(5.0, 10.0), 50.0);
        assert_eq!(fill_percent(15.0, 10.0), 100.0);
        assert_eq!(fill_percent(-1.0, 10.0), 0.0);
        assert_eq!(fill_percent(1.0, 0.0), 0.0);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(update_plates).initialize(&mut world);
    }
}
