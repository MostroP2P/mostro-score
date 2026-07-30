# Configuration file

`--init-config` scaffolds a starter configuration file and exits without generating a
report:

```bash
mostro-score --init-config
```

The file is written to `$XDG_CONFIG_HOME/mostro-score/config.toml` (or
`~/.config/mostro-score/config.toml` when `XDG_CONFIG_HOME` is unset), unless
`-d`/`--config-dir` overrides the directory. Pass `--force` to overwrite an existing
file.

## Format

```toml
# pubkey = "npub1..."

relays = ["wss://relay.mostro.network"]

# format = "console"

# view = "daily"

# color = "always"

# sections = ["activity", "stats"]
```

Only `relays` ships active by default; every other key is a commented-out example,
since none of them has a fixed compiled-in default worth freezing.

- `format`: `console`, `plain`, or `json`.
- `view`: `daily`, `monthly`, or `yearly`.
- `color`: `always` or `never`.
- `sections`: array of `fetch`, `activity`, `stats`, `recommendations`.

An invalid value anywhere in the file invalidates the whole file (all-or-nothing): the
tool warns to stderr and falls back to flags/environment variables/compiled defaults, it
never applies only the valid keys. A missing file is silent, not a warning.

## Precedence

For every value the config file can supply, the flag wins, then the environment
variable (where one exists), then the config file, then the compiled-in default.
