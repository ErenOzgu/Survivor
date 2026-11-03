//! Sound from the player's own client: spell casts and hits (the visual kits' sounds), enemy
//! death cries (their creature voice), and the zone's music (AreaTable → ZoneMusic). Files are the
//! client's WAV/MP3 in the MPQs, played on a small thread with `rodio`. Without a sound device
//! or WoW data it stays silent. Volumes: `ses=` and `muzik=` (0–1) in the settings file;
//! `RNS_MUTE=1` silences everything.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::{
    load_area_sound_catalog, load_creature_voice_catalog, load_sound_kit_catalog, AreaSoundCatalog,
    CreatureVoiceCatalog, SoundKitCatalog,
};
use benilla_world::terrain_stream::CurrentArea;

use super::enemies::{kill_enemies, Enemy};
use super::fx::FxBudget;
use super::visuals::Placeholders;
use super::CombatSet;

pub const MUTE_VAR: &str = "RNS_MUTE";

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, open_sound.after(benilla_assets::AssetSet::Open))
            .add_systems(
                Update,
                death_cries.in_set(CombatSet::Resolve).before(kill_enemies),
            )
            .add_systems(Update, (play_sounds, zone_music).in_set(CombatSet::Ui));
    }
}

/// Default volumes, when the settings file names none.
const SFX_VOLUME: f32 = 0.6;
const MUSIC_VOLUME: f32 = 0.35;
/// One kit plays at most this often (a hail of missiles is one sound, not twenty).
const KIT_GAP: f32 = 0.09;
/// At most this many sounds start in one frame.
const MAX_PER_FRAME: usize = 6;

/// What the sound thread is told.
enum Cmd {
    Sfx(Bytes, f32),
    /// Tracks to cycle, the silence between them (ms, min..max), volume.
    Music(Vec<Bytes>, [u32; 2], f32),
}

/// A shared file: read once from the archives, played many times.
#[derive(Clone)]
struct Bytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// The sound side: tables, the file cache and the line to the sound thread.
#[derive(Resource)]
pub struct Sound {
    tx: Sender<Cmd>,
    kits: Option<SoundKitCatalog>,
    areas: Option<AreaSoundCatalog>,
    voices: Option<CreatureVoiceCatalog>,
    cache: HashMap<String, Option<Bytes>>,
    last_played: HashMap<u32, f32>,
    sfx_volume: f32,
    music_volume: f32,
    /// The zone music playing.
    music_for: Option<u32>,
    seed: u64,
}

/// Picks one of a kit's weighted variations with `roll` in `0..1`.
pub fn pick_variation(files: &[(String, u32)], roll: f32) -> Option<&str> {
    let total: u32 = files.iter().map(|(_, w)| (*w).max(1)).sum();
    if total == 0 {
        return None;
    }
    let mut r = (roll.clamp(0.0, 0.9999) * total as f32) as u32;
    for (f, w) in files {
        let w = (*w).max(1);
        if r < w {
            return Some(f);
        }
        r -= w;
    }
    files.last().map(|(f, _)| f.as_str())
}

/// Whether a kit may sound again `now`, given when it last did.
pub fn may_play(last: Option<f32>, now: f32) -> bool {
    last.is_none_or(|t| now - t >= KIT_GAP)
}

fn volume_setting(key: &str, default: f32) -> f32 {
    std::fs::read_to_string(crate::settings::settings_path())
        .ok()
        .and_then(|t| crate::settings::setting(&t, key))
        .and_then(|v| v.replace(',', ".").parse::<f32>().ok())
        .map_or(default, |v| v.clamp(0.0, 1.0))
}

