//! PR 12 (003 FR-015/FR-015a/FR-016/FR-016a): the persisted-configuration file's TOML
//! schema, semantic validation, and warn-and-ignore degradation. `load_config_file`
//! collapses all 4 "no config value to use" cases (absent, I/O error, parse error,
//! semantic error) into `None`, warning to an injected `err` writer for the latter 3
//! only, never for the first (FR-015 is explicit that a missing file is silent).

use crate::cli::options::validate_relay_urls;
use crate::report::content::SectionFilter;
use crate::report::render::Format;
use crate::stats::grid::Granularity;
use serde::Deserialize;
use std::path::Path;

/// FR-016a's raw TOML schema. `deny_unknown_fields` rejects any other top-level key,
/// matching FR-016a's "Any other value is a semantic validation failure" text with no
/// additional hand-written check needed -- this surfaces as `toml::from_str`'s own
/// deserialize error (`LoadFailure::Parse`, not `LoadFailure::Semantic`), which is why
/// that failure's warning message stays generic rather than claiming invalid syntax.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfigFile {
    pubkey: Option<String>,
    relays: Option<Vec<String>>,
    format: Option<String>,
    view: Option<String>,
    color: Option<String>,
    sections: Option<Vec<String>>,
}

/// The validated, semantically-typed result of a successfully loaded and validated
/// configuration file (003 FR-016/FR-016a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    /// A config-sourced `--pubkey` value (003 FR-016/FR-016a, amended 2026-07-26),
    /// validated at load time with the same check `nostr_sdk::PublicKey::parse`
    /// performs for an explicit `--pubkey` flag, so a value reaching this field is
    /// always well-formed -- kept as the original `String`, not a parsed `PublicKey`,
    /// since the flag-based path already re-parses it at its own usage point and this
    /// module has no other reason to depend on `nostr_sdk`'s `PublicKey` type.
    pub pubkey: Option<String>,
    pub relays: Option<Vec<String>>,
    pub format: Option<Format>,
    pub view: Option<Granularity>,
    /// `Some(true)` for `color = "always"`, `Some(false)` for `color = "never"`, `None`
    /// when absent (automatic), per FR-016a.
    pub color_override: Option<bool>,
    /// A config-sourced `--sections` value (003 FR-016/FR-016a, amended 2026-07-26),
    /// validated at load time via `SectionFilter::from_tokens`. An empty array is
    /// treated as absent, same rule as `relays`.
    pub sections: Option<SectionFilter>,
}

/// Every way a configuration file fails to supply a usable value, distinguished
/// internally so only the last three warn (FR-015 vs. FR-015a).
enum LoadFailure {
    NotFound,
    Io(std::io::Error),
    Parse(toml::de::Error),
    Semantic(String),
}

/// FR-015/FR-015a: loads and validates the configuration file at `path`. Returns `None`
/// for all 4 cases (absent, I/O error, malformed TOML, semantically invalid value);
/// warnings are written to `err` for every case except "absent", which stays silent per
/// FR-015. FR-015a requires all-or-nothing application: a semantically invalid value
/// invalidates the entire file, never applying only its valid keys.
pub fn load_config_file(path: &Path, err: &mut impl std::io::Write) -> Option<ConfigFile> {
    match load_config_file_inner(path) {
        Ok(config) => Some(config),
        Err(LoadFailure::NotFound) => None,
        Err(LoadFailure::Io(io_error)) => {
            let _ = writeln!(
                err,
                "Warning: could not read configuration file {}: {io_error}; ignoring it.",
                path.display()
            );
            None
        }
        Err(LoadFailure::Parse(parse_error)) => {
            // `parse_error` covers both a genuine TOML syntax error and
            // `#[serde(deny_unknown_fields)]` rejecting an unrecognized key in
            // otherwise syntactically valid TOML -- the message stays deliberately
            // generic ("could not be parsed") rather than "is not valid TOML", which
            // would be misleading for the latter case.
            let _ = writeln!(
                err,
                "Warning: configuration file {} could not be parsed: {parse_error}; ignoring it.",
                path.display()
            );
            None
        }
        Err(LoadFailure::Semantic(message)) => {
            let _ = writeln!(
                err,
                "Warning: configuration file {} has an invalid value: {message}; ignoring it.",
                path.display()
            );
            None
        }
    }
}

fn load_config_file_inner(path: &Path) -> Result<ConfigFile, LoadFailure> {
    // `symlink_metadata` stats the path entry itself without following a symlink, so it
    // distinguishes "genuinely nothing at this path" (FR-015, silent) from "something
    // exists here" -- including a broken symlink, which FR-015a explicitly names as an
    // I/O failure that must warn. `read_to_string` alone cannot make this distinction:
    // following a dangling symlink also fails with `ErrorKind::NotFound`, which would
    // otherwise be misclassified as ordinary absence.
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(io_error) if io_error.kind() == std::io::ErrorKind::NotFound => {
            return Err(LoadFailure::NotFound);
        }
        Err(io_error) => return Err(LoadFailure::Io(io_error)),
    }

    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(io_error) => return Err(LoadFailure::Io(io_error)),
    };

    let raw: RawConfigFile = toml::from_str(&contents).map_err(LoadFailure::Parse)?;

    validate(raw).map_err(LoadFailure::Semantic)
}

