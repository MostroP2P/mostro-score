# Basic usage

The only required input is the node's pubkey (npub or hex):

```bash
mostro-score --pubkey npub1...
```

`--pubkey` can also come from the `MOSTRO_SCORE_PUBKEY` environment variable or a
[configuration file](../usage/config-file.md), in that order of precedence.

By default the tool queries the compiled-in relay (`wss://relay.mostro.network`) and
prints a colored report when standard output is a terminal, or plain text otherwise.
Pipe the output or redirect it to a file to get plain text automatically:

```bash
mostro-score --pubkey npub1... > report.txt
```
