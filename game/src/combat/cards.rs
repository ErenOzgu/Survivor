//! The level-up screen: the game pauses and shows up to three cards; 1/2/3 or a click picks one.
//! Each card is laid out like a collectible card: the spell's WoW icon over its school's talent
//! painting in an oval, the level on a gem, the name on a banner, the text on parchment (plain
//! colours without data).

use bevy::prelude::*;

use super::art::{scene_crop, CardArt, WowArt, TALENT_ARCANE, TALENT_FIRE, TALENT_FROST};
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

/// What a card belongs to: it sets the frame colour, the painting behind the icon and the label
/// on the card's foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum School {
    Frost,
    Fire,
    Arcane,
    Passive,
    Potion,
}

impl School {
    pub fn of(card: Card) -> Self {
        use crate::content::SkillId::*;
        match card {
            Card::New(id) | Card::Upgrade(id, _) | Card::Evolve(_, id) => match id {
                Frostbolt | FrostNova | Blizzard | WinterBolts => School::Frost,
                Fireball | Flamestrike | Pyroblast => School::Fire,
                ArcaneMissiles | ArcaneExplosion | Blink | ArcaneStorm => School::Arcane,
            },
            Card::NewPassive(_) | Card::UpgradePassive(..) => School::Passive,
            Card::Heal => School::Potion,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            School::Frost => "Buz",
            School::Fire => "Ateş",
            School::Arcane => "Arkan",
            School::Passive => "Pasif",
            School::Potion => "İksir",
        }
    }

    /// The frame's colour (sRGB).
    fn frame(self) -> [f32; 3] {
        match self {
            School::Frost => [0.42, 0.52, 0.86],
            School::Fire => [0.82, 0.36, 0.16],
            School::Arcane => [0.62, 0.36, 0.86],
            School::Passive => [0.36, 0.62, 0.36],
            School::Potion => [0.78, 0.24, 0.28],
        }
    }

    /// The school's key in [`crate::combat::art::SCENE_NEEDLES`].
    pub fn scene_key(self) -> &'static str {
        match self {
            School::Frost => "frost",
            School::Fire => "fire",
            School::Arcane => "arcane",
            School::Passive => "passive",
            School::Potion => "potion",
        }
    }

    /// The client's talent-tree painting for the school, behind the card's icon.
    pub fn painting(self) -> Option<&'static str> {
        match self {
            School::Frost => Some(TALENT_FROST),
            School::Fire => Some(TALENT_FIRE),
            School::Arcane => Some(TALENT_ARCANE),
            School::Passive | School::Potion => None,
        }
    }
}

/// The gem in the corner: the skill's level after the pick, a star for an evolution, a plus for
/// the potion.
pub fn card_gem(card: Card) -> String {
    match card {
        Card::New(_) | Card::NewPassive(_) => "1".to_string(),
        Card::Upgrade(_, to) | Card::UpgradePassive(_, to) => to.to_string(),
        Card::Evolve(..) => "★".to_string(),
        Card::Heal => "+".to_string(),
    }
}

/// The small gem under the name: how rare the pick is.
fn rarity(card: Card) -> Color {
    match card {
        Card::Upgrade(..) | Card::UpgradePassive(..) => Color::srgb(0.92, 0.92, 0.92),
        Card::New(_) => Color::srgb(0.25, 0.55, 1.0),
        Card::NewPassive(_) => Color::srgb(0.7, 0.35, 1.0),
        Card::Evolve(..) => Color::srgb(1.0, 0.55, 0.1),
        Card::Heal => Color::srgb(0.3, 0.85, 0.3),
    }
}

/// The skill's or passive's own colour: the oval's ground when no painting covers it.
fn card_color(card: Card) -> [f32; 3] {
    match card {
        Card::New(id) | Card::Upgrade(id, _) | Card::Evolve(_, id) => id.def().color,
        Card::NewPassive(p) | Card::UpgradePassive(p, _) => p.def().color,
        Card::Heal => [0.9, 0.2, 0.2],
    }
}

