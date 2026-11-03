//! Phase 5 in the game: the talent screen, opened from the main menu. Points come from all the XP
//! gathered in runs; a click buys a rank, R gives every point back.

use bevy::prelude::*;

use benilla_assets::{LockRecover, WorldAssets};
use benilla_formats::load_spell_catalog;

use super::art::{pick_spell, WowArt, DIALOG_BACKGROUND};
use super::hud::UiFont;
use super::save::Save;
use super::CombatSet;
use crate::content::talents::{xp_to_next_point, Column, TalentId, Talents, TIER_GATE};

pub struct TalentUiPlugin;

impl Plugin for TalentUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TalentScreen>()
            .init_resource::<TalentIcons>()
            .add_systems(
                Startup,
                load_talent_icons.after(benilla_assets::AssetSet::Open),
            )
            .add_systems(
                Update,
                talent_screen
                    .in_set(CombatSet::Ui)
                    .after(super::menu::main_menu),
            );
    }
}

#[derive(Resource, Default)]
pub struct TalentScreen {
    open: Option<Entity>,
    dirty: bool,
}

impl TalentScreen {
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn request_open(&mut self) {
        if self.open.is_none() {
            self.dirty = true;
        }
    }
}

/// Talent icons, from the WoW spells named in the talent table.
#[derive(Resource, Default)]
struct TalentIcons(Vec<(TalentId, Handle<Image>)>);

fn load_talent_icons(
    mut icons: ResMut<TalentIcons>,
    wa: Option<ResMut<WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut wa) = wa else {
        return;
    };
    let catalog = {
        let mut chain = wa.chain.lock_recover();
        load_spell_catalog(&mut chain)
    };
    let Ok(catalog) = catalog else {
        return;
    };
    for t in TalentId::ALL {
        let name = t.def().icon_spell;
        let rows = catalog
            .iter()
            .map(|(sid, d)| (sid, d.name.as_str(), d.rank.as_deref(), d.icon.as_deref()));
        let icon = pick_spell(rows, name)
            .and_then(|sid| catalog.get(sid))
            .and_then(|d| d.icon.as_deref())
            .and_then(|p| wa.sprite_texture(p, &mut images));
        match icon {
            Some(h) => icons.0.push((t, h)),
            None => warn!("rnsurvivors: no icon for talent {t:?} (spell {name:?})"),
        }
    }
}

#[derive(Component, Clone, Copy)]
struct TalentButton(TalentId);

/// The screen: Esc or T closes it, a click buys, R resets.
#[allow(clippy::too_many_arguments)]
fn talent_screen(
    mut commands: Commands,
    mut screen: ResMut<TalentScreen>,
    mut save: ResMut<Save>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Query<(&Interaction, &TalentButton), Changed<Interaction>>,
    font: Res<UiFont>,
    art: Res<WowArt>,
    icons: Res<TalentIcons>,
) {
    let Some(root) = screen.open else {
        if screen.dirty {
            screen.dirty = false;
            screen.open = Some(spawn_screen(&mut commands, &save, &font, &art, &icons));
        }
        return;
    };
    if keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyT]) {
        commands.entity(root).try_despawn();
        screen.open = None;
        return;
    }
    let mut changed = false;
    if keys.just_pressed(KeyCode::KeyR) && save.data.talents.spent() > 0 {
        save.data.talents = Talents::default();
        changed = true;
    }
    if let Some((_, b)) = buttons.iter().find(|(i, _)| **i == Interaction::Pressed) {
        let points = save.data.points();
        changed |= save.data.talents.buy(b.0, points);
    }
    if changed {
        save.write();
        commands.entity(root).try_despawn();
        screen.open = Some(spawn_screen(&mut commands, &save, &font, &art, &icons));
    }
}

