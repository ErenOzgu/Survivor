//! Building a requested model: resolve the request against the client tables, wait for the M2
//! and its render forms, then spawn the parts under a model root with its rig, palette slot and
//! animation player.
//!
//! This is a slim, server-less counterpart of the engine client's unit visual lane
//! (`benilla-app` `entities::display` / `entities::attach`, MIT OR Apache-2.0), built only on the
//! public `benilla-world` / `benilla-assets` / `benilla-formats` API: no mounts, corpses, spell
//! effects, emitters, billboard cards, fades or picking.

use benilla_assets::coords::wow_to_bevy;
use benilla_assets::{m2_url, skin_url, M2Model, ModelSubmesh};
use benilla_formats::{CharSkinSlot, EquipGeosets, ItemDisplay, ModelBlend};
use bevy::animation::transition::AnimationTransitions;
use bevy::asset::LoadState;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::MeshTag;
use bevy::prelude::*;

use benilla_world::doodad_anim::{AnimMatPart, MatAnim, UvLoops};
use benilla_world::model_render::{
    BatchVariants, EntityUvLane, M2BatchMaterials, ModelKind, ModelPart,
};
use benilla_world::rig_anim::{GlobalSeqDrive, RigPose};
use benilla_world::rig_palette::{RigPalettes, RigPart, RigSkin};

use super::catalog::{ModelData, Tables};
use super::look::{CharacterLook, Gear, GearItem, GearSlot};
use super::{gear, paths, ModelRequest, ModelRoot, ModelStatus};

/// The right hand's attachment id: a drawn main-hand weapon.
const ATTACH_HAND_RIGHT: u16 = 1;
/// The head attachment a helmet rides.
const ATTACH_HELM: u16 = 11;
/// Seconds a model may take to load before the request gives up.
const LOAD_TIMEOUT: f32 = 60.0;

/// A character body's resolved look.
#[derive(Clone, Debug)]
struct CharBody {
    race: u8,
    sex: u8,
    skin: u8,
    face: u8,
    hair_style: u8,
    hair_color: u8,
    facial_hair: u8,
    /// Worn ItemDisplayInfo ids by body slot (shirt … tabard).
    bodyslots: [u32; 8],
    helm: u32,
    /// A character-model NPC's shipped body atlas; `None` composites the body live.
    baked: Option<String>,
}

/// A held item waiting on its model.
struct PendingItem {
    display: u32,
    attach: u16,
    handle: Handle<M2Model>,
    dir: String,
    texture: Option<String>,
}

/// A resolved request, waiting on its assets.
#[derive(Component)]
pub(super) struct Pending {
    handle: Handle<M2Model>,
    path: String,
    dir: String,
    scale: f32,
    skins: [Option<String>; 3],
    body: Option<CharBody>,
    items: Vec<PendingItem>,
    since: f32,
    /// A spell effect model: its particle emitters and ribbons are spawned too.
    effect: bool,
}

/// Resolve each new request against the tables; it becomes [`Pending`] or fails.
pub(super) fn resolve_requests(
    mut commands: Commands,
    requests: Query<(Entity, &ModelRequest), Without<ModelStatus>>,
    data: Option<Res<ModelData>>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    let Some(data) = data else {
        return; // the tables load at startup; wait for them
    };
    for (entity, request) in &requests {
        let resolved = match data.tables.as_ref() {
            None => Err("no WoW data (see WOW_DATA)".to_string()),
            Some(t) => resolve(request, t, &asset_server, time.elapsed_secs()),
        };
        match resolved {
            Ok(p) => {
                info!(
                    "rnsurvivors: loading model {} for {entity} ({} item(s))",
                    p.path,
                    p.items.len()
                );
                commands.entity(entity).insert((p, ModelStatus::Loading));
            }
            Err(why) => {
                warn!("rnsurvivors: model for {entity} unavailable: {why}");
                commands.entity(entity).insert(ModelStatus::Failed(why));
            }
        }
    }
}

