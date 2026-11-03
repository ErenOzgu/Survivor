//! What the map boss leaves: a loot chest (a placeholder until phase 4 fills it) and a portal.
//! Stepping into the portal ends the map: the "Harita tamamlandı" screen shows the run's
//! summary and the map starts over (the next map does not exist yet; DESIGN.md leaves "how many
//! maps" open).

use bevy::prelude::*;

use super::boss_rules::{summary_lines, MapSummary};
use super::bosses::{BossAssets, BossNotice, BossRun, BossSet};
use super::cards::CardScreen;
use super::hud::UiFont;
use super::rules::flat_dist;
use super::{hero_on_ground, skill_label, CombatSet, Run};
use crate::content::bosses::ELWYNN_BOSSES;
use crate::game::Hero;

pub struct PortalPlugin;

impl Plugin for PortalPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MapCleared>()
            .init_resource::<ClearScreen>()
            .add_systems(
                Update,
                (open_chest, enter_portal)
                    .in_set(BossSet)
                    .run_if(hero_on_ground),
            )
            .add_systems(Update, spin_portals.in_set(CombatSet::Act))
            .add_systems(Update, close_clear_screen.in_set(CombatSet::Ui));
    }
}

/// Sent when the hero steps into the portal; the run resets on it, like on a death but without
/// the death banner.
#[derive(Message, Debug, Clone, Copy)]
pub struct MapCleared;

/// The boss's loot chest (phase 4 fills it).
#[derive(Component)]
pub struct Chest;

/// The way out of the map.
#[derive(Component)]
pub struct Portal {
    base_y: f32,
    age: f32,
}

/// How close the hero must come to open the chest or use the portal (yards).
pub const TOUCH: f32 = 1.8;

pub fn spawn_chest_and_portal(
    commands: &mut Commands,
    assets: &BossAssets,
    chest_at: Vec3,
    portal_at: Vec3,
) {
    commands.spawn((
        Name::new("loot chest"),
        Chest,
        Mesh3d(assets.chest.0.clone()),
        MeshMaterial3d(assets.chest.1.clone()),
        Transform::from_translation(chest_at + Vec3::Y * 0.4),
    ));
    let y = portal_at.y + 2.0;
    commands
        .spawn((
            Name::new("portal"),
            Portal {
                base_y: y,
                age: 0.0,
            },
            // Upright ring: the torus lies in XZ, so stand it up.
            Transform::from_translation(portal_at.with_y(y))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(assets.portal.0.clone()),
                MeshMaterial3d(assets.portal.1.clone()),
                Transform::default(),
            ));
            p.spawn((
                Mesh3d(assets.disc.clone()),
                MeshMaterial3d(assets.portal_core.clone()),
                Transform::from_scale(Vec3::new(1.25, 0.2, 1.25)),
            ));
        });
}

/// Touching the chest opens it. Phase 4 puts the boss loot here; for now it only says so.
fn open_chest(
    mut commands: Commands,
    mut notice: ResMut<BossNotice>,
    heroes: Query<&Transform, (With<Hero>, Without<Chest>)>,
    chests: Query<(Entity, &Transform), With<Chest>>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    for (e, xf) in &chests {
        if flat_dist(xf.translation, hero.translation) <= TOUCH {
            commands.entity(e).try_despawn();
            notice.show("Ganimet sandığı açıldı. (İçi Aşama 4'te dolacak.)");
        }
    }
}

fn spin_portals(time: Res<Time<Real>>, mut portals: Query<(&mut Portal, &mut Transform)>) {
    for (mut p, mut xf) in &mut portals {
        p.age += time.delta_secs();
        xf.rotation =
            Quat::from_rotation_y(p.age * 0.8) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        xf.translation.y = p.base_y + 0.25 * (p.age * 2.0).sin();
    }
}

/// The open "Harita tamamlandı" screen, if any.
#[derive(Resource, Default)]
pub struct ClearScreen {
    open: Option<Entity>,
}

impl ClearScreen {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// Stepping into the portal: summary screen up, game paused, and the map starts over.
#[allow(clippy::too_many_arguments)]
fn enter_portal(
    mut commands: Commands,
    mut screen: ResMut<ClearScreen>,
    mut clock: ResMut<Time<Virtual>>,
    mut cleared: MessageWriter<MapCleared>,
    run: Res<Run>,
    state: Res<BossRun>,
    cards: Res<CardScreen>,
    font: Res<UiFont>,
    heroes: Query<&Transform, (With<Hero>, Without<Portal>)>,
    portals: Query<&Transform, With<Portal>>,
) {
    if screen.open.is_some() || cards.is_open() {
        return;
    }
    let Ok(hero) = heroes.single() else {
        return;
    };
    if !portals
        .iter()
        .any(|p| flat_dist(p.translation, hero.translation) <= TOUCH)
    {
        return;
    }
    let summary = MapSummary {
        time: run.time,
        level: run.level,
        kills: run.kills,
        bosses: state
            .kill_order
            .iter()
            .map(|&i| ELWYNN_BOSSES[i].name())
            .collect(),
        skills: run
            .loadout
            .0
            .iter()
            .map(|(id, l)| skill_label(*id, *l))
            .collect(),
    };
    info!("rnsurvivors: map cleared — {summary:?}");
    cleared.write(MapCleared);
    clock.pause();

    let root = commands
        .spawn((
            Name::new("map cleared"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.05, 0.8)),
            GlobalZIndex(20),
            Button,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Harita tamamlandı!"),
                font.text(48.0),
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));
            p.spawn((
                Text::new("Elwynn Ormanı"),
                font.text(26.0),
                TextColor(Color::srgb(0.75, 0.85, 1.0)),
            ));
            for line in summary_lines(&summary) {
                p.spawn((Text::new(line), font.text(22.0), TextColor(Color::WHITE)));
            }
            p.spawn((
                Text::new("Sonraki harita henüz yok: harita baştan başlıyor."),
                font.text(20.0),
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
            p.spawn((
                Text::new("Devam etmek için Enter / Boşluk ya da tıkla"),
                font.text(20.0),
                TextColor(Color::srgb(0.7, 0.9, 0.7)),
            ));
        })
        .id();
    screen.open = Some(root);
}

fn close_clear_screen(
    mut commands: Commands,
    mut screen: ResMut<ClearScreen>,
    mut clock: ResMut<Time<Virtual>>,
    cards: Res<CardScreen>,
    keys: Res<ButtonInput<KeyCode>>,
    clicks: Query<&Interaction, Changed<Interaction>>,
) {
    let Some(root) = screen.open else {
        return;
    };
    let by_key = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]);
    let by_click = clicks.get(root).is_ok_and(|i| *i == Interaction::Pressed);
    if !by_key && !by_click {
        return;
    }
    commands.entity(root).try_despawn();
    screen.open = None;
    // A level-up screen keeps its own pause.
    if !cards.is_open() {
        clock.unpause();
    }
}

#[cfg(test)]
mod system_params {
    use super::*;
    use bevy::ecs::system::IntoSystem;

    fn init<M>(world: &mut World, s: impl IntoSystem<(), (), M>) {
        let mut s = IntoSystem::into_system(s);
        s.initialize(world);
    }

    /// Each system's parameters are compatible (no conflicting queries); Bevy checks it here.
    #[test]
    fn systems_initialize() {
        let mut world = World::new();
        init(&mut world, open_chest);
        init(&mut world, spin_portals);
        init(&mut world, enter_portal);
        init(&mut world, close_clear_screen);
    }
}