fn srgb(c: [f32; 3], k: f32) -> Color {
    Color::srgb(c[0] * k, c[1] * k, c[2] * k)
}

/// Top-to-bottom gradient between two colours.
fn vertical(top: Color, bottom: Color) -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::auto(top),
        ColorStop::auto(bottom),
    ]))
}

const SHADOW: TextShadow = TextShadow {
    offset: Vec2::splat(2.0),
    color: Color::srgba(0.0, 0.0, 0.0, 0.9),
};

/// The card's resting shadow, and its glow when hovered.
fn card_shadow(glow: Option<Color>) -> BoxShadow {
    match glow {
        None => BoxShadow::new(
            Color::srgba(0.0, 0.0, 0.0, 0.7),
            Val::Px(0.0),
            Val::Px(8.0),
            Val::Px(0.0),
            Val::Px(14.0),
        ),
        Some(c) => BoxShadow::new(c, Val::Px(0.0), Val::Px(0.0), Val::Px(6.0), Val::Px(26.0)),
    }
}

/// What a card is drawn from: the client art, the loaded images (for a painting's size) and
/// which painting the settings ask for.
struct CardLook<'a> {
    art: &'a WowArt,
    images: &'a Assets<Image>,
    mode: CardArt,
}

impl CardLook<'_> {
    /// The painting behind the icon and the part of it the oval shows.
    fn painting(&self, school: School) -> Option<(Handle<Image>, Rect)> {
        match self.mode {
            CardArt::Talent => {
                let img = self.art.ui(school.painting()?)?;
                // The middle of the 256² top-left piece, at the oval's proportions.
                Some((img, Rect::new(0.0, 22.0, 256.0, 240.0)))
            }
            CardArt::Loading => {
                let img = self.art.scenes.get(school.scene_key())?.clone();
                let size = self.images.get(&img)?.size_f32();
                Some((img, scene_crop(size.x, size.y, ART_W / ART_H, 0.62)))
            }
        }
    }
}

/// The colour a card glows in when hovered.
#[derive(Component)]
struct CardGlow(Color);

const CARD_W: f32 = 250.0;
const ART_W: f32 = 204.0;
const ART_H: f32 = 172.0;

