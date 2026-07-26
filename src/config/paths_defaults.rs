//! PR 12 (003 FR-014): compiled-in defaults and platform config-directory resolution.
//! `resolve_config_path` is the pure, unit-testable function implementing FR-014's Linux
//! rule directly from explicit inputs, following this codebase's existing
//! injected-parameter pattern (`now`, tty state, and every other process-dependent input
//! elsewhere in this codebase is threaded in rather than read directly, so the resolution
//! logic itself stays testable without touching real process state). `default_config_path`
//! is the real wiring-root function `main.rs` calls, reading the actual environment and
//! delegating to `resolve_config_path` for the Linux case.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The single compiled-default source of truth for the relay used when no explicit
/// `--relays` flag, `MOSTRO_SCORE_RELAYS` environment variable, or configuration-file
/// `relays` value is present (003 FR-002/FR-016).
pub const DEFAULT_RELAY: &str = "wss://relay.mostro.network";

const CONFIG_FILE_NAME: &str = "config.toml";
const APP_NAME: &str = "mostro-score";

/// 003 FR-014's Linux rule as a pure function: `$XDG_CONFIG_HOME/mostro-score/config.toml`
/// when `xdg_config_home` is set, else `{home_dir}/.config/mostro-score/config.toml`. When
/// `config_dir_override` is `Some`, it always wins regardless of the other two inputs,
/// bypassing platform resolution entirely — this is `-d`/`--config-dir`'s override
/// behavior (003 FR-014), which is platform-independent.
///
/// Takes `&OsStr`, not `&str`: a real home directory or `$XDG_CONFIG_HOME` value is not
/// guaranteed to be valid UTF-8 on Linux, and lossily converting through `String` (e.g.
/// via `to_string_lossy`) would silently corrupt such a path into a different,
/// non-existent one -- this function must preserve the exact bytes the OS gave it.
pub fn resolve_config_path(
    config_dir_override: Option<&Path>,
    xdg_config_home: Option<&OsStr>,
    home_dir: Option<&OsStr>,
) -> PathBuf {
    if let Some(override_dir) = config_dir_override {
        return override_dir.join(CONFIG_FILE_NAME);
    }

    // The XDG Base Directory Specification requires `$XDG_CONFIG_HOME` to be an
    // absolute path; a relative value (or empty string) is invalid per that spec, not
    // a usable override, so it falls through to the `~/.config` default exactly as if
    // the variable were unset -- matching the `directories` crate's own filtering of
    // XDG variables through an absolute-path check.
    let xdg_config_home = xdg_config_home.filter(|xdg| Path::new(xdg).is_absolute());
    let config_dir = match xdg_config_home {
        Some(xdg) => Path::new(xdg).join(APP_NAME),
        None => {
            let home = home_dir.unwrap_or_else(|| OsStr::new(""));
            Path::new(home).join(".config").join(APP_NAME)
        }
    };

    config_dir.join(CONFIG_FILE_NAME)
}

