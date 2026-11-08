#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$PWD"

./build.sh

VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" Resources/Info.plist)
RELEASE="$ROOT/dist/release"
rm -rf "$RELEASE"
mkdir -p "$RELEASE"

APP="$ROOT/dist/BoseBuddy.app"

echo "Creating zip..."
ditto -c -k --keepParent "$APP" "$RELEASE/BoseBuddy-$VERSION.zip"

echo "Creating dmg..."
DMG_DIR="$ROOT/dist/dmg"
rm -rf "$DMG_DIR"
mkdir -p "$DMG_DIR"
cp -R "$APP" "$DMG_DIR/"
ln -s /Applications "$DMG_DIR/Applications"
cat > "$DMG_DIR/READ ME - macOS blocked the app.txt" <<'TXT'
BoseBuddy is not notarized. macOS Gatekeeper blocks the first launch.
This is expected. Do not click "Move to Trash".

1. Open Applications in Finder.
2. Right-click BoseBuddy, choose Open, then Open.
3. If that does not work: double-click BoseBuddy, click Done, then
   System Settings > Privacy & Security > Open Anyway.

Terminal alternative:
  xattr -dr com.apple.quarantine /Applications/BoseBuddy.app
  open /Applications/BoseBuddy.app

Then allow Bluetooth and follow the setup guide.
Full steps: INSTALL.md in the repository.
TXT
hdiutil create -volname "BoseBuddy $VERSION" -srcfolder "$DMG_DIR" \
  -ov -format UDZO "$RELEASE/BoseBuddy-$VERSION.dmg" >/dev/null
rm -rf "$DMG_DIR"

echo "Creating source archive..."
STAGE=$(mktemp -d)
mkdir -p "$STAGE/BoseBuddy"
rsync -a \
  --exclude 'dist/' \
  --exclude '.git/' \
  --exclude '.DS_Store' \
  --exclude '*.zip' \
  --exclude '*.dmg' \
  "$ROOT/" "$STAGE/BoseBuddy/"
(cd "$STAGE" && zip -qr "$RELEASE/BoseBuddy-$VERSION-source.zip" BoseBuddy)
rm -rf "$STAGE"

echo "Checksums..."
(cd "$RELEASE" && shasum -a 256 * > SHA256SUMS.txt)

echo
ls -lh "$RELEASE"
echo
echo "Release artifacts in: $RELEASE"
