//! Phase 4 in the game: loot bags from elites and mini bosses, the boss chest's loot screen, the
//! inventory screen (I), the hero dressed in what it wears, and loot toasts.

use std::collections::HashMap;

use bevy::prelude::*;

use benilla_assets::WorldAssets;

use super::art::{WowArt, DIALOG_BACKGROUND};
use super::bosses::Boss;
use super::cards::CardScreen;
use super::enemies::{kill_enemies, Enemy};
use super::hud::UiFont;
use super::menu::MainMenu;
use super::pause::PauseMenu;
use super::portal::{ChestOpened, ClearScreen, MapCleared};
use super::rules::{flat_dist, Bonus, Stats};
use super::save::{Save, BAG_SIZE};
use super::{hero_on_ground, CombatRng, CombatSet, HeroDied, RestartRun, Run};
use crate::content::bosses::Rank;
use crate::content::items::{roll_drop, Item, ItemSlot, Rarity, Source};
use crate::game::Hero;
use crate::models::{self, CharacterLook, Gear, GearSlot, ModelData, ModelRequest, ModelRoot};

pub struct GearUiPlugin;

impl Plugin for GearUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .init_resource::<LootScreen>()
            .init_resource::<Toasts>()
            .init_resource::<IconCache>()
            .add_systems(Startup, build_loot_assets)
            .add_systems(
                Update,
                drop_from_bosses
                    .in_set(CombatSet::Resolve)
                    .before(kill_enemies),
            )
            .add_systems(
                Update,
                (pick_up_bags, spin_bags)
                    .in_set(CombatSet::Act)
                    .run_if(hero_on_ground),
            )
            .add_systems(Update, collect_bags_at_run_end.in_set(CombatSet::Reset))
            .add_systems(
                Update,
                (
                    open_chest_loot,
                    loot_screen,
                    inventory,
                    tint_slot_pictures,
                    show_toasts,
                    dress_hero,
                )
                    .chain()
                    .in_set(CombatSet::Ui)
                    // Esc is the pause menu's first: it must see this screen open.
                    .after(super::pause::pause_menu),
            );
    }
}

/// The models' slot for an item slot; jewellery and the back have none (an icon only).
pub fn gear_slot(slot: ItemSlot) -> Option<GearSlot> {
    match slot {
        ItemSlot::Head => Some(GearSlot::Head),
        ItemSlot::Chest => Some(GearSlot::Chest),
        ItemSlot::Hands => Some(GearSlot::Hands),
        ItemSlot::Legs => Some(GearSlot::Legs),
        ItemSlot::Feet => Some(GearSlot::Feet),
        ItemSlot::Weapon => Some(GearSlot::MainHand),
        ItemSlot::Neck | ItemSlot::Back | ItemSlot::Finger | ItemSlot::Trinket => None,
    }
}

pub fn rarity_color(r: Rarity) -> Color {
    let c = r.color();
    Color::srgb(c[0], c[1], c[2])
}

/// The difference wearing `new` instead of `worn` makes, as tooltip lines ("+%1.2 büyü hasarı").
pub fn compare_lines(new: &Item, worn: Option<&Item>) -> Vec<String> {
    let a = Bonus::from_gear(std::iter::once(new));
    let b = Bonus::from_gear(worn.into_iter());
    let pct = |v: f32| (v * 1000.0).round() / 10.0;
    let mut out = Vec::new();
    let mut push = |d: f32, pos: &str, neg: &str, hp: bool| {
        if d.abs() < 1e-4 {
            return;
        }
        let n = if hp {
            format!("{:.0}", d.abs())
        } else {
            format!("%{}", pct(d.abs()))
        };
        out.push(if d > 0.0 {
            format!("+{n} {pos}")
        } else {
            format!("-{n} {neg}")
        });
    };
    push(a.damage - b.damage, "hasar", "hasar", false);
    push(a.haste - b.haste, "hızlı bekleme", "hızlı bekleme", false);
    push(a.speed - b.speed, "koşu hızı", "koşu hızı", false);
    push(a.health - b.health, "can", "can", true);
    push(a.magnet - b.magnet, "XP çekimi", "XP çekimi", false);
    push(a.crit - b.crit, "kritik", "kritik", false);
    push(a.xp - b.xp, "XP", "XP", false);
    out
}

// -------------------------------------------------------------------------------------------
// Loot bags and the chest.

#[derive(Resource)]
struct LootAssets {
    mesh: Handle<Mesh>,
    by_rarity: HashMap<Rarity, Handle<StandardMaterial>>,
}