fn resolve(
    request: &ModelRequest,
    t: &Tables,
    asset_server: &AssetServer,
    now: f32,
) -> Result<Pending, String> {
    match request {
        ModelRequest::Creature { display_id } => {
            let m = t
                .creatures
                .model(*display_id)
                .ok_or_else(|| format!("no CreatureDisplayInfo row {display_id}"))?;
            if m.scale <= 0.0 {
                return Err(format!("display {display_id} has scale 0"));
            }
            // A character-model NPC (Defias, guards) wears its CreatureDisplayInfoExtra look.
            let body = m.npc_appearance.as_ref().map(|npc| CharBody {
                race: npc.race,
                sex: npc.sex,
                skin: npc.skin,
                face: npc.face,
                hair_style: npc.hair_style,
                hair_color: npc.hair_color,
                facial_hair: npc.facial_hair,
                bodyslots: std::array::from_fn(|i| npc.equipment[i + 2]),
                helm: npc.equipment[0],
                baked: npc.bake_name.clone(),
            });
            Ok(Pending {
                handle: asset_server.load(m2_url(&m.model_path)),
                dir: paths::model_dir(&m.model_path).to_string(),
                path: m.model_path,
                scale: m.scale,
                skins: m.textures,
                body,
                items: Vec::new(),
                since: now,
                effect: false,
            })
        }
        ModelRequest::Character(look) => resolve_character(look, t, asset_server, now),
        ModelRequest::Effect { path, scale } => Ok(Pending {
            handle: asset_server.load(m2_url(path)),
            dir: paths::model_dir(path).to_string(),
            path: path.clone(),
            scale: *scale,
            skins: [None, None, None],
            body: None,
            items: Vec::new(),
            since: now,
            effect: true,
        }),
    }
}

fn resolve_character(
    look: &CharacterLook,
    t: &Tables,
    asset_server: &AssetServer,
    now: f32,
) -> Result<Pending, String> {
    let (race, sex) = (look.race.0, look.sex.id());
    // The body's display through ChrRaces, as the client does; the folder convention otherwise.
    let (path, scale) = t
        .races
        .as_ref()
        .and_then(|r| r.body_display(race, sex))
        .and_then(|d| t.creatures.model(d))
        .filter(|m| m.scale > 0.0)
        .map(|m| (m.model_path, m.scale))
        .or_else(|| paths::character_model_path(look.race, look.sex).map(|p| (p, 1.0)))
        .ok_or_else(|| format!("no body model for race {race} sex {sex}"))?;
    let items = match &look.gear {
        Gear::Items(items) => items.clone(),
        Gear::MageDefault | Gear::MageWith(_) => {
            let outfit: Vec<(u32, u8)> = t
                .races
                .as_ref()
                .map(|r| {
                    r.start_outfit(race, gear::MAGE_CLASS, sex)
                        .iter()
                        .map(|i| (i.display_id, i.inv_type))
                        .collect()
                })
                .unwrap_or_default();
            match t.items.as_ref() {
                Some(cat) => {
                    let mut items = gear::mage_gear(&outfit, |id| cat.get(id), cat.iter());
                    if let Gear::MageWith(picks) = &look.gear {
                        gear::apply_picks(&mut items, picks, cat.iter());
                    }
                    items
                }
                None => Vec::new(),
            }
        }
    };
    let mut bodyslots = [0u32; 8];
    let mut helm = 0;
    let mut held = Vec::new();
    for item in &items {
        let row = t.items.as_ref().and_then(|c| c.get(item.display_id));
        info!(
            "rnsurvivors: hero wears {:?} display {} (model {:?}, textures {:?})",
            item.slot,
            item.display_id,
            row.and_then(|r| r.model[0].as_deref()),
            row.map(|r| r.region_textures.iter().flatten().collect::<Vec<_>>())
        );
        let Some(row) = row else {
            warn!(
                "rnsurvivors: item display {} is not in ItemDisplayInfo.dbc",
                item.display_id
            );
            continue;
        };
        match item.slot.body_index() {
            Some(i) => bodyslots[i] = item.display_id,
            None if item.slot == GearSlot::Head => {
                // The helm hides hair by its geosets and rides the head attachment as a model.
                helm = item.display_id;
                if let Some(model) = row.model[0].as_deref() {
                    held.push(PendingItem {
                        display: item.display_id,
                        attach: ATTACH_HELM,
                        handle: asset_server
                            .load(m2_url(&paths::helm_model_path(model, race, sex))),
                        dir: paths::HELM_DIR.to_string(),
                        texture: row.model_texture[0].clone(),
                    });
                }
            }
            None => {
                if let Some(p) = held_item(item, row, asset_server) {
                    held.push(p);
                }
            }
        }
    }
    Ok(Pending {
        handle: asset_server.load(m2_url(&path)),
        dir: paths::model_dir(&path).to_string(),
        path,
        scale,
        skins: Default::default(),
        body: Some(CharBody {
            race,
            sex,
            skin: look.skin,
            face: look.face,
            hair_style: look.hair_style,
            hair_color: look.hair_color,
            facial_hair: look.facial_hair,
            bodyslots,
            helm,
            baked: None,
        }),
        items: held,
        since: now,
        effect: false,
    })
}

