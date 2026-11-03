//! What lasts between runs, kept in a small text file in the player's profile (Windows:
//! `%APPDATA%\Rnsurvivors\kayit.txt`; `RNS_SAVE` names another file): records and totals, the
//! talent XP and bought talents (phase 5), and the gear worn and carried (phase 4).

use std::path::PathBuf;

use bevy::prelude::*;

use super::hero::{check_death, reset_run};
use super::portal::MapCleared;
use super::rules::Bonus;
use super::{CombatSet, HeroDied, RestartRun, Run};
use crate::content::crafting::{Food, Mat};
use crate::content::items::{Item, ItemSlot};
use crate::content::talents::{points_for_xp, Talents};
use crate::content::zones::ZoneId;

pub const SAVE_VAR: &str = "RNS_SAVE";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        let path = save_path();
        let data = std::fs::read_to_string(&path)
            .map(|t| SaveData::parse(&t))
            .unwrap_or_default();
        info!("rnsurvivors: save file {}", path.display());
        app.insert_resource(Save { data, path }).add_systems(
            Update,
            // After a death is noticed, before the run resets: the finished run is still there.
            record_run
                .in_set(CombatSet::Reset)
                .after(check_death)
                .before(reset_run),
        );
    }
}

/// The save and where it lives.
#[derive(Resource)]
pub struct Save {
    pub data: SaveData,
    pub path: PathBuf,
}

/// The most items the bag holds.
pub const BAG_SIZE: usize = 24;

impl Save {
    /// Writes the save now (after a change the player made).
    pub fn write(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&self.path, self.data.to_text()) {
            warn!(
                "rnsurvivors: could not save to {} ({e})",
                self.path.display()
            );
        }
    }
}

/// Records, totals and lasting progress over every run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SaveData {
    pub runs: u32,
    pub deaths: u32,
    pub maps_cleared: u32,
    pub total_kills: u64,
    /// Longest run, seconds.
    pub best_time: f32,
    pub best_level: u32,
    pub best_kills: u32,
    /// All XP ever gathered in runs; talent points come from it.
    pub talent_xp: u64,
    pub talents: Talents,
    /// Worn items, at most one per slot.
    pub equipped: Vec<Item>,
    /// Carried, not worn.
    pub bag: Vec<Item>,
    /// Crafting materials, by `Mat::index`.
    pub mats: [u32; 5],
    /// Cooked meals, by `Food::index`.
    pub foods: [u32; 3],
    /// The meal to take into the next run.
    pub food_pick: Option<Food>,
    /// The meal eaten for the run under way (its buff counts; not saved).
    pub active_food: Option<Food>,
    /// The zone picked for the runs, and the zones cleared (each opens the next).
    pub zone: ZoneId,
    pub cleared_zones: Vec<ZoneId>,
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Died,
    Cleared,
    /// Restarted or left from the pause menu.
    Abandoned,
}

