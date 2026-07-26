//! Terminal implementation of `models::core::ProgressReporter` (002 FR-014): shows a
//! one-line diagnostic once a relay fetch has run past the latency threshold, suppressed
//! when the destination is not a terminal. The writer and terminal-detection flag are
//! both injected at construction, rather than this type reading the real process
//! `stderr` directly inside `report_slow_fetch`, so it stays directly unit-testable,
//! matching every other `report` module's writer-injection convention (e.g.
//! `render::console`'s `out`/`err` parameters).

use crate::models::core::ProgressReporter;
use std::io::{IsTerminal, Write};
use std::sync::Mutex;

const SLOW_FETCH_MESSAGE: &str = "Still fetching from relays, this is taking longer than usual...";

/// `W` defaults to the real process `stderr` for production wiring; tests substitute an
/// in-memory writer to assert on the exact bytes written (or not written).
pub struct TerminalProgressReporter<W: Write = std::io::Stderr> {
    writer: Mutex<W>,
    is_terminal: bool,
}

impl TerminalProgressReporter<std::io::Stderr> {
    /// Production constructor: writes to the real `stderr`, detected as a terminal via
    /// `std::io::IsTerminal` (stable in std since Rust 1.70, so FR-014's off-tty
    /// suppression needs no new dependency). There is no `--quiet` flag yet (PR 10
    /// introduces flag wiring) — tty detection is the only suppression rule this
    /// implementation applies today.
    pub fn new() -> Self {
        Self {
            writer: Mutex::new(std::io::stderr()),
            is_terminal: std::io::stderr().is_terminal(),
        }
    }
}

impl Default for TerminalProgressReporter<std::io::Stderr> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Write> TerminalProgressReporter<W> {
    /// Test-only constructor: an explicit writer and terminal flag, so suppression and
    /// message content are both directly assertable without touching the real process
    /// `stderr` or depending on the test harness's own tty state.
    pub fn with_writer(writer: W, is_terminal: bool) -> Self {
        Self {
            writer: Mutex::new(writer),
            is_terminal,
        }
    }
}

impl<W: Write> ProgressReporter for TerminalProgressReporter<W> {
    fn report_slow_fetch(&self) {
        if !self.is_terminal {
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
    fn report_slow_fetch_writes_the_message_when_terminal() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reporter = TerminalProgressReporter::with_writer(SharedBuffer(buffer.clone()), true);

        reporter.report_slow_fetch();

        let written = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(written.contains("Still fetching"));
    }

    #[test]
    fn report_slow_fetch_is_suppressed_when_not_a_terminal() {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reporter = TerminalProgressReporter::with_writer(SharedBuffer(buffer.clone()), false);

        reporter.report_slow_fetch();

        assert!(buffer.lock().unwrap().is_empty());
    }
}
