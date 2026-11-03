//! The level-up screen: the game pauses and shows up to three cards; 1/2/3 or a click picks one.
//! Each card carries the spell's own WoW icon on a WoW dialog background (plain colours without
//! data).

use bevy::prelude::*;

use super::art::{WowArt, DIALOG_BACKGROUND};
use super::hero::HeroHealth;
use super::hud::UiFont;
use super::rules::{self, Card, CARDS, HEAL_CARD};
use super::{hero_on_ground, skill_label, CombatRng, CombatSet, Run};

pub struct CardsPlugin;

impl Plugin for CardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CardScreen>().add_systems(
            Update,
            (pick_card, open_cards.run_if(hero_on_ground), hover_cards)
                .chain()
                .in_set(CombatSet::Ui),
        );
    }
}

/// The open card screen, if any: its root UI node and the cards on it.
#[derive(Resource, Default)]
pub struct CardScreen {
    open: Option<(Entity, Vec<Card>)>,
}

impl CardScreen {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The cards on the open screen.
    pub fn cards(&self) -> Option<&[Card]> {
        self.open.as_ref().map(|(_, c)| c.as_slice())
    }
}

/// Which card a button is.
#[derive(Component)]
struct CardButton(usize);

/// A card's title and body, in Turkish.
pub fn card_text(card: Card) -> (String, String) {
    match card {
        Card::New(id) => (id.def().name.to_string(), id.def().desc.to_string()),
        Card::Upgrade(id, to) => (
            skill_label(id, to),
            id.def()
                .level_text
                .get(to as usize - 1)
                .copied()
                .unwrap_or("")
                .to_string(),
        ),
        Card::Evolve(_, into) => (into.def().name.to_string(), into.def().desc.to_string()),
        Card::NewPassive(p) | Card::UpgradePassive(p, _) => {
            (p.def().name.to_string(), p.def().line())
        }
        Card::Heal => (
            "Can iksiri".to_string(),
            format!("Öğrenilecek bir şey kalmadı. {HEAL_CARD:.0} can geri gelir."),
        ),
    }
}

/// The small line above a card's title: what kind of pick it is.
pub fn card_tag(card: Card) -> String {
    match card {
        Card::New(_) => "YENİ SKILL".to_string(),
        Card::Upgrade(_, to) | Card::UpgradePassive(_, to) => {
            format!("SEVİYE {} → {to}", to - 1)
        }
        Card::Evolve(..) => "★ EVRİM ★".to_string(),
        Card::NewPassive(_) => "YENİ PASİF".to_string(),
        Card::Heal => "İKSİR".to_string(),
    }
}

/// Level pips for a card: `(level after the pick, max level)`; none for a heal or an evolution.
pub fn card_pips(card: Card) -> Option<(u8, u8)> {
    match card {
        Card::New(id) => Some((1, id.max_level())),
        Card::Upgrade(id, to) => Some((to, id.max_level())),
        Card::NewPassive(p) => Some((1, p.max_level())),
        Card::UpgradePassive(p, to) => Some((to, p.max_level())),
        Card::Evolve(..) | Card::Heal => None,
    }
}

/// A card's icon from the client, if the art is loaded.
fn card_icon(card: Card, art: &WowArt) -> Option<Handle<Image>> {
    match card {
        Card::New(id) | Card::Upgrade(id, _) | Card::Evolve(_, id) => art.icon(id),
        Card::NewPassive(p) | Card::UpgradePassive(p, _) => art.passive_icon(p),
        Card::Heal => None,
    }
}

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);

fn card_color(card: Card) -> Color {
    let c = match card {
        Card::New(id) | Card::Upgrade(id, _) | Card::Evolve(_, id) => id.def().color,
        Card::NewPassive(p) | Card::UpgradePassive(p, _) => p.def().color,
        Card::Heal => [0.9, 0.2, 0.2],
    };
    Color::srgb(c[0], c[1], c[2])
}

