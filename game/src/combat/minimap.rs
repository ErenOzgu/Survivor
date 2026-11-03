//! The minimap, top-right, as the client draws it: the terrain's own minimap tile (the client's
//! `textures\Minimap` art, through `md5translate.trs`) in a circle inside the client's minimap
//! ring, north up, with the hero as a yellow arrow and dots: enemies red, bosses orange (the
//! skull), the chest gold, the portal violet. Without the art it is a dark disc in a gold ring.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::coords::bevy_to_wow;
use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{load_minimap_translate, MinimapTranslate};

use super::art::{WowArt, BOSS_SKULL, MINIMAP_BORDER, MINIMAP_ZOOM_IN, MINIMAP_ZOOM_OUT};
use super::bosses::Boss;
use super::enemies::Enemy;
use super::hud::UiFont;
use super::hud::{Fallback, WowSkin};
use super::portal::{Chest, Portal};
use super::CombatSet;
use crate::game::Hero;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MinimapTiles>()
            .add_systems(Startup, open_tiles.after(benilla_assets::AssetSet::Open))
            // After the UI font is in (Startup).
            .add_systems(PostStartup, spawn_minimap)
            .add_systems(
                Update,
                (fit_ring, zoom_and_zone, update_minimap).in_set(CombatSet::Ui),
            );
    }
}

/// The open world's map directory (Eastern Kingdoms).
const MAP_DIR: &str = "Azeroth";
/// Yards per ADT tile, and the pixels of one minimap tile.
const TILE_YARDS: f32 = 533.333_3;
const TILE_PX: f32 = 256.0;
/// Yards from the hero to the circle's edge at each zoom step (the buttons on the ring).
const ZOOMS: [f32; 4] = [140.0, 90.0, 60.0, 40.0];
const DEFAULT_ZOOM: usize = 1;
/// The ring's size on screen (the client draws it 192 pixels), and the map circle's place inside
/// it until the ring texture's hole is measured (centre and radius as shares of the ring).
const RING: f32 = 192.0;
const GUESS_HOLE: (Vec2, f32) = (Vec2::new(0.547, 0.48), 0.365);

/// The tile index and its tile names, loaded once; the zoom; where the ring's hole is.
#[derive(Resource)]
struct MinimapTiles {
    translate: Option<MinimapTranslate>,
    tiles: HashMap<(u32, u32), Option<Handle<Image>>>,
    zoom: usize,
    /// The hole in the ring texture (centre and radius, as shares of its size), once measured.
    hole: Option<(Vec2, f32)>,
}

impl Default for MinimapTiles {
    fn default() -> Self {
        Self {
            translate: None,
            tiles: HashMap::new(),
            zoom: DEFAULT_ZOOM,
            hole: None,
        }
    }
}

impl MinimapTiles {
    fn view_yards(&self) -> f32 {
        ZOOMS[self.zoom.min(ZOOMS.len() - 1)]
    }
}

/// The see-through hole inside a ring texture: the clear pixels that the clear area around the
/// ring does not reach (filled in from the edges). Its centre and radius as shares of the size.
pub fn ring_hole(w: usize, h: usize, alpha: impl Fn(usize, usize) -> u8) -> Option<(Vec2, f32)> {
    let clear = |x: usize, y: usize| alpha(x, y) < 96;
    let mut outside = vec![false; w * h];
    let mut stack = Vec::new();
    for x in 0..w {
        stack.push((x, 0));
        stack.push((x, h - 1));
    }
    for y in 0..h {
        stack.push((0, y));
        stack.push((w - 1, y));
    }
    while let Some((x, y)) = stack.pop() {
        let i = y * w + x;
        if outside[i] || !clear(x, y) {
            continue;
        }
        outside[i] = true;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < w {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < h {
            stack.push((x, y + 1));
        }
    }
    let (mut min, mut max, mut n) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN), 0usize);
    for y in 0..h {
        for x in 0..w {
            if clear(x, y) && !outside[y * w + x] {
                let p = Vec2::new(x as f32, y as f32);
                min = min.min(p);
                max = max.max(p);
                n += 1;
            }
        }
    }
    if n < 16 {
        return None;
    }
    let size = Vec2::new(w as f32, h as f32);
    let center = (min + max + Vec2::ONE) / 2.0 / size;
    let radius = ((max - min + Vec2::ONE) / 2.0 / size).min_element();
    Some((center, radius))
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
pub fn view_rect(at: Vec2, view_yards: f32) -> (Rect, Vec2) {
    let half = view_yards / TILE_YARDS * TILE_PX;
    let min = (at - Vec2::splat(half)).clamp(Vec2::ZERO, Vec2::splat(TILE_PX - 2.0 * half));
    let rect = Rect::from_corners(min, min + Vec2::splat(2.0 * half));
    (rect, (at - min) / (2.0 * half))
}

