//! CLI argument parsing. `main.rs` parses `Args`, resolves them through `cli::options`,
//! and dispatches into `mostro_score::run`.

use clap::{ArgAction, Parser, ValueEnum};

use crate::report::render::Format;
use crate::stats::grid::Granularity;

// Each flag has an explicit `help` attribute for the `-h`/`--help` text; clap prefers
// that over text extracted from a `///` doc comment, so the two never mix.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(
        short,
        long,
        env = "MOSTRO_SCORE_PUBKEY",
        help = "The node's public key to look up (npub or hex). Can also be set via the \
                MOSTRO_SCORE_PUBKEY environment variable or a saved configuration file. Not \
                required when using --init-config."
    )]
    pub pubkey: Option<String>,

    #[arg(
        short,
        long,
        env = "MOSTRO_SCORE_RELAYS",
        help = "Nostr relays to query, comma separated. Falls back to the \
                MOSTRO_SCORE_RELAYS environment variable, then to a saved configuration \
                file, then to a default public relay."
    )]
    pub relays: Option<String>,

    #[arg(
        long,
        value_enum,
        help = "How to print the report. Defaults to a saved configuration file value \
                if present, otherwise a colored console view in a terminal, or plain \
                text when not; json is always an explicit choice."
    )]
    pub format: Option<CliFormat>,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Force colored console output on, even when not printing to a terminal. \
                Has no effect on plain-text or json output."
    )]
    pub color: bool,

    #[arg(
        long = "no-color",
        action = ArgAction::SetTrue,
        help = "Force colored output off."
    )]
    pub no_color: bool,

    #[arg(
        short,
        long,
        action = ArgAction::SetTrue,
        help = "Hide progress messages while connecting and fetching data."
    )]
    pub quiet: bool,

    #[arg(
        long,
        help = "Only include activity on or after this date. Accepts YYYY-MM-DD or a \
                shorthand like 30d, 6mo, or 1y."
    )]
    pub since: Option<String>,

    #[arg(
        long,
        help = "Only include activity on or before this date. Same format as --since."
    )]
    pub until: Option<String>,

    #[arg(
        long,
        value_enum,
        help = "Group the activity chart by day, month, or year instead of choosing \
                automatically."
    )]
    pub view: Option<CliGranularity>,

    #[arg(
        long,
        help = "Only show these report sections, comma separated with no spaces: \
                fetch, activity, stats, recommendations. Has no effect with --format \
                json, which always includes everything."
    )]
    pub sections: Option<String>,

    /// `PathBuf`, not `String`: a filesystem path isn't guaranteed to be valid UTF-8 on
    /// Linux, and clap would reject an otherwise-valid non-UTF-8 path if this were a
    /// `String`.
    #[arg(
        short = 'd',
        long = "config-dir",
        help = "Read (or, with --init-config, write) the configuration file in this \
                directory instead of the default location."
    )]
    pub config_dir: Option<std::path::PathBuf>,

    #[arg(
        long = "init-config",
        action = ArgAction::SetTrue,
        help = "Create a starter configuration file (with the default relay active and \
                pubkey/format/view/color/sections shown as commented-out examples), \
                then exit without generating a report."
    )]
    pub init_config: bool,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Allow --init-config to overwrite an existing configuration file."
    )]
    pub force: bool,

    #[arg(
        short = 'o',
        long,
        help = "Write the report to this file instead of printing it. Not compatible \
                with the console format; defaults to plain text when --format is omitted."
    )]
    pub output: Option<std::path::PathBuf>,
}

/// Mirrors `report::render::Format` as a distinct type so `report::render` doesn't
/// depend on `clap`.
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

/// Mirrors `stats::grid::Granularity`, same reason as `CliFormat`.
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
