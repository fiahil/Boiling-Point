# Developer convenience targets. `make check` is the full local gate; CI runs
# fmt + lint + test-unit (it deliberately skips the server-booting tests).
.PHONY: check fmt lint test test-unit run firewall-check harness-sample \
        bench bench-study bench-dashboard

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

# --- Benchmarking suite (change `boom2-benchmarking`) ----------------------------
# Benchmarks MEASURE; tests gate. Nothing below ever blocks CI or a deploy (D1).
# The on-demand local flow is: `make bench` then (optionally) `make bench-study`,
# then `make bench-dashboard`, then open target/bench/benches.html.

# Run the criterion engine micro-benchmarks (per-merge in CI; on demand locally).
# Read the result as a TREND on the dashboard — single-run deltas inside the
# ~6–12% rerun noise are not regressions (D2). CI adds `--noplot`.
bench:
	cargo bench -p boiling-point-server --bench engine

# Run an on-demand balance study (Principle IV). With a STUDY config:
#   make bench-study STUDY=bench/balance-study/studies/explosion-band.toml
# Without one, a quick all-bot baseline (override GAMES/SEED):
#   make bench-study GAMES=2000 SEED=42
# Reports land in target/bench/studies/ (JSON for the dashboard + MD to read).
GAMES ?= 2000
SEED ?= 42
bench-study:
ifdef STUDY
	cargo run -q -p boiling-point-balance-study --bin balance_study -- \
		--study $(STUDY) --out target/bench/studies/$(notdir $(basename $(STUDY)))
else
	cargo run -q -p boiling-point-balance-study --bin balance_study -- \
		--games $(GAMES) --seed $(SEED) --out target/bench/studies/baseline
endif

# Collect the latest criterion run into the local history and render the single
# self-contained dashboard from that history + any study reports. Run `make bench`
# (and optionally `make bench-study`) first. Opens fully offline from disk.
bench-dashboard:
	cargo run -q -p boiling-point-bench-dashboard --bin bench_dashboard -- \
		collect --criterion-dir target/criterion --commit "$$(git rev-parse --short HEAD)" \
		--out "target/bench/history/$$(git rev-parse --short HEAD).json"
	cargo run -q -p boiling-point-bench-dashboard --bin bench_dashboard -- \
		render --history target/bench/history --studies target/bench/studies \
		--out target/bench/benches.html
	@echo "dashboard: open target/bench/benches.html"
