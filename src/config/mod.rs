//! Persisted-configuration subsystem: an optional, platform-located TOML file supplying
//! persisted preferences for `--relays`, `--format`, `--view`, and color mode, plus the
//! `--init-config` scaffolding mechanism and `--config-dir` override.

pub mod file;
pub mod init;
pub mod paths_defaults;
