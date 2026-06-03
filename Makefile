# Developer convenience targets. `make check` is the full local gate; CI runs
# fmt + lint + test-unit (it deliberately skips the server-booting tests).
.PHONY: check fmt lint test test-unit run playtest

# Full check: formatting, lints (warnings as errors), and the whole test suite.
check: fmt lint test

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

# Unit tests only: skips the transport::tests, which boot an in-process server.
test-unit:
	cargo test --workspace -- --skip transport::tests

# Run the server (loads + validates the embedded default content config).
run:
	cargo run -p boiling-point-server --bin boiling-point-server

# Solo playtest: server + agent opponents + the terminal client, tabled via the
# matchmaking queue. Pass flags through ARGS, e.g.
#   make playtest ARGS="--brain fallback --agents 3"
playtest:
	./scripts/playtest.sh $(ARGS)