fn spawn_screen(
    commands: &mut Commands,
    save: &Save,
    font: &UiFont,
    art: &WowArt,
    icons: &TalentIcons,
) -> Entity {
    let d = &save.data;
    let gold = Color::srgb(1.0, 0.82, 0.0);
    let dim = Color::srgb(0.7, 0.7, 0.7);
    let points = d.points();
    let mut root = commands.spawn((
        Name::new("talents"),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.85)),
        GlobalZIndex(45),
    ));
    root.with_children(|p| {
        let mut panel = p.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(18.0)),
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
            p.spawn((
                Text::new(format!(
                    "Talentlar   ·   Boş puan: {} / {}",
                    d.free_points(),
                    points
                )),
                font.text(28.0),
                TextColor(gold),
            ));
            p.spawn((
                Text::new(format!(
                    "Koşularda toplanan XP talent puanı getirir. Sonraki puana {} XP.",
                    xp_to_next_point(d.talent_xp)
                )),
                font.text(15.0),
                TextColor(dim),
            ));
            p.spawn(Node {
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|cols| {
                for col in Column::ALL {
                    let c = col.color();
                    let ccol = Color::srgb(c[0], c[1], c[2]);
                    cols.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        width: Val::Px(250.0),
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((
                            Text::new(format!("{}  ({})", col.name(), d.talents.spent_in(col))),
                            font.text(18.0),
                            TextColor(ccol),
                        ));
                        for t in TalentId::ALL.into_iter().filter(|t| t.def().column == col) {
                            talent_button(c, t, &d.talents, points, font, icons, ccol);
                        }
                    });
                }
            });
            p.spawn((
                Text::new("Tıkla: puan ver   ·   R: bütün puanları geri al   ·   Esc / T: kapat"),
                font.text(14.0),
                TextColor(dim),
            ));
        });
    });
    root.id()
}

fn talent_button(
    c: &mut ChildSpawnerCommands,
    t: TalentId,
    talents: &Talents,
    points: u32,
    font: &UiFont,
    icons: &TalentIcons,
    col: Color,
) {
    let d = t.def();
    let rank = talents.rank(t);
    let open = talents.spent_in(d.column) >= TIER_GATE[d.tier];
    let buyable = talents.can_buy(t, points);
    let border = if rank == d.max_rank {
        Color::srgb(1.0, 0.82, 0.0)
    } else if buyable {
        Color::srgb(0.3, 1.0, 0.3)
    } else {
        Color::srgba(1.0, 1.0, 1.0, 0.15)
    };
    let text_color = if open {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };
    c.spawn((
        TalentButton(t),
        Button,
        Node {
            column_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(6.0)),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor::all(border),
        BackgroundColor(Color::srgba(0.1, 0.1, 0.14, 0.92)),
    ))
    .with_children(|b| {
        let icon = icons
            .0
            .iter()
            .find(|(id, _)| *id == t)
            .map(|(_, h)| h.clone());
        b.spawn((
            Node {
                width: Val::Px(42.0),
                height: Val::Px(42.0),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(col.with_alpha(0.3)),
        ))
        .with_children(|i| match icon {
            Some(h) => {
                i.spawn((
                    ImageNode::new(h).with_color(if open {
                        Color::WHITE
                    } else {
                        Color::srgb(0.4, 0.4, 0.4)
                    }),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
            }
            None => {
                i.spawn((
                    Text::new(d.name.chars().next().unwrap_or('?').to_string()),
                    font.text(20.0),
                    TextColor(Color::WHITE),
                ));
            }
        });
        let lock = if open {
            String::new()
        } else {
            format!("\n(bu kolda {} puan gerekir)", TIER_GATE[d.tier])
        };
        b.spawn((
            Text::new(format!(
                "{}  {}/{}\n{}{lock}",
                d.name,
                rank,
                d.max_rank,
                d.line()
            )),
            font.text(14.0),
            TextColor(text_color),
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(talent_screen).initialize(&mut world);
        IntoSystem::into_system(load_talent_icons).initialize(&mut world);
    }
}