/// The real wiring-root function: reads the actual process environment and delegates to
/// `resolve_config_path` for the Linux case. `--config-dir`'s override always wins,
/// matching `resolve_config_path`'s own override-first behavior.
///
/// macOS/Windows behavior below is written to the spec's literal path descriptions (003
/// FR-014: `~/Library/Application Support/mostro-score/config.toml` on macOS,
/// `%APPDATA%\mostro-score\config.toml` on Windows) but is untested in this Linux
/// sandbox. Both use `directories::BaseDirs::config_dir()` (the base per-user config
/// directory: `~/Library/Application Support` on macOS, `{FOLDERID_RoamingAppData}` i.e.
/// `%APPDATA%` on Windows) with `mostro-score/config.toml` appended manually, the same
/// two-step construction the Linux branch above already uses -- deliberately not
/// `directories::ProjectDirs`, whose derived subpaths add an extra qualifier/org-based
/// path segment on some platforms (e.g. an extra `\config` component on Windows) that
/// FR-014's literal single-level `mostro-score/config.toml` layout does not call for.
pub fn default_config_path(config_dir_override: Option<&Path>) -> PathBuf {
    if let Some(override_dir) = config_dir_override {
        return override_dir.join(CONFIG_FILE_NAME);
    }

    #[cfg(target_os = "linux")]
    {
        let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
        let base_dirs = directories::BaseDirs::new();
        let home_dir = base_dirs.as_ref().map(|dirs| dirs.home_dir().as_os_str());
        resolve_config_path(None, xdg_config_home.as_deref(), home_dir)
    }

    #[cfg(not(target_os = "linux"))]
    {
        directories::BaseDirs::new()
            .map(|dirs| dirs.config_dir().join(APP_NAME).join(CONFIG_FILE_NAME))
            .unwrap_or_else(|| PathBuf::from(CONFIG_FILE_NAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_config_path_uses_xdg_config_home_when_set() {
        let path = resolve_config_path(
            None,
            Some(OsStr::new("/custom/xdg")),
            Some(OsStr::new("/home/user")),
        );
        assert_eq!(path, PathBuf::from("/custom/xdg/mostro-score/config.toml"));
    }

    #[test]
    fn resolve_config_path_falls_back_to_home_dot_config_when_xdg_is_unset() {
        let path = resolve_config_path(None, None, Some(OsStr::new("/home/user")));
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/mostro-score/config.toml")
        );
    }

    /// The XDG Base Directory Specification requires `$XDG_CONFIG_HOME` to be an
    /// absolute path; a relative value is invalid, not a usable override, and must fall
    /// back to `~/.config` exactly as if the variable were unset.
    #[test]
    fn resolve_config_path_falls_back_to_home_dot_config_when_xdg_is_a_relative_path() {
        let path = resolve_config_path(
            None,
            Some(OsStr::new("relative/xdg")),
            Some(OsStr::new("/home/user")),
        );
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/mostro-score/config.toml")
        );
    }

    #[test]
    fn resolve_config_path_falls_back_to_home_dot_config_when_xdg_is_empty() {
        let path = resolve_config_path(None, Some(OsStr::new("")), Some(OsStr::new("/home/user")));
        assert_eq!(
            path,
            PathBuf::from("/home/user/.config/mostro-score/config.toml")
        );
    }

    #[test]
    fn resolve_config_path_override_wins_regardless_of_xdg_or_home() {
        let path = resolve_config_path(
            Some(Path::new("/explicit/override")),
            Some(OsStr::new("/custom/xdg")),
            Some(OsStr::new("/home/user")),
        );
        assert_eq!(path, PathBuf::from("/explicit/override/config.toml"));
    }

    #[test]
    fn resolve_config_path_override_wins_even_with_no_xdg_or_home_at_all() {
        let path = resolve_config_path(Some(Path::new("/explicit/override")), None, None);
        assert_eq!(path, PathBuf::from("/explicit/override/config.toml"));
    }

    /// A real home directory is not guaranteed to be valid UTF-8 on Linux. `&OsStr`
    /// (not `&str`) preserves the exact bytes rather than lossily replacing invalid
    /// UTF-8 with `U+FFFD`, which would silently point at a different, non-existent
    /// path.
    #[test]
    #[cfg(unix)]
    fn resolve_config_path_preserves_non_utf8_bytes_in_the_home_directory() {
        use std::os::unix::ffi::OsStrExt;

        let non_utf8_home = OsStr::from_bytes(b"/home/\xffuser");
        let path = resolve_config_path(None, None, Some(non_utf8_home));

        assert_eq!(
            path.as_os_str().as_bytes(),
            b"/home/\xffuser/.config/mostro-score/config.toml"
        );
    }

    #[test]
    fn default_config_path_override_always_wins() {
        let path = default_config_path(Some(Path::new("/explicit/override")));
        assert_eq!(path, PathBuf::from("/explicit/override/config.toml"));
    }
}
