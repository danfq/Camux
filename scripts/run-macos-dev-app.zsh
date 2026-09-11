#!/bin/zsh

set -euo pipefail

# Tauri's runner replaces Cargo itself, so production builds and other Cargo
# commands should pass straight through. Only `cargo run` needs app staging.
if (( $# == 0 )) || [[ "$1" != "run" ]]; then
    exec cargo "$@"
fi
shift

typeset -a cargo_args app_args
typeset parsing_app_args=0

for argument in "$@"; do
    if (( parsing_app_args )); then
        app_args+=("$argument")
    elif [[ "$argument" == "--" ]]; then
        parsing_app_args=1
    else
        cargo_args+=("$argument")
    fi
done

cargo build "${cargo_args[@]}"

profile=debug
target_triple=""
target_dir="${CARGO_TARGET_DIR:-$PWD/target}"
binary_name=camux

for (( index = 1; index <= ${#cargo_args}; index++ )); do
    argument="${cargo_args[$index]}"
    case "$argument" in
        --release)
            profile=release
            ;;
        --profile)
            (( index++ ))
            profile="${cargo_args[$index]}"
            [[ "$profile" == "dev" ]] && profile=debug
            ;;
        --profile=*)
            profile="${argument#--profile=}"
            [[ "$profile" == "dev" ]] && profile=debug
            ;;
        --target)
            (( index++ ))
            target_triple="${cargo_args[$index]}"
            ;;
        --target=*)
            target_triple="${argument#--target=}"
            ;;
        --target-dir)
            (( index++ ))
            target_dir="${cargo_args[$index]}"
            ;;
        --target-dir=*)
            target_dir="${argument#--target-dir=}"
            ;;
        --bin)
            (( index++ ))
            binary_name="${cargo_args[$index]}"
            ;;
        --bin=*)
            binary_name="${argument#--bin=}"
            ;;
    esac
done

[[ "$target_dir" == /* ]] || target_dir="$PWD/$target_dir"
[[ -z "$target_triple" ]] || target_dir="$target_dir/$target_triple"

binary="$target_dir/$profile/$binary_name"
bundle="$target_dir/$profile/bundle/macos/Camux Dev.app"
bundle_executable="$bundle/Contents/MacOS/camux"
bundle_resources="$bundle/Contents/Resources"
script_dir="${0:A:h}"
project_dir="${script_dir:h}"

if [[ ! -x "$binary" ]]; then
    print -u2 "macOS dev runner could not find executable: $binary"
    exit 1
fi

mkdir -p "$bundle/Contents/MacOS" "$bundle_resources"
cp "$binary" "$bundle_executable"
cp "$project_dir/scripts/macos-dev/Info.plist" "$bundle/Contents/Info.plist"
cp "$project_dir/core/backend/target/macos-icon/Assets.car" "$bundle_resources/Assets.car"

exec "$bundle_executable" "${app_args[@]}"
