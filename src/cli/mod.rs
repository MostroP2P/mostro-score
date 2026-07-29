//! CLI argument parsing. `args` holds the clap-derived `Args` struct; `options` resolves
//! parsed flags, plus environment/automatic-detection state, into the values `main.rs`
//! and `mostro_score::run` need.

pub mod args;
pub mod duration;
pub mod options;
