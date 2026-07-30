//! `--init-config` scaffolding. Writes a starter configuration file with `relays` set to
//! the compiled-in default as an active value, and `pubkey`/`format`/`view`/`color`/
//! `sections` as commented-out examples -- none of the five has a fixed default to
//! freeze. This module only performs the file write; the exit-without-a-report and
//! `--force`-without-`--init-config` precedence wiring belong to `main.rs`/`cli::options`.

use crate::config::paths_defaults::DEFAULT_RELAY;
use crate::error::AppError;
use std::path::Path;

/// Writes a starter configuration file to `path`, creating the parent directory if
/// missing. Refuses to overwrite an existing file unless `force` is `true`, naming the
/// existing file's exact path in the returned error. The non-`force` path uses
/// `OpenOptions::create_new` rather than a `path.exists()` check followed by a separate
/// write, so a file created by another process between the two steps still causes this
/// call to fail atomically instead of silently overwriting it.
pub fn scaffold_config_file(path: &Path, force: bool) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let contents = starter_contents();

    if force {
        std::fs::write(path, contents)?;
        return Ok(());
    }

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write as _;
            file.write_all(contents.as_bytes())?;
            Ok(())
        }
        Err(io_error) if io_error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(AppError::UsageError(format!(
                "Configuration file already exists at {}; pass --force to overwrite it.",
                path.display()
            )))
        }
        Err(io_error) => Err(io_error.into()),
    }
}

/// Every one of the file's 6 keys is preceded by a short, plain-language comment
/// explaining what it does and which values it accepts. `relays` is the only active
/// (non-commented) key, since it is the only one with a fixed compiled-in default to
/// freeze; the other 5 are commented-out illustrative examples.
fn starter_contents() -> String {
    format!(
        "# The node's public key to look up (npub or hex). Same value --pubkey accepts.\n\
         # pubkey = \"npub1...\"\n\
         \n\
         # Nostr relays to query, one URL per array entry.\n\
         relays = [\"{DEFAULT_RELAY}\"]\n\
         \n\
         # How to print the report: console (colored, for a terminal), plain (no color,\n\
         # easy to grep), or json (machine-readable).\n\
         # format = \"console\"\n\
         \n\
         # Group the activity chart by day, month, or year.\n\
         # view = \"daily\"\n\
         \n\
         # Force colored output on or off: always or never.\n\
         # color = \"always\"\n\
         \n\
         # Only show these report sections: fetch, activity, stats, recommendations.\n\
         # sections = [\"activity\", \"stats\"]\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let unique = format!(
                "mostro-score-init-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn writes_successfully_when_absent() {
        let dir = TempDir::new();
        let path = dir.path().join("config.toml");

        scaffold_config_file(&path, false).expect("writes when absent");

        assert!(path.exists());
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = TempDir::new();
        let path = dir.path().join("nested").join("dirs").join("config.toml");

        scaffold_config_file(&path, false).expect("creates parents");

        assert!(path.exists());
    }

    #[test]
    fn refuses_to_overwrite_an_existing_file_without_force_naming_its_path() {
        let dir = TempDir::new();
        let path = dir.path().join("config.toml");
        scaffold_config_file(&path, false).expect("first write succeeds");

        let error =
            scaffold_config_file(&path, false).expect_err("second write without force fails");

        assert!(matches!(error, AppError::UsageError(_)));
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn overwrites_cleanly_when_force_is_true() {
        let dir = TempDir::new();
        let path = dir.path().join("config.toml");
        scaffold_config_file(&path, false).expect("first write succeeds");

        scaffold_config_file(&path, true).expect("force overwrite succeeds");

        assert!(path.exists());
    }

    #[test]
    fn written_content_has_an_active_relays_line_and_five_commented_examples() {
        let dir = TempDir::new();
        let path = dir.path().join("config.toml");

        scaffold_config_file(&path, false).expect("writes when absent");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains(&format!("relays = [\"{DEFAULT_RELAY}\"]")));
        assert!(contents.contains("# pubkey = "));
        assert!(contents.contains("# format = "));
        assert!(contents.contains("# view = "));
        assert!(contents.contains("# color = "));
        assert!(contents.contains("# sections = "));
    }

    #[test]
    fn every_key_is_preceded_by_an_explanatory_comment() {
        let dir = TempDir::new();
        let path = dir.path().join("config.toml");

        scaffold_config_file(&path, false).expect("writes when absent");

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();

        for key_line_prefix in [
            "# pubkey = ",
            "relays = ",
            "# format = ",
            "# view = ",
            "# color = ",
            "# sections = ",
        ] {
            let key_line_index = lines
                .iter()
                .position(|line| line.starts_with(key_line_prefix))
                .unwrap_or_else(|| panic!("key line '{key_line_prefix}' not found"));
            assert!(
                key_line_index > 0 && lines[key_line_index - 1].trim_start().starts_with('#'),
                "expected a comment line immediately before '{key_line_prefix}'"
            );
        }
    }
}
