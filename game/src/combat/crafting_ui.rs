//! Professions in play: enemies drop materials (into the save at once, with a note), and the
//! professions window (P, or the main menu) cooks meals, picks the one for the next run, forges
//! and enchants worn items. It pauses the game while open.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::WorldAssets;

use super::bosses::Boss;
use super::cards::CardScreen;
use super::enemies::{kill_enemies, Enemy};
use super::gear_ui::{rarity_color, Inventory, LootScreen, Toasts};
use super::hud::UiFont;
use super::menu::MainMenu;
use super::pause::PauseMenu;
use super::save::Save;
use super::{CombatRng, CombatSet};
use crate::content::crafting::{
    affords, cost_line, drops_for, forge_cost, pay, Food, Mat, BOSS_DROPS, ENCHANTS,
};
use crate::content::items::ItemSlot;

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraftScreen>()
            .init_resource::<MatIcons>()
            .add_systems(
                Update,
                drop_materials
                    .in_set(CombatSet::Resolve)
                    .before(kill_enemies),
            )
            .add_systems(
                Update,
                craft_screen
                    .in_set(CombatSet::Ui)
                    .after(super::pause::pause_menu),
            );
    }
}

/// Dying enemies drop materials by kind; bosses a handful. They go straight into the save.
fn drop_materials(
    mut save: ResMut<Save>,
    mut rng: ResMut<CombatRng>,
    mut toasts: ResMut<Toasts>,
    enemies: Query<(&Enemy, Has<Boss>)>,
) {
    let mut got: HashMap<Mat, u32> = HashMap::new();
    for (e, boss) in &enemies {
        if e.alive() || e.xp == 0 {
            continue;
        }
        if boss {
            for (m, n) in BOSS_DROPS {
                *got.entry(m).or_default() += n;
            }
            continue;
        }
        for (m, chance) in drops_for(e.kind) {
            if rng.0.f32() < *chance {
                *got.entry(*m).or_default() += 1;
            }
        }
    }
    for (m, n) in got {
        save.data.mats[m.index()] += n;
        toasts.push(format!("+{n} {}", m.name()), Color::srgb(0.85, 0.85, 0.7));
    }
}

/// The open window, and the worn slot picked for the smith and the enchanter.
#[derive(Resource, Default)]
pub struct CraftScreen {
    open: Option<Entity>,
    pick: Option<ItemSlot>,
    dirty: bool,
}

impl CraftScreen {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Opens it from another screen (the main menu).
    pub fn request_open(&mut self) {
        if self.open.is_none() {
            self.dirty = true;
        }
    }
}

/// The materials', meals' icons from the client.
#[derive(Resource, Default)]
struct MatIcons(HashMap<&'static str, Option<Handle<Image>>>);

impl MatIcons {
    fn get(
        &mut self,
        name: &'static str,
        wa: Option<&mut WorldAssets>,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        if let Some(h) = self.0.get(name) {
            return h.clone();
        }
        let h = wa?.sprite_texture(&crate::models::icon_path(name), images);
        self.0.insert(name, h.clone());
        h
    }
}

/// What a button does.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum Craft {
    Cook(Food),
    PickFood(Food),
    PickSlot(ItemSlot),
    Forge,
    Enchant(usize),
    Close,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn craft_screen(
    mut commands: Commands,
    mut screen: ResMut<CraftScreen>,
    mut save: ResMut<Save>,
    mut clock: ResMut<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &Craft), Changed<Interaction>>,
    (cards, pause, menu, inv, loot): (
        Res<CardScreen>,
        Res<PauseMenu>,
        Res<MainMenu>,
        Res<Inventory>,
        Res<LootScreen>,
    ),
    font: Res<UiFont>,
    mut icons: ResMut<MatIcons>,
    mut wa: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let others = cards.is_open() || pause.is_open() || inv.is_open() || loot.is_open();
    let toggle = keys.just_pressed(KeyCode::KeyP);
    match screen.open {
        None if screen.dirty => {}
        None if toggle && !others && !menu.is_open() => {
            clock.pause();
            screen.dirty = true;
        }
        None => return,
        Some(root) => {
            let closed = buttons
                .iter()
                .any(|(i, c)| *i == Interaction::Pressed && *c == Craft::Close);
            if toggle || closed || keys.just_pressed(KeyCode::Escape) {
                commands.entity(root).try_despawn();
                screen.open = None;
                if !others && !menu.is_open() {
                    clock.unpause();
                }
                return;
            }
        }
    }
    let mut changed = false;
    if let Some((_, c)) = buttons.iter().find(|(i, _)| **i == Interaction::Pressed) {
        let d = &mut save.data;
        match *c {
            Craft::Cook(f) => {
                if pay(&mut d.mats, f.def().cost) {
                    d.foods[f.index()] += 1;
                }
            }
            Craft::PickFood(f) => {
                d.food_pick = if d.food_pick == Some(f) {
                    None
                } else {
                    Some(f)
                };
            }
            Craft::PickSlot(s) => screen.pick = Some(s),
            Craft::Forge => {
                if let Some(slot) = screen.pick {
                    if let Some(j) = d.equipped.iter().position(|i| i.slot == slot) {
                        if let Some(cost) = forge_cost(d.equipped[j].forge) {
                            if pay(&mut d.mats, &cost) {
                                d.equipped[j].forge += 1;
                            }
                        }
                    }
                }
            }
            Craft::Enchant(k) => {
                if let (Some(slot), Some(e)) = (screen.pick, ENCHANTS.get(k)) {
                    if let Some(j) = d.equipped.iter().position(|i| i.slot == slot) {
                        if pay(&mut d.mats, e.cost) {
                            d.equipped[j].enchant = Some(e.bonus);
                        }
                    }
                }
            }
            Craft::Close => {}
        }
        save.write();
        changed = true;
    }
    if !changed && !screen.dirty {
        return;
    }
    screen.dirty = false;
    if let Some(root) = screen.open.take() {
        commands.entity(root).try_despawn();
    }
    if screen.pick.is_none() {
        screen.pick = save.data.equipped.first().map(|i| i.slot);
    }
    let mut icon = |n: &'static str| icons.get(n, wa.as_deref_mut(), &mut images);
    screen.open = Some(spawn_screen(
        &mut commands,
        &save.data,
        screen.pick,
        &font,
        &mut icon,
    ));
}

