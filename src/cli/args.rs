//! PR 9: the CLI argument-parsing surface (003 FR-001..FR-003, FR-010..FR-013a), moved
//! here from `main.rs` per this module's original stub scaffolding note. `main.rs` stays
//! the thin wiring root: it parses `Args`, resolves them through `cli::options`, and
//! dispatches into `mostro_score::run`.

use clap::{ArgAction, Parser, ValueEnum};

use crate::report::render::Format;
use crate::stats::grid::Granularity;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Mostro Pubkey (npub or hex) to analyze. Falls back to `MOSTRO_SCORE_PUBKEY` when
    /// omitted (003 FR-001/FR-003). Not required when `--init-config` is present (003
    /// FR-017); `main.rs` enforces its requiredness itself, as a usage error (exit code
    /// `2`, 003 FR-013a) for every other invocation, since clap's own native
    /// required-argument handling cannot express "required unless another flag is
    /// present".
    #[arg(short, long, env = "MOSTRO_SCORE_PUBKEY")]
    pub pubkey: Option<String>,

    /// Relays to connect to (comma separated). Precedence: this flag, then
    /// `MOSTRO_SCORE_RELAYS`, then the configuration file's `relays` value (003
    /// FR-016), then the compiled-in default `config::paths_defaults::DEFAULT_RELAY`
    /// (003 FR-002/FR-003). Omitted means "not explicitly set" — no `default_value`
    /// here, since a config-file value must be able to slot in between the
    /// environment variable and the compiled default (`cli::options::resolve_relays`
    /// composes the full chain).
    #[arg(short, long, env = "MOSTRO_SCORE_RELAYS")]
    pub relays: Option<String>,

    /// Output format: `console`, `plain`, or `json` (003 FR-010). Omitted means "not
    /// explicitly set" so `cli::options` can distinguish an explicit choice from the
    /// automatic default.
    #[arg(long, value_enum)]
    pub format: Option<CliFormat>,

    /// Force color output on for console format (003 FR-011). Mutually exclusive with
    /// `--no-color`.
    #[arg(long, action = ArgAction::SetTrue)]
    pub color: bool,

    /// Force color output off (003 FR-011). Mutually exclusive with `--color`.
    #[arg(long = "no-color", action = ArgAction::SetTrue)]
    pub no_color: bool,

    /// Suppress progress indicators and transient status narration (003 FR-012).
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,

    /// Scopes the activity grid's requested range to start on or after this date (003
    /// FR-004). Accepts an ISO 8601 calendar date or a relative shorthand (`30d`, `6mo`,
    /// `1y`), parsed later by `cli::duration`/`cli::options` — never validated here,
    /// since resolution needs a `now` value this flag-only layer has no business owning.
    #[arg(long)]
    pub since: Option<String>,

    /// Scopes the activity grid's requested range to end on or before this date (003
    /// FR-004). Same raw-string, parse-later contract as `--since`.
    #[arg(long)]
    pub until: Option<String>,

    /// Selects the activity grid's time-bucket granularity (003 FR-006). Omitted means
    /// "not explicitly set" so `cli::options` can distinguish an explicit choice from
    /// configuration-sourced or automatic selection.
    #[arg(long, value_enum)]
    pub view: Option<CliGranularity>,

    /// Restricts console/plain-text rendering to a comma-separated subset of the 4
    /// filterable section names (003 FR-008/FR-009): `fetch`, `activity`, `stats`,
    /// `recommendations`. Omitted means every section renders, matching current
    /// behavior. Has no effect on `--format json` (003 FR-008). Raw string, parsed and
    /// validated later by `cli::options`, matching `--since`/`--until`'s contract.
    #[arg(long)]
    pub sections: Option<String>,

    /// Overrides the directory the tool reads its configuration file from (003
    /// FR-014). When omitted, the platform-standard user configuration directory is
    /// used (`config::paths_defaults::default_config_path`). `PathBuf`, not `String`:
    /// a filesystem path is not guaranteed to be valid UTF-8 on Linux, and clap would
    /// reject an otherwise-valid non-UTF-8 path with a usage error if this were a
    /// `String`.
    #[arg(short = 'd', long = "config-dir")]
    pub config_dir: Option<std::path::PathBuf>,

    /// Writes a starter configuration file to the resolved config path and exits `0`
    /// without generating a report (003 FR-017/FR-019). Takes precedence over every
    /// report-generating flag passed alongside it in the same invocation.
    #[arg(long = "init-config", action = ArgAction::SetTrue)]
    pub init_config: bool,

    /// Allows `--init-config` to overwrite an existing configuration file (003
    /// FR-018). Rejected as a usage error when passed without `--init-config`.
    #[arg(long, action = ArgAction::SetTrue)]
    pub force: bool,
}

/// clap's own `ValueEnum`-derived mirror of `report::render::Format` (003 FR-010):
/// kept as a distinct type, rather than deriving `ValueEnum` on `Format` itself, so the
/// presentation-layer `report::render` module never depends on `clap` — `cli` is the
/// legitimate consumer of `report::render::Format`, not the other way around.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliFormat {
    Console,
    Plain,
    Json,
}

impl From<CliFormat> for Format {
    fn from(value: CliFormat) -> Self {
        match value {
            CliFormat::Console => Format::Console,
            CliFormat::Plain => Format::Plain,
            CliFormat::Json => Format::Json,
        }
    }
}

/// clap's own `ValueEnum`-derived mirror of `stats::grid::Granularity` (003 FR-006),
/// kept as a distinct type for the same reason `CliFormat` is kept distinct from
/// `report::render::Format`: `stats::grid` never depends on `clap`.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliGranularity {
    Daily,
    Monthly,
    Yearly,
}

impl From<CliGranularity> for Granularity {
    fn from(value: CliGranularity) -> Self {
        match value {
            CliGranularity::Daily => Granularity::Daily,
            CliGranularity::Monthly => Granularity::Monthly,
            CliGranularity::Yearly => Granularity::Yearly,
        }
    }
}
