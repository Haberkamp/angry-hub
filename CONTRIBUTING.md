# Contributing

## Setup

You need a recent [Rust](https://rustup.rs/) toolchain and [just](https://github.com/casey/just).

```bash
just setup
```

That installs `rustfmt` and `clippy`.

## Running locally

```bash
just run
```

`just run` sets `ANGRY_HUB_DISABLE_AUTO_UPDATE=1` so a GitHub release cannot replace your `cargo run` binary.

To disable auto-update when you launch the app some other way:

```bash
export ANGRY_HUB_DISABLE_AUTO_UPDATE=1
cargo run
```

Any of `1`, `true`, or `yes` turns checks off (case-insensitive). Unset the variable, or set it to anything else, to leave auto-update on.

## Checks

These match CI:

```bash
just fmt
just lint
just test
```