/// A main-hand weapon's model, from its ItemDisplayInfo row.
fn held_item(
    item: &GearItem,
    row: &ItemDisplay,
    asset_server: &AssetServer,
) -> Option<PendingItem> {
    let model = row.model[0].as_deref()?;
    Some(PendingItem {
        display: item.display_id,
        attach: ATTACH_HAND_RIGHT,
        handle: asset_server.load(m2_url(&paths::weapon_model_path(model))),
        dir: paths::WEAPON_DIR.to_string(),
        texture: row.model_texture[0].clone(),
    })
}

/// Build each pending model whose assets are ready.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn finish_models(
    mut commands: Commands,
    pending: Query<(Entity, &Pending)>,
    m2s: Res<Assets<M2Model>>,
    asset_server: Res<AssetServer>,
    mut forms: ResMut<benilla_world::model_forms::ModelForms>,
    mut mats: M2BatchMaterials,
    mut uv_reg: ResMut<benilla_world::doodad_anim::UvAnimMaterials>,
    mut anim_table: ResMut<benilla_world::mat_anim_table::MatAnimTable>,
    mut palettes: ResMut<RigPalettes>,
    mut images: ResMut<Assets<Image>>,
    data: Option<Res<ModelData>>,
    time: Res<Time>,
) {
    if !mats.ready() {
        return; // no light buffer yet, so no materials
    }
    let tables = data.as_deref().and_then(|d| d.tables.as_ref());
    for (owner, p) in &pending {
        let fail = |commands: &mut Commands, why: String| {
            warn!("rnsurvivors: model {} failed: {why}", p.path);
            commands
                .entity(owner)
                .remove::<Pending>()
                .insert(ModelStatus::Failed(why));
        };
        if let LoadState::Failed(e) = asset_server.load_state(&p.handle) {
            fail(&mut commands, format!("load error: {e}"));
            continue;
        }
        if time.elapsed_secs() - p.since > LOAD_TIMEOUT {
            fail(&mut commands, "timed out loading".into());
            continue;
        }
        let Some(model) = m2s.get(&p.handle) else {
            continue;
        };
        if !forms.require_rigged(&p.handle, 0) {
            continue;
        }
        // Held items: ready, or failed (dropped); still loading holds the body back.
        let mut items_ready = true;
        for item in &p.items {
            match asset_server.load_state(&item.handle) {
                LoadState::Failed(_) => {}
                _ => match m2s.get(&item.handle) {
                    Some(_) => items_ready &= forms.require_static(&item.handle, 0),
                    None => items_ready = false,
                },
            }
        }
        if !items_ready {
            continue;
        }
        let mut uv = EntityUvLane {
            reg: &mut uv_reg,
            table: &mut anim_table,
            instance: None,
        };
        let root = spawn_body(
            &mut commands,
            owner,
            p,
            model,
            tables,
            &forms,
            &mut mats,
            &mut uv,
            &mut palettes,
            &mut images,
            &asset_server,
            &m2s,
        );
        info!("rnsurvivors: model {} ready", p.path);
        commands
            .entity(owner)
            .remove::<Pending>()
            .insert((ModelStatus::Ready, ModelRoot(root)))
            .insert_if_new(super::ModelAnim::default());
    }
}

/// The per-part materials of a character body: body atlas, hair and extra skin.
struct CharTextures {
    body: Option<Handle<Image>>,
    hair: Option<Handle<Image>>,
    skin_extra: Option<Handle<Image>>,
}

