//! CLI argument parsing: `args` holds the clap-derived `Args` struct, `options` resolves
//! parsed flags into what `main.rs` and `mostro_score::run` need.

pub mod args;
pub mod duration;
pub mod options;
