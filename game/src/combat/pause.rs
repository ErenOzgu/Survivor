//! The pause menu (Esc): the game stops and shows the run so far (skills, passives, what the
//! passives add up to) with four choices: go on, start the map over, main menu, quit.

use bevy::app::AppExit;
use bevy::prelude::*;

use super::art::{WowArt, DIALOG_BACKGROUND};
use super::cards::CardScreen;
use super::hud::{clock, UiFont};
use super::portal::ClearScreen;
use super::rules::{Stats, BASE_CRIT};
use super::{skill_label, CombatSet, RestartRun, Run};

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseMenu>()
            .add_systems(Update, pause_menu.in_set(CombatSet::Ui));
    }
}

/// The open pause menu, if any.
#[derive(Resource, Default)]
pub struct PauseMenu {
    open: Option<Entity>,
}

impl PauseMenu {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

/// A menu choice.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Resume,
    Restart,
    MainMenu,
    Quit,
}

const CHOICES: [(Choice, &str); 4] = [
    (Choice::Resume, "1  Devam et  (Esc)"),
    (Choice::Restart, "2  Haritayı baştan başlat"),
    (Choice::MainMenu, "3  Ana menüye dön"),
    (Choice::Quit, "4  Oyundan çık"),
];

/// What the passives add up to, one Turkish line per changed stat.
pub fn stat_lines(s: &Stats) -> Vec<String> {
    let pct = |f: f32| (f * 100.0).round() as i32;
    let mut out = Vec::new();
    if s.damage_mul > 1.0 {
        out.push(format!("Büyü hasarı +%{}", pct(s.damage_mul - 1.0)));
    }
    if s.cooldown_mul < 1.0 {
        out.push(format!("Bekleme süreleri -%{}", pct(1.0 - s.cooldown_mul)));
    }
    if s.speed_mul > 1.0 {
        out.push(format!("Koşu hızı +%{}", pct(s.speed_mul - 1.0)));
    }
    out.push(format!("En yüksek can {:.0}", s.max_health));
    if s.magnet_mul > 1.0 {
        out.push(format!("XP çekim alanı +%{}", pct(s.magnet_mul - 1.0)));
    }
    let crit = if s.crit_chance > BASE_CRIT {
        " (pasifle)"
    } else {
        ""
    };
    out.push(format!("Kritik şansı %{}{crit}", pct(s.crit_chance)));
    if s.xp_mul > 1.0 {
        out.push(format!("XP +%{}", pct(s.xp_mul - 1.0)));
    }
    out
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn pause_menu(
    mut commands: Commands,
    mut menu: ResMut<PauseMenu>,
    mut clock_v: ResMut<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    cards: Res<CardScreen>,
    clear: Res<ClearScreen>,
    run: Res<Run>,
    save: Res<super::save::Save>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    buttons: Query<(&Interaction, &Choice), Changed<Interaction>>,
    main_menu: Res<super::menu::MainMenu>,
    inv: Res<super::gear_ui::Inventory>,
    (loot, craft): (
        Res<super::gear_ui::LootScreen>,
        Res<super::crafting_ui::CraftScreen>,
    ),
    mut restart: MessageWriter<RestartRun>,
    mut exit: MessageWriter<AppExit>,
) {
    let esc = keys.just_pressed(KeyCode::Escape);
    let Some(root) = menu.open else {
        // Another screen already holds the pause.
        if esc
            && !cards.is_open()
            && !clear.is_open()
            && !main_menu.is_open()
            && !inv.is_open()
            && !loot.is_open()
            && !craft.is_open()
        {
            clock_v.pause();
            menu.open = Some(spawn_menu(
                &mut commands,
                &run,
                &save.data.bonus(),
                &font,
                &art,
            ));
        }
        return;
    };
    let by_key = if esc || keys.any_just_pressed([KeyCode::Digit1, KeyCode::Numpad1]) {
        Some(Choice::Resume)
    } else if keys.any_just_pressed([KeyCode::Digit2, KeyCode::Numpad2]) {
        Some(Choice::Restart)
    } else if keys.any_just_pressed([KeyCode::Digit3, KeyCode::Numpad3]) {
        Some(Choice::MainMenu)
    } else if keys.any_just_pressed([KeyCode::Digit4, KeyCode::Numpad4]) {
        Some(Choice::Quit)
    } else {
        None
    };
    let by_click = buttons
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| *c);
    let Some(choice) = by_key.or(by_click) else {
        return;
    };
    commands.entity(root).try_despawn();
    menu.open = None;
    clock_v.unpause();
    match choice {
        Choice::Resume => {}
        Choice::Restart => {
            info!("rnsurvivors: map restarted from the pause menu");
            restart.write(RestartRun { to_menu: false });
        }
        Choice::MainMenu => {
            restart.write(RestartRun { to_menu: true });
        }
        Choice::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

fn spawn_menu(
    commands: &mut Commands,
    run: &Run,
    bonus: &super::rules::Bonus,
    font: &UiFont,
    art: &WowArt,
) -> Entity {
    let gold = Color::srgb(1.0, 0.82, 0.0);
    let dim = Color::srgb(0.8, 0.8, 0.8);
    let stats = Stats::from_parts(&run.passives, bonus);
    let mut root = commands.spawn((
        Name::new("pause menu"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        GlobalZIndex(30),
    ));
    root.with_children(|p| {
        let mut panel = p.spawn((
            Node {
                width: Val::Px(620.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.97)),
            BorderColor::all(gold.with_alpha(0.8)),
        ));
        if let Some(bg) = art.ui(DIALOG_BACKGROUND) {
            panel.insert(ImageNode::new(bg).with_mode(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            }));
        }
        panel.with_children(|p| {
            p.spawn((Text::new("Duraklatıldı"), font.text(34.0), TextColor(gold)));
            p.spawn((
                Text::new(format!(
                    "{}   ·   Level {}   ·   {} düşman",
                    clock(run.time),
                    run.level,
                    run.kills
                )),
                font.text(18.0),
                TextColor(dim),
            ));
            // Two columns: what the hero carries, and what it adds up to.
            p.spawn(Node {
                column_gap: Val::Px(30.0),
                ..default()
            })
            .with_children(|cols| {
                let skills: Vec<String> = run
                    .loadout
                    .0
                    .iter()
                    .map(|(id, l)| skill_label(*id, *l))
                    .collect();
                let passives: Vec<String> = run
                    .passives
                    .0
                    .iter()
                    .map(|(p, l)| format!("{} {l}/{}", p.def().name, p.max_level()))
                    .collect();
                let left = format!(
                    "Skill'ler\n{}\n\nPasifler\n{}",
                    skills.join("\n"),
                    if passives.is_empty() {
                        "—".to_string()
                    } else {
                        passives.join("\n")
                    }
                );
                cols.spawn((Text::new(left), font.text(18.0), TextColor(Color::WHITE)));
                cols.spawn((
                    Text::new(format!("Güçler\n{}", stat_lines(&stats).join("\n"))),
                    font.text(18.0),
                    TextColor(Color::WHITE),
                ));
            });
            for (choice, label) in CHOICES {
                p.spawn((
                    choice,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.12, 0.16, 0.95)),
                    BorderColor::all(gold.with_alpha(0.5)),
                ))
                .with_children(|b| {
                    b.spawn((Text::new(label), font.text(20.0), TextColor(Color::WHITE)));
                });
            }
        });
    });
    root.id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::rules::Passives;
    use crate::content::passives::PassiveId;

    #[test]
    fn stat_lines_show_what_changed() {
        let base = stat_lines(&Stats::default());
        assert_eq!(base.len(), 2, "{base:?}"); // health and crit always show
        let s = Stats::from_passives(&Passives(vec![
            (PassiveId::Power, 2),
            (PassiveId::Haste, 1),
        ]));
        let lines = stat_lines(&s);
        assert!(lines.contains(&"Büyü hasarı +%20".to_string()), "{lines:?}");
        assert!(
            lines.contains(&"Bekleme süreleri -%7".to_string()),
            "{lines:?}"
        );
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(pause_menu).initialize(&mut world);
    }
}