#[allow(clippy::too_many_arguments)]
fn spawn_body(
    commands: &mut Commands,
    owner: Entity,
    p: &Pending,
    model: &M2Model,
    tables: Option<&Tables>,
    forms: &benilla_world::model_forms::ModelForms,
    mats: &mut M2BatchMaterials,
    uv: &mut EntityUvLane<'_>,
    palettes: &mut RigPalettes,
    images: &mut Assets<Image>,
    asset_server: &AssetServer,
    m2s: &Assets<M2Model>,
) -> Entity {
    let root = commands
        .spawn((
            Name::new(format!("model {}", p.path)),
            Transform::from_scale(Vec3::splat(p.scale)),
            Visibility::default(),
            ChildOf(owner),
        ))
        .id();

    // The rig: pose buffer, palette slot, global-sequence channels.
    let rigged = !model.skeleton.joints.is_empty();
    let mut pose = rigged.then(|| RigPose::new(root, &model.skeleton));
    let mut slot = 0u16;
    if rigged {
        let skins = model.submeshes.iter().any(|s| s.billboard.is_none());
        if let Some(rig) = skins
            .then(|| {
                RigSkin::allocate_bones(
                    palettes,
                    model.skeleton.joints.len() as u32,
                    model.inverse_bindposes.clone(),
                )
            })
            .flatten()
        {
            slot = rig.slot;
            commands.entity(root).insert(rig);
        }
        if let Some(anims) = model.animations.as_ref() {
            if let Some(drive) =
                GlobalSeqDrive::new_rig(&anims.global_bones, model.skeleton.joints.len())
            {
                commands.entity(root).insert(drive);
            }
        }
    }
    if let Some(anims) = model.animations.as_ref() {
        commands.entity(root).insert((
            AnimationPlayer::default(),
            AnimationTransitions::new(),
            AnimationGraphHandle(anims.graph.clone()),
            anims.clone(),
        ));
    }

    // The character look: which geosets draw and the per-appearance textures.
    let (visible, char_tex) = match (&p.body, tables) {
        (Some(b), Some(t)) => (
            t.geosets.as_ref().map(|g| {
                g.visible_geosets(
                    b.race,
                    b.sex,
                    b.hair_style,
                    b.facial_hair,
                    &equip_geosets(t, b),
                )
            }),
            Some(char_textures(b, t, asset_server, images)),
        ),
        _ => (None, None),
    };
    let is_char = p.body.is_some();

    let bake_center = model.bounds.map_or(Vec3::ZERO, |b| {
        wow_to_bevy([
            (b.bbox_min[0] + b.bbox_max[0]) * 0.5,
            (b.bbox_min[1] + b.bbox_max[1]) * 0.5,
            (b.bbox_min[2] + b.bbox_max[2]) * 0.5,
        ])
    });
    let idle_aabb = model
        .animations
        .as_ref()
        .and_then(|a| a.idle_clip())
        .filter(|c| c.bounds_max != c.bounds_min)
        .map(|c| bevy::camera::primitives::Aabb::from_min_max(c.bounds_min, c.bounds_max));

    let built = forms.slices(&p.handle);
    let mut skipped_cards = 0;
    for (i, sub) in model.submeshes.iter().enumerate() {
        if sub.billboard.is_some() {
            skipped_cards += 1;
            continue;
        }
        if is_char {
            let shown = match &visible {
                Some(v) => v.contains(&sub.geoset_id),
                // Without the tables only the body and the region bases.
                None => sub.geoset_id == 0 || sub.geoset_id % 100 == 1,
            };
            if !shown {
                continue;
            }
        }
        let order = u16::try_from(i + 1).unwrap_or(u16::MAX);
        let variants = part_variants(sub, order, p, char_tex.as_ref(), mats, uv, asset_server);
        let Some(v) = variants else { continue };
        let Some((stat_mesh, stat_aabb)) = built.stat.get(i) else {
            continue;
        };
        let skinned_mesh = built.skin.and_then(|s| s.get(i));
        let skinned = slot != 0 && skinned_mesh.is_some();
        let mesh = match skinned_mesh {
            Some(sm) if skinned => sm.clone(),
            _ => stat_mesh.clone(),
        };
        let mut part = commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(v.steady.clone()),
            Transform::default(),
            ChildOf(root),
            ModelPart {
                kind: ModelKind::Creature,
                blend: sub.blend,
            },
            MeshTag(benilla_world::mesh_tag::spawn_tag(slot, 1.0)),
        ));
        if skinned {
            part.insert(RigPart(root));
            // The palette moves the vertices, so the bind box is not where it draws: no cull.
            if let Some(aabb) = idle_aabb.or(*stat_aabb) {
                part.insert((aabb, NoFrustumCulling));
            } else {
                part.insert(NoFrustumCulling);
            }
        } else if let Some(aabb) = stat_aabb {
            part.insert(*aabb);
        }
        if let Some(lit) = benilla_world::interior::part_interior_lit(
            &v.steady,
            Some(&v.interior),
            Some(&v.interior_bake),
            bake_center,
            root,
        ) {
            part.insert(lit);
        }
        if let Some(anim) = &sub.alpha_anim {
            part.insert(MatAnim::following(anim.clone(), root));
        }
        if UvLoops::of(sub).animates() {
            part.insert(AnimMatPart);
        }
    }
    if skipped_cards > 0 {
        debug!(
            "rnsurvivors: {} billboard card(s) of {} not drawn",
            skipped_cards, p.path
        );
    }

    // Held items at their attachment points, riding the bone's anchor.
    for item in &p.items {
        let Some(im) = m2s.get(&item.handle) else {
            warn!(
                "rnsurvivors: item display {} model did not load",
                item.display
            );
            continue;
        };
        let Some(att) = model.attachments.iter().find(|a| a.id == item.attach) else {
            continue;
        };
        let Some(anchor) = pose
            .as_mut()
            .and_then(|pose| pose.anchor_for(commands, root, att.bone))
        else {
            continue;
        };
        let item_root = commands
            .spawn((
                Name::new(format!("item {}", item.display)),
                Transform::from_translation(att.offset),
                Visibility::default(),
                ChildOf(anchor),
                benilla_world::model_fade::ParentModel(root),
            ))
            .id();
        let item_forms = forms.slices(&item.handle);
        for (i, sub) in im.submeshes.iter().enumerate() {
            if sub.billboard.is_some() {
                continue;
            }
            let texture = match (sub.char_slot, &item.texture) {
                (Some(CharSkinSlot::Object), Some(tex)) => {
                    Some(asset_server.load(skin_url(&item.dir, tex)))
                }
                _ => sub.texture.clone(),
            };
            let order = u16::try_from(i + 1).unwrap_or(u16::MAX);
            let Some(v) = mats.entity_variants(sub, texture, order, uv) else {
                continue;
            };
            let Some((mesh, aabb)) = item_forms.stat.get(i) else {
                continue;
            };
            let mut part = commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(v.steady.clone()),
                Transform::default(),
                ChildOf(item_root),
                ModelPart {
                    kind: ModelKind::Creature,
                    blend: sub.blend,
                },
                // The wearer's slot: its tint and shade, as a slotless held item in the engine.
                MeshTag(benilla_world::mesh_tag::spawn_tag(slot, 1.0)),
            ));
            if let Some(aabb) = aabb {
                part.insert(*aabb);
            }
            if UvLoops::of(sub).animates() {
                part.insert(AnimMatPart);
            }
        }
    }

    // A spell effect's particles and ribbons, riding the model root (as the client's effect
    // instances do, minus per-bone placement: spell models emit near their origin). When the
    // owner goes (a missile hits), the particles drain in place instead of vanishing.
    if p.effect {
        for em in &model.emitters {
            benilla_world::particles::spawn_emitter(
                commands,
                em,
                Transform::IDENTITY,
                benilla_world::particles::EmitterFrames {
                    owner: Some((root, [0.0; 3])),
                    anchor: Some(root),
                    alpha: None,
                    light_node: None,
                    on_owner_loss: benilla_world::particles::OwnerLoss::Drain,
                },
                benilla_world::particles::EmitClock::Effect(None),
            );
        }
        for rb in &model.ribbons {
            benilla_world::ribbons::spawn_ribbon(
                commands,
                rb,
                root,
                false,
                p.scale,
                benilla_world::ribbons::RibbonSeq::Fixed(0),
                None,
                None,
            );
        }
    }

    if let Some(pose) = pose {
        commands.entity(root).insert(pose);
    }
    root
}

