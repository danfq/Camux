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

# Release native bundles through GitHub Actions.
release:
    #!/bin/sh
    set -eu

    if [ -n "$(git status --porcelain)" ]; then
        echo "The worktree must be clean before releasing" >&2
        exit 1
    fi

    branch=$(git symbolic-ref --quiet --short HEAD) || {
        echo "Releases must be started from a branch, not a detached HEAD" >&2
        exit 1
    }

    git fetch origin "$branch"
    remote_head=$(git rev-parse "origin/$branch")
    local_head=$(git rev-parse HEAD)
    if [ "$local_head" != "$remote_head" ]; then
        echo "HEAD must match origin/$branch before releasing" >&2
        exit 1
    fi

    if ! command -v gh >/dev/null 2>&1; then
        echo "GitHub CLI (gh) is required to start a release" >&2
        exit 1
    fi

    gh workflow run release.yml --ref "$branch"
    echo "Release workflow started"

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
