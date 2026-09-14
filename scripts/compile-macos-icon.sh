#!/bin/sh

set -eu

if [ "$(uname -s)" != "Darwin" ]; then
    exit 0
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_dir=$(dirname -- "$script_dir")
icon_source="$project_dir/assets/macos_icon.icon"
output_dir="$project_dir/core/backend/target/macos-icon"
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/camux-icon.XXXXXX")

cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$output_dir" "$work_dir/output"

# Xcode 26.6 can leave its shared ibtoold process in a state where the next
# Icon Composer compilation crashes with an NSPlaceholderArray exception.
# Retire the idle worker and give its asynchronous shutdown time to finish.
actool_path=$(xcrun --find actool)
"$actool_path" --quit-all-idle-servers >/dev/null 2>&1 || true
sleep 1

"$actool_path" "$icon_source" \
    --compile "$work_dir/output" \
    --output-format human-readable-text \
    --notices \
    --warnings \
    --output-partial-info-plist "$work_dir/output/assetcatalog_generated_info.plist" \
    --app-icon Icon \
    --include-all-app-icons \
    --enable-on-demand-resources NO \
    --development-region en \
    --target-device mac \
    --minimum-deployment-target 26.0 \
    --platform macosx

cp "$work_dir/output/Assets.car" "$output_dir/Assets.car"