/// A part's material set: a character slot's per-appearance texture, else its own or the
/// creature's skin variation.
fn part_variants(
    sub: &ModelSubmesh,
    order: u16,
    p: &Pending,
    char_tex: Option<&CharTextures>,
    mats: &mut M2BatchMaterials,
    uv: &mut EntityUvLane<'_>,
    asset_server: &AssetServer,
) -> Option<BatchVariants> {
    if let (Some(slot), Some(ct)) = (sub.char_slot, char_tex) {
        let (tex, blend) = match slot {
            CharSkinSlot::Body => (ct.body.clone(), ModelBlend::Opaque),
            CharSkinSlot::Hair => (ct.hair.clone(), sub.blend),
            CharSkinSlot::SkinExtra => (ct.skin_extra.clone(), sub.blend),
            // A cape: no cloak is worn, so the built material.
            CharSkinSlot::Object => (None, sub.blend),
        };
        if let Some(tex) = tex {
            return mats.char_variants(tex, blend, sub.two_sided);
        }
    }
    let texture = match (&sub.texture, sub.skin_slot) {
        (Some(t), _) => Some(t.clone()),
        (None, Some(slot)) => p
            .skins
            .get(slot as usize)
            .and_then(|o| o.as_ref())
            .map(|name| asset_server.load(skin_url(&p.dir, name))),
        (None, None) => None,
    };
    mats.entity_variants(sub, texture, order, uv)
}

