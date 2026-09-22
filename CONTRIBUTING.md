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

## Checks

These match CI:

```bash
just fmt
just lint
just test
```
