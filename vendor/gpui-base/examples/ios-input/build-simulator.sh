#!/usr/bin/env bash
# Builds the probe for the iOS simulator and packages it as an .app (the shape of
# vendor/gpui-pre-ios/examples/ios/build-simulator.sh).
set -euo pipefail
example_directory="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
target_directory="${CARGO_TARGET_DIR:-"$example_directory/target"}"
app_directory="$target_directory/gpui-base-ios-input/GpuiBaseIosInput.app"
cd "$example_directory"
IPHONEOS_DEPLOYMENT_TARGET=15.0 cargo build --target aarch64-apple-ios-sim "$@"
mkdir -p "$app_directory"
cp "$target_directory/aarch64-apple-ios-sim/debug/gpui_base_ios_input" "$app_directory/gpui_base_ios_input"
cp "$example_directory/app/Info.plist" "$app_directory/Info.plist"
codesign --force --sign - "$app_directory"
printf 'Built simulator app: %s\n' "$app_directory"
