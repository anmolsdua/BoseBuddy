#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"

APP="dist/BoseBuddy.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

SOURCES=$(find Sources -name '*.swift' | sort)
BIN="$APP/Contents/MacOS/BoseBuddy"

build_arch() {
  local arch="$1"
  echo "Compiling $arch..."
  swiftc -O -whole-module-optimization -swift-version 5 \
    -target "${arch}-apple-macos14.0" \
    -framework AppKit -framework SwiftUI -framework IOBluetooth \
    -framework CoreBluetooth -framework ServiceManagement \
    $SOURCES -o "$APP/Contents/MacOS/BoseBuddy-$arch"
}

build_arch arm64
build_arch x86_64
lipo -create "$APP/Contents/MacOS/BoseBuddy-arm64" \
             "$APP/Contents/MacOS/BoseBuddy-x86_64" \
     -output "$BIN"
rm -f "$APP/Contents/MacOS/BoseBuddy-arm64" "$APP/Contents/MacOS/BoseBuddy-x86_64"

cp Resources/Info.plist "$APP/Contents/Info.plist"
for resource in AppIcon.icns MenuIcon.png MenuIcon@2x.png MenuIcon@3x.png; do
  if [ -f "Resources/$resource" ]; then
    cp "Resources/$resource" "$APP/Contents/Resources/$resource"
  fi
done

# A stable designated requirement keeps the Bluetooth TCC grant across
# rebuilds instead of tying it to the ad-hoc cdhash.
codesign --force --sign - --identifier com.bosebuddy.app \
  --requirements '=designated => identifier "com.bosebuddy.app"' \
  "$APP"

echo "Built $APP"
lipo -info "$BIN"
echo "Designated requirement:"
codesign -d -r- "$APP" 2>&1 | sed -n '1,4p'
