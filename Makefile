APP := build/Dotline.app
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' core/Cargo.toml | head -1)
# rustup's toolchain carries the Windows std; Homebrew's cargo does not.
RUSTUP_BIN := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin

.PHONY: all core app test run clean icon dmg exe dist
all: app

core:
	cd core && MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --release

test:
	cd core && cargo test

app: core
	cd app && swift build -c release
	rm -rf "$(APP)" && mkdir -p "$(APP)/Contents/MacOS" "$(APP)/Contents/Resources"
	cp app/.build/release/Dotline "$(APP)/Contents/MacOS/Dotline"
	cp app/Info.plist "$(APP)/Contents/Info.plist"
	cp -R app/Sources/Dotline/Resources/*.lproj "$(APP)/Contents/Resources/"
	cp app/Icon/AppIcon.icns "$(APP)/Contents/Resources/AppIcon.icns"
	codesign --force --sign - "$(APP)"

run: app
	open "$(APP)"

clean:
	rm -rf build dist app/.build core/target windows/target

# Re-render app/Icon/AppIcon.icns from app/Icon/make-icon.swift.
icon:
	cd app/Icon && swiftc -O make-icon.swift -o /tmp/dotline-make-icon && rm -rf AppIcon.iconset && mkdir AppIcon.iconset && \
	for s in 16 32 128 256 512; do /tmp/dotline-make-icon AppIcon.iconset/icon_$${s}x$${s}.png $$s; \
	/tmp/dotline-make-icon AppIcon.iconset/icon_$${s}x$${s}@2x.png $$((s*2)); done && \
	iconutil -c icns AppIcon.iconset -o AppIcon.icns && rm -rf AppIcon.iconset

# macOS disk image: drag Dotline to Applications.
dmg: app
	mkdir -p dist && rm -rf build/dmg && mkdir -p build/dmg
	cp -R "$(APP)" build/dmg/ && ln -s /Applications build/dmg/Applications
	rm -f dist/Dotline-$(VERSION)-macos-arm64.dmg
	hdiutil create -volname Dotline -srcfolder build/dmg -fs HFS+ -format UDZO -imagekey zlib-level=9 dist/Dotline-$(VERSION)-macos-arm64.dmg

# Windows tray app, cross-compiled with mingw-w64 (brew install mingw-w64).
exe:
	cd windows && PATH="$(RUSTUP_BIN):$$PATH" CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
		cargo build --release --target x86_64-pc-windows-gnu
	mkdir -p dist && cp windows/target/x86_64-pc-windows-gnu/release/Dotline.exe dist/Dotline-$(VERSION)-windows-x64.exe

dist: dmg exe