fn build_loot_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let by_rarity = Rarity::ALL
        .into_iter()
        .map(|r| {
            let c = r.color();
            (
                r,
                materials.add(StandardMaterial {
                    base_color: Color::srgb(c[0], c[1], c[2]),
                    unlit: true,
                    ..default()
                }),
            )
        })
        .collect();
    commands.insert_resource(LootAssets {
        mesh: meshes.add(Cuboid::new(0.5, 0.5, 0.5)),
        by_rarity,
    });
}

/// A dropped bag of items, waiting to be walked over.
#[derive(Component)]
pub struct LootBag {
    items: Vec<Item>,
    base_y: f32,
    age: f32,
}

/// On a boss whose drop was rolled.
#[derive(Component)]
struct Dropped;

const BAG_TOUCH: f32 = 1.8;

/// An elite or mini boss that dies drops a bag with one item.
fn drop_from_bosses(
    mut commands: Commands,
    run: Res<Run>,
    mut rng: ResMut<CombatRng>,
    assets: Res<LootAssets>,
    bosses: Query<(Entity, &Transform, &Enemy, &Boss), Without<Dropped>>,
) {
    for (e, xf, enemy, boss) in &bosses {
        if enemy.alive() || boss.def().rank == Rank::MapBoss {
            continue;
        }
        commands.entity(e).insert(Dropped);
        let items = roll_drop(&mut rng.0, Source::Elite, run.level);
        let best = items
            .iter()
            .map(|i| i.rarity)
            .max()
            .unwrap_or(Rarity::Common);
        info!(
            "rnsurvivors: {} dropped {:?}",
            boss.def().name(),
            items.iter().map(Item::name).collect::<Vec<_>>()
        );
        commands
            .spawn((
                Name::new("loot bag"),
                LootBag {
                    items,
                    base_y: xf.translation.y + 0.6,
                    age: 0.0,
                },
                Transform::from_translation(xf.translation + Vec3::Y * 0.6),
                Visibility::default(),
            ))
            .with_children(|c| {
                // The plain cube until the client's sack model is in (`fx`).
                c.spawn((
                    Mesh3d(assets.mesh.clone()),
                    MeshMaterial3d(assets.by_rarity[&best].clone()),
                    Transform::default(),
                    crate::models::ModelPlaceholder,
                ));
            });
    }
}

fn spin_bags(time: Res<Time<Real>>, mut bags: Query<(&mut LootBag, &mut Transform)>) {
    for (mut b, mut xf) in &mut bags {
        b.age += time.delta_secs();
        xf.rotation = Quat::from_rotation_y(b.age * 1.5);
        xf.translation.y = b.base_y + 0.15 * (b.age * 3.0).sin();
    }
}

fn pick_up_bags(
    mut commands: Commands,
    mut save: ResMut<Save>,
    mut toasts: ResMut<Toasts>,
    heroes: Query<&Transform, (With<Hero>, Without<LootBag>)>,
    bags: Query<(Entity, &Transform, &LootBag)>,
) {
    let Ok(hero) = heroes.single() else {
        return;
    };
    for (e, xf, bag) in &bags {
        if flat_dist(xf.translation, hero.translation) > BAG_TOUCH {
            continue;
        }
        commands.entity(e).try_despawn();
        take(&mut save, &mut toasts, bag.items.clone());
    }
}

/// Puts items in the bag, toasting each, and saves.
fn take(save: &mut Save, toasts: &mut Toasts, items: Vec<Item>) {
    for it in &items {
        toasts.push(format!("Ganimet: {}", it.name()), rarity_color(it.rarity));
    }
    let lost = save.data.take_loot(items);
    if !lost.is_empty() {
        toasts.push(
            format!("Çanta dolu! {} eşya bırakıldı.", lost.len()),
            Color::srgb(1.0, 0.4, 0.3),
        );
    }
    save.write();
}

/// A run that ends with bags still on the ground keeps them: they go into the bag.
fn collect_bags_at_run_end(
    mut commands: Commands,
    mut save: ResMut<Save>,
    mut toasts: ResMut<Toasts>,
    mut died: MessageReader<HeroDied>,
    mut cleared: MessageReader<MapCleared>,
    mut restarted: MessageReader<RestartRun>,
    bags: Query<(Entity, &LootBag)>,
) {
    let ended = died.read().count() + cleared.read().count() + restarted.read().count() > 0;
    if !ended {
        return;
    }
    for (e, bag) in &bags {
        commands.entity(e).try_despawn();
        take(&mut save, &mut toasts, bag.items.clone());
    }
}