fn validate(raw: RawConfigFile) -> Result<ConfigFile, String> {
    let pubkey = match raw.pubkey {
        None => None,
        Some(pubkey) => {
            nostr_sdk::PublicKey::parse(&pubkey)
                .map_err(|error| format!("'pubkey' is invalid: {error}"))?;
            Some(pubkey)
        }
    };

    let relays = match raw.relays {
        Some(relays) if relays.is_empty() => None,
        Some(relays) => {
            validate_relay_urls(&relays).map_err(|error| error.to_string())?;
            Some(relays)
        }
        None => None,
    };

    let format = match raw.format.as_deref() {
        None => None,
        Some("console") => Some(Format::Console),
        Some("plain") => Some(Format::Plain),
        Some("json") => Some(Format::Json),
        Some(other) => {
            return Err(format!(
                "'format' must be one of console/plain/json, got '{other}'"
            ));
        }
    };

    let view = match raw.view.as_deref() {
        None => None,
        Some("daily") => Some(Granularity::Daily),
        Some("monthly") => Some(Granularity::Monthly),
        Some("yearly") => Some(Granularity::Yearly),
        Some(other) => {
            return Err(format!(
                "'view' must be one of daily/monthly/yearly, got '{other}'"
            ));
        }
    };

    let color_override = match raw.color.as_deref() {
        None => None,
        Some("always") => Some(true),
        Some("never") => Some(false),
        Some(other) => {
            return Err(format!(
                "'color' must be one of always/never, got '{other}'"
            ));
        }
    };

    let sections = match raw.sections {
        Some(sections) if sections.is_empty() => None,
        Some(sections) => {
            let filter = SectionFilter::from_tokens(&sections).map_err(|invalid_tokens| {
                format!(
                    "'sections' has unrecognized value(s): {}. Valid section names are: \
                     fetch, activity, stats, recommendations",
                    invalid_tokens.join(", ")
                )
            })?;
            Some(filter)
        }
        None => None,
    };

    Ok(ConfigFile {
        pubkey,
        relays,
        format,
        view,
        color_override,
        sections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_file(dir: &tempfile_shim::TempDir, name: &str, contents: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    /// Minimal drop-in for a temp-directory helper without adding a new dev-dependency:
    /// creates a unique directory under `std::env::temp_dir()` and removes it on drop.
    mod tempfile_shim {
        use std::path::{Path, PathBuf};

        pub struct TempDir(PathBuf);

        impl TempDir {
            pub fn new() -> Self {
                let unique = format!(
                    "mostro-score-config-test-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                );
                let path = std::env::temp_dir().join(unique);
                std::fs::create_dir_all(&path).unwrap();
                Self(path)
            }

            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn absent_file_returns_none_and_writes_no_warning() {
        let dir = tempfile_shim::TempDir::new();
        let path = dir.path().join("does-not-exist.toml");
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(err.is_empty());
    }

    const TEST_PUBKEY_HEX: &str =
        "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

    #[test]
    fn valid_file_with_every_key_present_is_loaded() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(
            &dir,
            "config.toml",
            &format!(
                r#"
            pubkey = "{TEST_PUBKEY_HEX}"
            relays = ["wss://relay.example"]
            format = "plain"
            view = "monthly"
            color = "always"
            sections = ["activity", "stats"]
            "#
            ),
        );
        let mut err: Vec<u8> = Vec::new();

        let config = load_config_file(&path, &mut err).expect("valid file loads");

        assert!(err.is_empty());
        assert_eq!(config.pubkey, Some(TEST_PUBKEY_HEX.to_string()));
        assert_eq!(config.relays, Some(vec!["wss://relay.example".to_string()]));
        assert_eq!(config.format, Some(Format::Plain));
        assert_eq!(config.view, Some(Granularity::Monthly));
        assert_eq!(config.color_override, Some(true));
        let sections = config.sections.expect("sections present");
        assert!(sections.activity);
        assert!(sections.stats);
        assert!(!sections.fetch);
        assert!(!sections.recommendations);
    }

    #[test]
    fn valid_file_with_some_keys_absent_leaves_them_none() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"relays = ["wss://relay.example"]"#);
        let mut err: Vec<u8> = Vec::new();

        let config = load_config_file(&path, &mut err).expect("valid file loads");

        assert_eq!(config.pubkey, None);
        assert_eq!(config.relays, Some(vec!["wss://relay.example".to_string()]));
        assert_eq!(config.format, None);
        assert_eq!(config.view, None);
        assert_eq!(config.color_override, None);
        assert_eq!(config.sections, None);
    }

    /// FR-016/FR-016a (amended 2026-07-26): a valid config-sourced `pubkey` value loads
    /// successfully.
    #[test]
    fn valid_pubkey_value_is_loaded() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(
            &dir,
            "config.toml",
            &format!(r#"pubkey = "{TEST_PUBKEY_HEX}""#),
        );
        let mut err: Vec<u8> = Vec::new();

        let config = load_config_file(&path, &mut err).expect("valid file loads");

        assert!(err.is_empty());
        assert_eq!(config.pubkey, Some(TEST_PUBKEY_HEX.to_string()));
    }

    /// FR-015a/FR-016a: an invalid config-sourced `pubkey` value is a semantic
    /// validation failure, same tier as an invalid `format`/`view`/`color` value --
    /// warn and ignore the entire file.
    #[test]
    fn invalid_pubkey_value_is_a_semantic_validation_failure() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"pubkey = "not-a-valid-pubkey""#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    /// FR-016a: a valid config-sourced `sections` value loads successfully.
    #[test]
    fn valid_sections_value_is_loaded() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"sections = ["fetch", "stats"]"#);
        let mut err: Vec<u8> = Vec::new();

        let config = load_config_file(&path, &mut err).expect("valid file loads");

        assert!(err.is_empty());
        let sections = config.sections.expect("sections present");
        assert!(sections.fetch);
        assert!(sections.stats);
        assert!(!sections.activity);
        assert!(!sections.recommendations);
    }

    /// FR-016a: an unrecognized `sections` name is a semantic validation failure.
    #[test]
    fn unrecognized_sections_name_is_a_semantic_validation_failure() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"sections = ["bogus"]"#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    /// FR-016a: an empty `sections` array is treated as absent, same rule as `relays`,
    /// not an error and not "show nothing".
    #[test]
    fn empty_sections_array_is_treated_as_absent() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", "sections = []");
        let mut err: Vec<u8> = Vec::new();

        let config = load_config_file(&path, &mut err).expect("empty sections is not an error");

        assert!(err.is_empty());
        assert_eq!(config.sections, None);
    }

    #[test]
    fn malformed_toml_returns_none_and_warns_identifying_the_file() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", "this is not [valid toml");
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        let message = String::from_utf8(err).unwrap();
        assert!(message.contains(&path.display().to_string()));
    }

    #[test]
    fn unreadable_file_simulated_as_a_directory_returns_none_and_warns() {
        let dir = tempfile_shim::TempDir::new();
        let path = dir.path().join("config.toml");
        std::fs::create_dir_all(&path).unwrap();
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        let message = String::from_utf8(err).unwrap();
        assert!(message.contains(&path.display().to_string()));
    }

    /// FR-015a explicitly names a broken symlink as an I/O failure that must warn --
    /// distinct from FR-015's silent "genuinely absent" case, even though both fail
    /// `read_to_string` with the same `ErrorKind::NotFound`.
    #[test]
    #[cfg(unix)]
    fn broken_symlink_returns_none_and_warns_instead_of_staying_silent() {
        let dir = tempfile_shim::TempDir::new();
        let path = dir.path().join("config.toml");
        std::os::unix::fs::symlink(dir.path().join("does-not-exist-target"), &path).unwrap();
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(
            !err.is_empty(),
            "a broken symlink must warn, not be treated as ordinary absence"
        );
    }

    #[test]
    fn semantically_invalid_format_returns_none_and_warns() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"format = "xml""#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    #[test]
    fn semantically_invalid_view_returns_none_and_warns() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"view = "weekly""#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    #[test]
    fn semantically_invalid_color_returns_none_and_warns() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"color = "sometimes""#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    #[test]
    fn malformed_relay_url_is_a_semantic_validation_failure() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"relays = ["not-a-url"]"#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    /// FR-015a: one valid key plus one semantically invalid key invalidates the ENTIRE
    /// file — the valid key is NEVER silently applied alongside the bad one. This is the
    /// all-or-nothing regression test FR-015a requires.
    #[test]
    fn one_invalid_key_invalidates_the_whole_file_never_applying_the_valid_key() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(
            &dir,
            "config.toml",
            r#"
            relays = ["wss://relay.example"]
            format = "xml"
            "#,
        );
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(
            result.is_none(),
            "a semantically invalid format must invalidate the whole file, including the \
             otherwise-valid relays key"
        );
        assert!(!err.is_empty());
    }

    /// FR-016a: an unrecognized top-level key is rejected via `deny_unknown_fields`,
    /// surfacing internally as a `toml::from_str` deserialize error rather than this
    /// module's own `validate()` step -- the observable behavior (ignore the whole
    /// file, warn) is identical either way, which is what this test asserts.
    #[test]
    fn unrecognized_top_level_key_is_rejected_and_the_whole_file_is_ignored() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", r#"unknown_key = "value""#);
        let mut err: Vec<u8> = Vec::new();

        let result = load_config_file(&path, &mut err);

        assert!(result.is_none());
        assert!(!err.is_empty());
    }

    /// FR-016a: an empty `relays` array is treated as absent, falling through the chain,
    /// not as "no relays to query" and not as an error.
    #[test]
    fn empty_relays_array_is_treated_as_absent() {
        let dir = tempfile_shim::TempDir::new();
        let path = write_file(&dir, "config.toml", "relays = []");
        let mut err: Vec<u8> = Vec::new();

        let config = load_config_file(&path, &mut err).expect("empty relays is not an error");

        assert!(err.is_empty());
        assert_eq!(config.relays, None);
    }
}
