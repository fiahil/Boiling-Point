# Ops: admin surface & balance dashboard

The admin surface (`admin-ui`) has two parts at runtime:

1. **The admin API + thin web app** — served by the server on an **isolated port**
   (`:8081`), distinct from the player WebSocket (`:8080`). Operators authenticate
   with bearer tokens; the read surface is a projection of the span stream and the
   command plane goes through the server's authoritative primitives.
2. **The balance dashboard** — embedded Grafana over Prometheus, where the
   Principle IV figures live (`ops/grafana/`).

## Running the server with the admin surface

```sh
# Operator tokens (separate from anonymous player session tokens):
export BP_ADMIN_TOKEN=$(openssl rand -hex 16)            # elevated: reveal + control
export BP_ADMIN_OBSERVER_TOKEN=$(openssl rand -hex 16)   # observer: read-only
# Optional: export traces to an OTLP backend (secrets are redacted at the boundary)
# export BP_OTLP_ENDPOINT=http://localhost:4317

cargo run -p boiling-point-server
```

- Player WebSocket: `ws://localhost:8080/ws`
- Admin web app: `http://localhost:8081/admin/` (paste the bearer token to connect)
- Unsampled Prometheus metrics: `http://localhost:9090/`

With **no** tokens set the admin API authenticates no one (every request is
rejected) — the player server is unaffected.

## Balance dashboard (Grafana + Prometheus)

```sh
cd ops/grafana
docker compose up -d
# Grafana:    http://localhost:3000  (dashboard "Boiling Point — Balance")
# Prometheus: http://localhost:9091
```

Prometheus scrapes the server at `host.docker.internal:9090` (run the server on
the host first). The dashboard is provisioned from
`dashboards/boiling-point-balance.json`; every figure derives from the
**unsampled** Prometheus source, never a sampled trace (design D6).

### Embedding behind admin auth

The admin web app embeds Grafana in the **Balance** tab. Point it at your Grafana
panel/dashboard URL once:

```js
// in the admin web app's browser console
localStorage.setItem("bp_grafana_url", "http://localhost:3000/d/bp-balance?kiosk");
```

`GF_SECURITY_ALLOW_EMBEDDING=true` permits the `<iframe>`. The cross-origin
(`:8081` → `:3000`) iframe can't carry Grafana's `SameSite=Lax` session cookie, so
to make the embed render in local dev the compose enables **`GF_AUTH_ANONYMOUS_ENABLED=true`
(Viewer)**. That makes the dashboard publicly viewable on `:3000`.

> **Production:** set `GF_AUTH_ANONYMOUS_ENABLED=false` and deploy Grafana
> **same-origin** behind the same reverse proxy that fronts the admin API, so the
> embed is reachable only by an authenticated operator (`balance-dashboard`:
> "Embed is gated by admin auth").

## What comes from where (unsampled sources only)

| Figure | Source |
|---|---|
| Explosion rate, cards/round, durations, dominant-colour, timeout, reconnection, reshuffle | Prometheus (this dashboard) **and** the projection (`GET /admin/balance`) |
| Live rooms, phase, queue depth, stuck flags | projection open-span registry (`GET /admin/rooms`) |
| Hidden-state reveal (boiling point, hands, volatility, modifiers) | projection, from open-span attributes, elevated-only (`GET /admin/rooms/{code}/reveal`) |
| Per-game replay | projection replay buffer (`GET /admin/replay`) |
