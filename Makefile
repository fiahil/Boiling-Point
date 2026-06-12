# Developer convenience targets. `make check` is the full local gate; CI runs
# fmt + lint + test-unit (it deliberately skips the server-booting tests).
.PHONY: check fmt lint test test-unit run firewall-check harness-sample

# Full check: formatting, lints (warnings as errors), the whole test suite, and
# the client/server dependency firewall.
check: fmt lint firewall-check test

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy -p boiling-point-ai-client --all-features --all-targets -- -D warnings

test:
	cargo test --workspace
	cargo test -p boiling-point-ai-client --all-features

# Unit tests only: skips the transport::tests, which boot an in-process server.
test-unit:
	cargo test --workspace -- --skip transport::tests

# The AI-client dependency firewall (boom2-ai-client 3.1, Constitution I/D2):
# with default features the client crate must share ONLY the protocol crate
# with the server — the server crate may appear solely behind the opt-in
# `harness` feature (the batch-runner host).
firewall-check:
	@if cargo tree -p boiling-point-ai-client -e normal --no-default-features | grep -q "boiling-point-server"; then \
		echo "FIREWALL BREACH: clients/ai depends on the server crate outside the harness feature"; \
		exit 1; \
	fi
	@cargo check -p boiling-point-ai-client --no-default-features -q
	@echo "firewall ok: clients/ai shares only the protocol crate"

# A small pinned seeded balance sample through the AI client's harness mode
# (boom2-ai-client 8.1) — fast enough for CI, deterministic by construction.
harness-sample:
	cargo run -p boiling-point-ai-client --features harness --bin balance_tester -- \
		--games 200 --seed 424242 --report target/harness-sample

# Run the server (loads + validates the embedded default content config).
run:
	cargo run -p boiling-point-server --bin boiling-point-server
