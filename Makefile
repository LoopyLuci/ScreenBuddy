.PHONY: all check test build release clean lint fmt docker run

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

release:
	cargo build --release --workspace

clean:
	cargo clean

docker:
	docker compose build

run:
	cargo run --bin screenbuddy
