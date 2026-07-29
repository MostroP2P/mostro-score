//! PR 12: binary-level tests for persisted configuration (003 FR-014..FR-019) —
//! `--init-config`, `--config-dir`, `--force`, and the config-file precedence chain.
//! `src/config/*.rs`'s own unit tests cover the pure resolution/validation functions in
//! isolation; these tests exercise the real `clap` parsing, real filesystem I/O, and the
//! real process exit codes end to end, which only a subprocess can prove.

const TEST_PUBKEY_HEX: &str = "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let unique = format!(
            "mostro-score-{label}-{}-{}",
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

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// T214/T218: `--init-config` writes a starter file to the resolved (here, overridden
/// via `--config-dir`) location, and exits `0`.
#[test]
fn init_config_writes_a_starter_file_to_the_overridden_config_dir() {
    let dir = TempDir::new("init-config");

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--init-config",
            "--config-dir",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(0));
    let config_path = dir.path().join("config.toml");
    assert!(config_path.exists());
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("relays = [\"wss://relay.mostro.network\"]"));
}

/// T218/FR-018: `--init-config` refuses to overwrite an existing file without
/// `--force`, exits non-zero, and names the existing file's path.
#[test]
fn init_config_refuses_to_overwrite_an_existing_file_without_force() {
    let dir = TempDir::new("init-config-overwrite");
    let config_dir = dir.path().to_str().unwrap();

    assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--init-config", "--config-dir", config_dir])
        .output()
        .expect("first run succeeds");

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--init-config", "--config-dir", config_dir])
        .output()
        .expect("second run without --force");

    assert_ne!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let config_path = dir.path().join("config.toml");
    assert!(stderr.contains(&config_path.display().to_string()));
}

/// T218/FR-018: `--init-config --force` overwrites cleanly.
#[test]
fn init_config_overwrites_cleanly_with_force() {
    let dir = TempDir::new("init-config-force");
    let config_dir = dir.path().to_str().unwrap();

    assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--init-config", "--config-dir", config_dir])
        .output()
        .expect("first run succeeds");

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--init-config", "--force", "--config-dir", config_dir])
        .output()
        .expect("second run with --force");

    assert_eq!(output.status.code(), Some(0));
}

/// T220/FR-019: `--init-config` takes precedence over `--pubkey`, performing only the
/// scaffold action, never generating a report.
#[test]
fn init_config_takes_precedence_over_pubkey_and_never_generates_a_report() {
    let dir = TempDir::new("init-config-precedence");

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--init-config",
            "--config-dir",
            dir.path().to_str().unwrap(),
            "--pubkey",
            TEST_PUBKEY_HEX,
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("NODE IDENTITY"));
}

/// T220/FR-019: `--init-config` short-circuits before any report-scoped validation, so
/// an otherwise-invalid report-scoped flag alongside it (here, an unrecognized
/// `--sections` token) is never evaluated -- the scaffold action still succeeds and
/// exits `0`, rather than surfacing FR-009's validation error.
#[test]
fn init_config_short_circuits_before_an_otherwise_invalid_sections_flag_is_validated() {
    let dir = TempDir::new("init-config-precedence-invalid-sections");

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--init-config",
            "--config-dir",
            dir.path().to_str().unwrap(),
            "--sections",
            "bogus",
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(dir.path().join("config.toml").exists());
}

