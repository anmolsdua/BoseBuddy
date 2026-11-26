.PHONY: build run selftest install install-dev package msix clean

build:
	./build.sh

run: build
	open dist/BoseBuddy.app

selftest: build
	./dist/BoseBuddy.app/Contents/MacOS/BoseBuddy --selftest

install: build
	rm -rf /Applications/BoseBuddy.app
	cp -R dist/BoseBuddy.app /Applications/BoseBuddy.app
	open /Applications/BoseBuddy.app

# Dev loop: build with full Xcode when it's installed (the macOS 27 SDK's
# SwiftUI @State macro needs Xcode's compiler plugins, which the Command Line
# Tools don't ship), quit the running app, install to /Applications, relaunch
# and show the first log lines. The fixed signing identifier keeps the
# Bluetooth permission across reinstalls.
XCODE_DEVELOPER_DIR := /Applications/Xcode.app/Contents/Developer
LOG := $(HOME)/Library/Logs/BoseBuddy.log

install-dev:
	@if [ -z "$$DEVELOPER_DIR" ] && [ -d "$(XCODE_DEVELOPER_DIR)" ]; then \
		export DEVELOPER_DIR="$(XCODE_DEVELOPER_DIR)"; \
	fi; ./build.sh
	@if pgrep -x BoseBuddy >/dev/null; then \
		echo "Quitting running BoseBuddy..."; pkill -x BoseBuddy; \
		for i in $$(seq 1 25); do pgrep -x BoseBuddy >/dev/null || break; sleep 0.2; done; \
	fi
	rm -rf /Applications/BoseBuddy.app
	cp -R dist/BoseBuddy.app /Applications/BoseBuddy.app
	@START=$$(date -u +%Y-%m-%dT%H:%M:%SZ); open /Applications/BoseBuddy.app; \
	echo "Launched /Applications/BoseBuddy.app; waiting for it to connect..."; sleep 10; \
	awk -v t="$$START" '$$1 >= t' "$(LOG)" 2>/dev/null | grep -v 'peers:' | tail -5 || true

package:
	./package.sh

# Windows only (GNU make on Windows, e.g. `winget install ezwinports.make`):
# build QC Buddy and package it as an MSIX for the Microsoft Store, in
# windows/app/src-tauri/target/msix. See windows/RELEASING.md.
msix:
	powershell -NoProfile -ExecutionPolicy Bypass -File windows/app/scripts/package-msix.ps1 -Build

clean:
	rm -rf dist
