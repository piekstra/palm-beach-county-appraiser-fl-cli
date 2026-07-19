# Convenience targets for pbca.

.PHONY: build test lint fmt fmt-check check install

build:
	cargo build --release

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

check: fmt-check lint test

install:
	cargo install --path .