/// T220/FR-019: `--init-config` against a malformed existing configuration file at the
/// resolved path performs only the overwrite check (FR-018), never loading or warning
/// about the existing file's content -- `--force` overwrites it cleanly with no
/// config-parsing warning on stderr.
#[test]
fn init_config_with_force_never_warns_about_a_malformed_existing_config_file() {
    let dir = TempDir::new("init-config-malformed-existing");
    std::fs::write(dir.path().join("config.toml"), "format = \"xml\"\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--init-config",
            "--force",
            "--config-dir",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
}

/// T216/T217/FR-018: `--force` without `--init-config` is a usage error (exit `2`),
/// rendered through the resolved `--format`, so `--format json --force` produces a JSON
/// fatal envelope rather than a plain-text usage error.
#[test]
fn force_without_init_config_with_explicit_json_format_produces_a_json_fatal_envelope() {
    let dir = TempDir::new("force-without-init-config-json");
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--format",
            "json",
            "--force",
            "--config-dir",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is a valid JSON fatal envelope");
    assert_eq!(json["error"]["code"], "usage_error");
}

/// T216/FR-018: `--force` without `--init-config` in the default (non-JSON) format is
/// still a plain usage error on stderr, exit `2`.
#[test]
fn force_without_init_config_is_a_usage_error() {
    let dir = TempDir::new("force-without-init-config-plain");
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--force",
            "--config-dir",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--init-config"));
}

/// T222/T223 (003 FR-016): a config file's `relays` value is honored when neither
/// `--relays` nor `MOSTRO_SCORE_RELAYS` are present — reaching relay-connection failure
/// (exit `3`) with the config file's own (unreachable) relay proves it was the value
/// actually used.
#[test]
fn config_file_relays_value_is_honored_when_no_flag_or_env_var_is_present() {
    let dir = TempDir::new("config-relays");
    std::fs::write(
        dir.path().join("config.toml"),
        "relays = [\"ws://127.0.0.1:1\"]\n",
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--config-dir",
            dir.path().to_str().unwrap(),
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    // The config file's relay is unreachable, so the run fails with exit code 3
    // (RelaysUnreachable) rather than succeeding with the compiled default relay —
    // proving the config-sourced value was actually used.
    assert_eq!(output.status.code(), Some(3));
}

/// T222/T223 (003 FR-016): an explicit `--relays` flag overrides the config file's
/// value.
#[test]
fn explicit_relays_flag_overrides_the_config_file_value() {
    let dir = TempDir::new("config-relays-override");
    std::fs::write(
        dir.path().join("config.toml"),
        "relays = [\"ws://127.0.0.1:1\"]\n",
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--config-dir",
            dir.path().to_str().unwrap(),
            "--relays",
            "not-a-url",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    // The explicit (malformed) --relays flag reaches relay well-formedness validation
    // (exit 2), never the config file's (unreachable but well-formed) value.
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-url"));
}

/// T222/T223 (003 FR-015a): a config file with a semantically invalid value is warned
/// about and ignored entirely -- the invocation proceeds as if no config file existed,
/// rather than surfacing a config-parsing usage error. An explicit, deliberately
/// unreachable local `--relays` value keeps this test's assertion (warned, not a usage
/// error) independent of the invalid `format` value under test, without depending on a
/// real network connection to the compiled default relay.
#[test]
fn a_semantically_invalid_config_file_is_ignored_and_falls_back_to_compiled_defaults() {
    let dir = TempDir::new("config-invalid");
    std::fs::write(dir.path().join("config.toml"), "format = \"xml\"\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--config-dir",
            dir.path().to_str().unwrap(),
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("config.toml"));
    assert_ne!(output.status.code(), Some(2));
}

/// 003 FR-011: a config-sourced `format` value governs a fatal error raised *before* a
/// report is generated (missing `--pubkey`), the same way an explicit `--format json`
/// flag would -- not just a successfully resolved report. Regression test for a bug
/// found in review where `error_render_format` was computed before the config file was
/// loaded, so a config-sourced `format = "json"` was silently ignored for exactly this
/// class of early fatal error.
#[test]
fn config_sourced_json_format_governs_the_missing_pubkey_fatal_error() {
    let dir = TempDir::new("config-format-missing-pubkey");
    std::fs::write(dir.path().join("config.toml"), "format = \"json\"\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--config-dir", dir.path().to_str().unwrap()])
        .env_remove("MOSTRO_SCORE_PUBKEY")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is a valid JSON fatal envelope");
    assert_eq!(json["error"]["code"], "usage_error");
}

/// 003 FR-016/FR-016a (amended 2026-07-26): a config-sourced `pubkey` value is honored
/// when neither `--pubkey` nor `MOSTRO_SCORE_PUBKEY` are present -- reaching relay
/// connection with the config file's own pubkey proves it was actually used (a
/// deliberately unreachable local relay makes the run fail fast at exit `3`, rather
/// than depending on a real network connection to the compiled default relay).
#[test]
fn config_file_pubkey_value_is_honored_when_no_flag_or_env_var_is_present() {
    let dir = TempDir::new("config-pubkey");
    std::fs::write(
        dir.path().join("config.toml"),
        format!("pubkey = \"{TEST_PUBKEY_HEX}\"\nrelays = [\"ws://127.0.0.1:1\"]\n"),
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--config-dir", dir.path().to_str().unwrap()])
        .env_remove("MOSTRO_SCORE_PUBKEY")
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(3));
}

/// 003 FR-016: an explicit `--pubkey` flag overrides the config file's value.
#[test]
fn explicit_pubkey_flag_overrides_the_config_file_value() {
    let dir = TempDir::new("config-pubkey-override");
    std::fs::write(
        dir.path().join("config.toml"),
        format!("pubkey = \"{TEST_PUBKEY_HEX}\"\n"),
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--config-dir",
            dir.path().to_str().unwrap(),
            "--pubkey",
            "not-a-valid-pubkey",
        ])
        .env_remove("MOSTRO_SCORE_PUBKEY")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(5));
}

