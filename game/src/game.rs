//! [`GamePlugin`]: the Rnsurvivors game on top of the engine. Phase 1 is the hero walking on the
//! streamed terrain under a following, angled top-down camera.

use bevy::camera::{PerspectiveProjection, Projection};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::view::Hdr;

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::AdtTile;
use benilla_world::schedule::WorldStage;
use benilla_world::terrain_stream::{terrain_height_under, TerrainStreamer};

use crate::config::{self, StartPoint};
use crate::motion;

/// Near plane in yards, as the engine's `nearclip` default.
const NEAR: f32 = benilla_world::view::NEARCLIP_DEFAULT;
/// Vertical field of view (55°): narrower than a free camera, for the top-down framing.
const FOVY: f32 = 0.96;
/// Camera turn speed for Q/E, radians per second.
const TURN_SPEED: f32 = 1.8;

/// The env var overriding the hero's run speed (yards per second).
pub const SPEED_VAR: &str = "RNS_SPEED";

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let speed = std::env::var(SPEED_VAR)
            .ok()
            .and_then(|v| config::parse_positive(&v))
            .unwrap_or(motion::HERO_SPEED);
        let start = StartPoint::from_env();
        info!("rnsurvivors: starting at WoW {:?}", start.wow());
        app.insert_resource(Start(start))
            .insert_resource(HeroSpeed(speed))
            .add_systems(
                Startup,
                (spawn_hero, spawn_camera).after(benilla_assets::AssetSet::Open),
            )
            .add_systems(
                Update,
                (move_hero, follow_camera)
                    .chain()
                    .in_set(WorldStage::Input),
            );
    }
}

/// The run's start point as a resource.
#[derive(Resource, Clone, Copy)]
pub struct Start(pub StartPoint);

/// The hero's run speed in yards per second.
#[derive(Resource, Clone, Copy)]
pub struct HeroSpeed(pub f32);

/// The player's character. Its `Transform` is its feet, in Bevy space.
#[derive(Component, Default)]
pub struct Hero {
    /// Standing on streamed terrain; false until the ground under it is resident.
    pub grounded: bool,
}

/// The follow camera's state.
#[derive(Component)]
pub struct FollowCam {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

fn spawn_hero(
    mut commands: Commands,
    start: Res<Start>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let feet = wow_to_bevy(start.0.wow());
    // Placeholder body: an unlit capsule standing on its feet, and a nose showing where it faces.
    // A real WoW model needs the engine's unit/rig lane, which lives in `benilla-app` (phase 2+).
    let body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.75, 0.2),
        unlit: true,
        ..default()
    });
    let nose = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.1, 0.1),
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            Name::new("hero"),
            Hero::default(),
            Transform::from_translation(feet),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Capsule3d::new(0.45, 1.1))),
                MeshMaterial3d(body),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.25, 0.25, 0.5))),
                MeshMaterial3d(nose),
                Transform::from_xyz(0.0, 1.5, -0.5),
            ));
        });
}

fn spawn_camera(
    mut commands: Commands,
    start: Res<Start>,
    msaa: Res<benilla_world::view::MsaaSetting>,
) {
    let target = wow_to_bevy(start.0.wow()) + Vec3::Y * motion::CAM_LOOK_HEIGHT;
    let cam = FollowCam {
        yaw: 0.0,
        pitch: motion::CAM_PITCH,
        distance: motion::CAM_DISTANCE,
    };
    let eye = target + motion::camera_offset(cam.yaw, cam.pitch, cam.distance);
    // The same camera bundle as the engine's world viewer, so every engine pass finds its camera.
    commands.spawn((
        Name::new("follow camera"),
        Camera3d::default(),
        benilla_world::view::WorldCamera,
        msaa.level(),
        Projection::from(PerspectiveProjection {
            far: benilla_world::view::CAM_FAR,
            near: NEAR,
            fov: FOVY,
            ..default()
        }),
        Hdr,
        Tonemapping::None,
        benilla_world::ffx_glow::FfxGlow::WORLD,
        Transform::from_translation(eye).looking_at(target, Vec3::Y),
        cam,
    ));
}

/// WASD moves the hero relative to the camera; it follows the terrain heightfield.
fn move_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    speed: Res<HeroSpeed>,
    streamer: Option<Res<TerrainStreamer>>,
    adt_tiles: Option<Res<Assets<AdtTile>>>,
    cams: Query<&FollowCam>,
    mut heroes: Query<(&mut Transform, &mut Hero)>,
) {
    let Ok((mut xf, mut hero)) = heroes.single_mut() else {
        return;
    };
    let yaw = cams.single().map(|c| c.yaw).unwrap_or(0.0);

    let mut input = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        input.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        input.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        input.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        input.x -= 1.0;
    }
    let dir = motion::move_dir(input, yaw);
    // No walking off into unstreamed void: hold still until the ground is under the hero.
    if hero.grounded {
        xf.translation = motion::step(xf.translation, dir, speed.0, time.delta_secs());
        if let Some(face) = motion::facing_yaw(dir) {
            xf.rotation = Quat::from_rotation_y(face);
        }
    }

    let ground = match (&streamer, &adt_tiles) {
        (Some(s), Some(a)) => terrain_height_under(s, a, xf.translation),
        _ => None,
    };
    let (y, grounded) = motion::follow_ground(xf.translation.y, ground);
    if grounded && !hero.grounded {
        info!("rnsurvivors: hero on the ground at y={y:.1}");
    }
    xf.translation.y = y;
    hero.grounded = grounded;
}

/// The camera follows the hero from behind and above; the wheel zooms, Q/E turn it.
fn follow_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    heroes: Query<&Transform, (With<Hero>, Without<FollowCam>)>,
    mut cams: Query<(&mut Transform, &mut FollowCam)>,
) {
    let (Ok(hero), Ok((mut xf, mut cam))) = (heroes.single(), cams.single_mut()) else {
        return;
    };
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if lines != 0.0 {
        cam.distance = motion::zoom(cam.distance, lines);
    }
    if keys.pressed(KeyCode::KeyQ) {
        cam.yaw += TURN_SPEED * time.delta_secs();
    }
    if keys.pressed(KeyCode::KeyE) {
        cam.yaw -= TURN_SPEED * time.delta_secs();
    }
    let target = hero.translation + Vec3::Y * motion::CAM_LOOK_HEIGHT;
    xf.translation = target + motion::camera_offset(cam.yaw, cam.pitch, cam.distance);
    xf.look_at(target, Vec3::Y);
}
