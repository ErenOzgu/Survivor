//! How the world looks around the fight: a soft shadow under the hero and every enemy, the time
//! of day (a run starts in the afternoon and the sun sets as the bosses come), and the camera's
//! framing. Settings file: `saat=akis` (the default), `saat=gunduz` or a fixed `saat=18:30`;
//! `kamera=yakin|orta|uzak`.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use benilla_world::lighting::WorldTime;

use super::enemies::Enemy;
use super::{CombatSet, Run};
use crate::game::{FollowCam, Hero};

pub struct WorldLookPlugin;

impl Plugin for WorldLookPlugin {
    fn build(&self, app: &mut App) {
        let text = std::fs::read_to_string(crate::settings::settings_path()).unwrap_or_default();
        let clock = crate::settings::setting(&text, "saat")
            .and_then(|v| DayClock::parse(&v))
            .unwrap_or_default();
        let framing = crate::settings::setting(&text, "kamera")
            .and_then(|v| Framing::parse(&v))
            .unwrap_or_default();
        app.insert_resource(clock)
            .insert_resource(framing)
            .add_systems(Startup, build_shadow)
            .add_systems(
                Update,
                (frame_camera, tick_day, add_shadows).in_set(CombatSet::Ui),
            );
    }
}

/// The time of day.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub enum DayClock {
    /// From the afternoon to dusk over a run.
    #[default]
    Flowing,
    /// A fixed minute of the day.
    Fixed(f32),
}

/// A run's afternoon start and dusk end (minutes of the day), and how long the dusk takes.
const DAY_START: f32 = 13.0 * 60.0;
const DAY_END: f32 = 20.5 * 60.0;
const DAY_RUN_SECS: f32 = 12.0 * 60.0;

impl DayClock {
    pub fn parse(v: &str) -> Option<Self> {
        let v = v.trim().to_lowercase();
        match v.as_str() {
            "akis" | "akış" => return Some(DayClock::Flowing),
            "gunduz" | "gündüz" => return Some(DayClock::Fixed(12.0 * 60.0)),
            "gece" => return Some(DayClock::Fixed(23.0 * 60.0)),
            _ => {}
        }
        let (h, m) = v.split_once(':')?;
        let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
        (h < 24 && m < 60).then(|| DayClock::Fixed((h * 60 + m) as f32))
    }

    /// The minute of the day at `run_secs` into a run.
    pub fn minute(self, run_secs: f32) -> f32 {
        match self {
            DayClock::Fixed(m) => m,
            DayClock::Flowing => {
                let t = (run_secs / DAY_RUN_SECS).clamp(0.0, 1.0);
                DAY_START + (DAY_END - DAY_START) * t
            }
        }
    }
}

fn tick_day(clock: Res<DayClock>, run: Res<Run>, time: Option<ResMut<WorldTime>>) {
    let Some(mut time) = time else {
        return;
    };
    let m = clock.minute(run.time);
    time.minute_f = m;
    time.minute = m as u32 % 1440;
    time.day = (m / 1440.0) as f64;
    time.live = true;
}

/// The camera's distance and pitch.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Framing {
    Near,
    #[default]
    Middle,
    Far,
}

impl Framing {
    pub fn parse(v: &str) -> Option<Self> {
        match v.trim().to_lowercase().as_str() {
            "yakin" | "yakın" => Some(Framing::Near),
            "orta" => Some(Framing::Middle),
            "uzak" => Some(Framing::Far),
            _ => None,
        }
    }

    /// `(distance in yards, pitch in radians)`: lower and closer shows more of the world, higher
    /// and farther more of the fight.
    pub fn view(self) -> (f32, f32) {
        match self {
            Framing::Near => (20.0, -0.72),
            Framing::Middle => (27.0, -0.86),
            Framing::Far => (36.0, -1.0),
        }
    }
}

fn frame_camera(framing: Res<Framing>, mut cams: Query<&mut FollowCam, Added<FollowCam>>) {
    for mut cam in &mut cams {
        let (distance, pitch) = framing.view();
        cam.distance = distance;
        cam.pitch = pitch;
    }
}

/// The shared shadow: a dark disc that fades out at its rim.
#[derive(Resource)]
struct ShadowLook {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

/// A soft disc texture: black, opaque in the middle, clear at the rim.
pub fn shadow_alpha(r: f32) -> f32 {
    let t = (1.0 - r).clamp(0.0, 1.0);
    (t * t * (3.0 - 2.0 * t)) * 0.55
}

fn build_shadow(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let dx = (x as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / N as f32 * 2.0 - 1.0;
            let a = shadow_alpha((dx * dx + dy * dy).sqrt());
            data.extend_from_slice(&[0, 0, 0, (a * 255.0) as u8]);
        }
    }
    let image = Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(images.add(image)),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
    commands.insert_resource(ShadowLook { mesh, material });
}

/// Puts a shadow under each new hero and enemy, sized to its body.
fn add_shadows(
    mut commands: Commands,
    look: Option<Res<ShadowLook>>,
    heroes: Query<Entity, Added<Hero>>,
    enemies: Query<(Entity, &Enemy), Added<Enemy>>,
) {
    let Some(look) = look else {
        return;
    };
    let mut put = |e: Entity, radius: f32| {
        commands.entity(e).with_child((
            Name::new("shadow"),
            Mesh3d(look.mesh.clone()),
            MeshMaterial3d(look.material.clone()),
            Transform::from_xyz(0.0, 0.06, 0.0).with_scale(Vec3::splat(radius)),
        ));
    };
    for e in &heroes {
        put(e, 0.9);
    }
    for (e, enemy) in &enemies {
        put(e, (enemy.radius * 1.4).max(0.6));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_and_camera_settings() {
        assert_eq!(DayClock::parse("akış"), Some(DayClock::Flowing));
        assert_eq!(DayClock::parse("18:30"), Some(DayClock::Fixed(1110.0)));
        assert_eq!(DayClock::parse("25:00"), None);
        let f = DayClock::Flowing;
        assert_eq!(f.minute(0.0), DAY_START);
        assert_eq!(f.minute(1e6), DAY_END);
        assert!(f.minute(300.0) > DAY_START);
        assert_eq!(Framing::parse("Uzak"), Some(Framing::Far));
        assert!(Framing::Near.view().0 < Framing::Far.view().0);
        assert!(shadow_alpha(0.0) > shadow_alpha(0.7) && shadow_alpha(1.0) == 0.0);
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(tick_day).initialize(&mut world);
        IntoSystem::into_system(frame_camera).initialize(&mut world);
        IntoSystem::into_system(add_shadows).initialize(&mut world);
    }
}
