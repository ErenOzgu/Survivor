//! The minimap, top-right, as the client draws it: the terrain's own minimap tile (the client's
//! `textures\Minimap` art, through `md5translate.trs`) in a circle inside the client's minimap
//! ring, north up, with the hero as a yellow arrow and dots: enemies red, bosses orange (the
//! skull), the chest gold, the portal violet. Without the art it is a dark disc in a gold ring.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::coords::bevy_to_wow;
use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{load_minimap_translate, MinimapTranslate};

use super::art::{WowArt, BOSS_SKULL, MINIMAP_BORDER};
use super::bosses::Boss;
use super::enemies::Enemy;
use super::hud::{Fallback, WowSkin};
use super::portal::{Chest, Portal};
use super::CombatSet;
use crate::game::Hero;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapTiles>()
            .add_systems(
                Startup,
                (
                    open_tiles.after(benilla_assets::AssetSet::Open),
                    spawn_minimap,
                ),
            )
            .add_systems(Update, update_minimap.in_set(CombatSet::Ui));
    }
}

/// The open world's map directory (Eastern Kingdoms).
const MAP_DIR: &str = "Azeroth";
/// Yards per ADT tile, and the pixels of one minimap tile.
const TILE_YARDS: f32 = 533.333_3;
const TILE_PX: f32 = 256.0;
/// Yards from the hero to the circle's edge.
const VIEW_YARDS: f32 = 90.0;
/// The ring (client pixels) and the map circle inside it.
const RING: f32 = 192.0;
const MAP: f32 = 140.0;
const MAP_CENTER: Vec2 = Vec2::new(105.0, 92.0);

/// The tile index and its tile names, loaded once.
#[derive(Resource, Default)]
struct MinimapTiles {
    translate: Option<MinimapTranslate>,
    tiles: HashMap<(u32, u32), Option<Handle<Image>>>,
}

fn open_tiles(wa: Option<Res<WorldAssets>>, mut tiles: ResMut<MinimapTiles>) {
    let Some(wa) = wa else {
        return;
    };
    let mut chain = wa.chain.lock_recover();
    match load_minimap_translate(&mut chain) {
        Ok(t) => tiles.translate = Some(t),
        Err(e) => warn!("rnsurvivors: minimap tiles unavailable ({e:#})"),
    }
}

/// The ADT tile `(x, y)` under a WoW position and where in that tile's 256² minimap art it falls
/// (pixels, x east, y south).
pub fn tile_at(wow: [f32; 3]) -> ((u32, u32), Vec2) {
    let fx = 32.0 - wow[1] / TILE_YARDS;
    let fy = 32.0 - wow[0] / TILE_YARDS;
    let (tx, ty) = (fx.floor().clamp(0.0, 63.0), fy.floor().clamp(0.0, 63.0));
    (
        (tx as u32, ty as u32),
        Vec2::new((fx - tx) * TILE_PX, (fy - ty) * TILE_PX),
    )
}

/// The part of a tile the circle shows around `at` (its pixels), kept inside the tile, and where
/// `at` falls in the circle as a share of its size (0.5 = the centre).
pub fn view_rect(at: Vec2) -> (Rect, Vec2) {
    let half = VIEW_YARDS / TILE_YARDS * TILE_PX;
    let min = (at - Vec2::splat(half)).clamp(Vec2::ZERO, Vec2::splat(TILE_PX - 2.0 * half));
    let rect = Rect::from_corners(min, min + Vec2::splat(2.0 * half));
    (rect, (at - min) / (2.0 * half))
}

/// Where a point `offset` yards from the hero (Bevy x east, z south) shows on the circle, as a
/// share of its size, given the hero's share; `None` beyond the rim.
pub fn dot_share(hero: Vec2, offset: Vec3) -> Option<Vec2> {
    let d = Vec2::new(offset.x, offset.z) / (2.0 * VIEW_YARDS);
    let p = hero + d;
    ((p - Vec2::splat(0.5)).length() <= 0.48).then_some(p)
}

#[derive(Component)]
struct MapImage;
#[derive(Component)]
struct DotLayer;
#[derive(Component)]
struct HeroArrow;

