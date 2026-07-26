//! The CLI argument-parsing module: `args` holds the clap-derived `Args` struct (moved
//! out of `main.rs` in PR 9, 003 FR-001..FR-013a), and `options` resolves those parsed
//! flags, plus environment/automatic-detection state, into the values `main.rs` and
//! `mostro_score::run` need.

pub mod args;
pub mod duration;
pub mod options;
