//! The `rnsurvivors` launcher: boots the benilla world engine with no server, login or UI and
//! adds [`game::GamePlugin`]. The boot sequence follows the engine's world viewer
//! (`benilla-world/src/worldview.rs`, MIT OR Apache-2.0), minus its free-fly camera and its
//! check/screenshot instruments.

mod combat;
mod config;
mod content;
mod game;
mod motion;

use bevy::prelude::*;

use benilla_world::build_id::BuildId;

fn main() -> AppExit {
    // A dev build also looks for `<repo>/WoW/Data` (a link to an install, git-ignored).
    benilla_formats::set_project_folder(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    let mut app = App::new();
    app.insert_resource(BuildId {
        version: env!("CARGO_PKG_VERSION"),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        ..Default::default()
    });

    // `RNS_SURVEY=1`: an engine system whose parameters are unmet warns instead of panicking, so
    // one run names every missing resource.
    if std::env::var("RNS_SURVEY").as_deref() == Ok("1") {
        warn!("rnsurvivors: SURVEY mode — unmet dependencies are warnings, not panics");
        app.set_error_handler(bevy::ecs::error::warn);
    }

    // The `mpq://` source must be registered before `AssetPlugin` (in `DefaultPlugins`) builds.
    match benilla_formats::wow_data() {
        Some(data_dir) => {
            if let Err(e) = benilla_assets::register_mpq_source(&mut app, &data_dir) {
                eprintln!("rnsurvivors: mpq:// source unavailable ({e:#})");
            }
        }
        None => eprintln!(
            "rnsurvivors: no WoW 1.12.1 Data folder found — set WOW_DATA. Looked in {:?}",
            benilla_formats::candidates()
        ),
    }

    let background = benilla_world::bgwin::background_run();
    app.add_plugins(benilla_world::boot::tuned_default_plugins(Window {
        title: "Rnsurvivors".into(),
        resolution: std::env::var("WOW_WIN")
            .ok()
            .and_then(|v| {
                let (w, h) = v.split_once('x')?;
                Some(UVec2::new(w.parse().ok()?, h.parse().ok()?))
            })
            .unwrap_or(UVec2::new(1600, 900))
            .into(),
        present_mode: if std::env::var("WOW_NOVSYNC").as_deref() == Ok("1") {
            bevy::window::PresentMode::AutoNoVsync
        } else {
            bevy::window::PresentMode::default()
        },
        focused: !background,
        ..default()
    }))
    .add_plugins(benilla_world::thread_qos::ThreadQosPlugin)
    .add_plugins(benilla_world::bgwin::BgWinPlugin)
    .add_plugins(benilla_world::mac_quit::MacQuitPlugin);

    // The engine's whole plugin group, as the client and the world viewer add it.
    app.add_plugins(benilla_world::world_plugins::WorldPlugins);
    // The world is always live: there is no session to open or close it.
    app.insert_resource(benilla_world::schedule::WorldLive(true));

    app.add_plugins(game::GamePlugin);
    app.add_plugins(combat::CombatPlugin);

    // After `AssetPlugin`: the loaders go into the live `AssetServer`.
    benilla_assets::register_asset_loaders(&mut app);

    app.run()
}