fn open_sound(mut commands: Commands, wa: Option<Res<WorldAssets>>) {
    if std::env::var(MUTE_VAR).as_deref() == Ok("1") {
        info!("rnsurvivors: {MUTE_VAR}=1, no sound");
        return;
    }
    let Some(wa) = wa else {
        return;
    };
    let (kits, areas, voices) = {
        let mut chain = wa.chain.lock_recover();
        (
            load_sound_kit_catalog(&mut chain).ok(),
            load_area_sound_catalog(&mut chain).ok(),
            load_creature_voice_catalog(&mut chain).ok(),
        )
    };
    if kits.is_none() {
        warn!("rnsurvivors: SoundEntries unavailable, no sound");
        return;
    }
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("rnsurvivors sound".into())
        .spawn(move || sound_thread(rx))
        .ok();
    commands.insert_resource(Sound {
        tx,
        kits,
        areas,
        voices,
        cache: HashMap::new(),
        last_played: HashMap::new(),
        sfx_volume: volume_setting("ses", SFX_VOLUME),
        music_volume: volume_setting("muzik", MUSIC_VOLUME),
        music_for: None,
        seed: 0x9E37_79B9,
    });
}

impl Sound {
    fn roll(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }

    fn file(&mut self, wa: &WorldAssets, path: &str) -> Option<Bytes> {
        if let Some(c) = self.cache.get(path) {
            return c.clone();
        }
        let bytes = wa.read_file_or_loose(path).map(|b| Bytes(Arc::new(b)));
        if bytes.is_none() {
            debug!("rnsurvivors: sound file {path} not found");
        }
        self.cache.insert(path.to_string(), bytes.clone());
        bytes
    }

    /// Plays a client sound file (`Sound\\…wav`) once, at `volume` (0–1) of the effects level.
    pub fn play_file(&mut self, wa: &WorldAssets, path: &str, volume: f32) {
        if let Some(bytes) = self.file(wa, path) {
            let _ = self.tx.send(Cmd::Sfx(bytes, volume * self.sfx_volume));
        }
    }

    /// Plays one variation of `kit` now, if it did not just play.
    fn play_kit(&mut self, wa: &WorldAssets, kit: u32, now: f32) {
        if !may_play(self.last_played.get(&kit).copied(), now) {
            return;
        }
        let roll = self.roll();
        let Some((path, volume)) = self
            .kits
            .as_ref()
            .and_then(|k| k.get(kit))
            .and_then(|k| pick_variation(&k.files, roll).map(|p| (p.to_string(), k.volume)))
        else {
            return;
        };
        let Some(bytes) = self.file(wa, &path) else {
            return;
        };
        self.last_played.insert(kit, now);
        let _ = self
            .tx
            .send(Cmd::Sfx(bytes, volume.max(0.05) * self.sfx_volume));
    }
}

/// Sounds asked for this frame (spell casts and hits through `FxBudget`).
fn play_sounds(
    sound: Option<ResMut<Sound>>,
    wa: Option<Res<WorldAssets>>,
    mut budget: ResMut<FxBudget>,
    time: Res<Time<Real>>,
) {
    let wanted = budget.take_sounds();
    let (Some(mut sound), Some(wa)) = (sound, wa) else {
        return;
    };
    let now = time.elapsed_secs();
    for kit in wanted.into_iter().take(MAX_PER_FRAME) {
        sound.play_kit(&wa, kit, now);
    }
}

/// Enemies dying this frame cry out (their creature voice's death sound), a few at a time.
fn death_cries(
    sound: Option<ResMut<Sound>>,
    wa: Option<Res<WorldAssets>>,
    assets: Res<Placeholders>,
    time: Res<Time<Real>>,
    enemies: Query<&Enemy>,
) {
    let (Some(mut sound), Some(wa)) = (sound, wa) else {
        return;
    };
    let now = time.elapsed_secs();
    let mut n = 0;
    for e in &enemies {
        if e.alive() || n >= 3 {
            continue;
        }
        let Some(display) = assets.displays.get(&e.kind) else {
            continue;
        };
        let kit = sound
            .voices
            .as_ref()
            .and_then(|v| v.for_display(*display))
            .map(|v| v.death)
            .filter(|k| *k != 0);
        if let Some(kit) = kit {
            sound.play_kit(&wa, kit, now);
            n += 1;
        }
    }
}