/// The chest's loot, shown before it goes into the bag.
#[derive(Resource, Default)]
pub struct LootScreen {
    open: Option<(Entity, Vec<Item>)>,
}

impl LootScreen {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

#[allow(clippy::too_many_arguments)]
fn open_chest_loot(
    mut commands: Commands,
    mut opened: MessageReader<ChestOpened>,
    mut screen: ResMut<LootScreen>,
    mut clock: ResMut<Time<Virtual>>,
    mut rng: ResMut<CombatRng>,
    run: Res<Run>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    mut icons: ResMut<IconCache>,
    data: Option<Res<ModelData>>,
    mut wa: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    if opened.read().count() == 0 || screen.is_open() {
        return;
    }
    let items = roll_drop(&mut rng.0, Source::Chest, run.level);
    clock.pause();
    let gold = Color::srgb(1.0, 0.82, 0.0);
    let mut root = commands.spawn((
        Name::new("loot screen"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(18.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        GlobalZIndex(25),
        Button,
    ));
    root.with_children(|p| {
        p.spawn((
            Text::new("Ganimet sandığı!"),
            font.text(36.0),
            TextColor(gold),
        ));
        p.spawn(Node {
            column_gap: Val::Px(18.0),
            ..default()
        })
        .with_children(|row| {
            for it in &items {
                let icon = icons.get(it, data.as_deref(), wa.as_deref_mut(), &mut images);
                item_card(row, it, None, &font, &art, icon, 250.0);
            }
        });
        p.spawn((
            Text::new("Enter ya da tıkla: hepsini çantaya al"),
            font.text(18.0),
            TextColor(Color::srgb(0.85, 0.85, 0.85)),
        ));
    });
    screen.open = Some((root.id(), items));
}

#[allow(clippy::too_many_arguments)]
fn loot_screen(
    mut commands: Commands,
    mut screen: ResMut<LootScreen>,
    mut clock: ResMut<Time<Virtual>>,
    mut save: ResMut<Save>,
    mut toasts: ResMut<Toasts>,
    keys: Res<ButtonInput<KeyCode>>,
    clicks: Query<&Interaction, Changed<Interaction>>,
    cards: Res<CardScreen>,
) {
    let Some((root, _)) = &screen.open else {
        return;
    };
    let root = *root;
    let by_key = keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]);
    let by_click = clicks.get(root).is_ok_and(|i| *i == Interaction::Pressed);
    if !by_key && !by_click {
        return;
    }
    let (_, items) = screen.open.take().expect("open");
    take(&mut save, &mut toasts, items);
    commands.entity(root).try_despawn();
    if !cards.is_open() {
        clock.unpause();
    }
}

// -------------------------------------------------------------------------------------------
// Icons: each item's look comes from the client's ItemDisplayInfo; its icon from the same row.

#[derive(Resource, Default)]
pub struct IconCache(HashMap<(ItemSlot, u32), Option<Handle<Image>>>);

impl IconCache {
    pub(super) fn get(
        &mut self,
        it: &Item,
        data: Option<&ModelData>,
        wa: Option<&mut WorldAssets>,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let key = (it.slot, it.look);
        if let Some(h) = self.0.get(&key) {
            return h.clone();
        }
        let wa = wa?;
        let handle = match gear_slot(it.slot) {
            Some(gs) => {
                let data = data?;
                // The item tables may still be loading: nothing is remembered until they are in.
                data.tables.as_ref()?.items.as_ref()?;
                models::item_look(data, gs, it.look)
                    .and_then(|(_, icon)| icon)
                    .and_then(|path| wa.sprite_texture(&path, images))
            }
            None => {
                let names = it.slot.icon_names();
                names
                    .get(it.look as usize % names.len().max(1))
                    .and_then(|n| wa.sprite_texture(&models::icon_path(n), images))
            }
        };
        self.0.insert(key, handle.clone());
        handle
    }
}

/// An item as a card: icon, name in its rarity's colour, its lines, and how it compares.
fn item_card(
    p: &mut ChildSpawnerCommands,
    it: &Item,
    worn: Option<&Item>,
    font: &UiFont,
    art: &WowArt,
    icon: Option<Handle<Image>>,
    width: f32,
) {
    let color = rarity_color(it.rarity);
    let mut card = p.spawn((
        Node {
            width: Val::Px(width),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.06, 0.06, 0.09, 0.96)),
        BorderColor::all(color),
    ));
    if let Some(bg) = art.ui(DIALOG_BACKGROUND) {
        card.insert(ImageNode::new(bg).with_mode(NodeImageMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: 1.0,
        }));
    }
    card.with_children(|c| {
        c.spawn(Node {
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|h| {
            icon_box(h, it, icon, font, 48.0);
            h.spawn((Text::new(it.name()), font.text(20.0), TextColor(color)));
        });
        c.spawn((
            Text::new(it.lines().join("\n")),
            font.text(16.0),
            TextColor(Color::srgb(0.92, 0.92, 0.88)),
        ));
        let cmp = compare_lines(it, worn);
        if worn.is_some() && !cmp.is_empty() {
            c.spawn((
                Text::new(format!("Giyilene göre:\n{}", cmp.join("\n"))),
                font.text(15.0),
                TextColor(Color::srgb(0.75, 0.75, 0.75)),
            ));
        }
    });
}