fn spawn_minimap(mut commands: Commands) {
    let px = |v: f32| Val::Px(v);
    commands
        .spawn((
            Name::new("minimap"),
            Node {
                position_type: PositionType::Absolute,
                top: px(2.0),
                right: px(2.0),
                width: px(RING),
                height: px(RING),
                ..default()
            },
        ))
        .with_children(|r| {
            let map = Node {
                position_type: PositionType::Absolute,
                left: px(MAP_CENTER.x - MAP / 2.0),
                top: px(MAP_CENTER.y - MAP / 2.0),
                width: px(MAP),
                height: px(MAP),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            };
            r.spawn((
                map.clone(),
                BackgroundColor(Color::srgba(0.08, 0.12, 0.06, 0.85)),
            ));
            r.spawn((map.clone(), MapImage, Visibility::Hidden));
            r.spawn((
                Node {
                    border: UiRect::all(px(3.0)),
                    ..map.clone()
                },
                BorderColor::all(Color::srgb(0.75, 0.6, 0.3)),
                Fallback(MINIMAP_BORDER),
            ));
            r.spawn((map.clone(), DotLayer));
            r.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    width: px(RING),
                    height: px(RING),
                    ..default()
                },
                WowSkin::new(MINIMAP_BORDER, [0.0, 0.0, 1.0, 1.0]),
            ));
        });
}

/// The arrow's clockwise turn on screen for a hero facing `yaw` (see `motion::facing_yaw`): the
/// square's sharp top-left corner, at -135° on screen, is turned onto the facing.
pub fn arrow_turn(yaw: f32) -> f32 {
    // Facing on the map: x east (right), z south (down).
    let (dx, dz) = (-yaw.sin(), -yaw.cos());
    dz.atan2(dx) + std::f32::consts::FRAC_PI_4 * 3.0
}

/// A dot on the minimap.
fn dot(layer: &mut ChildSpawnerCommands, at: Vec2, size: f32, color: Color, round: bool) {
    layer.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(at.x * MAP - size / 2.0),
            top: Val::Px(at.y * MAP - size / 2.0),
            width: Val::Px(size),
            height: Val::Px(size),
            border_radius: BorderRadius::all(if round {
                Val::Percent(50.0)
            } else {
                Val::Px(1.0)
            }),
            ..default()
        },
        BackgroundColor(color),
    ));
}

