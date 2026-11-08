.PHONY: build run selftest install package clean

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

package:
	./package.sh

clean:
	rm -rf dist