fn small_icon(p: &mut ChildSpawnerCommands, img: Option<Handle<Image>>, size: f32) {
    let mut n = p.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.3)),
        BackgroundColor(Color::srgba(0.2, 0.2, 0.25, 0.9)),
    ));
    if let Some(img) = img {
        n.insert(ImageNode::new(img));
    }
}

fn button(p: &mut ChildSpawnerCommands, what: Craft, label: &str, enabled: bool, font: &UiFont) {
    p.spawn((
        what,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(if enabled {
            Color::srgba(0.3, 0.08, 0.04, 0.95)
        } else {
            Color::srgba(0.15, 0.15, 0.15, 0.8)
        }),
        BorderColor::all(if enabled {
            Color::srgb(0.9, 0.7, 0.3)
        } else {
            Color::srgba(0.5, 0.5, 0.5, 0.5)
        }),
    ))
    .with_children(|b| {
        b.spawn((
            Text::new(label),
            font.text(15.0),
            TextColor(if enabled {
                Color::WHITE
            } else {
                Color::srgb(0.55, 0.55, 0.55)
            }),
        ));
    });
}

fn column(
    p: &mut ChildSpawnerCommands,
    title: &str,
    font: &UiFont,
    f: impl FnOnce(&mut ChildSpawnerCommands),
) {
    p.spawn((
        Node {
            width: Val::Px(330.0),
            min_height: Val::Px(440.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(14.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.05, 0.04, 0.96)),
        BorderColor::all(Color::srgba(0.75, 0.6, 0.3, 0.55)),
    ))
    .with_children(|c| {
        c.spawn((
            Text::new(title),
            font.text(22.0),
            TextColor(Color::srgb(1.0, 0.82, 0.3)),
        ));
        f(c);
    });
}

fn spawn_screen(
    commands: &mut Commands,
    d: &super::save::SaveData,
    pick: Option<ItemSlot>,
    font: &UiFont,
    icon: &mut dyn FnMut(&'static str) -> Option<Handle<Image>>,
) -> Entity {
    let dim = Color::srgb(0.72, 0.7, 0.66);
    commands
        .spawn((
            Name::new("professions"),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
            GlobalZIndex(46),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("Meslekler"),
                font.text(34.0),
                TextColor(Color::srgb(1.0, 0.82, 0.3)),
            ));
            root.spawn(Node {
                column_gap: Val::Px(14.0),
                ..default()
            })
            .with_children(|cols| {
                // ---- Materials.
                column(cols, "Malzemeler", font, |c| {
                    for m in Mat::ALL {
                        c.spawn(Node {
                            column_gap: Val::Px(8.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|r| {
                            small_icon(r, icon(m.icon()), 32.0);
                            r.spawn((
                                Text::new(format!("{}  ×{}", m.name(), d.mats[m.index()])),
                                font.text(16.0),
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                    c.spawn((
                        Text::new(
                            "Kobold: bakır, keten · Kurt: et · Örümcek: bacak · Falcı: keten, toz \
                             · Bosslar: toz ve fazlası",
                        ),
                        font.text(13.0),
                        TextColor(dim),
                    ));
                });
                // ---- Cooking.
                column(cols, "Aşçılık", font, |c| {
                    for f in Food::ALL {
                        let def = f.def();
                        let picked = d.food_pick == Some(f);
                        c.spawn((
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(4.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                border: UiRect::all(Val::Px(if picked { 2.0 } else { 1.0 })),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BorderColor::all(if picked {
                                Color::srgb(0.3, 1.0, 0.3)
                            } else {
                                Color::srgba(0.6, 0.6, 0.6, 0.35)
                            }),
                        ))
                        .with_children(|r| {
                            r.spawn(Node {
                                column_gap: Val::Px(8.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|t| {
                                small_icon(t, icon(def.icon), 30.0);
                                t.spawn((
                                    Text::new(format!("{}  ×{}", def.name, d.foods[f.index()])),
                                    font.text(17.0),
                                    TextColor(Color::WHITE),
                                ));
                            });
                            r.spawn((
                                Text::new(def.desc),
                                font.text(14.0),
                                TextColor(Color::srgb(0.4, 1.0, 0.4)),
                            ));
                            r.spawn((
                                Text::new(format!("Gerekli: {}", cost_line(def.cost))),
                                font.text(13.0),
                                TextColor(dim),
                            ));
                            r.spawn(Node {
                                column_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|b| {
                                button(
                                    b,
                                    Craft::Cook(f),
                                    "Pişir",
                                    affords(&d.mats, def.cost),
                                    font,
                                );
                                button(
                                    b,
                                    Craft::PickFood(f),
                                    if picked {
                                        "Sonraki koşuya: seçili"
                                    } else {
                                        "Sonraki koşuya götür"
                                    },
                                    d.foods[f.index()] > 0 || picked,
                                    font,
                                );
                            });
                        });
                    }
                });
                // ---- Blacksmithing and enchanting, on a worn item.
                column(cols, "Demircilik ve Enchant", font, |c| {
                    if d.equipped.is_empty() {
                        c.spawn((
                            Text::new("Önce bir eşya giy (I)."),
                            font.text(15.0),
                            TextColor(dim),
                        ));
                        return;
                    }
                    c.spawn(Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(4.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for it in &d.equipped {
                            let sel = pick == Some(it.slot);
                            r.spawn((
                                Craft::PickSlot(it.slot),
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                                    border: UiRect::all(Val::Px(if sel { 2.0 } else { 1.0 })),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                BorderColor::all(if sel {
                                    Color::WHITE
                                } else {
                                    rarity_color(it.rarity)
                                }),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new(it.slot.name()),
                                    font.text(13.0),
                                    TextColor(rarity_color(it.rarity)),
                                ));
                            });
                        }
                    });
                    let Some(it) = pick.and_then(|s| d.worn(s)) else {
                        return;
                    };
                    c.spawn((
                        Text::new(it.name()),
                        font.text(18.0),
                        TextColor(rarity_color(it.rarity)),
                    ));
                    c.spawn((
                        Text::new(it.lines().join("\n")),
                        font.text(13.0),
                        TextColor(Color::WHITE),
                    ));
                    match forge_cost(it.forge) {
                        Some(cost) => {
                            c.spawn((
                                Text::new(format!("Döv (+{}): {}", it.forge + 1, cost_line(&cost))),
                                font.text(13.0),
                                TextColor(dim),
                            ));
                            button(c, Craft::Forge, "Döv", affords(&d.mats, &cost), font);
                        }
                        None => {
                            c.spawn((
                                Text::new("En yüksek dövme seviyesinde."),
                                font.text(13.0),
                                TextColor(dim),
                            ));
                        }
                    }
                    c.spawn((
                        Text::new("Enchant (eskisinin yerine geçer):"),
                        font.text(13.0),
                        TextColor(dim),
                    ));
                    for (k, e) in ENCHANTS.iter().enumerate() {
                        button(
                            c,
                            Craft::Enchant(k),
                            &format!(
                                "{}: {} ({})",
                                e.name,
                                e.bonus.0.line(e.bonus.1),
                                cost_line(e.cost)
                            ),
                            affords(&d.mats, e.cost),
                            font,
                        );
                    }
                });
            });
            root.spawn(Node::default()).with_children(|r| {
                button(r, Craft::Close, "Kapat (P / Esc)", true, font);
            });
        })
        .id()
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_params() {
        use bevy::ecs::system::{IntoSystem, System};
        let mut world = bevy::prelude::World::new();
        IntoSystem::into_system(super::drop_materials).initialize(&mut world);
        IntoSystem::into_system(super::craft_screen).initialize(&mut world);
    }
}
