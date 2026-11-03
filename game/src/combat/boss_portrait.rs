//! The boss pointer's arrow and 3D portrait. The arrow, beside the "Elit …: n yard" line, turns
//! toward the first target on screen. For an elite, a copy of its model stands in a booth sixty
//! yards under the hero (inside the ground, out of the world camera's sight but near enough
//! to be drawn), and a small camera films its head into a round portrait.

use bevy::asset::RenderAssetUsages;
use bevy::camera::{PerspectiveProjection, Projection, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::{Hdr, Msaa};

use super::visuals::Placeholders;
use super::CombatSet;
use crate::content::bosses::ELWYNN_BOSSES;
use crate::game::Hero;
use crate::models::{ModelAnim, ModelRequest};

pub struct BossPortraitPlugin;

impl Plugin for BossPortraitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PointerAim>()
            .add_systems(Startup, make_booth)
            .add_systems(Update, (run_booth, point_arrow).in_set(CombatSet::Ui));
    }
}

/// Where the pointer's first line aims (screen angle, clockwise from up) and whose it is.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PointerAim {
    pub angle: Option<f32>,
    /// The boss (index into `ELWYNN_BOSSES`) the first line is about, if it is one.
    pub boss: Option<usize>,
}

#[derive(Component)]
pub struct PointerPortrait;
#[derive(Component)]
pub struct PointerArrow;

/// The booth: its image, and the model and camera while a boss is shown.
#[derive(Resource)]
struct Booth {
    image: Handle<Image>,
    showing: Option<(usize, Entity, Entity)>,
}

/// How far under the hero the booth stands (yards).
const BOOTH_DEPTH: f32 = 60.0;
const PORTRAIT_PX: u32 = 160;

fn make_booth(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: PORTRAIT_PX,
            height: PORTRAIT_PX,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[12, 10, 8, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    commands.insert_resource(Booth {
        image: images.add(image),
        showing: None,
    });
}

/// The booth's model spot under `hero`, and its camera: in front of the model's face (models
/// face `-Z`), a little above, looking at the head at `head` yards up.
pub fn booth_places(hero: Vec3, head: f32) -> (Vec3, Transform) {
    let spot = hero - Vec3::Y * BOOTH_DEPTH;
    let target = spot + Vec3::Y * head;
    let eye = target + Vec3::NEG_Z * (1.2 + head * 0.9) + Vec3::Y * 0.15;
    (
        spot,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
    )
}

/// A boss's head height for framing: its body kind's size.
fn head_height(idx: usize) -> f32 {
    let def = ELWYNN_BOSSES[idx];
    1.6 * def.body.def().scale.max(0.5) * def.stats.scale.max(0.5)
}

#[allow(clippy::type_complexity)]
fn run_booth(
    mut commands: Commands,
    aim: Res<PointerAim>,
    booth: Option<ResMut<Booth>>,
    assets: Res<Placeholders>,
    heroes: Query<&Transform, (With<Hero>, Without<BoothPart>)>,
    mut parts: Query<(&BoothPart, &mut Transform), Without<Hero>>,
    mut portraits: Query<(Entity, &mut Visibility, Option<&ImageNode>), With<PointerPortrait>>,
) {
    let Some(mut booth) = booth else {
        return;
    };
    let want = aim.boss.and_then(|idx| {
        let body = ELWYNN_BOSSES[idx].body;
        assets.displays.get(&body).map(|d| (idx, *d))
    });
    let hero = heroes.single().ok().map(|h| h.translation);
    // Change of subject: the old booth goes.
    if booth.showing.map(|s| s.0) != want.map(|w| w.0) {
        if let Some((_, model, cam)) = booth.showing.take() {
            commands.entity(model).try_despawn();
            commands.entity(cam).try_despawn();
        }
        if let (Some((idx, display_id)), Some(hero)) = (want, hero) {
            let (spot, cam_at) = booth_places(hero, head_height(idx));
            let model = commands
                .spawn((
                    Name::new("portrait booth model"),
                    BoothPart::Model,
                    Transform::from_translation(spot),
                    Visibility::default(),
                    ModelRequest::Creature { display_id },
                    ModelAnim::default(),
                ))
                .id();
            let cam = commands
                .spawn((
                    Name::new("portrait booth camera"),
                    BoothPart::Camera,
                    Camera3d::default(),
                    Camera {
                        order: -11,
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.06, 0.05, 0.04)),
                        ..default()
                    },
                    RenderTarget::Image(booth.image.clone().into()),
                    Projection::from(PerspectiveProjection {
                        fov: 0.7,
                        near: 0.1,
                        // Only the booth: nothing past its model.
                        far: 12.0,
                        aspect_ratio: 1.0,
                        ..default()
                    }),
                    Hdr,
                    Tonemapping::None,
                    Msaa::Off,
                    cam_at,
                ))
                .id();
            booth.showing = Some((idx, model, cam));
        }
    }
    // The booth keeps under the hero (so the world around it stays streamed and drawn).
    if let (Some((idx, _, _)), Some(hero)) = (booth.showing, hero) {
        let (spot, cam_at) = booth_places(hero, head_height(idx));
        for (part, mut xf) in &mut parts {
            match part {
                BoothPart::Model => xf.translation = spot,
                BoothPart::Camera => *xf = cam_at,
            }
        }
    }
    for (e, mut vis, img) in &mut portraits {
        let show = booth.showing.is_some();
        *vis = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if show && img.is_none() {
            commands
                .entity(e)
                .insert(ImageNode::new(booth.image.clone()));
        }
    }
}

/// A part of the booth.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BoothPart {
    Model,
    Camera,
}

/// The arrow turns toward the first target (its sharp corner, top-left, points along it).
fn point_arrow(
    aim: Res<PointerAim>,
    mut arrows: Query<(&mut UiTransform, &mut Visibility), With<PointerArrow>>,
) {
    for (mut xf, mut vis) in &mut arrows {
        match aim.angle {
            Some(a) => {
                xf.rotation = Rot2::radians(std::f32::consts::FRAC_PI_4 + a);
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_booth_is_under_the_hero_and_faces_the_model() {
        let (spot, cam) = booth_places(Vec3::new(10.0, 50.0, 3.0), 2.0);
        assert_eq!(spot, Vec3::new(10.0, -10.0, 3.0));
        // In front of the model (its -Z side), looking back at it.
        assert!(cam.translation.z < spot.z);
        assert!(cam.forward().z > 0.5);
        assert!(
            cam.translation.distance(spot) < 12.0,
            "within the far plane"
        );
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = World::new();
        IntoSystem::into_system(run_booth).initialize(&mut world);
        IntoSystem::into_system(point_arrow).initialize(&mut world);
    }
}
