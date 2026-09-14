set shell := ["sh", "-eu", "-c"]

# List the available recipes.
default:
    @just --list

# Run the desktop app in development mode.
[working-directory("core/backend")]
run *args:
    sh ../../scripts/install-dev-icon.sh
    ../app/node_modules/.bin/tauri dev -- {{ args }}

# Remove the Linux desktop entry used for development window icons.
remove-dev-icon:
    sh scripts/install-dev-icon.sh --remove

# Build the desktop application.
[working-directory("core/backend")]
build:
    ../app/node_modules/.bin/tauri build

# Build the frontend only.
[working-directory("core/app")]
build-app:
    bun run build

# Type-check all Rust backend targets and features.
[working-directory("core/backend")]
check:
    cargo check --all-targets --all-features

# Run all Rust backend tests.
[working-directory("core/backend")]
test:
    cargo test --all-targets --all-features

# Format Rust backend sources.
[working-directory("core/backend")]
fmt:
    cargo fmt --all

# Run Clippy on the Rust backend and treat warnings as errors.
[working-directory("core/backend")]
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Remove Rust backend build artifacts.
[working-directory("core/backend")]
clean:
    cargo clean
