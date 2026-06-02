#!/usr/bin/env bash
#
# playtest.sh — one-command solo playtest: bring up the server, fill a table with
# Claude/heuristic agents, and drop you into the terminal client. Everyone enters
# the matchmaking queue, so the table assembles itself (no invite codes to share).
#
# Usage:
#   scripts/playtest.sh [options]
#
# Options:
#   --agents N            Number of agent opponents to launch (default 3, so you + N = a table of 4)
#   --difficulty LEVEL    Agent difficulty: easy | hard (default hard)
#   --brain KIND          Agent brain: claude | fallback (default claude)
#                         claude  = real Claude opponents (needs Claude Code auth; costs tokens)
#                         fallback = zero-cost local heuristic (no auth, instant moves)
#   --persona NAME        Force one persona for all agents: gambler|turtle|bandwagoner|trickster
#                         (default: rotate through all four for variety)
#   --name NAME           Your display name in the client (default "You")
#   --url WS              Server WebSocket URL (default ws://127.0.0.1:8080/ws)
#   --no-build            Skip `cargo build` (use existing target/debug binaries)
#   -h, --help            Show this help
#
# The server and agents log to .playtest/ ; they are torn down when the client exits.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ---- defaults ---------------------------------------------------------------
AGENTS=3
DIFFICULTY=hard
BRAIN=claude
PERSONA=""
NAME="You"
URL="ws://127.0.0.1:8080/ws"
BUILD=1

usage() { sed -n '2,30p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --agents)     AGENTS="$2"; shift 2 ;;
    --difficulty) DIFFICULTY="$2"; shift 2 ;;
    --brain)      BRAIN="$2"; shift 2 ;;
    --persona)    PERSONA="$2"; shift 2 ;;
    --name)       NAME="$2"; shift 2 ;;
    --url)        URL="$2"; shift 2 ;;
    --no-build)   BUILD=0; shift ;;
    -h|--help)    usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

case "$BRAIN" in claude|fallback) ;; *) echo "--brain must be claude|fallback" >&2; exit 2 ;; esac
case "$DIFFICULTY" in easy|hard) ;; *) echo "--difficulty must be easy|hard" >&2; exit 2 ;; esac

PORT="$(printf '%s' "$URL" | sed -E 's#.*://[^:/]+:([0-9]+).*#\1#')"
[[ "$PORT" =~ ^[0-9]+$ ]] || PORT=8080

LOG_DIR="$ROOT/.playtest"
mkdir -p "$LOG_DIR"
SERVER_BIN="$ROOT/target/debug/boiling-point-server"
TUI_BIN="$ROOT/target/debug/boiling-point-tui"
PERSONAS=(gambler turtle bandwagoner trickster)
PIDS=()

cleanup() {
  echo
  echo "tearing down playtest…"
  for pid in "${PIDS[@]:-}"; do
    [[ -n "$pid" ]] || continue
    pkill -P "$pid" 2>/dev/null || true   # SDK subprocesses, if any
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# ---- build ------------------------------------------------------------------
if [[ "$BUILD" -eq 1 ]]; then
  echo "building server + terminal client…"
  cargo build -p boiling-point-server -p boiling-point-tui
fi
[[ -x "$SERVER_BIN" && -x "$TUI_BIN" ]] || { echo "binaries missing — run without --no-build" >&2; exit 1; }

# Agents need the Node package installed (node_modules present).
if [[ ! -d "$ROOT/agent-harness/node_modules" ]]; then
  echo "installing agent-harness dependencies…"
  ( cd "$ROOT/agent-harness" && npm install )
fi
if [[ "$BRAIN" == "claude" ]]; then
  echo "NOTE: --brain claude uses your Claude Code login and consumes tokens."
  echo "      Use --brain fallback for free, instant seat-fillers."
fi

# ---- server -----------------------------------------------------------------
echo "starting server (logs: $LOG_DIR/server.log)…"
"$SERVER_BIN" >"$LOG_DIR/server.log" 2>&1 &
PIDS+=("$!")

# Wait for the WebSocket port to accept connections.
for _ in $(seq 1 50); do
  if (exec 3<>"/dev/tcp/127.0.0.1/$PORT") 2>/dev/null; then exec 3>&- 3<&-; break; fi
  sleep 0.2
done

# ---- agents -----------------------------------------------------------------
echo "launching $AGENTS agent(s): brain=$BRAIN difficulty=$DIFFICULTY persona=${PERSONA:-rotating}…"
for i in $(seq 1 "$AGENTS"); do
  if [[ -n "$PERSONA" ]]; then p="$PERSONA"; else p="${PERSONAS[$(( (i - 1) % ${#PERSONAS[@]} ))]}"; fi
  node --experimental-strip-types "$ROOT/agent-harness/src/cli.ts" \
    --enqueue --brain "$BRAIN" --difficulty "$DIFFICULTY" --persona "$p" \
    --url "$URL" --name "$p-$i" \
    >"$LOG_DIR/agent-$i.log" 2>&1 &
  PIDS+=("$!")
done

# ---- client (foreground) ----------------------------------------------------
echo "starting terminal client — you are queued as \"$NAME\". Ctrl-C to quit."
echo
"$TUI_BIN" --connect "$URL" --enqueue --name "$NAME"