impl SaveData {
    /// One `key=value` per line; items one per `equip=`/`bag=` line.
    pub fn to_text(&self) -> String {
        let mut s = format!(
            "# Rnsurvivors kayıt dosyası\n\
             runs={}\ndeaths={}\nmaps_cleared={}\ntotal_kills={}\n\
             best_time={:.1}\nbest_level={}\nbest_kills={}\n\
             talent_xp={}\ntalents={}\n",
            self.runs,
            self.deaths,
            self.maps_cleared,
            self.total_kills,
            self.best_time,
            self.best_level,
            self.best_kills,
            self.talent_xp,
            self.talents.to_save()
        );
        for it in &self.equipped {
            s.push_str(&format!("equip={}\n", it.to_save()));
        }
        for it in &self.bag {
            s.push_str(&format!("bag={}\n", it.to_save()));
        }
        let counts = |keys: Vec<(&str, u32)>| {
            keys.into_iter()
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| format!("{k}:{n}"))
                .collect::<Vec<_>>()
                .join(",")
        };
        s.push_str(&format!(
            "mats={}\n",
            counts(
                Mat::ALL
                    .iter()
                    .map(|m| (m.key(), self.mats[m.index()]))
                    .collect()
            )
        ));
        s.push_str(&format!(
            "foods={}\n",
            counts(
                Food::ALL
                    .iter()
                    .map(|f| (f.key(), self.foods[f.index()]))
                    .collect()
            )
        ));
        if let Some(f) = self.food_pick {
            s.push_str(&format!("food_pick={}\n", f.key()));
        }
        s.push_str(&format!("zone={}\n", self.zone.key()));
        if !self.cleared_zones.is_empty() {
            let keys: Vec<&str> = self.cleared_zones.iter().map(|z| z.key()).collect();
            s.push_str(&format!("cleared_zones={}\n", keys.join(",")));
        }
        s
    }

    /// Talent points earned in all.
    pub fn points(&self) -> u32 {
        points_for_xp(self.talent_xp)
    }

    /// Points not spent yet.
    pub fn free_points(&self) -> u32 {
        self.points().saturating_sub(self.talents.spent())
    }

    /// What worn gear and talents add to the hero.
    pub fn bonus(&self) -> Bonus {
        let mut b = Bonus::from_gear(self.equipped.iter()).plus(Bonus::from_talents(&self.talents));
        if let Some(f) = self.active_food {
            b = b.plus(Bonus::from_affix(f.def().buff));
        }
        b
    }

    pub fn worn(&self, slot: ItemSlot) -> Option<&Item> {
        self.equipped.iter().find(|i| i.slot == slot)
    }

    /// Puts loot in the bag; what does not fit is returned.
    pub fn take_loot(&mut self, items: Vec<Item>) -> Vec<Item> {
        let mut left = Vec::new();
        for it in items {
            if self.bag.len() < BAG_SIZE {
                self.bag.push(it);
            } else {
                left.push(it);
            }
        }
        left
    }

    /// Wears the bag item at `i`; what was in its slot goes to the bag.
    pub fn equip(&mut self, i: usize) {
        if i >= self.bag.len() {
            return;
        }
        let it = self.bag.remove(i);
        if let Some(j) = self.equipped.iter().position(|e| e.slot == it.slot) {
            let old = std::mem::replace(&mut self.equipped[j], it);
            self.bag.insert(i, old);
        } else {
            self.equipped.push(it);
        }
    }

    /// Takes off what is worn in `slot`, if the bag has room.
    pub fn unequip(&mut self, slot: ItemSlot) {
        if self.bag.len() >= BAG_SIZE {
            return;
        }
        if let Some(j) = self.equipped.iter().position(|e| e.slot == slot) {
            let it = self.equipped.remove(j);
            self.bag.push(it);
        }
    }

    pub fn discard(&mut self, i: usize) {
        if i < self.bag.len() {
            self.bag.remove(i);
        }
    }

    /// Reads [`Self::to_text`]; unknown keys, comments and bad numbers are skipped, so an old or
    /// hand-edited file still loads.
    pub fn parse(text: &str) -> Self {
        let mut d = SaveData::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "runs" => d.runs = v.parse().unwrap_or(0),
                "deaths" => d.deaths = v.parse().unwrap_or(0),
                "maps_cleared" => d.maps_cleared = v.parse().unwrap_or(0),
                "total_kills" => d.total_kills = v.parse().unwrap_or(0),
                "best_time" => {
                    d.best_time = v
                        .parse::<f32>()
                        .ok()
                        .filter(|t| t.is_finite())
                        .unwrap_or(0.0)
                }
                "best_level" => d.best_level = v.parse().unwrap_or(0),
                "best_kills" => d.best_kills = v.parse().unwrap_or(0),
                "talent_xp" => d.talent_xp = v.parse().unwrap_or(0),
                "talents" => d.talents = Talents::from_save(v),
                "equip" => {
                    if let Some(it) = Item::from_save(v) {
                        // One per slot; a second one goes to the bag.
                        if d.equipped.iter().any(|e| e.slot == it.slot) {
                            d.bag.push(it);
                        } else {
                            d.equipped.push(it);
                        }
                    }
                }
                "bag" => {
                    if let Some(it) = Item::from_save(v) {
                        d.bag.push(it);
                    }
                }
                "mats" | "foods" => {
                    for (key, n) in v
                        .split(',')
                        .filter_map(|p| p.split_once(':'))
                        .filter_map(|(a, b)| Some((a.trim(), b.trim().parse::<u32>().ok()?)))
                    {
                        if let Some(m) = Mat::from_key(key) {
                            d.mats[m.index()] = n;
                        } else if let Some(f) = Food::from_key(key) {
                            d.foods[f.index()] = n;
                        }
                    }
                }
                "food_pick" => d.food_pick = Food::from_key(v),
                "zone" => d.zone = ZoneId::from_key(v).unwrap_or_default(),
                "cleared_zones" => {
                    d.cleared_zones = v.split(',').filter_map(ZoneId::from_key).collect();
                }
                _ => {}
            }
        }
        d.bag.truncate(BAG_SIZE);
        d
    }

    /// Adds a finished run (its XP feeds the talents); returns whether it set a new best time.
    pub fn record(&mut self, time: f32, level: u32, kills: u32, xp: u32, ending: Ending) -> bool {
        self.runs += 1;
        self.talent_xp += xp as u64;
        match ending {
            Ending::Died => self.deaths += 1,
            Ending::Cleared => {
                self.maps_cleared += 1;
                let z = crate::content::zones::current();
                if !self.cleared_zones.contains(&z) {
                    self.cleared_zones.push(z);
                }
            }
            Ending::Abandoned => {}
        }
        self.total_kills += kills as u64;
        self.best_level = self.best_level.max(level);
        self.best_kills = self.best_kills.max(kills);
        let new_best = time > self.best_time;
        if new_best {
            self.best_time = time;
        }
        new_best
    }
}