#[allow(clippy::too_many_arguments)]
fn open_cards(
    mut commands: Commands,
    mut screen: ResMut<CardScreen>,
    mut rng: ResMut<CombatRng>,
    mut clock: ResMut<Time<Virtual>>,
    run: Res<Run>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    (images, mut mode): (Res<Assets<Image>>, Local<Option<CardArt>>),
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
    let mode = *mode.get_or_insert_with(CardArt::from_settings);
    let look = CardLook {
        art: &art,
        images: &images,
        mode,
    };
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
                row_gap: Val::Px(30.0),
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
                column_gap: Val::Px(34.0),
                ..default()
            })
            .with_children(|row| {
                for (i, card) in cards.iter().enumerate() {
                    spawn_card(row, i, *card, &font, &look);
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

/// One card, laid out like a collectible card: an oval painting with the spell's icon, a gem
/// with the level in the corner, the name on a banner, the text on parchment, the school on the
/// foot.
fn spawn_card(
    row: &mut ChildSpawnerCommands,
    i: usize,
    card: Card,
    font: &UiFont,
    look: &CardLook,
) {
    let art = look.art;
    let (title, body) = card_text(card);
    let title = match card {
        Card::Upgrade(id, _) => id.def().name.to_string(),
        _ => title,
    };
    let school = School::of(card);
    let frame = school.frame();
    let glow = match card {
        Card::Evolve(..) => Color::srgb(1.0, 0.6, 0.15),
        _ => srgb(frame, 1.25),
    };
    let banner_gold = vertical(Color::srgb(0.78, 0.62, 0.34), Color::srgb(0.5, 0.36, 0.16));
    let banner_edge = Color::srgb(0.32, 0.22, 0.1);

    row.spawn((
        CardButton(i),
        CardGlow(glow),
        Button,
        Node {
            width: Val::Px(CARD_W),
            height: Val::Px(370.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(12.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(18.0)),
            ..default()
        },
        BorderColor::all(srgb(frame, 0.85)),
        vertical(srgb(frame, 0.45), srgb(frame, 0.16)),
        card_shadow(None),
        UiTransform::default(),
    ))
    .with_children(|c| {
        // The oval painting: the school's talent art, the spell's icon in front.
        c.spawn((
            Node {
                width: Val::Px(ART_W),
                height: Val::Px(ART_H),
                border: UiRect::all(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::srgb(0.72, 0.58, 0.3)),
            vertical(srgb(card_color(card), 0.8), srgb(card_color(card), 0.2)),
        ))
        .with_children(|a| {
            let painting = look.painting(school);
            if let Some((img, rect)) = &painting {
                a.spawn((
                    ImageNode {
                        image: img.clone(),
                        rect: Some(*rect),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Percent(50.0)),
                        ..default()
                    },
                ));
            }
            let size = if painting.is_some() { 84.0 } else { 104.0 };
            match card_icon(card, art) {
                Some(icon) => {
                    a.spawn((
                        Node {
                            width: Val::Px(size),
                            height: Val::Px(size),
                            border: UiRect::all(Val::Px(3.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.85, 0.7, 0.35)),
                        BoxShadow::new(
                            Color::srgba(0.0, 0.0, 0.0, 0.8),
                            Val::Px(0.0),
                            Val::Px(4.0),
                            Val::Px(2.0),
                            Val::Px(10.0),
                        ),
                    ))
                    .with_children(|f| {
                        f.spawn((
                            ImageNode::new(icon),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                ..default()
                            },
                        ));
                    });
                }
                None => {
                    a.spawn((
                        Text::new(title.chars().next().unwrap_or('?').to_string()),
                        font.text(72.0),
                        TextColor(Color::WHITE),
                        SHADOW,
                    ));
                }
            }
        });

        // The name on a banner over the painting's foot.
        c.spawn((
            Node {
                width: Val::Px(CARD_W - 6.0),
                height: Val::Px(40.0),
                margin: UiRect::top(Val::Px(-24.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(banner_edge),
            banner_gold.clone(),
            BoxShadow::new(
                Color::srgba(0.0, 0.0, 0.0, 0.6),
                Val::Px(0.0),
                Val::Px(3.0),
                Val::Px(0.0),
                Val::Px(6.0),
            ),
        ))
        .with_children(|b| {
            let size = if title.chars().count() > 16 {
                18.0
            } else {
                22.0
            };
            b.spawn((
                Text::new(title),
                font.text(size),
                TextColor(Color::WHITE),
                SHADOW,
            ));
        });

        // The rarity gem between the name and the text.
        c.spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(20.0),
                margin: UiRect::vertical(Val::Px(-4.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            BorderColor::all(banner_edge),
            BackgroundColor(rarity(card)),
            ZIndex(1),
        ));

        // The text on parchment: the kind of pick, what it does, the level pips.
        c.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(10.0), Val::Px(16.0)),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.55, 0.42, 0.22)),
            vertical(Color::srgb(0.88, 0.81, 0.66), Color::srgb(0.74, 0.65, 0.48)),
            ZIndex(-1),
        ))
        .with_children(|t| {
            let tag_color = match card {
                Card::New(_) | Card::NewPassive(_) => Color::srgb(0.1, 0.45, 0.1),
                Card::Evolve(..) => Color::srgb(0.6, 0.25, 0.0),
                _ => Color::srgb(0.45, 0.28, 0.05),
            };
            t.spawn((
                Text::new(card_tag(card)),
                font.text(14.0),
                TextColor(tag_color),
            ));
            t.spawn((
                Text::new(body),
                font.text(16.0),
                TextColor(Color::srgb(0.16, 0.1, 0.04)),
                TextLayout::new_with_justify(Justify::Center),
            ));
            if let Some((lvl, max)) = card_pips(card) {
                t.spawn(Node {
                    column_gap: Val::Px(4.0),
                    margin: UiRect::top(Val::Px(2.0)),
                    ..default()
                })
                .with_children(|pips| {
                    for n in 1..=max {
                        let fill = if n < lvl {
                            Color::srgb(0.5, 0.34, 0.1)
                        } else if n == lvl {
                            Color::srgb(0.15, 0.6, 0.15)
                        } else {
                            Color::srgba(0.0, 0.0, 0.0, 0.15)
                        };
                        pips.spawn((
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(7.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(fill),
                        ));
                    }
                });
            }
        });

        // The school on the foot.
        c.spawn((
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(2.0)),
                margin: UiRect::top(Val::Px(-14.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BorderColor::all(banner_edge),
            banner_gold,
        ))
        .with_children(|f| {
            f.spawn((
                Text::new(school.label()),
                font.text(16.0),
                TextColor(Color::WHITE),
                SHADOW,
            ));
        });

        // The level gem in the top-left corner.
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-18.0),
                top: Val::Px(-18.0),
                width: Val::Px(54.0),
                height: Val::Px(54.0),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::srgb(0.85, 0.92, 1.0)),
            vertical(Color::srgb(0.45, 0.7, 1.0), Color::srgb(0.08, 0.22, 0.65)),
            BoxShadow::new(
                Color::srgba(0.0, 0.0, 0.0, 0.7),
                Val::Px(0.0),
                Val::Px(3.0),
                Val::Px(0.0),
                Val::Px(6.0),
            ),
        ))
        .with_children(|g| {
            g.spawn((
                Text::new(card_gem(card)),
                font.text(30.0),
                TextColor(Color::WHITE),
                SHADOW,
            ));
        });

        // The key that picks it, in the top-right corner.
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-12.0),
                top: Val::Px(-12.0),
                width: Val::Px(30.0),
                height: Val::Px(30.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(GOLD),
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.95)),
        ))
        .with_children(|k| {
            k.spawn((
                Text::new(format!("{}", i + 1)),
                font.text(16.0),
                TextColor(GOLD),
            ));
        });
    });
}

