//! The default mage gear: a robe and a staff, chosen from the client's own tables.
//!
//! First the class's starting outfit from `CharStartOutfit.dbc` for the hero's race and sex (a
//! Troll mage starts in a robe with a staff); a slot it leaves empty (a race that cannot be a mage
//! in 1.12) is filled from `ItemDisplayInfo.dbc` by shape: the lowest display id whose row is a
//! robe, and the lowest whose model is a staff. Nothing here copies another game's item lists.

use benilla_formats::ItemDisplay;

use super::look::{GearItem, GearSlot};

/// The mage class id in `ChrClasses.dbc`.
pub const MAGE_CLASS: u8 = 8;

/// A robe row: the robe geoset bit (`geosetGroup[2]`) and art on the upper torso and upper legs.
pub fn is_robe(d: &ItemDisplay) -> bool {
    d.geoset_groups[2] != 0 && d.region_textures[3].is_some() && d.region_textures[5].is_some()
}

/// A staff row: its model is one of the `Stave_*` weapon models.
pub fn is_staff(d: &ItemDisplay) -> bool {
    d.model[0]
        .as_deref()
        .is_some_and(|m| m.to_ascii_lowercase().starts_with("stave"))
}

/// The robe and staff for a mage. `outfit` is the class's `(display id, inventory type)` start
/// outfit; `catalog` iterates every ItemDisplayInfo row; `get` looks one up.
pub fn mage_gear<'a>(
    outfit: &[(u32, u8)],
    get: impl Fn(u32) -> Option<&'a ItemDisplay>,
    catalog: impl Iterator<Item = (u32, &'a ItemDisplay)>,
) -> Vec<GearItem> {
    let from_outfit = |slot: GearSlot| {
        outfit
            .iter()
            .filter(|(id, inv)| *id != 0 && GearSlot::from_inventory_type(*inv) == Some(slot))
            .map(|(id, _)| *id)
            .find(|&id| get(id).is_some())
    };
    let mut robe = from_outfit(GearSlot::Chest);
    let mut staff = from_outfit(GearSlot::MainHand);
    let (need_robe, need_staff) = (robe.is_none(), staff.is_none());
    if need_robe || need_staff {
        // The lowest matching id, so the pick does not depend on the catalog's iteration order.
        for (id, d) in catalog {
            if need_robe && is_robe(d) {
                robe = Some(robe.map_or(id, |r| r.min(id)));
            }
            if need_staff && is_staff(d) {
                staff = Some(staff.map_or(id, |s| s.min(id)));
            }
        }
    }
    let mut out = Vec::new();
    if let Some(id) = robe {
        out.push(GearItem {
            slot: GearSlot::Chest,
            display_id: id,
        });
    }
    if let Some(id) = staff {
        out.push(GearItem {
            slot: GearSlot::MainHand,
            display_id: id,
        });
    }
    out
}

/// Whether an `ItemDisplayInfo` row looks like an item for `slot`, by what it paints and carries.
/// Body regions: 0 upper arm, 1 lower arm, 2 hand, 3 upper torso, 4 lower torso, 5 upper leg,
/// 6 lower leg, 7 foot.
pub fn fits_slot(d: &ItemDisplay, slot: GearSlot) -> bool {
    let r = |i: usize| d.region_textures[i].is_some();
    let model = d.model[0].as_deref().map(str::to_ascii_lowercase);
    match slot {
        GearSlot::Chest => is_robe(d),
        GearSlot::MainHand => is_staff(d),
        GearSlot::Hands => r(2) && !r(3) && !r(5) && model.is_none(),
        GearSlot::Legs => r(5) && r(6) && !r(3) && d.geoset_groups[2] == 0 && model.is_none(),
        GearSlot::Feet => r(7) && !r(3) && !r(5) && model.is_none(),
        GearSlot::Head => model.is_some_and(|m| m.starts_with("helm")),
        _ => false,
    }
}

/// Every display id that fits `slot`, lowest first.
pub fn slot_displays<'a>(
    catalog: impl Iterator<Item = (u32, &'a ItemDisplay)>,
    slot: GearSlot,
) -> Vec<u32> {
    let mut ids: Vec<u32> = catalog
        .filter(|(_, d)| fits_slot(d, slot))
        .map(|(id, _)| id)
        .collect();
    ids.sort_unstable();
    ids
}