fn icon_box(
    p: &mut ChildSpawnerCommands,
    it: &Item,
    icon: Option<Handle<Image>>,
    font: &UiFont,
    size: f32,
) {
    let color = rarity_color(it.rarity);
    p.spawn((
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            border: UiRect::all(Val::Px(2.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(color),
        BackgroundColor(color.with_alpha(0.25)),
    ))
    .with_children(|b| match icon {
        Some(img) => {
            b.spawn((
                ImageNode::new(img),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
            ));
        }
        None => {
            b.spawn((
                Text::new(it.slot.name().chars().next().unwrap_or('?').to_string()),
                font.text(size * 0.5),
                TextColor(Color::WHITE),
            ));
        }
    });
}

// -------------------------------------------------------------------------------------------
// The inventory screen.

/// What is selected on the inventory screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selected {
    Bag(usize),
    Worn(ItemSlot),
}

#[derive(Resource, Default)]
pub struct Inventory {
    open: Option<Entity>,
    selected: Option<Selected>,
    /// The screen needs drawing again.
    dirty: bool,
}

impl Inventory {
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

#[derive(Component, Clone, Copy)]
struct InvButton(Selected);

/// The inventory: I toggles it during a run (it pauses), the main menu opens it too. Click an
/// item to select it, click it again or press E to wear it; U takes off, X throws away.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn inventory(
    mut commands: Commands,
    mut inv: ResMut<Inventory>,
    mut save: ResMut<Save>,
    mut clock: ResMut<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &InvButton), Changed<Interaction>>,
    (cards, pause, menu, clear, loot): (
        Res<CardScreen>,
        Res<PauseMenu>,
        Res<MainMenu>,
        Res<ClearScreen>,
        Res<LootScreen>,
    ),
    run: Res<Run>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    mut icons: ResMut<IconCache>,
    data: Option<Res<ModelData>>,
    mut wa: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    (figure, closes): (
        Option<Res<super::char_view::CharView>>,
        Query<&Interaction, (Changed<Interaction>, With<InvClose>)>,
    ),
) {
    let others = cards.is_open() || pause.is_open() || clear.is_open() || loot.is_open();
    let toggle = keys.just_pressed(KeyCode::KeyI);
    match inv.open {
        None if inv.dirty => {}
        None if toggle && !others && !menu.is_open() => {
            clock.pause();
            inv.dirty = true;
        }
        None => return,
        Some(root) => {
            let closed = closes.iter().any(|i| *i == Interaction::Pressed);
            if toggle || closed || keys.just_pressed(KeyCode::Escape) {
                commands.entity(root).try_despawn();
                inv.open = None;
                inv.selected = None;
                if !others && !menu.is_open() {
                    clock.unpause();
                }
                return;
            }
        }
    }

    // Input on the open screen.
    let mut changed = false;
    if let Some((_, b)) = buttons.iter().find(|(i, _)| **i == Interaction::Pressed) {
        let sel = b.0;
        if inv.selected == Some(sel) {
            match sel {
                Selected::Bag(i) => save.data.equip(i),
                Selected::Worn(slot) => save.data.unequip(slot),
            }
            inv.selected = None;
            save.write();
        } else {
            inv.selected = Some(sel);
        }
        changed = true;
    }
    match inv.selected {
        Some(Selected::Bag(i)) if keys.just_pressed(KeyCode::KeyE) => {
            save.data.equip(i);
            inv.selected = None;
            save.write();
            changed = true;
        }
        Some(Selected::Bag(i)) if keys.just_pressed(KeyCode::KeyX) => {
            save.data.discard(i);
            inv.selected = None;
            save.write();
            changed = true;
        }
        Some(Selected::Worn(slot)) if keys.just_pressed(KeyCode::KeyU) => {
            save.data.unequip(slot);
            inv.selected = None;
            save.write();
            changed = true;
        }
        _ => {}
    }
    if !changed && !inv.dirty {
        return;
    }
    inv.dirty = false;
    if let Some(root) = inv.open.take() {
        commands.entity(root).try_despawn();
    }
    let mut icon = |it: &Item| icons.get(it, data.as_deref(), wa.as_deref_mut(), &mut images);
    inv.open = Some(spawn_inventory(
        &mut commands,
        &save.data,
        inv.selected,
        &run,
        &font,
        &art,
        figure.map(|f| f.image.clone()),
        &mut icon,
    ));
}

