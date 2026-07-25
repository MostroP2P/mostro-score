//! Stub scaffolding for the typed error taxonomy. At this point in history `main()`
//! still returns the generic `Result<(), Box<dyn std::error::Error>>` alias and relies
//! on the runtime's implicit termination on error, with no distinct exit-code mapping
//! logic to move verbatim. The real `AppError` taxonomy (`thiserror`-backed, mapped to
//! 002 FR-019's six exit codes) is implemented in PR 2 (T060-T063).

pub mod exit_code;

/// Placeholder error type; variants land with PR 2's taxonomy.
#[allow(dead_code)]
pub struct AppError;
