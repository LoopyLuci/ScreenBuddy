.PHONY: all check test build release clean lint fmt docker run apk stage-release

all: check test build

check:
	cargo check --workspace --all-targets

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test --workspace --verbose

build:
	cargo build --workspace

release: apk
	cargo build --release --workspace

# Android debug APK
apk:
	cd ScreenBuddy-Android && ./gradlew assembleDebug

# Staged release artifacts for GitHub Releases
stage-release:
	mkdir -p dist
	cp target/release/screenbuddy.exe dist/ScreenBuddy-v$(VERSION)-windows-x64.exe
	cp ScreenBuddy-Android/app/build/outputs/apk/release/app-release.apk dist/ScreenBuddy-v$(VERSION)-android.apk

clean:
	cargo clean

docker:
	docker compose build

run:
	cargo run --bin screenbuddy
