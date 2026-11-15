# Setup

Toolchain prerequisites and the local development loop for coppice.

## Prerequisites

- [`rustup`](https://rustup.rs) **1.28 or newer**: `just setup` relies on
  `rustup toolchain install` reading `rust-toolchain.toml`, which older
  versions do not do (some distribution packages are older; install rustup
  from rustup.rs if yours is). The Rust version is pinned in
  `rust-toolchain.toml` (currently 1.98.1, with `rustfmt` and `clippy`), and
  rustup selects it automatically inside this repository.
- A **C linker**, which Rust needs to link the binary and the tests:
  `build-essential` on Debian/Ubuntu, the Xcode command-line tools on macOS,
  the MSVC build tools on Windows. Without one, `just check` still passes
  (it does not link) but `just test` fails with `linker 'cc' not found`.
- [`just`](https://github.com/casey/just) for the task recipes.

No other system libraries are needed yet. The live-monitor crate will add
libzmq, and the test harness will add a C++ compiler and CMake to build
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
just check    # rustfmt --check, the lint opt-in check, then clippy with warnings as errors (CI equivalent)
just test     # cargo test across the workspace
```

Run `just check && just test` before every commit; CI runs the same.
`cargo clippy` and `cargo test` run with `--locked`, so a change to
dependencies must include the updated `Cargo.lock`.

Every crate under `crates/` must take the workspace lints (`[lints]` with
`workspace = true` in its `Cargo.toml`). `just check` fails on a crate that
does not, because otherwise it would silently skip the workspace-wide
`unsafe_code = "forbid"`.

## Test fixtures

Fixtures are self-authored or openly licensed, with provenance recorded in
`tests/fixtures/README.md`. Vendor sample files are never committed. See
`docs/engineering-spec.md` §8 for what each feature is tested against.
