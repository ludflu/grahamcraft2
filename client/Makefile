.PHONY: build run test lint help

help:
	@echo "Targets:"
	@echo "  make build  - compile release binary"
	@echo "  make run    - start the Bevy game client"
	@echo "  make test   - run unit tests"
	@echo "  make lint   - run clippy and rustfmt checks"
	@echo ""
	@echo "Connect to a remote server:"
	@echo "  make run ARGS=\"--server 192.168.1.10:50051\""
	@echo "  cargo run -- --server 192.168.1.10"

build:
	cargo build --release

run:
	cargo run -- $(ARGS)

test:
	cargo test

lint:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check
