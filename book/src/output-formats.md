# Output formats

## console

Colored, table-based, for an interactive terminal. The automatic default when standard
output is a terminal.

## plain

The same 5 sections and labels as console, no color, one `label: value` line per
metric (including each field of a repeated record, like a relay or an activity bucket),
so the output stays easy to grep or parse line by line. The automatic default when
standard output is redirected or piped.

## json

Machine-readable. Always an explicit choice via `--format json`, never auto-selected.
Every top-level key is always present, regardless of a node's data completeness or
`--sections` (which only filters console/plain-text rendering). A value that is not
applicable serializes as `null`, never an invented number, `NaN`, or infinity.

A fatal error (e.g. all relays unreachable) with `--format json` renders as a distinct
envelope, `{"schema_version": ..., "error": {"code", "message", "relays"}}`, on the same
stream a successful report would use — never a report-shaped document with its fields
left `null`.
