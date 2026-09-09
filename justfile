set shell := ["sh", "-eu", "-c"]

# List the available recipes.
default:
    @just --list

# Run the Rust application, forwarding any arguments.
run *args:
    cargo run -- {{ args }}

# Build the Rust application in debug mode.
build:
    cargo build

# Type-check all Rust targets and features.
check:
    cargo check --all-targets --all-features

# Run all Rust tests.
test:
    cargo test --all-targets --all-features

# Format Rust sources.
fmt:
    cargo fmt --all

# Run Clippy and treat warnings as errors.
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Remove Rust build artifacts.
clean:
    cargo clean
