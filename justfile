# coppice — standard command interface (Exbound convention).
# The Rust toolchain version is pinned in rust-toolchain.toml.

# List available recipes
default:
    @just --list

# Install the pinned Rust toolchain and its components (needs rustup >= 1.28)
setup:
    rustup toolchain install

# Format sources in place
fmt:
    cargo fmt --all

# Lint, with warnings as errors
lint: lints-opted-in
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Fail if any crate does not take the workspace lints
lints-opted-in:
    @for f in crates/*/Cargo.toml; do awk '/^\[lints\]/{s=1;next} /^\[/{s=0} s && /^workspace *= *true/{ok=1} END{exit !ok}' "$f" || { echo "$f: missing '[lints] workspace = true'" >&2; exit 1; }; done

# Format-check + lint + typecheck — the CI equivalent
check: && lint
    cargo fmt --all --check

# Run the test suite
test:
    cargo test --workspace --locked

# Needs cmake and a C++17 compiler. Set BTCPP_SOURCE_DIR to an extracted
# 4.9.0 tree to build without downloading.
[doc("Build the BehaviorTree.CPP harness and test .btlog against its output")]
oracle:
    cmake -S tests/harness -B target/harness -DCMAKE_BUILD_TYPE=Release "-DFETCHCONTENT_SOURCE_DIR_BEHAVIORTREE_CPP=${BTCPP_SOURCE_DIR:-}"
    cmake --build target/harness --parallel
    rm -rf target/btlog-oracle
    target/harness/btlog_harness target/btlog-oracle
    COPPICE_BTLOG_ORACLE="$PWD/target/btlog-oracle" cargo test -p coppice-btlog --test oracle --locked -- --include-ignored
