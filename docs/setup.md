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
  the MSVC build tools on Windows. Without one, `just test` fails with a
  "linker not found" error.
- A **C++ compiler** (`g++` from `build-essential`, Clang, or MSVC). The
  live-monitor crate's ZeroMQ binding compiles the libzmq 4.3.4 source it
  bundles and links it statically (design decision D4), so no system
  libzmq is needed, and `just check` builds it too.
- [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny) **0.22.1**
  (`cargo install --locked cargo-deny@0.22.1`), which `just check` runs
  to hold every dependency to `deny.toml` (design decision D9).
- [`just`](https://github.com/casey/just) for the task recipes.

`just check` and `just test` need nothing else. Building after a change to
dependencies downloads the new crates once into cargo's cache; in a
sandboxed agent session that cannot write `~/.cargo/registry`, a person
runs `cargo fetch` once, outside the sandbox. `just oracle` (the `.btlog`
oracle, spec §8) also needs CMake 3.16.3 or later and a C++17 compiler
using GCC's libstdc++ (as on Linux; not libc++ or MSVC, whose clocks the
harness's timing assumption does not hold for), to build BehaviorTree.CPP 4.9.0 and the
harness in `tests/harness/`. It runs with CMake 3.31.6 and GCC 14.2 locally,
and GCC 13.3 in CI. It downloads the library's release
archive from GitHub, or builds offline from an extracted copy named by
`BTCPP_SOURCE_DIR`.

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
just check    # rustfmt --check, the lint opt-in check, clippy with warnings as errors, then cargo-deny (CI equivalent)
just test     # cargo test across the workspace
```

Run `just check && just test` before every commit; CI runs the same.
`cargo clippy` and `cargo test` run with `--locked`, so a change to
dependencies must include the updated `Cargo.lock`.

Every crate under `crates/` must take the workspace lints, spelled exactly
as a `[lints]` table header followed by a `workspace = true` line in its
`Cargo.toml` (other TOML spellings that cargo accepts are rejected by the
check). `just check` fails on a crate that
does not, because otherwise it would silently skip the workspace-wide
`unsafe_code = "forbid"`.

## Test fixtures

Fixtures are self-authored or openly licensed, with provenance recorded in
`tests/fixtures/README.md`. Vendor sample files are never committed. See
`docs/engineering-spec.md` §8 for what each feature is tested against.