/// The character window's scale over the client's own pixels.
const CW_SCALE: f32 = 1.2;

fn cw(v: f32) -> Val {
    Val::Px(v * CW_SCALE)
}

/// A box in the character window, in client pixels.
fn cw_at(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: cw(left),
        top: cw(top),
        width: cw(w),
        height: cw(h),
        ..default()
    }
}

/// The paper doll's slots (the 1.12 window's places, 37 pixels, 41 apart): the picture index in
/// `SLOT_PICTURES`, where it sits, and the game's slot it holds (the rest are for later).
pub fn doll_slots() -> Vec<(usize, Vec2, Option<ItemSlot>)> {
    let left: [(usize, Option<ItemSlot>); 8] = [
        (0, Some(ItemSlot::Head)),
        (1, Some(ItemSlot::Neck)),
        (2, None),
        (3, Some(ItemSlot::Back)), // back: the chest picture
        (3, Some(ItemSlot::Chest)),
        (4, None),
        (5, None),
        (6, None),
    ];
    let right: [(usize, Option<ItemSlot>); 8] = [
        (7, Some(ItemSlot::Hands)),
        (8, None),
        (9, Some(ItemSlot::Legs)),
        (10, Some(ItemSlot::Feet)),
        (11, Some(ItemSlot::Finger)),
        (11, None),
        (12, Some(ItemSlot::Trinket)),
        (12, None),
    ];
    let mut out = Vec::new();
    for (i, (pic, slot)) in left.into_iter().enumerate() {
        out.push((pic, Vec2::new(21.0, 74.0 + 41.0 * i as f32), slot));
    }
    for (i, (pic, slot)) in right.into_iter().enumerate() {
        out.push((pic, Vec2::new(305.0, 74.0 + 41.0 * i as f32), slot));
    }
    out.push((13, Vec2::new(122.0, 386.0), Some(ItemSlot::Weapon)));
    out.push((14, Vec2::new(163.0, 386.0), None));
    out.push((15, Vec2::new(204.0, 386.0), None));
    out
}

/// The window's two stat boxes: (label, value) rows.
pub fn doll_stats(s: &Stats) -> [Vec<(String, String)>; 2] {
    let pct = |f: f32| format!("{:+.0}%", f * 100.0);
    [
        vec![
            ("Can".into(), format!("{:.0}", s.max_health)),
            ("Hasar".into(), pct(s.damage_mul - 1.0)),
            ("Hızlanma".into(), pct(1.0 - s.cooldown_mul)),
            ("Kritik".into(), format!("{:.1}%", s.crit_chance * 100.0)),
            (
                "Kritik hasarı".into(),
                format!("{:.0}%", s.crit_mul * 100.0),
            ),
        ],
        vec![
            ("Koşu hızı".into(), pct(s.speed_mul - 1.0)),
            ("Yenilenme".into(), format!("{:.1}/sn", s.regen)),
            ("Toplama".into(), pct(s.magnet_mul - 1.0)),
            ("XP".into(), pct(s.xp_mul - 1.0)),
        ],
    ]
}

/// The window's close button.
#[derive(Component)]
struct InvClose;