/// A hovered card grows a little and glows in its colour.
#[allow(clippy::type_complexity)]
fn hover_cards(
    mut cards: Query<
        (&Interaction, &CardGlow, &mut UiTransform, &mut BoxShadow),
        (Changed<Interaction>, With<CardButton>),
    >,
) {
    for (i, glow, mut xf, mut shadow) in &mut cards {
        let lit = matches!(i, Interaction::Hovered | Interaction::Pressed);
        xf.scale = Vec2::splat(if lit { 1.06 } else { 1.0 });
        *shadow = card_shadow(lit.then_some(glow.0));
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

    #[test]
    fn schools_and_gems() {
        assert_eq!(School::of(Card::New(SkillId::Blizzard)), School::Frost);
        assert_eq!(
            School::of(Card::Evolve(SkillId::Fireball, SkillId::Pyroblast)),
            School::Fire
        );
        assert_eq!(School::of(Card::Upgrade(SkillId::Blink, 2)), School::Arcane);
        assert_eq!(School::of(Card::Heal), School::Potion);
        assert!(School::Frost.painting().is_some() && School::Potion.painting().is_none());
        assert_eq!(card_gem(Card::Upgrade(SkillId::Fireball, 4)), "4");
        assert_eq!(card_gem(Card::New(SkillId::Fireball)), "1");
        assert_eq!(card_gem(Card::Heal), "+");
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