/// The zone's music: when the hero's area changes, its ZoneMusic day tracks start cycling.
fn zone_music(
    sound: Option<ResMut<Sound>>,
    wa: Option<Res<WorldAssets>>,
    area: Option<Res<CurrentArea>>,
) {
    let (Some(mut sound), Some(wa), Some(area)) = (sound, wa, area) else {
        return;
    };
    let Some(area_id) = area.0 else {
        return;
    };
    if sound.music_for == Some(area_id) {
        return;
    }
    sound.music_for = Some(area_id);
    let Some((kit_id, silence)) = sound.areas.as_ref().and_then(|a| {
        let audio = a.resolve(area_id)?;
        let zm = audio.music?;
        Some((zm.sounds[0], [zm.silence_min[0], zm.silence_max[0]]))
    }) else {
        return;
    };
    let files: Vec<String> = sound
        .kits
        .as_ref()
        .and_then(|k| k.get(kit_id))
        .map(|k| k.files.iter().map(|(f, _)| f.clone()).collect())
        .unwrap_or_default();
    let tracks: Vec<Bytes> = files.iter().filter_map(|f| sound.file(&wa, f)).collect();
    if tracks.is_empty() {
        return;
    }
    info!(
        "rnsurvivors: zone music for area {area_id}: {} track(s)",
        tracks.len()
    );
    let volume = sound.music_volume;
    let _ = sound.tx.send(Cmd::Music(tracks, silence, volume));
}

/// The sound thread: plays effects as they come and cycles the music with silence between
/// tracks. Without an output device it only drains its commands.
fn sound_thread(rx: Receiver<Cmd>) {
    let Ok((_stream, handle)) = rodio::OutputStream::try_default() else {
        warn!("rnsurvivors: no sound device");
        while rx.recv().is_ok() {}
        return;
    };
    use rodio::Source;
    let mut tracks: Vec<Bytes> = Vec::new();
    let mut silence = [0u32; 2];
    let mut volume = MUSIC_VOLUME;
    let mut music: Option<rodio::Sink> = None;
    let mut next_track: Option<Instant> = None;
    let mut turn = 0usize;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Cmd::Sfx(bytes, v)) => {
                if let Ok(dec) = rodio::Decoder::new(Cursor::new(bytes)) {
                    let _ = handle.play_raw(dec.amplify(v).convert_samples());
                }
            }
            Ok(Cmd::Music(t, s, v)) => {
                tracks = t;
                silence = s;
                volume = v;
                if let Some(m) = music.take() {
                    m.stop();
                }
                next_track = Some(Instant::now());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if music.as_ref().is_some_and(|m| m.empty()) {
            music = None;
            let span = silence[1].saturating_sub(silence[0]).max(1);
            let wait = silence[0] + (turn as u32 * 7919) % span;
            next_track = Some(Instant::now() + Duration::from_millis(wait as u64));
        }
        if music.is_none() && !tracks.is_empty() && next_track.is_some_and(|t| Instant::now() >= t)
        {
            turn = turn.wrapping_add(1);
            let track = tracks[turn % tracks.len()].clone();
            if let (Ok(sink), Ok(dec)) = (
                rodio::Sink::try_new(&handle),
                rodio::Decoder::new(Cursor::new(track)),
            ) {
                sink.set_volume(volume);
                sink.append(dec);
                music = Some(sink);
            }
            next_track = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variations_follow_their_weights() {
        let files = vec![("a.wav".to_string(), 1), ("b.wav".to_string(), 3)];
        assert_eq!(pick_variation(&files, 0.0), Some("a.wav"));
        assert_eq!(pick_variation(&files, 0.3), Some("b.wav"));
        assert_eq!(pick_variation(&files, 1.0), Some("b.wav"));
        assert_eq!(pick_variation(&[], 0.5), None);
        // Zero weights count as one.
        let even = vec![("a".to_string(), 0), ("b".to_string(), 0)];
        assert_eq!(pick_variation(&even, 0.7), Some("b"));
    }

    #[test]
    fn a_kit_does_not_stutter() {
        assert!(may_play(None, 1.0));
        assert!(!may_play(Some(1.0), 1.05));
        assert!(may_play(Some(1.0), 1.2));
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(open_sound).initialize(&mut world);
        IntoSystem::into_system(play_sounds).initialize(&mut world);
        IntoSystem::into_system(death_cries).initialize(&mut world);
        IntoSystem::into_system(zone_music).initialize(&mut world);
    }
}