#[allow(clippy::too_many_arguments)]
fn spawn_inventory(
    commands: &mut Commands,
    d: &super::save::SaveData,
    selected: Option<Selected>,
    run: &Run,
    font: &UiFont,
    art: &WowArt,
    figure: Option<Handle<Image>>,
    icon: &mut dyn FnMut(&Item) -> Option<Handle<Image>>,
) -> Entity {
    use super::art::{CLASS_ICONS, CLOSE_BUTTON, PAPERDOLL, SLOT_PICTURES};
    use super::hud::{Fallback, WowSkin};
    let gold = Color::srgb(1.0, 0.82, 0.0);
    let dim = Color::srgb(0.75, 0.75, 0.75);
    let shadow = TextShadow {
        offset: Vec2::splat(1.0),
        color: Color::BLACK,
    };
    let stats = Stats::from_parts(&run.passives, &d.bonus());
    let mut root = commands.spawn((
        Name::new("inventory"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        GlobalZIndex(45),
    ));
    root.with_children(|p| {
        // The character window: the client's paper doll frame, 384×512 pixels.
        p.spawn(Node {
            width: cw(384.0),
            height: cw(512.0),
            ..default()
        })
        .with_children(|w| {
            w.spawn((
                Node {
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..cw_at(12.0, 12.0, 336.0, 428.0)
                },
                BackgroundColor(Color::srgba(0.06, 0.06, 0.08, 0.97)),
                BorderColor::all(gold.with_alpha(0.7)),
                Fallback(PAPERDOLL[0]),
            ));
            for (i, (x, y, pw)) in [
                (0.0, 0.0, 256.0),
                (256.0, 0.0, 128.0),
                (0.0, 256.0, 256.0),
                (256.0, 256.0, 128.0),
            ]
            .into_iter()
            .enumerate()
            {
                w.spawn((
                    cw_at(x + 2.0, y - 1.0, pw, 256.0),
                    WowSkin::new(PAPERDOLL[i], [0.0, 0.0, 1.0, 1.0]),
                ));
            }
            // The figure, over the frame's middle (as the client draws its model).
            let mut fig = w.spawn((
                cw_at(66.0, 76.0, 232.0, 215.0),
                BackgroundColor(Color::srgb(0.04, 0.035, 0.03)),
            ));
            if let Some(img) = figure {
                fig.insert(ImageNode::new(img));
            }
            w.spawn((
                Node {
                    border_radius: BorderRadius::all(Val::Percent(50.0)),
                    ..cw_at(7.0, 6.0, 60.0, 60.0)
                },
                BackgroundColor(Color::srgb(0.25, 0.3, 0.6)),
                WowSkin::new(
                    CLASS_ICONS,
                    crate::content::classes::current().def().icon_uv,
                ),
            ));
            w.spawn((
                InvClose,
                Button,
                Node {
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..cw_at(323.0, 8.0, 32.0, 32.0)
                },
                WowSkin::new(CLOSE_BUTTON, [0.0, 0.0, 1.0, 1.0]),
            ));
            // Name and level.
            w.spawn(Node {
                justify_content: JustifyContent::Center,
                ..cw_at(70.0, 15.0, 260.0, 16.0)
            })
            .with_children(|n| {
                n.spawn((
                    Text::new(crate::content::classes::current().def().name),
                    font.text(14.0 * CW_SCALE),
                    TextColor(Color::WHITE),
                    shadow,
                ));
            });
            w.spawn(Node {
                justify_content: JustifyContent::Center,
                ..cw_at(70.0, 34.0, 260.0, 14.0)
            })
            .with_children(|n| {
                n.spawn((
                    Text::new(format!(
                        "Seviye {} {}   ·   Çanta {}/{BAG_SIZE}",
                        run.level,
                        crate::content::classes::current().def().name,
                        d.bag.len()
                    )),
                    font.text(11.0 * CW_SCALE),
                    TextColor(gold),
                    shadow,
                ));
            });
            // The slots.
            for (pic, at, slot) in doll_slots() {
                let worn = slot.and_then(|s| d.worn(s));
                let sel = slot.is_some_and(|s| selected == Some(Selected::Worn(s)));
                let border = match (sel, worn) {
                    (true, _) => Color::WHITE,
                    (false, Some(it)) => rarity_color(it.rarity),
                    _ => Color::srgba(0.0, 0.0, 0.0, 0.0),
                };
                let mut cell = w.spawn((
                    Node {
                        border: UiRect::all(Val::Px(2.0)),
                        ..cw_at(at.x, at.y, 37.0, 37.0)
                    },
                    BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.9)),
                    BorderColor::all(border),
                ));
                if let Some(s) = slot {
                    cell.insert((InvButton(Selected::Worn(s)), Button));
                }
                cell.with_children(|c| {
                    let picture = Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    };
                    match worn.and_then(&mut *icon) {
                        Some(img) => {
                            c.spawn((ImageNode::new(img), picture));
                        }
                        None => {
                            // The empty slot's picture; a slot for later is dimmer.
                            let tint = if slot.is_some() { 1.0 } else { 0.45 };
                            c.spawn((
                                picture,
                                WowSkin::new(SLOT_PICTURES[pic], [0.0, 0.0, 1.0, 1.0]),
                                ImageNodeTint(tint),
                            ));
                        }
                    }
                });
            }
            // The stat boxes under the figure.
            for (i, rows) in doll_stats(&stats).into_iter().enumerate() {
                w.spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(cw(5.0), cw(3.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..cw_at(67.0 + 117.0 * i as f32, 293.0, 113.0, 88.0)
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
                    BorderColor::all(Color::srgba(0.6, 0.6, 0.6, 0.5)),
                ))
                .with_children(|b| {
                    for (label, value) in rows {
                        b.spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            width: Val::Percent(100.0),
                            ..default()
                        })
                        .with_children(|r| {
                            r.spawn((
                                Text::new(label),
                                font.text(10.5 * CW_SCALE),
                                TextColor(gold),
                            ));
                            r.spawn((
                                Text::new(value),
                                font.text(10.5 * CW_SCALE),
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });
            }
        });

        // The bag beside it, and the selection's card.
        p.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.95)),
            BorderColor::all(gold.with_alpha(0.6)),
        ))
        .with_children(|c| {
            c.spawn((
                Text::new(format!("Çanta {}/{BAG_SIZE}", d.bag.len())),
                font.text(18.0),
                TextColor(gold),
            ));
            c.spawn(Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(6, 44.0),
                row_gap: Val::Px(4.0),
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|g| {
                for i in 0..BAG_SIZE {
                    let sel = selected == Some(Selected::Bag(i));
                    let mut cell = g.spawn((
                        Node {
                            width: Val::Px(44.0),
                            height: Val::Px(44.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BorderColor::all(if sel {
                            Color::WHITE
                        } else {
                            Color::srgba(1.0, 1.0, 1.0, 0.12)
                        }),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                    ));
                    if let Some(it) = d.bag.get(i) {
                        let ic = icon(it);
                        cell.insert((InvButton(Selected::Bag(i)), Button));
                        cell.with_children(|b| icon_box(b, it, ic, font, 40.0));
                    }
                }
            });
            let (item, worn, hint) = match selected {
                Some(Selected::Bag(i)) => d
                    .bag
                    .get(i)
                    .map(|it| (it, d.worn(it.slot), "Tekrar tıkla ya da E: giy   ·   X: at")),
                Some(Selected::Worn(s)) => d
                    .worn(s)
                    .map(|it| (it, None, "Tekrar tıkla ya da U: çıkar")),
                None => None,
            }
            .map_or((None, None, ""), |(a, b, h)| (Some(a), b, h));
            match item {
                Some(it) => {
                    let ic = icon(it);
                    item_card(c, it, worn, font, art, ic, 284.0);
                    c.spawn((Text::new(hint), font.text(14.0), TextColor(dim)));
                }
                None => {
                    c.spawn((
                        Text::new("Bir eşyaya tıkla."),
                        font.text(15.0),
                        TextColor(dim),
                    ));
                }
            }
            c.spawn((
                Text::new("I ya da Esc: kapat"),
                font.text(14.0),
                TextColor(dim),
            ));
        });
    });
    root.id()
}

