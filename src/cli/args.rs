//! PR 9: the CLI argument-parsing surface (003 FR-001..FR-003, FR-010..FR-013a), moved
//! here from `main.rs` per this module's original stub scaffolding note. `main.rs` stays
//! the thin wiring root: it parses `Args`, resolves them through `cli::options`, and
//! dispatches into `mostro_score::run`.

use clap::{ArgAction, Parser, ValueEnum};

use crate::report::render::Format;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Mostro Pubkey (npub or hex) to analyze. Falls back to `MOSTRO_SCORE_PUBKEY` when
    /// omitted (003 FR-001/FR-003); with neither present, clap's own required-argument
    /// handling rejects with its native usage error, exit code `2` (003 FR-013a).
    #[arg(short, long, env = "MOSTRO_SCORE_PUBKEY")]
    pub pubkey: String,

    /// Relays to connect to (comma separated). Precedence: this flag, then
    /// `MOSTRO_SCORE_RELAYS`, then the compiled-in default (003 FR-002/FR-003).
    #[arg(
        short,
        long,
        default_value = "wss://relay.mostro.network",
        env = "MOSTRO_SCORE_RELAYS"
    )]
    pub relays: String,

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
