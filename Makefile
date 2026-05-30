# Developer convenience targets. `make check` is the gate CI runs.
.PHONY: check fmt lint test run

# Full check: formatting, lints (warnings as errors), and tests.
check: fmt lint test

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

# Run the server (loads + validates the embedded default content config).
run:
	cargo run -p boiling-point-server --bin boiling-point-server