/// The most enemy dots drawn.
const MAX_DOTS: usize = 120;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_minimap(
    mut commands: Commands,
    mut tiles: ResMut<MinimapTiles>,
    wa: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    art: Res<WowArt>,
    heroes: Query<&Transform, With<Hero>>,
    enemies: Query<(&Transform, Has<Boss>), (With<Enemy>, Without<Hero>)>,
    chests: Query<&GlobalTransform, With<Chest>>,
    portals: Query<&GlobalTransform, With<Portal>>,
    mut map: Query<(&mut ImageNode, &mut Visibility), With<MapImage>>,
    mut maps_bare: Query<(Entity, &mut Visibility), (With<MapImage>, Without<ImageNode>)>,
    layers: Query<Entity, With<DotLayer>>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    let (tile, at) = tile_at(bevy_to_wow(hero.translation));
    let (rect, mut hero_share) = view_rect(at);

    // The terrain tile under the hero.
    let tiles = &mut *tiles;
    let handle = match (&tiles.translate, wa) {
        (Some(t), Some(mut wa)) => tiles
            .tiles
            .entry(tile)
            .or_insert_with(|| {
                let hash = t.tile(MAP_DIR, tile.0, tile.1)?;
                let stem = hash.strip_suffix(".blp").unwrap_or(hash);
                wa.sprite_texture(&format!("textures\\Minimap\\{stem}"), &mut images)
            })
            .clone(),
        _ => None,
    };
    match handle {
        Some(h) => {
            for (e, mut vis) in &mut maps_bare {
                commands.entity(e).insert(ImageNode {
                    image: h.clone(),
                    rect: Some(rect),
                    ..default()
                });
                *vis = Visibility::Inherited;
            }
            for (mut img, mut vis) in &mut map {
                img.image = h.clone();
                img.rect = Some(rect);
                *vis = Visibility::Inherited;
            }
        }
        None => {
            // No terrain art: nothing to keep inside, the hero stays in the middle.
            hero_share = Vec2::splat(0.5);
            for (_, mut vis) in &mut map {
                *vis = Visibility::Hidden;
            }
        }
    }

    // The dots, redrawn each frame.
    let Ok(layer) = layers.single() else {
        return;
    };
    let skull = art.ui(BOSS_SKULL);
    commands.entity(layer).despawn_related::<Children>();
    commands.entity(layer).with_children(|l| {
        let off = |p: Vec3| p - hero.translation;
        for gt in &portals {
            if let Some(s) = dot_share(hero_share, off(gt.translation())) {
                dot(l, s, 10.0, Color::srgb(0.7, 0.35, 1.0), true);
            }
        }
        for gt in &chests {
            if let Some(s) = dot_share(hero_share, off(gt.translation())) {
                dot(l, s, 8.0, Color::srgb(1.0, 0.82, 0.1), false);
            }
        }
        let mut n = 0;
        for (xf, boss) in &enemies {
            let Some(s) = dot_share(hero_share, off(xf.translation)) else {
                continue;
            };
            if boss {
                match &skull {
                    Some(img) => {
                        l.spawn((
                            ImageNode::new(img.clone()),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(s.x * MAP - 8.0),
                                top: Val::Px(s.y * MAP - 8.0),
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                ..default()
                            },
                        ));
                    }
                    None => dot(l, s, 10.0, Color::srgb(1.0, 0.45, 0.1), true),
                }
            } else if n < MAX_DOTS {
                n += 1;
                dot(l, s, 4.0, Color::srgb(1.0, 0.15, 0.1), true);
            }
        }
        // The hero: a yellow arrow pointing where it faces (north up).
        let (yaw, _, _) = hero.rotation.to_euler(EulerRot::YXZ);
        l.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(hero_share.x * MAP - 5.0),
                top: Val::Px(hero_share.y * MAP - 5.0),
                width: Val::Px(10.0),
                height: Val::Px(10.0),
                border_radius: BorderRadius::new(
                    Val::Px(0.0),
                    Val::Px(5.0),
                    Val::Px(5.0),
                    Val::Px(5.0),
                ),
                ..default()
            },
            BackgroundColor(Color::srgb(1.0, 0.9, 0.2)),
            BorderColor::all(Color::BLACK),
            // The square's sharp corner (top-left) points along the facing.
            UiTransform::from_rotation(Rot2::radians(arrow_turn(yaw))),
            HeroArrow,
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_and_views() {
        // The middle of the map grid is tile (32, 32), its top-left corner.
        let (t, at) = tile_at([0.0, 0.0, 0.0]);
        assert_eq!(t, (32, 32));
        assert!(at.length() < 1e-3);
        // East (smaller WoW y) moves right in the tile; south (smaller x) moves down.
        let (_, a) = tile_at([-100.0, -100.0, 0.0]);
        assert!(a.x > 0.0 && a.y > 0.0);
        // A centred view keeps the hero in the middle; at a tile corner it shifts.
        let (r, s) = view_rect(Vec2::splat(128.0));
        assert!((s - Vec2::splat(0.5)).length() < 1e-4);
        assert!((r.width() - 2.0 * VIEW_YARDS / TILE_YARDS * TILE_PX).abs() < 1e-3);
        let (r, s) = view_rect(Vec2::splat(2.0));
        assert!(r.min.x >= 0.0 && s.x < 0.5);
        // Dots: near ones show, far ones are past the rim.
        assert!(dot_share(Vec2::splat(0.5), Vec3::new(10.0, 0.0, 0.0)).is_some());
        assert!(dot_share(Vec2::splat(0.5), Vec3::new(200.0, 0.0, 0.0)).is_none());
        // Facing north (-z, yaw 0): the corner turns from up-left to straight up (+45°).
        assert!((arrow_turn(0.0) - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(update_minimap).initialize(&mut world);
        IntoSystem::into_system(spawn_minimap).initialize(&mut world);
        IntoSystem::into_system(open_tiles).initialize(&mut world);
    }
}