/// Replaces (or adds) each picked slot in `items` with the display its seed picks among the
/// slot's candidates; a slot with no candidate keeps what it had.
pub fn apply_picks<'a>(
    items: &mut Vec<GearItem>,
    picks: &[(GearSlot, u32)],
    catalog: impl Iterator<Item = (u32, &'a ItemDisplay)>,
) {
    let rows: Vec<(u32, &ItemDisplay)> = catalog.collect();
    for &(slot, seed) in picks {
        let ids = slot_displays(rows.iter().copied(), slot);
        if ids.is_empty() {
            continue;
        }
        let display_id = ids[seed as usize % ids.len()];
        items.retain(|i| i.slot != slot);
        items.push(GearItem { slot, display_id });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn robe() -> ItemDisplay {
        let mut d = ItemDisplay::default();
        d.geoset_groups = [1, 0, 1];
        d.region_textures[3] = Some("Robe_TU".into());
        d.region_textures[5] = Some("Robe_LU".into());
        d
    }

    fn staff() -> ItemDisplay {
        let mut d = ItemDisplay::default();
        d.model[0] = Some("stave_2h_long_a_01.m2".into());
        d
    }

    fn sword() -> ItemDisplay {
        let mut d = ItemDisplay::default();
        d.model[0] = Some("sword_1h_short_a_01.m2".into());
        d
    }

    fn pick(outfit: &[(u32, u8)], rows: &HashMap<u32, ItemDisplay>) -> Vec<GearItem> {
        mage_gear(
            outfit,
            |id| rows.get(&id),
            rows.iter().map(|(k, v)| (*k, v)),
        )
    }

    #[test]
    fn slots_are_told_apart_and_picks_replace() {
        let mut gloves = ItemDisplay::default();
        gloves.region_textures[2] = Some("Glove_HA".into());
        let mut boots = ItemDisplay::default();
        boots.region_textures[7] = Some("Boot_FO".into());
        let mut staff = ItemDisplay::default();
        staff.model[0] = Some("Stave_2H_A_01.mdx".into());
        let rows: HashMap<u32, ItemDisplay> =
            [(10, gloves), (11, boots), (12, staff), (20, robe())]
                .into_iter()
                .collect();
        let cat = || rows.iter().map(|(id, d)| (*id, d));
        assert_eq!(slot_displays(cat(), GearSlot::Hands), vec![10]);
        assert_eq!(slot_displays(cat(), GearSlot::Feet), vec![11]);
        assert_eq!(slot_displays(cat(), GearSlot::MainHand), vec![12]);
        assert_eq!(slot_displays(cat(), GearSlot::Chest), vec![20]);
        let mut items = vec![GearItem {
            slot: GearSlot::MainHand,
            display_id: 99,
        }];
        apply_picks(
            &mut items,
            &[
                (GearSlot::MainHand, 7),
                (GearSlot::Hands, 3),
                (GearSlot::Head, 1),
            ],
            cat(),
        );
        assert!(items.contains(&GearItem {
            slot: GearSlot::MainHand,
            display_id: 12
        }));
        assert!(items.contains(&GearItem {
            slot: GearSlot::Hands,
            display_id: 10
        }));
        assert_eq!(items.len(), 2, "no helm candidate: head unchanged");
    }

    #[test]
    fn the_start_outfit_wins() {
        let rows = HashMap::from([(10, robe()), (20, staff()), (5, robe()), (6, staff())]);
        // Robe (20), two-hand weapon (17) and a shirt (4) that is not taken.
        let got = pick(&[(10, 20), (20, 17), (5, 4)], &rows);
        assert_eq!(
            got,
            vec![
                GearItem {
                    slot: GearSlot::Chest,
                    display_id: 10
                },
                GearItem {
                    slot: GearSlot::MainHand,
                    display_id: 20
                },
            ]
        );
    }

    #[test]
    fn an_empty_outfit_falls_back_to_the_lowest_matching_rows() {
        let rows = HashMap::from([
            (30, robe()),
            (12, robe()),
            (40, staff()),
            (41, staff()),
            (1, sword()),
            (2, ItemDisplay::default()),
        ]);
        let got = pick(&[], &rows);
        assert_eq!(
            got,
            vec![
                GearItem {
                    slot: GearSlot::Chest,
                    display_id: 12
                },
                GearItem {
                    slot: GearSlot::MainHand,
                    display_id: 40
                },
            ]
        );
    }

    #[test]
    fn an_outfit_id_missing_from_the_catalog_is_skipped() {
        let rows = HashMap::from([(7, staff()), (8, robe())]);
        let got = pick(&[(999, 20), (7, 17)], &rows);
        assert_eq!(got[0].display_id, 8, "robe from the catalog");
        assert_eq!(got[1].display_id, 7, "staff from the outfit");
    }

    #[test]
    fn shapes_are_recognised() {
        assert!(is_robe(&robe()));
        assert!(!is_robe(&staff()));
        assert!(is_staff(&staff()));
        assert!(!is_staff(&sword()));
    }
}