/// Where a point `offset` yards from the hero (Bevy x east, z south) shows on the circle, as a
/// share of its size, given the hero's share; `None` beyond the rim.
pub fn dot_share(hero: Vec2, offset: Vec3, view_yards: f32) -> Option<Vec2> {
    let d = Vec2::new(offset.x, offset.z) / (2.0 * view_yards);
    let p = hero + d;
    ((p - Vec2::splat(0.5)).length() <= 0.48).then_some(p)
}

#[derive(Component)]
struct MapImage;
/// The nodes sized to the ring's hole.
#[derive(Component)]
struct MapCircle;
/// The zoom buttons (+1 closer, -1 farther) and the zone's name over the ring.
#[derive(Component)]
struct ZoomButton(i32);
#[derive(Component)]
struct ZoneText;
#[derive(Component)]
struct DotLayer;
#[derive(Component)]
struct HeroArrow;

/// A node filling the ring's hole (`hole`: centre and radius as shares of the ring).
fn hole_node(hole: (Vec2, f32)) -> Node {
    let (c, r) = hole;
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px((c.x - r) * RING),
        top: Val::Px((c.y - r) * RING),
        width: Val::Px(2.0 * r * RING),
        height: Val::Px(2.0 * r * RING),
        border_radius: BorderRadius::all(Val::Percent(50.0)),
        ..default()
    }
}

fn spawn_minimap(mut commands: Commands, font: Res<UiFont>) {
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
            let map = hole_node(GUESS_HOLE);
            r.spawn((
                map.clone(),
                MapCircle,
                BackgroundColor(Color::srgba(0.08, 0.12, 0.06, 0.9)),
            ));
            r.spawn((map.clone(), MapCircle, MapImage, Visibility::Hidden));
            r.spawn((
                Node {
                    border: UiRect::all(px(3.0)),
                    ..map.clone()
                },
                MapCircle,
                BorderColor::all(Color::srgb(0.75, 0.6, 0.3)),
                Fallback(MINIMAP_BORDER),
            ));
            r.spawn((map.clone(), MapCircle, DotLayer));
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
            // The zone's name on the ring's top, as the client shows it.
            r.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(20.0),
                top: px(2.0),
                width: px(RING - 30.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|z| {
                z.spawn((
                    Text::new(""),
                    font.text(13.0),
                    TextColor(Color::srgb(1.0, 0.82, 0.0)),
                    TextShadow {
                        offset: Vec2::splat(1.0),
                        color: Color::BLACK,
                    },
                    ZoneText,
                ));
            });
            // Zoom in and out, on the ring's lower right.
            for (dir, path, at) in [
                (1, MINIMAP_ZOOM_IN, Vec2::new(0.82, 0.7)),
                (-1, MINIMAP_ZOOM_OUT, Vec2::new(0.7, 0.84)),
            ] {
                r.spawn((
                    ZoomButton(dir),
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(at.x * RING - 14.0),
                        top: px(at.y * RING - 14.0),
                        width: px(28.0),
                        height: px(28.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.1, 0.08, 0.05, 0.9)),
                    WowSkin::new(path, [0.0, 0.0, 1.0, 1.0]),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(if dir > 0 { "+" } else { "−" }),
                        font.text(16.0),
                        TextColor(Color::srgb(1.0, 0.82, 0.0)),
                        Fallback(path),
                    ));
                });
            }
        });
}

/// Measures the ring texture's hole once it is in, and fits the map circle to it.
fn fit_ring(
    art: Res<WowArt>,
    images: Res<Assets<Image>>,
    mut tiles: ResMut<MinimapTiles>,
    mut circles: Query<&mut Node, With<MapCircle>>,
) {
    if tiles.hole.is_some() {
        return;
    }
    let Some(img) = art.ui(MINIMAP_BORDER).and_then(|h| images.get(&h)) else {
        return;
    };
    let (w, h) = (img.width() as usize, img.height() as usize);
    // The pixels stay on the CPU only until the first upload: measured in the first frames, or
    // the measured 1.12 ring's place is kept.
    let hole = img
        .data
        .as_ref()
        .filter(|d| d.len() >= w * h * 4)
        .and_then(|data| ring_hole(w, h, |x, y| data[(y * w + x) * 4 + 3]))
        .unwrap_or(GUESS_HOLE);
    info!(
        "rnsurvivors: minimap ring hole at {:.3},{:.3} radius {:.3}",
        hole.0.x, hole.0.y, hole.1
    );
    tiles.hole = Some(hole);
    let fit = hole_node(hole);
    for mut n in &mut circles {
        let border = n.border;
        *n = Node {
            border,
            ..fit.clone()
        };
    }
}

