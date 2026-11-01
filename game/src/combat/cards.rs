//! The level-up screen: the game pauses and shows up to three cards; 1/2/3 or a click picks one.

use bevy::prelude::*;

use super::hero::HeroHealth;
use super::hud::UiFont;
use super::rules::{self, Card, CARDS, HEAL_CARD};
use super::{hero_on_ground, skill_label, CombatRng, CombatSet, Run};

pub struct CardsPlugin;

impl Plugin for CardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CardScreen>().add_systems(
            Update,
            (pick_card, open_cards.run_if(hero_on_ground))
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
}

/// Which card a button is.
#[derive(Component)]
struct CardButton(usize);

/// A card's title and body, in Turkish.
pub fn card_text(card: Card) -> (String, String) {
    match card {
        Card::New(id) => (
            format!("YENİ: {}", id.def().name),
            id.def().desc.to_string(),
        ),
        Card::Upgrade(id, to) => (
            skill_label(id, to),
            id.def()
                .level_text
                .get(to as usize - 1)
                .copied()
                .unwrap_or("")
                .to_string(),
        ),
        Card::Heal => (
            "Can iksiri".to_string(),
            format!("Bütün skill'ler en üst seviyede. {HEAL_CARD:.0} can geri gelir."),
        ),
    }
}

fn card_color(card: Card) -> Color {
    let c = match card {
        Card::New(id) | Card::Upgrade(id, _) => id.def().color,
        Card::Heal => [0.9, 0.2, 0.2],
    };
    Color::srgb(c[0], c[1], c[2])
}

fn open_cards(
    mut commands: Commands,
    mut screen: ResMut<CardScreen>,
    mut rng: ResMut<CombatRng>,
    mut clock: ResMut<Time<Virtual>>,
    run: Res<Run>,
    font: Res<UiFont>,
) {
    if screen.is_open() || run.pending_cards == 0 {
        return;
    }
    let cards = rules::roll_cards(&run.loadout, &mut rng.0, CARDS);
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
                    let (title, body) = card_text(*card);
                    row.spawn((
                        CardButton(i),
                        Button,
                        Node {
                            width: Val::Px(280.0),
                            min_height: Val::Px(190.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            padding: UiRect::all(Val::Px(14.0)),
                            border: UiRect::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.08, 0.12, 0.95)),
                        BorderColor::all(card_color(*card)),
                    ))
                    .with_children(|c| {
                        c.spawn((
                            Text::new(format!("{}", i + 1)),
                            font.text(18.0),
                            TextColor(Color::srgb(0.7, 0.7, 0.7)),
                        ));
                        c.spawn((
                            Text::new(title),
                            font.text(24.0),
                            TextColor(card_color(*card)),
                        ));
                        c.spawn((Text::new(body), font.text(18.0), TextColor(Color::WHITE)));
                    });
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
    }
}
