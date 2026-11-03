//! The main menu: shown when the game starts (and when the pause menu leaves to it), over the
//! streaming world, with the records from the save. 1/Enter plays, 2 opens the inventory, 3 the
//! talents, 4 quits.
//! `RNS_NOMENU=1` (and the autopilot) skip it.

use bevy::app::AppExit;
use bevy::prelude::*;

use super::art::{WowArt, DIALOG_BACKGROUND};
use super::hud::{clock, UiFont};
use super::save::{Save, SaveData};
use super::{CombatSet, RestartRun};

pub const NOMENU_VAR: &str = "RNS_NOMENU";

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        let skip = std::env::var(NOMENU_VAR).as_deref() == Ok("1")
            || std::env::var(super::autopilot::AUTOPLAY_VAR).as_deref() == Ok("1");
        app.insert_resource(MainMenu {
            open: None,
            pending: !skip,
            refresh: false,
        })
        .add_systems(Update, main_menu.in_set(CombatSet::Ui));
    }
}

/// The main menu: open, or waiting to open (at start, or after "Ana menü").
#[derive(Resource, Default)]
pub struct MainMenu {
    open: Option<Entity>,
    pending: bool,
    /// Redraw when it shows again.
    refresh: bool,
}

impl MainMenu {
    pub fn is_open(&self) -> bool {
        self.open.is_some() || self.pending
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuChoice {
    Play,
    Inventory,
    Talents,
    Quit,
}

/// The records panel's lines, in Turkish.
pub fn record_lines(d: &SaveData) -> Vec<String> {
    if d.runs == 0 {
        return vec!["Henüz koşu yok. İlk koşun seni bekliyor!".to_string()];
    }
    vec![
        format!("En uzun koşu: {}", clock(d.best_time)),
        format!("En yüksek level: {}", d.best_level),
        format!("Bir koşuda en çok düşman: {}", d.best_kills),
        format!(
            "Toplam: {} koşu, {} ölüm, {} harita tamamlandı, {} düşman",
            d.runs, d.deaths, d.maps_cleared, d.total_kills
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
pub(super) fn main_menu(
    mut commands: Commands,
    mut menu: ResMut<MainMenu>,
    mut clock_v: ResMut<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    save: Res<Save>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    mut restarted: MessageReader<RestartRun>,
    buttons: Query<(&Interaction, &MenuChoice), Changed<Interaction>>,
    mut exit: MessageWriter<AppExit>,
    mut inv: ResMut<super::gear_ui::Inventory>,
    mut talents: ResMut<super::talent_ui::TalentScreen>,
) {
    if restarted.read().any(|r| r.to_menu) {
        menu.pending = true;
    }
    if menu.pending && menu.open.is_none() {
        menu.pending = false;
        clock_v.pause();
        menu.open = Some(spawn_menu(&mut commands, &save.data, &font, &art));
        return;
    }
    let Some(root) = menu.open else {
        return;
    };
    // A screen opened from here takes the keys until it closes; then the menu shows anew (the
    // records and points may have changed).
    if inv.is_open() || talents.is_open() {
        menu.refresh = true;
        return;
    }
    if menu.refresh {
        menu.refresh = false;
        commands.entity(root).try_despawn();
        menu.open = Some(spawn_menu(&mut commands, &save.data, &font, &art));
        return;
    }
    let by_key = if keys.any_just_pressed([
        KeyCode::Digit1,
        KeyCode::Numpad1,
        KeyCode::Enter,
        KeyCode::NumpadEnter,
        KeyCode::Space,
    ]) {
        Some(MenuChoice::Play)
    } else if keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2, KeyCode::KeyI]) {
        Some(MenuChoice::Inventory)
    } else if keys.any_just_pressed([KeyCode::Digit3, KeyCode::Numpad3, KeyCode::KeyT]) {
        Some(MenuChoice::Talents)
    } else if keys.any_just_pressed([KeyCode::Digit4, KeyCode::Numpad4]) {
        Some(MenuChoice::Quit)
    } else {
        None
    };
    let by_click = buttons
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| *c);
    match by_key.or(by_click) {
        Some(MenuChoice::Play) => {
            commands.entity(root).try_despawn();
            menu.open = None;
            clock_v.unpause();
        }
        Some(MenuChoice::Inventory) => inv.request_open(),
        Some(MenuChoice::Talents) => talents.request_open(),
        Some(MenuChoice::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
    }
}

fn spawn_menu(commands: &mut Commands, save: &SaveData, font: &UiFont, art: &WowArt) -> Entity {
    let gold = Color::srgb(1.0, 0.82, 0.0);
    let mut root = commands.spawn((
        Name::new("main menu"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
        GlobalZIndex(40),
    ));
    root.with_children(|p| {
        let mut panel = p.spawn((
            Node {
                width: Val::Px(560.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(26.0)),
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
            BorderColor::all(gold.with_alpha(0.85)),
        ));
        if let Some(bg) = art.ui(DIALOG_BACKGROUND) {
            panel.insert(ImageNode::new(bg).with_mode(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            }));
        }
        panel.with_children(|p| {
            p.spawn((Text::new("Rnsurvivors"), font.text(52.0), TextColor(gold)));
            p.spawn((
                Text::new("Elwynn Ormanı · Goldshire"),
                font.text(20.0),
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
            p.spawn((
                Text::new(record_lines(save).join("\n")),
                font.text(17.0),
                TextColor(Color::WHITE),
                TextLayout::new_with_justify(Justify::Center),
                Node {
                    margin: UiRect::vertical(Val::Px(8.0)),
                    ..default()
                },
            ));
            let free = save.free_points();
            if free > 0 {
                p.spawn((
                    Text::new(format!("{free} boş talent puanın var!")),
                    font.text(18.0),
                    TextColor(Color::srgb(0.3, 1.0, 0.3)),
                ));
            }
            for (choice, label) in [
                (MenuChoice::Play, "1  Oyna  (Enter)".to_string()),
                (
                    MenuChoice::Inventory,
                    format!(
                        "2  Envanter  ({} eşya)",
                        save.bag.len() + save.equipped.len()
                    ),
                ),
                (
                    MenuChoice::Talents,
                    format!("3  Talentlar  ({free} boş puan)"),
                ),
                (MenuChoice::Quit, "4  Çık".to_string()),
            ] {
                p.spawn((
                    choice,
                    Button,
                    Node {
                        width: Val::Px(340.0),
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.12, 0.16, 0.95)),
                    BorderColor::all(gold.with_alpha(0.5)),
                ))
                .with_children(|b| {
                    b.spawn((Text::new(label), font.text(22.0), TextColor(Color::WHITE)));
                });
            }
            p.spawn((
                Text::new("WASD yürü · Q/E kamera · I envanter · Esc duraklat"),
                font.text(15.0),
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        });
    });
    root.id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::save::Ending;

    #[test]
    fn records_read_well() {
        assert_eq!(record_lines(&SaveData::default()).len(), 1);
        let mut d = SaveData::default();
        d.record(452.0, 17, 610, 0, Ending::Died);
        let lines = record_lines(&d);
        assert_eq!(lines[0], "En uzun koşu: 07:32");
        assert!(lines[3].contains("1 koşu") && lines[3].contains("1 ölüm"));
    }

    #[test]
    fn a_pending_menu_counts_as_open() {
        let m = MainMenu {
            open: None,
            pending: true,
            refresh: false,
        };
        assert!(m.is_open(), "nothing else may open on the first frame");
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(main_menu).initialize(&mut world);
    }
}