#[allow(clippy::too_many_arguments)]
fn open_cards(
    mut commands: Commands,
    mut screen: ResMut<CardScreen>,
    mut rng: ResMut<CombatRng>,
    mut clock: ResMut<Time<Virtual>>,
    run: Res<Run>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    pause: Res<super::pause::PauseMenu>,
    main_menu: Res<super::menu::MainMenu>,
    (inv, loot): (
        Res<super::gear_ui::Inventory>,
        Res<super::gear_ui::LootScreen>,
    ),
) {
    if screen.is_open()
        || run.pending_cards == 0
        || pause.is_open()
        || main_menu.is_open()
        || inv.is_open()
        || loot.is_open()
    {
        return;
    }
    let cards = rules::roll_cards(&run.loadout, &run.passives, &mut rng.0, CARDS);
    clock.pause();

    let root = commands
        .spawn((
            Name::new("level-up cards"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(10),
        ))
        .with_children(|p| {
            let more = if run.pending_cards > 1 {
                format!("  (+{} seçim daha)", run.pending_cards - 1)
            } else {
                String::new()
            };
            p.spawn((
                Text::new(format!("Seviye atladın! Bir kart seç.{more}")),
                font.text(34.0),
                TextColor(Color::srgb(1.0, 0.85, 0.3)),
            ));
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(20.0),
                ..default()
            })
            .with_children(|row| {
                for (i, card) in cards.iter().enumerate() {
                    spawn_card(row, i, *card, &font, &art);
                }
            });
            p.spawn((
                Text::new("1 / 2 / 3 tuşu ya da tıkla"),
                font.text(18.0),
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
            ));
        })
        .id();
    screen.open = Some((root, cards));
}

