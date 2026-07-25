//! Library target. Holds every constitution module (`cli`, `config`, `error`, `fetch`,
//! `models`, `report`, `stats`) so `tests/` integration tests can exercise them without a
//! binary-only crate, per the plan's Structure Decision. `main.rs` is the thin binary
//! target: argument parsing, wiring, and dispatch only. PR 1 Step D (T041) relocates the
//! wrapped `run()` function here as this crate's public entry point; until then this
//! file only wires the seven modules together.

pub mod cli;
pub mod config;
pub mod error;
pub mod fetch;
pub mod models;
pub mod report;
pub mod stats;
