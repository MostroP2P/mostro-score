//! Stub scaffolding for exit-code mapping. The real mapping (`0`/`1`/`2`/`3`/`5` per 002
//! FR-019, plus the JSON error envelope's `code` strings) is implemented in PR 2
//! (T062-T063) and extended with exit code `4` in PR 3.

/// Placeholder; real exit codes land with PR 2's `AppError` -> exit-code mapping.
#[allow(dead_code)]
pub const PLACEHOLDER_EXIT_CODE: i32 = 0;
