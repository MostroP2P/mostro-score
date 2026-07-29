//! Binary-level tests for `-o`/`--output` (003 FR-020). `cli::options`'s own unit tests
//! (`resolve_context_default`/`validate_output_format`) cover the pure format-resolution
//! rules in isolation; these tests exercise the real `clap` parsing, real filesystem I/O,
//! and the real process exit codes end to end.
//!
//! A genuinely successful report render (exit `0`, real file content) needs a relay that
//! actually returns usable events -- this project deliberately carries no committed
//! golden-baseline fixture or local mock relay (see the "chore: remove committed
//! golden-baseline fixtures" history), so a live, data-bearing relay is not something this
//! suite can depend on without becoming flaky. The tests below instead pin every part of
//! `--output`'s behavior that is provable before a relay is ever queried: the two
//! validation rules themselves (FR-020's format constraint) and the destination-file
//! open failure path, both of which run ahead of any network attempt.

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

fn isolated_command() -> assert_cmd::Command {
    let mut command = assert_cmd::Command::cargo_bin("mostro-score").unwrap();
    command.args([
        "--config-dir",
        "/nonexistent-mostro-score-test-isolation-dir",
    ]);
    command
}

/// 003 FR-020: `--output` combined with an explicit `--format console` is rejected as a
/// usage error (exit `2`) before any relay is queried -- proven here by never supplying
/// any `--relays` value at all: an unreachable-relay failure (exit `3`) would prove the
/// rejection did not happen before the connection attempt.
#[test]
fn output_combined_with_explicit_console_format_is_a_usage_error() {
    let dir = TempDir::new("output-console-rejected");
    let output_path = dir.path().join("report.txt");

    let output = isolated_command()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--format",
            "console",
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--output"));
    assert!(!output_path.exists());
}

/// 003 FR-020: `--output` is valid with an explicit `--format json` -- the usage error
/// above is specific to `console`, not to every explicit `--format` value. Proven by
/// reaching relay-connection failure (exit `3`) with a deliberately unreachable relay,
/// rather than the format-rejection usage error (exit `2`).
#[test]
fn output_combined_with_explicit_json_format_is_not_rejected() {
    let dir = TempDir::new("output-json-accepted");
    let output_path = dir.path().join("report.json");

    let output = isolated_command()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--format",
            "json",
            "--output",
            output_path.to_str().unwrap(),
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(3));
}

/// 003 FR-020: `--output` with no `--format` at all does not hit the console-only usage
/// error either -- proven the same way, by reaching relay-connection failure (exit `3`)
/// rather than a format-rejection usage error (exit `2`). This is the omitted-`--format`
/// case FR-020 says must resolve straight to `plain`, skipping specs/002-cli-report-design
/// FR-010's terminal-detection default.
#[test]
fn output_with_no_format_resolves_past_the_console_only_validation() {
    let dir = TempDir::new("output-no-format-accepted");
    let output_path = dir.path().join("report.txt");

    let output = isolated_command()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--output",
            output_path.to_str().unwrap(),
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(3));
}

/// 003 FR-020: `--output` combined with `--color` and no explicit `--format` still
/// resolves to plain rather than console -- regression test for a bug where `--color`'s
/// automatic plain-to-console upgrade ran unconditionally, undoing `--output`'s forced
/// plain default and causing this exact combination to fail with a usage error (exit
/// `2`) instead of proceeding normally.
#[test]
fn output_combined_with_color_and_no_explicit_format_still_resolves_to_plain() {
    let dir = TempDir::new("output-color-no-format");
    let output_path = dir.path().join("report.txt");

    let output = isolated_command()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--output",
            output_path.to_str().unwrap(),
            "--color",
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    // Reaching relay-connection failure (exit 3) rather than the format-rejection usage
    // error (exit 2) proves --color did not upgrade the forced plain default to console.
    assert_eq!(output.status.code(), Some(3));
}

/// 003 FR-020: a destination that cannot be created (its parent directory does not
/// exist) surfaces as a fatal error before any relay is queried -- proven by using a
/// deliberately unreachable relay alongside it: if the file-open failure did not happen
/// first, the run would instead fail with the relay's own exit code (`3`), not this I/O
/// failure's exit code (`1`, `AppError::Other` via `AppError`'s `#[from] std::io::Error`).
#[test]
fn output_to_an_uncreatable_path_is_a_fatal_error_before_any_relay_is_queried() {
    let dir = TempDir::new("output-uncreatable-path");
    let output_path = dir.path().join("does-not-exist-parent").join("report.txt");

    let output = isolated_command()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--output",
            output_path.to_str().unwrap(),
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(!output_path.exists());
}

/// 003 FR-020: when `mostro_score::run` fails after the destination file was already
/// created (truncated by `File::create`), the stray empty file is removed rather than
/// left behind -- otherwise it could be mistaken for real (empty) output, since no
/// confirmation message is printed on a failed run. A deliberately unreachable relay
/// guarantees a run failure (exit `3`) that happens after the file has already been
/// opened for writing.
#[test]
fn output_file_is_removed_when_the_run_fails_after_it_was_created() {
    let dir = TempDir::new("output-cleanup-on-failure");
    let output_path = dir.path().join("report.txt");

    let output = isolated_command()
        .args([
            "--pubkey",
            TEST_PUBKEY_HEX,
            "--output",
            output_path.to_str().unwrap(),
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(3));
    assert!(
        !output_path.exists(),
        "a failed run must not leave a stray empty output file behind"
    );
}