/// A dimmer empty-slot picture (applied once its texture is on).
#[derive(Component)]
struct ImageNodeTint(f32);

fn tint_slot_pictures(mut q: Query<(&ImageNodeTint, &mut ImageNode), Added<ImageNode>>) {
    for (t, mut img) in &mut q {
        img.color = Color::srgba(t.0, t.0, t.0, 1.0);
    }
}

// -------------------------------------------------------------------------------------------
// The hero wears its gear.

/// When what is worn changes, the hero's model is rebuilt wearing it (over the default robe and
/// staff). A gear set by `RNS_HERO_GEAR` is left alone.
fn dress_hero(
    mut commands: Commands,
    save: Res<Save>,
    heroes: Query<(Entity, &ModelRequest, Option<&ModelRoot>), With<Hero>>,
    mut worn_last: Local<Option<Vec<(GearSlot, u32)>>>,
) {
    let Ok((e, request, root)) = heroes.single() else {
        return;
    };
    let ModelRequest::Character(look) = request else {
        return;
    };
    if matches!(look.gear, Gear::Items(_)) {
        return;
    }
    let picks: Vec<(GearSlot, u32)> = save
        .data
        .equipped
        .iter()
        .filter_map(|it| gear_slot(it.slot).map(|g| (g, it.look)))
        .collect();
    if worn_last.as_ref() == Some(&picks) {
        return;
    }
    *worn_last = Some(picks.clone());
    let gear = if picks.is_empty() {
        Gear::MageDefault
    } else {
        Gear::MageWith(picks)
    };
    if look.gear == gear {
        return;
    }
    let look = CharacterLook {
        gear,
        ..look.clone()
    };
    models::rebuild_model(
        &mut commands,
        e,
        ModelRequest::Character(look),
        root.map(|r| r.0),
    );
}