/// The zoom buttons, and the zone's name from the area under the hero.
fn zoom_and_zone(
    mut tiles: ResMut<MinimapTiles>,
    buttons: Query<(&Interaction, &ZoomButton), Changed<Interaction>>,
    area: Option<Res<benilla_world::terrain_stream::CurrentArea>>,
    names: Local<Option<Option<benilla_formats::AreaTableCatalog>>>,
    wa: Option<Res<WorldAssets>>,
    mut texts: Query<&mut Text, With<ZoneText>>,
    mut shown: Local<Option<u32>>,
) {
    for (i, b) in &buttons {
        if *i == Interaction::Pressed {
            let z = tiles.zoom as i32 + b.0;
            tiles.zoom = z.clamp(0, ZOOMS.len() as i32 - 1) as usize;
        }
    }
    let mut names = names;
    let Some(id) = area.and_then(|a| a.0) else {
        return;
    };
    if *shown == Some(id) {
        return;
    }
    let catalog = names.get_or_insert_with(|| {
        wa.as_ref().and_then(|wa| {
            let mut chain = wa.chain.lock_recover();
            benilla_formats::load_area_table_catalog(&mut chain).ok()
        })
    });
    let Some(name) = catalog.as_ref().and_then(|c| c.name(id)) else {
        return;
    };
    *shown = Some(id);
    for mut t in &mut texts {
        t.0 = name.to_string();
    }
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
            left: Val::Percent(at.x * 100.0),
            top: Val::Percent(at.y * 100.0),
            margin: UiRect::new(
                Val::Px(-size / 2.0),
                Val::ZERO,
                Val::Px(-size / 2.0),
                Val::ZERO,
            ),
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
    let view = tiles.view_yards();
    let (rect, mut hero_share) = view_rect(at, view);

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
            if let Some(s) = dot_share(hero_share, off(gt.translation()), view) {
                dot(l, s, 10.0, Color::srgb(0.7, 0.35, 1.0), true);
            }
        }
        for gt in &chests {
            if let Some(s) = dot_share(hero_share, off(gt.translation()), view) {
                dot(l, s, 8.0, Color::srgb(1.0, 0.82, 0.1), false);
            }
        }
        let mut n = 0;
        for (xf, boss) in &enemies {
            let Some(s) = dot_share(hero_share, off(xf.translation), view) else {
                continue;
            };
            if boss {
                match &skull {
                    Some(img) => {
                        l.spawn((
                            ImageNode::new(img.clone()),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(s.x * 100.0),
                                top: Val::Percent(s.y * 100.0),
                                margin: UiRect::new(
                                    Val::Px(-8.0),
                                    Val::ZERO,
                                    Val::Px(-8.0),
                                    Val::ZERO,
                                ),
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
                left: Val::Percent(hero_share.x * 100.0),
                top: Val::Percent(hero_share.y * 100.0),
                margin: UiRect::new(Val::Px(-5.0), Val::ZERO, Val::Px(-5.0), Val::ZERO),
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
        let (r, s) = view_rect(Vec2::splat(128.0), 90.0);
        assert!((s - Vec2::splat(0.5)).length() < 1e-4);
        assert!((r.width() - 2.0 * 90.0 / TILE_YARDS * TILE_PX).abs() < 1e-3);
        let (r, s) = view_rect(Vec2::splat(2.0), 90.0);
        assert!(r.min.x >= 0.0 && s.x < 0.5);
        // Dots: near ones show, far ones are past the rim.
        assert!(dot_share(Vec2::splat(0.5), Vec3::new(10.0, 0.0, 0.0), 90.0).is_some());
        assert!(dot_share(Vec2::splat(0.5), Vec3::new(200.0, 0.0, 0.0), 90.0).is_none());
        // A ring 20 px wide around a 24 px clear hole in a 64 px texture.
        let hole = ring_hole(64, 64, |x, y| {
            let d = Vec2::new(x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 30.0).length();
            if (12.0..22.0).contains(&d) {
                255
            } else {
                0
            }
        })
        .unwrap();
        assert!(
            (hole.0 - Vec2::new(0.5, 30.0 / 64.0)).length() < 0.02,
            "{hole:?}"
        );
        assert!((hole.1 - 12.0 / 64.0).abs() < 0.02);
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
        IntoSystem::into_system(fit_ring).initialize(&mut world);
        IntoSystem::into_system(zoom_and_zone).initialize(&mut world);
    }
}
