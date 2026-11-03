//! The main menu: shown when the game starts (and when the pause menu leaves to it), over the
//! streaming world. Three panels: the class (the Mage; the others locked for later) with its
//! spells, the character (its live figure, what it wears, what gear and talents give, the
//! records), and the zone (Elwynn Forest with its bosses' times; more to come); under them the
//! big play button. 1/Enter plays, 2 opens the inventory, 3 the talents, 4 quits.
//! `RNS_NOMENU=1` (and the autopilot) skip it.

use bevy::app::AppExit;
use bevy::prelude::*;

use super::art::WowArt;
use super::hud::{clock, TitleFont, UiFont, WowSkin};
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
        .add_systems(
            Update,
            (main_menu, dim_locked, hide_hud).in_set(CombatSet::Ui),
        );
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
    mut look: MenuLook,
) {
    if restarted.read().any(|r| r.to_menu) {
        menu.pending = true;
    }
    if menu.pending && menu.open.is_none() {
        menu.pending = false;
        clock_v.pause();
        menu.open = Some(spawn_menu(
            &mut commands,
            &save.data,
            &font,
            &art,
            &mut look,
        ));
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
    // The item tables and the art come in after the menu first shows: draw it again with them.
    let tables = look
        .data
        .as_ref()
        .is_some_and(|d| d.tables.as_ref().is_some_and(|t| t.items.is_some()));
    if (tables && !*look.seen_tables) || art.is_changed() {
        menu.refresh = true;
    }
    *look.seen_tables |= tables;
    if menu.refresh {
        menu.refresh = false;
        commands.entity(root).try_despawn();
        menu.open = Some(spawn_menu(
            &mut commands,
            &save.data,
            &font,
            &art,
            &mut look,
        ));
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

/// What the menu draws with beyond the save: the title font, the figure, item icons.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct MenuLook<'w, 's> {
    title: Option<Res<'w, TitleFont>>,
    figure: Option<Res<'w, super::char_view::CharView>>,
    icons: ResMut<'w, super::gear_ui::IconCache>,
    data: Option<Res<'w, crate::models::ModelData>>,
    wa: Option<ResMut<'w, benilla_assets::WorldAssets>>,
    images: ResMut<'w, Assets<Image>>,
    seen_tables: Local<'s, bool>,
}

/// The class grid: each class's square in the character-creation icons, and whether it plays.
const CLASSES: [(&str, [f32; 4], bool); 9] = [
    ("Savaşçı", [0.0, 0.0, 0.25, 0.25], false),
    ("Büyücü", [0.25, 0.0, 0.49609375, 0.25], true),
    ("Haydut", [0.49609375, 0.0, 0.7421875, 0.25], false),
    ("Druid", [0.7421875, 0.0, 0.98828125, 0.25], false),
    ("Avcı", [0.0, 0.25, 0.25, 0.5], false),
    ("Şaman", [0.25, 0.25, 0.49609375, 0.5], false),
    ("Rahip", [0.49609375, 0.25, 0.7421875, 0.5], false),
    ("Kara Büyücü", [0.7421875, 0.25, 0.98828125, 0.5], false),
    ("Paladin", [0.0, 0.5, 0.25, 0.75], false),
];

/// The zones: Elwynn plays; the rest are on the way.
const ZONES: [(&str, &str); 4] = [
    ("Elwynn Ormanı", "Goldshire çevresi"),
    ("Westfall", "Yakında"),
    ("Redridge Dağları", "Yakında"),
    ("Duskwood", "Yakında"),
];

/// The mage's class colour.
const MAGE_BLUE: Color = Color::srgb(0.41, 0.8, 0.94);

fn panel_node(width: f32) -> Node {
    Node {
        width: Val::Px(width),
        height: Val::Px(560.0),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(10.0),
        padding: UiRect::all(Val::Px(18.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        overflow: Overflow::clip(),
        ..default()
    }
}

fn panel_look() -> (BackgroundGradient, BorderColor) {
    (
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::auto(Color::srgba(0.09, 0.07, 0.06, 0.95)),
            ColorStop::auto(Color::srgba(0.03, 0.025, 0.02, 0.95)),
        ])),
        BorderColor::all(Color::srgba(0.75, 0.6, 0.3, 0.55)),
    )
}

/// A panel's heading, with a thin rule either side.
fn heading(p: &mut ChildSpawnerCommands, text: &str, title: &TextFont) {
    let rule = Color::srgba(0.75, 0.6, 0.3, 0.45);
    p.spawn(Node {
        width: Val::Percent(100.0),
        align_items: AlignItems::Center,
        column_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|h| {
        let line = Node {
            flex_grow: 1.0,
            height: Val::Px(1.0),
            ..default()
        };
        h.spawn((line.clone(), BackgroundColor(rule)));
        h.spawn((
            Text::new(text),
            title.clone(),
            TextColor(Color::srgb(1.0, 0.82, 0.3)),
        ));
        h.spawn((line, BackgroundColor(rule)));
    });
}

fn icon_tile(p: &mut ChildSpawnerCommands, img: Option<Handle<Image>>, size: f32, border: Color) {
    let mut t = p.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BorderColor::all(border),
        BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.9)),
    ));
    if let Some(img) = img {
        t.insert(ImageNode::new(img));
    }
}

