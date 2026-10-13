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
	./tools/release.sh $(VERSION) $(if $(SIGN),--sign,) $(if $(NOTES),--notes "$(NOTES)",)

clean:
	cargo clean

docker:
	docker compose build

run:
	cargo run --bin screenbuddy
