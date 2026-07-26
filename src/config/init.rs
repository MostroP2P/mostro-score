//! PR 12 (003 FR-017/FR-018): `--init-config` scaffolding. Writes a starter configuration
//! file with `relays` set to the compiled-in default as an active value, and
//! `format`/`view`/`color` as commented-out examples — none of the three has a fixed
//! default to freeze, per FR-017. This module only performs the file write; the
//! exit-`0`-without-a-report and `--force`-without-`--init-config` precedence wiring
//! belong to `main.rs`/`cli::options`.

use crate::config::paths_defaults::DEFAULT_RELAY;
use crate::error::AppError;
use std::path::Path;

/// FR-017/FR-018: writes a starter configuration file to `path`, creating the parent
/// directory if missing. Refuses to overwrite an existing file unless `force` is `true`,
/// naming the existing file's exact path in the returned error. The non-`force` path
/// uses `OpenOptions::create_new` rather than a `path.exists()` check followed by a
/// separate write, so a file created by another process between the two steps still
/// causes this call to fail atomically instead of silently overwriting it.
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

fn starter_contents() -> String {
    format!(
        "relays = [\"{DEFAULT_RELAY}\"]\n\
         \n\
         # format = \"console\"\n\
         # view = \"daily\"\n\
         # color = \"always\"\n"
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
    fn written_content_has_an_active_relays_line_and_three_commented_examples() {
        let dir = TempDir::new();
        let path = dir.path().join("config.toml");

        scaffold_config_file(&path, false).expect("writes when absent");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains(&format!("relays = [\"{DEFAULT_RELAY}\"]")));
        assert!(contents.contains("# format = "));
        assert!(contents.contains("# view = "));
        assert!(contents.contains("# color = "));
    }
}
