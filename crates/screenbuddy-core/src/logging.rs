//! Structured logging.
//!
//! The crate had a handful of `tracing` calls and never installed a subscriber,
//! so every one of them was discarded and a user reporting a problem had nothing
//! to send. This installs a subscriber writing to both the console and a log
//! file, at a level the user can choose.
//!
//! The file writer is a small `MakeWriter` rather than a dependency: a rolling
//! appender is unnecessary for one process writing one file per run.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A writer that appends to one shared file from any thread.
#[derive(Clone)]
struct FileWriter {
    file: Arc<Mutex<std::fs::File>>,
}

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut f = self
            .file
            .lock()
            .map_err(|_| std::io::Error::other("log lock poisoned"))?;
        f.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut f = self
            .file
            .lock()
            .map_err(|_| std::io::Error::other("log lock poisoned"))?;
        f.flush()
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for FileWriter {
    type Writer = FileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Normalise a level name, falling back to `info` for anything unrecognised.
///
/// A bad value must not silently disable logging, which is the failure mode that
/// makes a support request useless.
pub fn normalise_level(level: &str) -> &'static str {
    match level.trim().to_ascii_lowercase().as_str() {
        "off" | "none" => "off",
        "error" => "error",
        "warn" | "warning" => "warn",
        "debug" => "debug",
        "trace" => "trace",
        _ => "info",
    }
}

/// Where the log file lives for this run.
pub fn default_log_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    base.join("ScreenBuddy").join("screenbuddy.log")
}

/// Install the global subscriber.
///
/// `log_path` of `None` logs to the console only. Returns an error string when
/// the file cannot be opened, so the caller can report it rather than losing
/// logging silently.
pub fn init(level: &str, log_path: Option<&Path>) -> Result<(), String> {
    use tracing_subscriber::fmt;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::EnvFilter;

    let level = normalise_level(level);
    if level == "off" {
        return Ok(());
    }
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    let console = fmt::layer().with_target(false);

    match log_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("could not open {}: {e}", path.display()))?;
            let writer = FileWriter {
                file: Arc::new(Mutex::new(file)),
            };
            let file_layer = fmt::layer()
                .with_target(false)
                .with_ansi(false)
                .with_writer(writer);
            tracing_subscriber::registry()
                .with(filter)
                .with(console)
                .with(file_layer)
                .try_init()
                .map_err(|e| e.to_string())
        }
        None => tracing_subscriber::registry()
            .with(filter)
            .with(console)
            .try_init()
            .map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_levels_are_kept() {
        for l in ["error", "warn", "info", "debug", "trace", "off"] {
            assert_eq!(normalise_level(l), l);
        }
    }

    #[test]
    fn levels_are_case_and_whitespace_insensitive() {
        assert_eq!(normalise_level("  INFO "), "info");
        assert_eq!(normalise_level("Warning"), "warn");
    }

    #[test]
    fn an_unknown_level_falls_back_to_info_not_off() {
        // Falling back to "off" would silently discard every log line.
        assert_eq!(normalise_level("verbose"), "info");
        assert_eq!(normalise_level(""), "info");
        assert_eq!(normalise_level("42"), "info");
    }

    #[test]
    fn the_default_log_path_is_beside_the_other_app_data() {
        let p = default_log_path();
        assert!(p.to_string_lossy().ends_with("screenbuddy.log"));
        assert!(p.to_string_lossy().contains("ScreenBuddy"));
    }

    #[test]
    fn the_file_writer_appends_and_can_be_cloned() {
        let dir = std::env::temp_dir().join("screenbuddy_log_writer_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("w.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();
        let mut w = FileWriter {
            file: Arc::new(Mutex::new(file)),
        };
        w.write_all(b"first\n").unwrap();
        let mut w2 = w.clone();
        w2.write_all(b"second\n").unwrap();
        w.flush().unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("first"));
        assert!(written.contains("second"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn off_installs_nothing_and_does_not_error() {
        // A second init in one process fails, so this only checks the off path
        // returns Ok without touching the global subscriber.
        assert!(init("off", None).is_ok());
    }
}
