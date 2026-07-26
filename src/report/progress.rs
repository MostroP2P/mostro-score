//! Terminal implementation of `models::core::ProgressReporter` (002 FR-014): shows a
//! one-line diagnostic once a relay fetch has run past the latency threshold, suppressed
//! when the destination is not a terminal or when `--quiet` is set (003 FR-012 explicitly
//! names FR-014's progress indicators as something `--quiet` must suppress). The writer
//! and terminal-detection flag are both injected at construction, rather than this type
//! reading the real process `stderr` directly inside `report_slow_fetch`, so it stays
//! directly unit-testable, matching every other `report` module's writer-injection
//! convention (e.g. `render::console`'s `out`/`err` parameters).

use crate::models::core::ProgressReporter;
use std::io::{IsTerminal, Write};
use std::sync::Mutex;

const SLOW_FETCH_MESSAGE: &str = "Still fetching from relays, this is taking longer than usual...";

/// `W` defaults to the real process `stderr` for production wiring; tests substitute an
/// in-memory writer to assert on the exact bytes written (or not written).
pub struct TerminalProgressReporter<W: Write = std::io::Stderr> {
    writer: Mutex<W>,
    is_terminal: bool,
    quiet: bool,
}

impl TerminalProgressReporter<std::io::Stderr> {
    /// Production constructor: writes to the real `stderr`, detected as a terminal via
    /// `std::io::IsTerminal` (stable in std since Rust 1.70, so FR-014's off-tty
    /// suppression needs no new dependency). `quiet` is `--quiet`'s resolved value
    /// (003 FR-012, wired at construction in `main.rs` once `RunOptions` is resolved).
    pub fn new(quiet: bool) -> Self {
        Self {
            writer: Mutex::new(std::io::stderr()),
            is_terminal: std::io::stderr().is_terminal(),
            quiet,
        }
    }
}

impl Default for TerminalProgressReporter<std::io::Stderr> {
    fn default() -> Self {
        Self::new(false)
    }
}

impl<W: Write> TerminalProgressReporter<W> {
    /// Test-only constructor: an explicit writer, terminal flag, and quiet flag, so
    /// suppression and message content are both directly assertable without touching the
    /// real process `stderr` or depending on the test harness's own tty state.
    pub fn with_writer(writer: W, is_terminal: bool, quiet: bool) -> Self {
        Self {
            writer: Mutex::new(writer),
            is_terminal,
            quiet,
        }
    }
}

impl<W: Write> ProgressReporter for TerminalProgressReporter<W> {
    fn report_slow_fetch(&self) {
        if !self.is_terminal || self.quiet {
            return;
        }
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writeln!(writer, "{SLOW_FETCH_MESSAGE}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Clone)]
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn report_slow_fetch_writes_the_message_when_terminal_and_not_quiet() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reporter =
            TerminalProgressReporter::with_writer(SharedBuffer(buffer.clone()), true, false);

        reporter.report_slow_fetch();

        let written = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(written.contains("Still fetching"));
    }

    #[test]
    fn report_slow_fetch_is_suppressed_when_not_a_terminal() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reporter =
            TerminalProgressReporter::with_writer(SharedBuffer(buffer.clone()), false, false);

        reporter.report_slow_fetch();

        assert!(buffer.lock().unwrap().is_empty());
    }

    /// 003 FR-012: `--quiet` suppresses FR-014's progress indicator, even on a real
    /// terminal.
    #[test]
    fn report_slow_fetch_is_suppressed_when_quiet_even_on_a_terminal() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reporter =
            TerminalProgressReporter::with_writer(SharedBuffer(buffer.clone()), true, true);

        reporter.report_slow_fetch();

        assert!(buffer.lock().unwrap().is_empty());
    }
}
