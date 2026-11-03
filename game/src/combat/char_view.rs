//! The character window's (and main menu's) live figure: while one is open a small camera stands in front
//! of the hero and films it into an image the window shows (the engine supports such off-screen
//! cameras; they are not the world camera).

use bevy::asset::RenderAssetUsages;
use bevy::camera::{PerspectiveProjection, Projection, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::{Hdr, Msaa};

use super::gear_ui::Inventory;
use super::CombatSet;
use crate::game::Hero;

pub struct CharViewPlugin;

impl Plugin for CharViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_target)
            .add_systems(Update, film_hero.in_set(CombatSet::Ui));
    }
}

/// The figure's image size (pixels): the window's model area at twice its size.
pub const VIEW_W: u32 = 466;
pub const VIEW_H: u32 = 430;

/// The image the camera films into, and the camera while the window is open.
#[derive(Resource)]
pub struct CharView {
    pub image: Handle<Image>,
    cam: Option<Entity>,
}

fn make_target(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: VIEW_W,
            height: VIEW_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[10, 9, 8, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    commands.insert_resource(CharView {
        image: images.add(image),
        cam: None,
    });
}

/// Where the camera stands for a hero at `feet` facing `yaw`: in front of it, a little above,
/// looking at its chest.
pub fn view_from(feet: Vec3, yaw: f32) -> Transform {
    let forward = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    let target = feet + Vec3::Y * 1.05;
    let eye = target + forward * 3.6 + Vec3::Y * 0.25;
    Transform::from_translation(eye).looking_at(target, Vec3::Y)
}

fn film_hero(
    mut commands: Commands,
    inv: Res<Inventory>,
    menu: Res<super::menu::MainMenu>,
    view: Option<ResMut<CharView>>,
    heroes: Query<&Transform, (With<Hero>, Without<FilmCam>)>,
    mut cams: Query<&mut Transform, With<FilmCam>>,
) {
    let Some(mut view) = view else {
        return;
    };
    let hero = heroes.single().ok();
    // The character window and the main menu show the figure.
    match (inv.is_open() || menu.is_open(), view.cam, hero) {
        (true, None, Some(h)) => {
            let (yaw, _, _) = h.rotation.to_euler(EulerRot::YXZ);
            let cam = commands
                .spawn((
                    Name::new("character window camera"),
                    FilmCam,
                    Camera3d::default(),
                    Camera {
                        // Before the world camera.
                        order: -10,
                        clear_color: ClearColorConfig::Custom(Color::srgb(0.04, 0.035, 0.03)),
                        ..default()
                    },
                    RenderTarget::Image(view.image.clone().into()),
                    Projection::from(PerspectiveProjection {
                        fov: 0.62,
                        near: 0.1,
                        far: 60.0,
                        aspect_ratio: VIEW_W as f32 / VIEW_H as f32,
                        ..default()
                    }),
                    Hdr,
                    Tonemapping::None,
                    Msaa::Off,
                    view_from(h.translation, yaw),
                ))
                .id();
            view.cam = Some(cam);
        }
        (true, Some(_), Some(h)) => {
            let (yaw, _, _) = h.rotation.to_euler(EulerRot::YXZ);
            for mut xf in &mut cams {
                *xf = view_from(h.translation, yaw);
            }
        }
        (false, Some(cam), _) | (true, Some(cam), None) => {
            commands.entity(cam).try_despawn();
            view.cam = None;
        }
        _ => {}
    }
}

/// The character window's camera.
#[derive(Component)]
struct FilmCam;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_faces_the_hero() {
        let xf = view_from(Vec3::ZERO, 0.0);
        // In front (the hero faces -Z), looking back at it.
        assert!(xf.translation.z < 0.0);
        assert!(xf.forward().z > 0.5);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = World::new();
        IntoSystem::into_system(film_hero).initialize(&mut world);
    }
}