fn spawn_menu(
    commands: &mut Commands,
    save: &SaveData,
    font: &UiFont,
    art: &WowArt,
    look: &mut MenuLook,
) -> Entity {
    use super::art::CLASS_ICONS;
    use crate::content::bosses::ELWYNN_BOSSES;
    use crate::content::SkillId;
    let gold = Color::srgb(1.0, 0.82, 0.0);
    let dim = Color::srgb(0.72, 0.7, 0.66);
    let shadow = TextShadow {
        offset: Vec2::splat(2.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.9),
    };
    let title = |size: f32| match &look.title {
        Some(t) => t.text(size),
        None => font.text(size),
    };
    let free = save.free_points();
    let mut root = commands.spawn((
        Name::new("main menu"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(14.0),
            ..default()
        },
        BackgroundGradient::from(RadialGradient::new(
            UiPosition::CENTER,
            RadialGradientShape::FarthestCorner,
            vec![
                ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.45), 0.0),
                ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.85), 100.0),
            ],
        )),
        GlobalZIndex(40),
    ));
    root.with_children(|p| {
        // The title and the top buttons.
        p.spawn((
            Text::new("Rnsurvivors"),
            title(64.0),
            TextColor(Color::srgb(1.0, 0.8, 0.35)),
            shadow,
        ));
        p.spawn(Node {
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            for (choice, label) in [
                (MenuChoice::Inventory, "2  Karakter".to_string()),
                (MenuChoice::Talents, format!("3  Talentlar ({free})")),
                (MenuChoice::Quit, "4  Çık".to_string()),
            ] {
                let lit = choice == MenuChoice::Talents && free > 0;
                row.spawn((
                    choice,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.06, 0.04, 0.95)),
                    BorderColor::all(if lit {
                        Color::srgb(0.3, 1.0, 0.3)
                    } else {
                        Color::srgba(0.75, 0.6, 0.3, 0.6)
                    }),
                ))
                .with_children(|b| {
                    b.spawn((Text::new(label), font.text(19.0), TextColor(Color::WHITE)));
                });
            }
        });

        p.spawn(Node {
            column_gap: Val::Px(18.0),
            ..default()
        })
        .with_children(|cols| {
            // ---- The class.
            let (bg, border) = panel_look();
            cols.spawn((panel_node(330.0), bg, border))
                .with_children(|c| {
                    heading(c, "Sınıf", &title(26.0));
                    c.spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::px(3, 76.0),
                        row_gap: Val::Px(8.0),
                        column_gap: Val::Px(14.0),
                        ..default()
                    })
                    .with_children(|g| {
                        for (name, uv, plays) in CLASSES {
                            g.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                row_gap: Val::Px(3.0),
                                ..default()
                            })
                            .with_children(|cell| {
                                cell.spawn((
                                    Node {
                                        width: Val::Px(54.0),
                                        height: Val::Px(54.0),
                                        border: UiRect::all(Val::Px(2.0)),
                                        border_radius: BorderRadius::all(Val::Px(5.0)),
                                        ..default()
                                    },
                                    BorderColor::all(if plays {
                                        MAGE_BLUE
                                    } else {
                                        Color::srgba(0.4, 0.4, 0.4, 0.5)
                                    }),
                                    BackgroundColor(Color::srgba(0.2, 0.2, 0.25, 0.8)),
                                    WowSkin::new(CLASS_ICONS, uv),
                                    MenuDim(!plays),
                                ));
                                cell.spawn((
                                    Text::new(name),
                                    font.text(13.0),
                                    TextColor(if plays { MAGE_BLUE } else { dim.with_alpha(0.6) }),
                                ));
                            });
                        }
                    });
                    c.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            margin: UiRect::top(Val::Px(6.0)),
                            ..default()
                        },
                        Text::new("Büyücü"),
                        title(28.0),
                        TextColor(MAGE_BLUE),
                    ));
                    c.spawn((
                        Text::new(
                            "Buzla yavaşlatır, ateşle yakar, arkanla her yeri vurur. Büyüleri \
                             kendiliğinden atılır; sen kaç, topla, seç.",
                        ),
                        font.text(15.0),
                        TextColor(Color::srgb(0.9, 0.88, 0.84)),
                    ));
                    c.spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|r| {
                        let stats = super::rules::Stats::from_parts(&Default::default(), &save.bonus());
                        for (k, v) in [
                            ("CAN", format!("{:.0}", stats.max_health)),
                            ("İLK BÜYÜ", "Frostbolt".to_string()),
                            ("BÖLGE", "1 / 1".to_string()),
                        ] {
                            r.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    flex_direction: FlexDirection::Column,
                                    padding: UiRect::all(Val::Px(6.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgba(0.75, 0.6, 0.3, 0.35)),
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
                            ))
                            .with_children(|b| {
                                b.spawn((Text::new(k), font.text(11.0), TextColor(dim)));
                                b.spawn((Text::new(v), font.text(16.0), TextColor(Color::WHITE)));
                            });
                        }
                    });
                    c.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        Text::new("BÜYÜ HAVUZU"),
                        font.text(12.0),
                        TextColor(dim),
                    ));
                    c.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(5.0),
                        row_gap: Val::Px(5.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for id in SkillId::every().filter(|id| !id.is_evolved()) {
                            icon_tile(r, art.icon(id), 32.0, Color::srgba(0.6, 0.6, 0.6, 0.6));
                        }
                    });
                });

            // ---- The character.
            let (bg, border) = panel_look();
            cols.spawn((panel_node(420.0), bg, border))
                .with_children(|c| {
                    c.spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(14.0),
                        ..default()
                    })
                    .with_children(|top| {
                        let mut fig = top.spawn((
                            Node {
                                width: Val::Px(150.0),
                                height: Val::Px(185.0),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.75, 0.6, 0.3, 0.5)),
                            BackgroundColor(Color::srgb(0.04, 0.035, 0.03)),
                        ));
                        if let Some(f) = &look.figure {
                            fig.insert(ImageNode::new(f.image.clone()));
                        }
                        top.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|n| {
                            n.spawn((Text::new("Büyücü"), title(30.0), TextColor(Color::WHITE)));
                            n.spawn((Text::new("Elwynn'in koruyucusu"), font.text(15.0), TextColor(MAGE_BLUE)));
                            n.spawn((
                                Text::new(record_lines(save).join("\n")),
                                font.text(14.0),
                                TextColor(Color::srgb(0.88, 0.86, 0.8)),
                            ));
                        });
                    });
                    heading(c, "Ekipman", &title(22.0));
                    c.spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for slot in crate::content::items::ItemSlot::ALL {
                            let worn = save.worn(slot);
                            let img = worn.and_then(|it| {
                                look.icons.get(
                                    it,
                                    look.data.as_deref(),
                                    look.wa.as_deref_mut(),
                                    &mut look.images,
                                )
                            });
                            let border = worn.map_or(Color::srgba(0.5, 0.5, 0.5, 0.4), |it| {
                                super::gear_ui::rarity_color(it.rarity)
                            });
                            icon_tile(r, img, 42.0, border);
                        }
                    });
                    let lines = super::pause::stat_lines(&super::rules::Stats::from_parts(
                        &Default::default(),
                        &save.bonus(),
                    ));
                    c.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                        Text::new(format!("Eşya ve talentlardan: {}", lines.join(" · "))),
                        font.text(14.0),
                        TextColor(Color::srgb(0.35, 1.0, 0.35)),
                    ));
                    c.spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(10.0),
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|r| {
                        for (choice, big, small) in [
                            (
                                MenuChoice::Inventory,
                                "Karakter",
                                format!("{} eşya · I", save.bag.len() + save.equipped.len()),
                            ),
                            (
                                MenuChoice::Talents,
                                "Talentlar",
                                format!("{free} boş puan · T"),
                            ),
                        ] {
                            r.spawn((
                                choice,
                                Button,
                                Node {
                                    flex_grow: 1.0,
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(8.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.25, 0.05, 0.03, 0.85)),
                                BorderColor::all(Color::srgba(0.85, 0.65, 0.3, 0.7)),
                            ))
                            .with_children(|b| {
                                b.spawn((Text::new(big), title(22.0), TextColor(gold)));
                                b.spawn((Text::new(small), font.text(13.0), TextColor(dim)));
                            });
                        }
                    });
                });

            // ---- The zone.
            let (bg, border) = panel_look();
            cols.spawn((panel_node(360.0), bg, border))
                .with_children(|c| {
                    heading(c, "Bölge", &title(26.0));
                    for (i, (name, sub)) in ZONES.iter().enumerate() {
                        let open = i == 0;
                        c.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                padding: UiRect::all(Val::Px(10.0)),
                                border: UiRect::all(Val::Px(if open { 2.0 } else { 1.0 })),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BorderColor::all(if open {
                                Color::srgb(1.0, 0.8, 0.35)
                            } else {
                                Color::srgba(0.5, 0.5, 0.5, 0.35)
                            }),
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, if open { 0.5 } else { 0.3 })),
                        ))
                        .with_children(|z| {
                            z.spawn((
                                Text::new(*name),
                                title(if open { 26.0 } else { 20.0 }),
                                TextColor(if open { gold } else { dim.with_alpha(0.6) }),
                            ));
                            z.spawn((Text::new(*sub), font.text(13.0), TextColor(dim)));
                            if open {
                                z.spawn(Node {
                                    flex_wrap: FlexWrap::Wrap,
                                    column_gap: Val::Px(6.0),
                                    row_gap: Val::Px(6.0),
                                    margin: UiRect::top(Val::Px(4.0)),
                                    ..default()
                                })
                                .with_children(|chips| {
                                    for b in ELWYNN_BOSSES.iter() {
                                        let when = b
                                            .arrives_at()
                                            .map_or("Bekliyor".to_string(), clock);
                                        chips
                                            .spawn((
                                                Node {
                                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                                    border: UiRect::all(Val::Px(1.0)),
                                                    border_radius: BorderRadius::all(Val::Px(9.0)),
                                                    ..default()
                                                },
                                                BorderColor::all(Color::srgba(0.85, 0.5, 0.2, 0.7)),
                                            ))
                                            .with_children(|t| {
                                                t.spawn((
                                                    Text::new(format!("{when} · {}", b.name())),
                                                    font.text(13.0),
                                                    TextColor(Color::srgb(1.0, 0.75, 0.45)),
                                                ));
                                            });
                                    }
                                });
                            }
                        });
                    }
                });
        });

        // The big play button.
        p.spawn((
            MenuChoice::Play,
            Button,
            Node {
                width: Val::Px(560.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(Color::srgb(0.45, 0.08, 0.04)),
                ColorStop::auto(Color::srgb(0.2, 0.03, 0.02)),
            ])),
            BorderColor::all(Color::srgb(1.0, 0.75, 0.35)),
            BoxShadow::new(
                Color::srgba(1.0, 0.5, 0.15, 0.35),
                Val::Px(0.0),
                Val::Px(0.0),
                Val::Px(2.0),
                Val::Px(18.0),
            ),
        ))
        .with_children(|b| {
            b.spawn((Text::new("Elwynn Ormanı'na gir"), title(38.0), TextColor(gold), shadow));
            b.spawn((
                Text::new("Enter ya da 1"),
                font.text(14.0),
                TextColor(Color::srgb(0.95, 0.85, 0.75)),
            ));
        });
        p.spawn((
            Text::new(
                "WASD ya da iki fare tuşu: koş · sağ fare: dön · 1 2 3: kart · Z X C: cooldownlar · \
                 I: karakter · Esc: duraklat",
            ),
            font.text(14.0),
            TextColor(dim),
        ));
    });
    root.id()
}

/// The in-run HUD (the roots named `hud …`, the minimap) hides while the main menu is up.
#[allow(clippy::type_complexity)]
fn hide_hud(
    menu: Res<MainMenu>,
    mut roots: Query<(&Name, &mut Visibility), (With<Node>, Without<ChildOf>)>,
    mut was: Local<Option<bool>>,
) {
    let open = menu.is_open();
    if *was == Some(open) {
        return;
    }
    *was = Some(open);
    for (name, mut vis) in &mut roots {
        let n = name.as_str();
        if n.starts_with("hud ") || n == "minimap" || n == "nameplate" {
            *vis = if open {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
}

/// A locked class's icon is dimmed once its texture is on.
#[derive(Component)]
struct MenuDim(bool);

fn dim_locked(mut q: Query<(&MenuDim, &mut ImageNode), Added<ImageNode>>) {
    for (d, mut img) in &mut q {
        if d.0 {
            img.color = Color::srgba(0.35, 0.35, 0.35, 0.9);
        }
    }
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
        IntoSystem::into_system(hide_hud).initialize(&mut world);
    }
}
