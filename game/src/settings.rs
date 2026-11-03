//! Player settings that live next to the save (Windows: `%APPDATA%\Rnsurvivors\ayarlar.txt`),
//! written by the launcher (`Rnsurvivors.bat`). For now one key: `wow_data`, the WoW 1.12.1
//! `Data` folder, so `WOW_DATA` need not be set by hand.

use std::path::PathBuf;

/// The settings file: beside the save.
pub fn settings_path() -> PathBuf {
    crate::combat::save::save_path().with_file_name("ayarlar.txt")
}

/// The value of `key` in a `key=value` text (a UTF-8 BOM and spaces are ignored).
pub fn setting(text: &str, key: &str) -> Option<String> {
    text.trim_start_matches('\u{feff}')
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().to_string())
        })
        .filter(|v| !v.is_empty())
}

/// When `WOW_DATA` is not set, takes the Data folder from the settings file. Call before the
/// engine looks for the data.
pub fn apply_wow_data() {
    if std::env::var_os("WOW_DATA").is_some() {
        return;
    }
    let path = settings_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    if let Some(dir) = setting(&text, "wow_data") {
        eprintln!("rnsurvivors: WoW data from {}: {dir}", path.display());
        // Single-threaded here: nothing else has started yet.
        std::env::set_var("WOW_DATA", dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_launcher_file() {
        let t = "\u{feff}wow_data=C:\\Oyunlar\\WoW 1.12\\Data\r\nother = 1\n";
        assert_eq!(
            setting(t, "wow_data").as_deref(),
            Some("C:\\Oyunlar\\WoW 1.12\\Data")
        );
        assert_eq!(setting(t, "other").as_deref(), Some("1"));
        assert_eq!(setting(t, "missing"), None);
        assert_eq!(setting("wow_data=  \n", "wow_data"), None);
    }

    #[test]
    fn lives_beside_the_save() {
        assert_eq!(settings_path().file_name().unwrap(), "ayarlar.txt");
    }
}