// -------------------------------------------------------------------------------------------
// Toasts.

/// Short notes on the right side: loot picked up.
#[derive(Resource, Default)]
pub struct Toasts {
    queue: Vec<(String, Color)>,
}

impl Toasts {
    pub fn push(&mut self, text: String, color: Color) {
        self.queue.push((text, color));
    }
}

#[derive(Component)]
struct Toast {
    left: f32,
}

fn show_toasts(
    mut commands: Commands,
    real: Res<Time<Real>>,
    mut toasts: ResMut<Toasts>,
    font: Res<UiFont>,
    mut live: Query<(Entity, &mut Toast, &mut Node, &mut TextColor)>,
) {
    let dt = real.delta_secs();
    let mut n = 0;
    for (e, mut t, mut node, mut color) in &mut live {
        t.left -= dt;
        if t.left <= 0.0 {
            commands.entity(e).try_despawn();
            continue;
        }
        node.top = Val::Px(180.0 + 26.0 * n as f32);
        color.0 = color.0.with_alpha(t.left.min(1.0));
        n += 1;
    }
    for (text, color) in toasts.queue.drain(..) {
        commands.spawn((
            Name::new("toast"),
            Toast { left: 5.0 },
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(18.0),
                top: Val::Px(180.0 + 26.0 * n as f32),
                ..default()
            },
            Text::new(text),
            font.text(18.0),
            TextColor(color),
            TextShadow::default(),
            GlobalZIndex(50),
        ));
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::items::Affix;

    fn item(slot: ItemSlot, affixes: Vec<(Affix, f32)>) -> Item {
        Item {
            slot,
            rarity: Rarity::Uncommon,
            level: 1,
            affixes,
            look: 0,
            forge: 0,
            enchant: None,
        }
    }

    #[test]
    fn comparisons_show_gains_and_losses() {
        let new = item(
            ItemSlot::Head,
            vec![(Affix::Power, 0.05), (Affix::Health, 10.0)],
        );
        let worn = item(
            ItemSlot::Head,
            vec![(Affix::Power, 0.02), (Affix::Crit, 0.03)],
        );
        let lines = compare_lines(&new, Some(&worn));
        assert!(lines.contains(&"+%3 hasar".to_string()), "{lines:?}");
        assert!(lines.contains(&"+10 can".to_string()), "{lines:?}");
        assert!(lines.contains(&"-%3 kritik".to_string()), "{lines:?}");
        assert_eq!(compare_lines(&new, Some(&new)), Vec::<String>::new());
    }

    #[test]
    fn every_slot_maps_to_a_model_slot_or_icons() {
        let mut seen = std::collections::HashSet::new();
        for s in ItemSlot::ALL {
            match gear_slot(s) {
                Some(g) => assert!(seen.insert(g)),
                None => assert!(!s.icon_names().is_empty(), "{s:?}"),
            }
        }
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(drop_from_bosses).initialize(&mut world);
        IntoSystem::into_system(pick_up_bags).initialize(&mut world);
        IntoSystem::into_system(spin_bags).initialize(&mut world);
        IntoSystem::into_system(collect_bags_at_run_end).initialize(&mut world);
        IntoSystem::into_system(open_chest_loot).initialize(&mut world);
        IntoSystem::into_system(loot_screen).initialize(&mut world);
        IntoSystem::into_system(inventory).initialize(&mut world);
        IntoSystem::into_system(dress_hero).initialize(&mut world);
        IntoSystem::into_system(show_toasts).initialize(&mut world);
        IntoSystem::into_system(build_loot_assets).initialize(&mut world);
    }
}
