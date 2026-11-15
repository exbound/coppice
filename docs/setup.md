# Setup

Toolchain prerequisites and the local development loop for coppice.

## Prerequisites

- [`rustup`](https://rustup.rs). The Rust version is pinned in
  `rust-toolchain.toml` (currently 1.98.1, with `rustfmt` and `clippy`);
  rustup selects it automatically inside this repository.
- [`just`](https://github.com/casey/just) for the task recipes.

No system libraries are needed yet. The live-monitor crate will add libzmq,
and the test harness will add a C++ compiler and CMake to build
BehaviorTree.CPP; this file will say so when they do.

## First-time setup

```sh
just setup
```

This installs the pinned toolchain and its components through rustup. In a
sandboxed agent session that cannot write `~/.rustup`, a person runs it once,
outside the sandbox.

## Development loop

```sh
just fmt      # format
just check    # rustfmt --check + clippy with warnings as errors (CI equivalent)
just test     # cargo test across the workspace
```

Run `just check && just test` before every commit; CI runs the same. Cargo
runs with `--locked`, so a change to dependencies must include the updated
`Cargo.lock`.

## Test fixtures

Fixtures are self-authored or openly licensed, with provenance recorded in
`tests/fixtures/README.md`. Vendor sample files are never committed. See
`docs/engineering-spec.md` §8 for what each feature is tested against.