/// One card: key number, icon, kind, name, what it does, level pips.
fn spawn_card(row: &mut ChildSpawnerCommands, i: usize, card: Card, font: &UiFont, art: &WowArt) {
    let (title, body) = card_text(card);
    let title = match card {
        Card::Upgrade(id, _) => id.def().name.to_string(),
        _ => title,
    };
    let color = card_color(card);
    let evolution = matches!(card, Card::Evolve(..));
    let mut root = row.spawn((
        CardButton(i),
        Button,
        Node {
            width: Val::Px(260.0),
            min_height: Val::Px(320.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(14.0)),
            border: UiRect::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.06, 0.09, 0.96)),
        BorderColor::all(if evolution {
            GOLD
        } else {
            GOLD.with_alpha(0.7)
        }),
    ));
    if let Some(bg) = art.ui(DIALOG_BACKGROUND) {
        root.insert(ImageNode::new(bg).with_mode(NodeImageMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: 1.0,
        }));
    }
    root.with_children(|c| {
        c.spawn((
            Text::new(format!("{}", i + 1)),
            font.text(18.0),
            TextColor(Color::srgb(0.75, 0.75, 0.75)),
        ));
        // The icon in a gold frame; a coloured square with the initial without data.
        let icon = card_icon(card, art);
        c.spawn((
            Node {
                width: Val::Px(72.0),
                height: Val::Px(72.0),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(color.with_alpha(0.35)),
        ))
        .with_children(|f| match icon {
            Some(img) => {
                f.spawn((
                    ImageNode::new(img),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
            }
            None => {
                f.spawn((
                    Text::new(title.chars().next().unwrap_or('?').to_string()),
                    font.text(40.0),
                    TextColor(Color::WHITE),
                ));
            }
        });
        let tag_color = match card {
            Card::New(_) | Card::NewPassive(_) => Color::srgb(0.3, 1.0, 0.3),
            Card::Evolve(..) => Color::srgb(1.0, 0.5, 1.0),
            _ => GOLD,
        };
        c.spawn((
            Text::new(card_tag(card)),
            font.text(15.0),
            TextColor(tag_color),
        ));
        c.spawn((Text::new(title), font.text(26.0), TextColor(color)));
        c.spawn((
            Text::new(body),
            font.text(17.0),
            TextColor(Color::srgb(0.95, 0.95, 0.9)),
            TextLayout::new_with_justify(Justify::Center),
        ));
        if let Some((lvl, max)) = card_pips(card) {
            c.spawn(Node {
                column_gap: Val::Px(5.0),
                margin: UiRect::top(Val::Auto),
                ..default()
            })
            .with_children(|pips| {
                for n in 1..=max {
                    let fill = if n < lvl {
                        GOLD.with_alpha(0.8)
                    } else if n == lvl {
                        Color::srgb(0.3, 1.0, 0.3)
                    } else {
                        Color::srgba(1.0, 1.0, 1.0, 0.12)
                    };
                    pips.spawn((
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(fill),
                    ));
                }
            });
        }
    });
}

/// A hovered card lights its frame.
#[allow(clippy::type_complexity)]
fn hover_cards(
    mut cards: Query<(&Interaction, &mut BorderColor), (Changed<Interaction>, With<CardButton>)>,
) {
    for (i, mut b) in &mut cards {
        *b = BorderColor::all(match i {
            Interaction::Hovered | Interaction::Pressed => Color::WHITE,
            Interaction::None => GOLD.with_alpha(0.7),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn pick_card(
    mut commands: Commands,
    mut screen: ResMut<CardScreen>,
    mut clock: ResMut<Time<Virtual>>,
    mut run: ResMut<Run>,
    mut health: ResMut<HeroHealth>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &CardButton), Changed<Interaction>>,
) {
    let Some((root, cards)) = &screen.open else {
        return;
    };
    const KEYS: [[KeyCode; 2]; 3] = [
        [KeyCode::Digit1, KeyCode::Numpad1],
        [KeyCode::Digit2, KeyCode::Numpad2],
        [KeyCode::Digit3, KeyCode::Numpad3],
    ];
    let by_key = KEYS
        .iter()
        .take(cards.len())
        .position(|ks| keys.any_just_pressed(*ks));
    let by_click = buttons
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, b)| b.0);
    let Some(card) = by_key.or(by_click).and_then(|i| cards.get(i).copied()) else {
        return;
    };
    match card {
        Card::Heal => health.heal(HEAL_CARD),
        Card::NewPassive(_) | Card::UpgradePassive(..) => run.passives.apply(card),
        _ => run.loadout.apply(card),
    }
    info!("rnsurvivors: picked {card:?}");
    run.pending_cards = run.pending_cards.saturating_sub(1);
    commands.entity(*root).try_despawn();
    screen.open = None;
    clock.unpause();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::SkillId;

    #[test]
    fn every_card_has_text() {
        for s in SkillId::ALL {
            let (t, b) = card_text(Card::New(s));
            assert!(t.contains(s.def().name) && !b.is_empty());
            for l in 2..=s.max_level() {
                let (t, b) = card_text(Card::Upgrade(s, l));
                assert!(t.contains(s.def().name) && !b.is_empty(), "{s:?} {l}");
            }
        }
        assert!(!card_text(Card::Heal).1.is_empty());
        for (b, _, into) in crate::content::skills::EVOLUTIONS {
            let c = Card::Evolve(b, into);
            assert_eq!(card_text(c).0, into.def().name);
            assert!(card_tag(c).contains("EVRİM") && card_pips(c).is_none());
        }
        for p in crate::content::passives::PassiveId::ALL {
            let (t, b) = card_text(Card::NewPassive(p));
            assert!(!t.is_empty() && !b.contains("{}"));
            assert_eq!(
                card_pips(Card::UpgradePassive(p, 2)),
                Some((2, p.max_level()))
            );
        }
    }

    #[test]
    fn tags_and_pips() {
        let s = SkillId::Fireball;
        assert_eq!(card_tag(Card::Upgrade(s, 3)), "SEVİYE 2 → 3");
        assert_eq!(card_pips(Card::Upgrade(s, 3)), Some((3, s.max_level())));
        assert_eq!(card_pips(Card::New(s)), Some((1, s.max_level())));
        assert_eq!(card_pips(Card::Heal), None);
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
        init(&mut world, open_cards);
        init(&mut world, pick_card);
        init(&mut world, hover_cards);
    }
}
