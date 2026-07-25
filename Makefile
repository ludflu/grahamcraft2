.PHONY: proto run run-client run-server test-server assets help

PROTO_DIR := proto
GENERATED_DIR := grahamcraft2/generated

help:
	@echo "Targets:"
	@echo "  make proto        - generate Python gRPC stubs"
	@echo "  make assets       - note: player model is procedural (no download)"
	@echo "  make run          - start the Ursina (Python) client"
	@echo "  make run-client   - start the Bevy (Rust) client"
	@echo "  make run-server   - start the Rust game server"
	@echo "  make test-server  - run Rust server tests"
	@echo ""
	@echo "Remote server (both clients):"
	@echo "  make run-client SERVER=192.168.1.10"
	@echo "  make run-client ARGS=\"--server 192.168.1.10:50051\""
	@echo "  make run ARGS=\"--server 192.168.1.10\""

assets:
	@echo "Player uses procedural cubes; no character assets required."

proto:
	uv run python -m grpc_tools.protoc \
		-I $(PROTO_DIR) \
		--python_out=$(GENERATED_DIR) \
		--grpc_python_out=$(GENERATED_DIR) \
		$(PROTO_DIR)/game.proto
	@python -c "from pathlib import Path; path = Path('$(GENERATED_DIR)/game_pb2_grpc.py'); text = path.read_text(); path.write_text(text.replace('import game_pb2', 'from . import game_pb2', 1))"

run:
	uv run python main.py $(if $(SERVER),--server $(SERVER),) $(ARGS)

run-client:
	$(MAKE) -C client run ARGS="$(if $(SERVER),--server $(SERVER),) $(ARGS)"

run-server:
	$(MAKE) -C server run

test-server:
	$(MAKE) -C server test
