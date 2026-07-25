.PHONY: build run test lint help

help:
	@echo "Targets:"
	@echo "  make build  - compile release binary"
	@echo "  make run    - start the gRPC game server"
	@echo "  make test   - run unit tests"
	@echo "  make lint   - run clippy and rustfmt checks"

build:
	cargo build --release

run:
	cargo run

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check
