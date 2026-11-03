//! Live scenes on the level-up cards: while the cards are up, for each skill card a copy of the
//! hero stands in a booth under the world (out of the world camera's sight, near enough to be
//! drawn) casting that skill: its cast animation over and over, the spell's own effect model in
//! front of it, filmed by a small camera into the card's picture. The card screen holds the
//! fight, not the clock, so the scenes play.

use bevy::asset::RenderAssetUsages;
use bevy::camera::{PerspectiveProjection, Projection, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::{Hdr, Msaa};

use super::art::WowArt;
use super::cards::CardScreen;
use super::fx::Burst;
use super::rules::Card;
use super::CombatSet;
use crate::content::SkillId;
use crate::game::Hero;
use crate::models::{AnimState, ModelAnim, ModelRequest, ModelStatus};

pub struct CardScenePlugin;

impl Plugin for CardScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_targets)
            .add_systems(Update, run_scenes.in_set(CombatSet::Ui));
    }
}

pub const SCENE_W: u32 = 420;
pub const SCENE_H: u32 = 300;
/// How far under the hero the booths stand, and how far apart.
const DEPTH: f32 = 70.0;
const APART: f32 = 16.0;
/// One cast's length in the loop (seconds).
const CYCLE: f32 = 1.6;

/// The three cards' pictures, and the booths while the cards are up.
#[derive(Resource)]
pub struct CardScenes {
    images: [Handle<Image>; 3],
    booths: Vec<Booth>,
    built_for: Option<Vec<Card>>,
}

struct Booth {
    skill: SkillId,
    spot: Vec3,
    model: Entity,
    cam: Entity,
    clock: f32,
}

impl CardScenes {
    /// The live picture for card `i`, when the cards can have one (the hero's model is in).
    pub fn image(&self, i: usize) -> Option<Handle<Image>> {
        self.images.get(i).cloned()
    }
}

/// The skill a card shows.
pub fn card_skill(card: Card) -> Option<SkillId> {
    match card {
        Card::New(id) | Card::Upgrade(id, _) | Card::Evolve(_, id) => Some(id),
        _ => None,
    }
}

fn target() -> Image {
    let mut image = Image::new_fill(
        Extent3d {
            width: SCENE_W,
            height: SCENE_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[10, 9, 12, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image
}

fn make_targets(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(CardScenes {
        images: [(); 3].map(|_| images.add(target())),
        booths: Vec::new(),
        built_for: None,
    });
}

/// Booth `i`'s spot under `hero`, and its camera: three-quarters in front of the caster (it
/// faces `-Z`), looking past it toward where the spell goes.
pub fn scene_places(hero: Vec3, i: usize) -> (Vec3, Transform) {
    let spot = hero - Vec3::Y * DEPTH + Vec3::X * (i as f32 - 1.0) * APART;
    let eye = spot + Vec3::new(2.6, 1.9, -2.4);
    let look = spot + Vec3::new(0.0, 1.1, -1.6);
    (
        spot,
        Transform::from_translation(eye).looking_at(look, Vec3::Y),
    )
}

/// The effect a scene shows and where: the missile (or impact) in front of the hands, a ground
/// area in front of the feet, small.
fn scene_effect(art: &WowArt, skill: SkillId, spot: Vec3) -> Option<(String, Vec3, f32)> {
    let fx = art.fx.get(&skill)?;
    if let Some(p) = fx.missile.clone().or_else(|| fx.impact.clone()) {
        return Some((p, spot + Vec3::new(0.0, 1.3, -1.4), 0.8));
    }
    fx.area
        .clone()
        .map(|p| (p, spot + Vec3::new(0.0, 0.05, -3.0), 0.35))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn run_scenes(
    mut commands: Commands,
    time: Res<Time>,
    screen: Res<CardScreen>,
    scenes: Option<ResMut<CardScenes>>,
    art: Res<WowArt>,
    heroes: Query<(&Transform, &ModelRequest, Option<&ModelStatus>), With<Hero>>,
    mut anims: Query<&mut ModelAnim>,
) {
    let Some(mut scenes) = scenes else {
        return;
    };
    let cards: Option<Vec<Card>> = screen.cards().map(|c| c.to_vec());
    if cards != scenes.built_for {
        // The cards changed (or went): the old booths go, new ones for the new cards.
        for b in scenes.booths.drain(..) {
            commands.entity(b.model).try_despawn();
            commands.entity(b.cam).try_despawn();
        }
        scenes.built_for = cards.clone();
        let (Some(cards), Ok((hero, look, status))) = (cards, heroes.single()) else {
            return;
        };
        if !matches!(status, Some(ModelStatus::Ready)) {
            return;
        }
        for (i, card) in cards.iter().enumerate().take(3) {
            let Some(skill) = card_skill(*card) else {
                continue;
            };
            let (spot, cam_at) = scene_places(hero.translation, i);
            let model = commands
                .spawn((
                    Name::new("card scene caster"),
                    Transform::from_translation(spot),
                    Visibility::default(),
                    look.clone(),
                    ModelAnim::default(),
                ))
                .id();
            let cam = commands
                .spawn((
                    Name::new("card scene camera"),
                    Camera3d::default(),
                    Camera {
                        order: -20 + i as isize,
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.03, 0.03, 0.05)),
                        ..default()
                    },
                    RenderTarget::Image(scenes.images[i].clone().into()),
                    Projection::from(PerspectiveProjection {
                        fov: 0.75,
                        near: 0.1,
                        far: 9.0,
                        aspect_ratio: SCENE_W as f32 / SCENE_H as f32,
                        ..default()
                    }),
                    Hdr,
                    Tonemapping::None,
                    Msaa::Off,
                    cam_at,
                ))
                .id();
            scenes.booths.push(Booth {
                skill,
                spot,
                model,
                cam,
                // Start the first cast at once.
                clock: CYCLE,
            });
        }
        return;
    }
    // The loop: every cycle the caster casts again and the spell's effect plays anew.
    let dt = time.delta_secs();
    for b in &mut scenes.booths {
        b.clock += dt;
        if b.clock < CYCLE {
            continue;
        }
        b.clock = 0.0;
        if let Ok(mut a) = anims.get_mut(b.model) {
            a.play(AnimState::Cast);
        }
        if let Some((path, at, scale)) = scene_effect(&art, b.skill, b.spot) {
            commands.spawn((
                Name::new("card scene effect"),
                Burst::lasting(CYCLE + 0.2),
                Transform::from_translation(at),
                Visibility::default(),
                ModelRequest::Effect { path, scale },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn booths_stand_apart_under_the_hero() {
        let hero = Vec3::new(5.0, 40.0, 5.0);
        let (a, cam) = scene_places(hero, 0);
        let (b, _) = scene_places(hero, 1);
        assert!(a.y < hero.y - 50.0);
        assert!(a.distance(b) >= APART);
        // Its camera is close enough to see its own booth, not the next one.
        assert!(cam.translation.distance(a) < 9.0);
        assert!(cam.translation.distance(b) > 9.0);
        assert_eq!(card_skill(Card::Heal), None);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = World::new();
        IntoSystem::into_system(run_scenes).initialize(&mut world);
    }
}