/// 003 FR-015a: an invalid config-sourced `pubkey` value invalidates the whole file
/// and is never applied -- the invocation falls through to `--pubkey`'s existing
/// missing-value usage error when neither the flag nor its environment variable is
/// present either.
#[test]
fn invalid_config_pubkey_falls_back_to_the_missing_pubkey_usage_error() {
    let dir = TempDir::new("config-pubkey-invalid");
    std::fs::write(
        dir.path().join("config.toml"),
        "pubkey = \"not-a-pubkey\"\n",
    )
    .unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--config-dir", dir.path().to_str().unwrap()])
        .env_remove("MOSTRO_SCORE_PUBKEY")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("config.toml"));
}

/// 003 FR-016/FR-016a (amended 2026-07-26): a config-sourced `sections` value narrows
/// console/plain-text output the same way `--sections` would.
#[test]
fn config_file_sections_value_is_honored_when_flag_is_absent() {
    let dir = TempDir::new("config-sections");
    std::fs::write(dir.path().join("config.toml"), "sections = [\"stats\"]\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--config-dir",
            dir.path().to_str().unwrap(),
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    // The relay is deliberately unreachable so the run fails fast (exit 3) rather than
    // depending on a real network connection; a non-2 exit code and no config-parsing
    // warning together prove the `sections` key loaded and validated cleanly, which is
    // this test's actual concern -- `cli::options`'s own unit tests cover the
    // rendering-narrowing precedence itself.
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("config.toml"));
}

/// FR-018: refusing to overwrite an existing config file must be atomic, not a
/// `path.exists()` check followed by a separate write -- a file created between those
/// two steps must still be refused rather than silently overwritten. This test cannot
/// reproduce the race itself, but pins down the observable contract the atomic
/// `create_new`-based implementation must uphold: an existing file is always refused
/// without `--force`, regardless of how recently it was created.
#[test]
fn init_config_refuses_an_existing_file_even_when_created_immediately_beforehand() {
    let dir = TempDir::new("init-config-atomic");
    let config_path = dir.path().join("config.toml");
    std::fs::write(&config_path, "relays = [\"wss://preexisting.example\"]\n").unwrap();

    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--init-config",
            "--config-dir",
            dir.path().to_str().unwrap(),
        ])
        .output()
        .expect("binary runs");

    assert_ne!(output.status.code(), Some(0));
    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains("wss://preexisting.example"));
}