/// The worn geoset selectors, as the engine client's `equip_geosets` reads them.
fn equip_geosets(t: &Tables, b: &CharBody) -> EquipGeosets {
    let mut eg = EquipGeosets::default();
    let Some(cat) = t.items.as_ref() else {
        return eg;
    };
    let mut worn: [Option<&ItemDisplay>; 8] = [None; 8];
    for (i, id) in b.bodyslots.iter().enumerate() {
        if *id != 0 {
            worn[i] = cat.get(*id);
            eg.bodyslots[i] = worn[i].map(|row| row.geoset_groups);
        }
    }
    eg.forearm_dressed = benilla_formats::forearm_dressed(&worn);
    if b.helm != 0 {
        eg.helm_vis = cat.get(b.helm).and_then(ItemDisplay::worn_helm_vis);
    }
    eg
}

/// The body atlas (composited now, on this thread, or the NPC's baked one), hair and extra skin.
fn char_textures(
    b: &CharBody,
    t: &Tables,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
) -> CharTextures {
    let Some(sections) = t.sections.as_ref() else {
        return CharTextures {
            body: None,
            hair: None,
            skin_extra: None,
        };
    };
    let body = match &b.baked {
        Some(name) => Some(asset_server.load(paths::baked_npc_url(name))),
        None => {
            let mut worn: [Option<&ItemDisplay>; 8] = [None; 8];
            if let Some(cat) = t.items.as_ref() {
                for (i, id) in b.bodyslots.iter().enumerate() {
                    if *id != 0 {
                        worn[i] = cat.get(*id);
                    }
                }
            }
            let mut chain = benilla_assets::LockRecover::lock_recover(&*t.chain);
            match sections.composite_body(
                &mut chain,
                b.race,
                b.sex,
                b.skin,
                b.face,
                b.facial_hair,
                b.hair_style,
                b.hair_color,
                worn,
                None,
                false,
            ) {
                Ok(Some(atlas)) => Some(images.add(benilla_assets::repeat_texture_authored(
                    benilla_assets::for_upload(atlas),
                    (true, true),
                ))),
                Ok(None) => {
                    warn!(
                        "rnsurvivors: no skin row for race {} sex {} skin {}",
                        b.race, b.sex, b.skin
                    );
                    None
                }
                Err(e) => {
                    warn!("rnsurvivors: body skin composite failed: {e:#}");
                    None
                }
            }
        }
    };
    let hair = sections
        .hair_mesh_texture(b.race, b.sex, b.hair_style, b.hair_color)
        .map(|path| asset_server.load(paths::mpq_url(path)));
    let skin_extra = sections
        .skin_extra_texture(b.race, b.sex, b.skin)
        .map(|path| asset_server.load(paths::mpq_url(path)));
    CharTextures {
        body,
        hair,
        skin_extra,
    }
}