/// Where the save lives: `RNS_SAVE`, else the platform's per-user data folder.
pub fn save_path() -> PathBuf {
    save_path_from(
        std::env::var_os(SAVE_VAR),
        std::env::var_os("APPDATA"),
        std::env::var_os("XDG_DATA_HOME"),
        std::env::var_os("HOME"),
    )
}

fn save_path_from(
    over: Option<std::ffi::OsString>,
    appdata: Option<std::ffi::OsString>,
    xdg: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> PathBuf {
    if let Some(p) = over.filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let dir = appdata
        .map(|a| PathBuf::from(a).join("Rnsurvivors"))
        .or_else(|| xdg.map(|x| PathBuf::from(x).join("rnsurvivors")))
        .or_else(|| home.map(|h| PathBuf::from(h).join(".local/share/rnsurvivors")));
    match dir {
        Some(d) => d.join("kayit.txt"),
        None => PathBuf::from("rnsurvivors-kayit.txt"),
    }
}

fn record_run(
    run: Res<Run>,
    mut save: ResMut<Save>,
    mut died: MessageReader<HeroDied>,
    mut cleared: MessageReader<MapCleared>,
    mut restarted: MessageReader<RestartRun>,
) {
    let ending = if died.read().count() > 0 {
        Ending::Died
    } else if cleared.read().count() > 0 {
        Ending::Cleared
    } else if restarted.read().count() > 0 {
        Ending::Abandoned
    } else {
        return;
    };
    let before = save.data.points();
    // The meal's buff was for this run.
    save.data.active_food = None;
    let best = save
        .data
        .record(run.time, run.level, run.kills, run.xp_total, ending);
    if best {
        info!("rnsurvivors: new best time {:.0} s", run.time);
    }
    let gained = save.data.points() - before;
    if gained > 0 {
        info!("rnsurvivors: {gained} new talent point(s)");
    }
    save.write();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crafting_round_trips() {
        let mut d = SaveData::default();
        d.mats[Mat::CopperOre.index()] = 7;
        d.foods[Food::SpiderSoup.index()] = 2;
        d.food_pick = Some(Food::SpiderSoup);
        let mut it = crate::content::items::roll_item(
            &mut crate::combat::rules::Rng::new(3),
            crate::content::items::Source::Elite,
            5,
        );
        it.forge = 2;
        it.enchant = Some((crate::content::items::Affix::Crit, 0.04));
        d.bag.push(it.clone());
        let back = SaveData::parse(&d.to_text());
        assert_eq!(back.mats, d.mats);
        assert_eq!(back.foods, d.foods);
        assert_eq!(back.food_pick, d.food_pick);
        assert_eq!(back.bag[0].forge, 2);
        assert_eq!(back.bag[0].enchant, it.enchant);
        // The forge makes the bonuses stronger, the enchant adds one.
        assert!(it.bonus(crate::content::items::Affix::Crit) >= 0.04);
    }

    #[test]
    fn round_trips() {
        let mut d = SaveData::default();
        d.record(312.4, 14, 520, 300, Ending::Died);
        d.record(605.0, 22, 1400, 2000, Ending::Cleared);
        let back = SaveData::parse(&d.to_text());
        assert_eq!(back.runs, 2);
        assert_eq!(back.deaths, 1);
        assert_eq!(back.maps_cleared, 1);
        assert_eq!(back.total_kills, 1920);
        assert_eq!(back.best_level, 22);
        assert_eq!(back.best_kills, 1400);
        assert!((back.best_time - 605.0).abs() < 0.1);
        assert_eq!(back.talent_xp, 2300);
    }

    #[test]
    fn gear_and_talents_survive_the_save() {
        use crate::combat::rules::Rng;
        use crate::content::items::{roll_drop, Source};
        use crate::content::talents::TalentId;
        let mut d = SaveData {
            talent_xp: 1000,
            ..SaveData::default()
        };
        let mut rng = Rng::new(3);
        assert!(d
            .take_loot(roll_drop(&mut rng, Source::Chest, 9))
            .is_empty());
        d.equip(0);
        assert_eq!(d.equipped.len(), 1);
        assert_eq!(d.bag.len(), 2);
        d.talents.buy(TalentId::Fervor, d.points());
        let back = SaveData::parse(&d.to_text());
        assert_eq!(back, d);
        assert!(back.bonus().damage > 0.0);
    }

    #[test]
    fn equipping_swaps_and_the_bag_has_a_limit() {
        use crate::combat::rules::Rng;
        use crate::content::items::{roll_item, Source};
        let mut rng = Rng::new(8);
        let mut d = SaveData::default();
        let mut a = roll_item(&mut rng, Source::Elite, 1);
        a.slot = ItemSlot::Head;
        let mut b = a.clone();
        b.look = a.look + 1;
        d.take_loot(vec![a.clone(), b.clone()]);
        d.equip(0);
        d.equip(0); // b replaces a, a goes back to the bag
        assert_eq!(d.worn(ItemSlot::Head), Some(&b));
        assert_eq!(d.bag, vec![a.clone()]);
        d.unequip(ItemSlot::Head);
        assert!(d.worn(ItemSlot::Head).is_none() && d.bag.len() == 2);
        let left = d.take_loot(vec![a.clone(); BAG_SIZE]);
        assert_eq!(d.bag.len(), BAG_SIZE);
        assert_eq!(left.len(), 2);
        d.discard(0);
        assert_eq!(d.bag.len(), BAG_SIZE - 1);
    }

    #[test]
    fn records_only_go_up() {
        let mut d = SaveData::default();
        assert!(d.record(100.0, 5, 50, 0, Ending::Abandoned));
        assert!(!d.record(80.0, 3, 10, 0, Ending::Died));
        assert_eq!((d.best_level, d.best_kills), (5, 50));
        assert!((d.best_time - 100.0).abs() < 1e-6);
    }

    #[test]
    fn bad_files_still_load() {
        let d = SaveData::parse("garbage\nruns=x\nbest_level=7\nbest_time=NaN\nfuture_key=1\n");
        assert_eq!(d.best_level, 7);
        assert_eq!(d.runs, 0);
        assert_eq!(d.best_time, 0.0);
    }

    #[test]
    fn save_lands_in_the_profile() {
        use std::ffi::OsString;
        let p = save_path_from(
            None,
            Some(OsString::from("C:/Users/U/AppData/Roaming")),
            None,
            None,
        );
        assert!(p.ends_with("Rnsurvivors/kayit.txt"));
        let p = save_path_from(
            Some(OsString::from("x.txt")),
            Some(OsString::from("A")),
            None,
            None,
        );
        assert_eq!(p, PathBuf::from("x.txt"));
        let p = save_path_from(None, None, None, Some(OsString::from("/home/u")));
        assert!(p.ends_with(".local/share/rnsurvivors/kayit.txt"));
        assert_eq!(
            save_path_from(None, None, None, None),
            PathBuf::from("rnsurvivors-kayit.txt")
        );
    }

    #[test]
    fn system_params() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        IntoSystem::into_system(record_run).initialize(&mut world);
    }
}
