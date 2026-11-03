//! [`GamePlugin`]: the Rnsurvivors game on top of the engine. Phase 1 is the hero walking on the
//! streamed terrain under a following, angled top-down camera.

use bevy::camera::{PerspectiveProjection, Projection};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
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
/// Right-mouse look: radians per pixel of mouse motion, and the pitch range (looking down).
const MOUSE_TURN: f32 = 0.006;
const MOUSE_PITCH: f32 = 0.004;
const PITCH_RANGE: (f32, f32) = (-1.4, -0.25);

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
                (move_hero, follow_camera).chain().in_set(WorldStage::Input),
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
    /// The current velocity (yards per second), easing toward the keys' direction.
    pub vel: Vec3,
    /// Where the body faces (yaw, radians), turning smoothly toward the goal.
    pub facing: f32,
    /// Height above the ground in a jump (yards), and its upward speed.
    pub air: f32,
    pub rise: f32,
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
    // The WoW character model (`crate::models`, Troll female mage unless `RNS_HERO_*` says
    // otherwise). The capsule and nose below show until it is ready, and stay if it fails.
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
            crate::models::ModelRequest::Character(crate::models::CharacterLook::hero_from_env()),
            crate::models::AutoLocomotion::default(),
            crate::models::DebugCenter,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Capsule3d::new(0.45, 1.1))),
                MeshMaterial3d(body),
                Transform::from_xyz(0.0, 1.0, 0.0),
                crate::models::ModelPlaceholder,
            ));
            p.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.25, 0.25, 0.5))),
                MeshMaterial3d(nose),
                Transform::from_xyz(0.0, 1.5, -0.5),
                crate::models::ModelPlaceholder,
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

/// WASD moves the hero relative to the camera, easing into each new direction; the body turns
/// smoothly toward where it goes. While the right mouse button is held it faces the camera's
/// forward (as in WoW) unless it is walking another way. It follows the terrain heightfield.
#[allow(clippy::too_many_arguments)]
fn move_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    speed: Res<HeroSpeed>,
    streamer: Option<Res<TerrainStreamer>>,
    adt_tiles: Option<Res<Assets<AdtTile>>>,
    cams: Query<&FollowCam>,
    mut heroes: Query<(
        &mut Transform,
        &mut Hero,
        Option<&mut crate::models::ModelAnim>,
    )>,
) {
    let Ok((mut xf, mut hero, anim)) = heroes.single_mut() else {
        return;
    };
    let yaw = cams.single().map(|c| c.yaw).unwrap_or(0.0);
    let dt = time.delta_secs();
    // Space jumps (from the ground); the body rises and falls back under gravity.
    if dt > 0.0 && hero.grounded && hero.air <= 0.0 && keys.just_pressed(KeyCode::Space) {
        hero.rise = motion::JUMP_SPEED;
    }
    let was_air = hero.air > 0.0 || hero.rise > 0.0;
    (hero.air, hero.rise) = motion::jump_step(hero.air, hero.rise, dt);
    let in_air = hero.air > 0.0 || hero.rise > 0.0;

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
    // Both mouse buttons held: run forward, as in WoW.
    if mouse.pressed(MouseButton::Right) && mouse.pressed(MouseButton::Left) {
        input.y = input.y.max(1.0);
    }
    let dir = motion::move_dir(input, yaw);
    // Back (S) walks backwards, slower, still facing forward.
    let back = motion::backpedals(input);
    let pace = if back {
        speed.0 * motion::BACKPEDAL_MUL
    } else {
        speed.0
    };
    // No walking off into unstreamed void: hold still until the ground is under the hero.
    if hero.grounded {
        hero.vel = motion::approach(hero.vel, dir * pace, motion::HERO_ACCEL, dt);
        xf.translation += hero.vel * dt;
        let goal = if back {
            // Face away from the way it goes (the camera's forward, turned by A/D).
            motion::facing_yaw(-hero.vel).or(Some(yaw))
        } else {
            motion::facing_yaw(hero.vel)
                .filter(|_| hero.vel.length() > 0.5)
                .or_else(|| mouse.pressed(MouseButton::Right).then_some(yaw))
        };
        if let Some(goal) = goal {
            hero.facing = motion::turn_toward(hero.facing, goal, motion::HERO_TURN * dt);
        }
        xf.rotation = Quat::from_rotation_y(hero.facing);
    } else {
        hero.vel = Vec3::ZERO;
    }

    if let Some(mut anim) = anim {
        use crate::models::AnimState;
        let backing = back && hero.vel.length() > 0.5;
        if in_air {
            // The take-off clip once, then the air loop under it.
            if !was_air {
                anim.play(AnimState::JumpStart);
            }
            anim.state = AnimState::Jump;
        } else if was_air {
            anim.state = AnimState::Idle;
            // Landing on the spot bends the knees; running on, the gait picks straight up.
            if hero.vel.length() < 1.5 {
                anim.play(AnimState::JumpLand);
            }
        } else if backing {
            anim.state = AnimState::Backpedal;
        } else if (was_air && anim.state == AnimState::Jump) || anim.state == AnimState::Backpedal {
            anim.state = AnimState::Idle;
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
    // In a jump the feet are that high over the ground.
    xf.translation.y = if grounded { y + hero.air } else { y };
    hero.grounded = grounded;
}

/// The camera follows the hero from behind and above; the wheel zooms, Q/E turn it.
#[allow(clippy::too_many_arguments)]
fn follow_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion_in: Res<AccumulatedMouseMotion>,
    heroes: Query<(&Transform, &Hero), Without<FollowCam>>,
    mut cams: Query<(&mut Transform, &mut FollowCam)>,
    mut cursors: Query<&mut bevy::window::CursorOptions>,
) {
    let (Ok((hero, body)), Ok((mut xf, mut cam))) = (heroes.single(), cams.single_mut()) else {
        return;
    };
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if lines != 0.0 {
        cam.distance = motion::zoom(cam.distance, lines);
    }
    // Right mouse held: the mouse turns the camera (and so the hero), the cursor hides.
    let looking = mouse.pressed(MouseButton::Right);
    if looking {
        cam.yaw -= motion_in.delta.x * MOUSE_TURN;
        cam.pitch =
            (cam.pitch - motion_in.delta.y * MOUSE_PITCH).clamp(PITCH_RANGE.0, PITCH_RANGE.1);
    }
    if mouse.just_pressed(MouseButton::Right) || mouse.just_released(MouseButton::Right) {
        for mut c in &mut cursors {
            c.visible = !looking;
            c.grab_mode = if looking {
                bevy::window::CursorGrabMode::Confined
            } else {
                bevy::window::CursorGrabMode::None
            };
        }
    }
    if keys.pressed(KeyCode::KeyQ) {
        cam.yaw += TURN_SPEED * time.delta_secs();
    }
    if keys.pressed(KeyCode::KeyE) {
        cam.yaw -= TURN_SPEED * time.delta_secs();
    }
    // Not up and down with a jump: the camera keeps to the ground under the hero.
    let target = hero.translation + Vec3::Y * (motion::CAM_LOOK_HEIGHT - body.air);
    xf.translation = target + motion::camera_offset(cam.yaw, cam.pitch, cam.distance);
    xf.look_at(target, Vec3::Y);
}
