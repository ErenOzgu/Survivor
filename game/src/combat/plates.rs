//! Health bars over enemies, in the style of WoW's nameplates: a red status bar (the client's own
//! `UI-StatusBar` texture) that follows the enemy on screen. A bar appears once an enemy is hurt,
//! so a fresh wave does not bury the screen in bars.

use bevy::prelude::*;

use super::art::{WowArt, STATUS_BAR};
use super::bosses::Boss;
use super::enemies::Enemy;
use super::CombatSet;
use crate::content::EnemyKind;
use crate::game::FollowCam;

pub struct PlatesPlugin;

impl Plugin for PlatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_plates.in_set(CombatSet::Ui));
    }
}

/// Bar size in logical pixels.
const PLATE_W: f32 = 48.0;
const PLATE_H: f32 = 6.0;
const PLATE_RED: Color = Color::srgb(0.85, 0.1, 0.05);

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

#[allow(clippy::type_complexity)]
fn update_plates(
    mut commands: Commands,
    art: Res<WowArt>,
    cams: Query<(&Camera, &GlobalTransform), With<FollowCam>>,
    // Elites and bosses have their own named bar (`boss_hud`).
    enemies: Query<(Entity, &GlobalTransform, &Enemy, Has<HasPlate>), Without<Boss>>,
    mut plates: Query<(Entity, &Plate, &mut Node, &mut Visibility, &Children)>,
    mut fills: Query<&mut Node, (With<PlateFill>, Without<Plate>)>,
) {
    // New bars for enemies just hurt.
    for (e, _, enemy, has) in &enemies {
        if has || !shows_plate(enemy.health, enemy.max_health) {
            continue;
        }
        commands.entity(e).insert(HasPlate);
        let fill_img = art.ui(STATUS_BAR);
        commands
            .spawn((
                Name::new("health bar"),
                Plate { owner: e },
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(PLATE_W),
                    height: Val::Px(PLATE_H),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                BorderColor::all(Color::BLACK),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                let mut fill = p.spawn((
                    PlateFill,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
                match fill_img {
                    Some(img) => {
                        fill.insert(
                            ImageNode::new(img)
                                .with_color(PLATE_RED)
                                .with_mode(NodeImageMode::Stretch),
                        );
                    }
                    None => {
                        fill.insert(BackgroundColor(PLATE_RED));
                    }
                }
            });
    }

    let cam = cams.single().ok();
    for (pe, plate, mut node, mut vis, children) in &mut plates {
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
                node.left = Val::Px(p.x - PLATE_W / 2.0);
                node.top = Val::Px(p.y - PLATE_H);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
        for c in children.iter() {
            if let Ok(mut f) = fills.get_mut(c) {
                f.width = Val::Percent(fill_percent(enemy.health, enemy.max_health));
            }
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
