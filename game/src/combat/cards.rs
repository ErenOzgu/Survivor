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

/// How rare a pick is, WoW's item-quality colours: an upgrade is good (green), a new passive
/// rare (blue), a new skill epic (purple), an evolution legendary (orange), the potion common.
pub fn rarity(card: Card) -> (&'static str, Color) {
    match card {
        Card::Heal => ("SIRADAN", Color::srgb(0.92, 0.92, 0.92)),
        Card::Upgrade(..) | Card::UpgradePassive(..) => ("İYİ", Color::srgb(0.12, 1.0, 0.0)),
        Card::NewPassive(_) => ("NADİR", Color::srgb(0.0, 0.44, 0.87)),
        Card::New(_) => ("EPİK", Color::srgb(0.64, 0.21, 0.93)),
        Card::Evolve(..) => ("EFSANEVİ", Color::srgb(1.0, 0.5, 0.0)),
    }
}

/// The line under the name: what kind of thing it is.
pub fn card_kind(card: Card) -> &'static str {
    match School::of(card) {
        School::Frost => "Buz büyüsü",
        School::Fire => "Ateş büyüsü",
        School::Arcane => "Arkan büyüsü",
        School::Passive => "Pasif",
        School::Potion => "İksir",
    }
}

/// The card's foot: what the pick does to what you have.
pub fn card_footer(card: Card) -> String {
    match card {
        Card::New(_) => "Yeni skill: boş bir yuvaya girer".to_string(),
        Card::Upgrade(id, _) => format!("{} skillini güçlendirir", id.def().name),
        Card::Evolve(from, _) => format!("{} buna dönüşür", from.def().name),
        Card::NewPassive(_) => "Yeni pasif".to_string(),
        Card::UpgradePassive(p, _) => format!("{} pasifini güçlendirir", p.def().name),
        Card::Heal => "Hemen içilir".to_string(),
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

/// The card's glow in its rarity colour: soft at rest, strong when hovered.
fn card_shadow(color: Color, lit: bool) -> BoxShadow {
    let (alpha, spread, blur) = if lit {
        (0.9, 6.0, 30.0)
    } else {
        (0.45, 1.0, 18.0)
    };
    BoxShadow::new(
        color.with_alpha(alpha),
        Val::Px(0.0),
        Val::Px(0.0),
        Val::Px(spread),
        Val::Px(blur),
    )
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

const CARD_W: f32 = 260.0;
const ART_W: f32 = 204.0;
const ART_H: f32 = 170.0;

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
    (inv, loot, craft): (
        Res<super::gear_ui::Inventory>,
        Res<super::gear_ui::LootScreen>,
        Res<super::crafting_ui::CraftScreen>,
    ),
) {
    if screen.is_open()
        || run.pending_cards == 0
        || pause.is_open()
        || main_menu.is_open()
        || inv.is_open()
        || loot.is_open()
        || craft.is_open()
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

/// One card: the key and the kind of pick on top, the icon in a glowing frame, the rarity, the
/// name in its colour, the school, what it does, the level pips, and on the foot what it does to
/// what you have. Over a faint painting of the school (the settings' `kart=`).
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
    let (rarity_name, color) = rarity(card);
    let dim = Color::srgb(0.62, 0.6, 0.58);

    row.spawn((
        CardButton(i),
        CardGlow(color),
        Button,
        Node {
            width: Val::Px(CARD_W),
            height: Val::Px(350.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::new(Val::Px(14.0), Val::Px(14.0), Val::Px(38.0), Val::Px(10.0)),
            row_gap: Val::Px(4.0),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            overflow: Overflow::clip(),
            ..default()
        },
        BorderColor::all(color),
        vertical(
            Color::srgb(0.08, 0.07, 0.1),
            Color::srgb(0.025, 0.025, 0.035),
        ),
        card_shadow(color, false),
        UiTransform::default(),
    ))
    .with_children(|c| {
        // The school's painting, faint, behind the top half.
        if let Some((img, rect)) = look.painting(school) {
            c.spawn((
                ImageNode {
                    image: img,
                    rect: Some(rect),
                    color: Color::srgba(1.0, 1.0, 1.0, 0.28),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(ART_H),
                    ..default()
                },
            ));
        }
        // The key that picks it, top-left.
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(dim),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|k| {
            k.spawn((
                Text::new(format!("{}", i + 1)),
                font.text(14.0),
                TextColor(Color::WHITE),
            ));
        });
        // The kind of pick, top-right.
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(11.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BorderColor::all(color.with_alpha(0.8)),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|t| {
            t.spawn((Text::new(card_tag(card)), font.text(12.0), TextColor(color)));
        });

        // The icon in a frame that glows in the rarity colour.
        c.spawn((
            Node {
                width: Val::Px(78.0),
                height: Val::Px(78.0),
                margin: UiRect::bottom(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(color),
            BackgroundColor(srgb(card_color(card), 0.35)),
            BoxShadow::new(
                color.with_alpha(0.7),
                Val::Px(0.0),
                Val::Px(0.0),
                Val::Px(2.0),
                Val::Px(16.0),
            ),
        ))
        .with_children(|f| match card_icon(card, art) {
            Some(icon) => {
                f.spawn((
                    ImageNode::new(icon),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                ));
            }
            None => {
                f.spawn((
                    Text::new(card_gem(card)),
                    font.text(40.0),
                    TextColor(Color::WHITE),
                    SHADOW,
                ));
            }
        });

        c.spawn((Text::new(rarity_name), font.text(13.0), TextColor(color)));
        let size = if title.chars().count() > 16 {
            22.0
        } else {
            26.0
        };
        c.spawn((
            Text::new(title),
            font.text(size),
            TextColor(color),
            TextLayout::new_with_justify(Justify::Center),
            SHADOW,
        ));
        c.spawn((Text::new(card_kind(card)), font.text(15.0), TextColor(dim)));
        c.spawn((
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
            Text::new(body),
            font.text(17.0),
            TextColor(Color::srgb(0.86, 0.95, 0.86)),
            TextLayout::new_with_justify(Justify::Center),
        ));
        if let Some((lvl, max)) = card_pips(card) {
            c.spawn(Node {
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .with_children(|pips| {
                for n in 1..=max {
                    let fill = if n < lvl {
                        color.with_alpha(0.55)
                    } else if n == lvl {
                        color
                    } else {
                        Color::srgba(1.0, 1.0, 1.0, 0.12)
                    };
                    pips.spawn((
                        Node {
                            width: Val::Px(14.0),
                            height: Val::Px(6.0),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(fill),
                    ));
                }
            });
        }
        // The foot: what it does to what you have.
        c.spawn((
            Node {
                margin: UiRect::top(Val::Auto),
                width: Val::Percent(100.0),
                padding: UiRect::top(Val::Px(8.0)),
                border: UiRect::top(Val::Px(1.0)),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor::all(color.with_alpha(0.35)),
        ))
        .with_children(|f| {
            f.spawn((
                Text::new(card_footer(card)),
                font.text(14.0),
                TextColor(dim),
                TextLayout::new_with_justify(Justify::Center),
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
        xf.scale = Vec2::splat(if lit { 1.05 } else { 1.0 });
        *shadow = card_shadow(glow.0, lit);
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
