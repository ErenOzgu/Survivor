//! A crash leaves a note: the panic's message, where it happened and the stack go to
//! `rnsurvivors-cokme.txt` beside the settings (Windows: `%APPDATA%\Rnsurvivors\`), so a crash
//! whose console window closed can still be read. The launcher also keeps the whole console
//! output of the last run in `rnsurvivors-log.txt` there.

use std::io::Write;
use std::path::PathBuf;

/// The crash note's file.
pub fn crash_path() -> PathBuf {
    crate::settings::settings_path().with_file_name("rnsurvivors-cokme.txt")
}

/// One crash note: when (seconds since 1970), the message and place, the stack.
pub fn crash_note(when: u64, version: &str, message: &str, stack: &str) -> String {
    format!("==== Rnsurvivors {version} çöktü (unix {when}) ====\n{message}\n\n{stack}\n\n")
}

/// Installs the hook: the note is written, then the usual panic report runs.
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let message = format!("[{thread}] {info}");
        let stack = std::backtrace::Backtrace::force_capture().to_string();
        let path = crash_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = f.write_all(
                crash_note(when, env!("CARGO_PKG_VERSION"), &message, &stack).as_bytes(),
            );
            eprintln!("rnsurvivors: crash note written to {}", path.display());
        }
        default(info);
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn note_names_the_crash() {
        let n = super::crash_note(5, "0.1.0", "[main] boom at a.rs:1", "stack");
        assert!(n.contains("boom") && n.contains("0.1.0") && n.ends_with("stack\n\n"));
        assert!(super::crash_path().ends_with("rnsurvivors-cokme.txt"));
    }
}
