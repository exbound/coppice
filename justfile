# coppice — standard command interface (Exbound convention).
# The Rust toolchain version is pinned in rust-toolchain.toml.

# List available recipes
default:
    @just --list

# Install the pinned Rust toolchain and its components
setup:
    rustup toolchain install

# Format sources in place
fmt:
    cargo fmt --all

# Lint, with warnings as errors
lint:
    cargo clippy --workspace --all-targets --locked -- -D warnings

# Format-check + lint + typecheck — the CI equivalent
check:
    cargo fmt --all --check
    just lint

# Run the test suite
test:
    cargo test --workspace --locked
