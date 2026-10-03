//! Where ScreenBuddy keeps user data on the current OS.
//!
//! Both the agent store and the session store persist here, so the location is
//! defined once. Getting this wrong is user-visible: a profile saved to one
//! path and read from another looks like data loss.

use std::path::PathBuf;

/// Directory holding all persistent ScreenBuddy state.
///
/// Follows each platform's convention: `%APPDATA%` on Windows,
/// `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` or
/// `~/.local/share` on Linux and other Unix.
pub fn data_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(fallback_home)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|home| {
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
            })
            .unwrap_or_else(fallback_home)
    } else {
        // Respect XDG_DATA_HOME first; it is how a Linux user redirects state.
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })
            .unwrap_or_else(fallback_home)
    };

    base.join("ScreenBuddy")
}

/// Home directory, or the current directory if even that is unavailable.
///
/// A last resort rather than a real answer: it keeps the app working in a
/// sandboxed environment rather than failing to start over a path.
fn fallback_home() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Ensure the data directory exists, creating it if needed.
pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Path to a file in the data directory.
pub fn data_file(name: &str) -> PathBuf {
    data_dir().join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_dir_is_named_for_the_app() {
        // A shared directory would collide with other apps.
        assert_eq!(
            data_dir().file_name().and_then(|n| n.to_str()),
            Some("ScreenBuddy")
        );
    }

    #[test]
    fn the_data_dir_is_absolute() {
        let dir = data_dir();
        assert!(
            dir.is_absolute(),
            "a relative data dir breaks as soon as the cwd changes: {}",
            dir.display()
        );
    }

    #[test]
    fn data_files_live_under_the_data_dir() {
        let file = data_file("agents.json");
        assert_eq!(file.parent(), Some(data_dir().as_path()));
        assert_eq!(
            file.file_name().and_then(|n| n.to_str()),
            Some("agents.json")
        );
    }

    #[test]
    fn ensure_creates_the_directory_if_needed() {
        let dir = ensure_data_dir().expect("create data dir");
        assert!(dir.is_dir(), "{} was not created", dir.display());
    }

    #[test]
    fn repeated_calls_are_stable() {
        // Sessions and agents must not disagree about where data lives.
        assert_eq!(data_dir(), data_dir());
    }
}
