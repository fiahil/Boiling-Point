"use strict";
// Thin admin client. No build step (Constitution II: fully agent-writable). It
// reads the span-sourced projection over the admin API and issues commands; all
// requests carry the operator bearer token. The reveal and control are only
// reachable here, never on the player wire.

const $ = (sel) => document.querySelector(sel);
const el = (tag, cls, text) => {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
};

let token = localStorage.getItem("bp_admin_token") || "";
let activityAbort = null;
let roomsTimer = null;

// ---- API helper ---------------------------------------------------------

async function api(path, opts = {}) {
  const headers = Object.assign({ Authorization: `Bearer ${token}` }, opts.headers || {});
  const res = await fetch(path, Object.assign({}, opts, { headers }));
  if (res.status === 401) throw new Error("unauthorized — check the operator token");
  if (res.status === 403) throw new Error("forbidden — this action needs the elevated role");
  return res;
}

async function getJson(path) { return (await api(path)).json(); }

// ---- connection / role --------------------------------------------------

async function connect() {
  token = $("#token").value.trim() || token;
  localStorage.setItem("bp_admin_token", token);
  try {
    const fleet = await getJson("/admin/fleet");
    // The fleet endpoint accepts observer or elevated; probe a command's
    // authorization to display the effective role.
    const role = await probeRole();
    $("#role").textContent = role;
    $("#role").className = "pill " + (role === "elevated" ? "warn" : "ok");
    renderFleet(fleet);
    startRooms();
    startActivity();
    loadBalance();
  } catch (e) {
    $("#role").textContent = "denied";
    $("#role").className = "pill";
    alert(e.message);
  }
}

async function probeRole() {
  // A HEAD-like probe: reveal on a non-existent room returns 404 (elevated) or
  // 403 (observer). We only read the status, never any data.
  const res = await fetch("/admin/rooms/__probe__/reveal", {
    headers: { Authorization: `Bearer ${token}` },
  });
  return res.status === 403 ? "observer" : "elevated";
}

// ---- fleet --------------------------------------------------------------

function metricCard(label, value, sub) {
  const c = el("div", "card");
  c.appendChild(el("h3", null, label));
  const m = el("div", "metric");
  m.appendChild(document.createTextNode(value));
  if (sub) m.appendChild(el("small", null, " " + sub));
  c.appendChild(m);
  return c;
}

function renderFleet(f) {
  const box = $("#fleet-cards");
  box.innerHTML = "";
  box.appendChild(metricCard("Live rooms", f.rooms));
  box.appendChild(metricCard("In-flight games", f.games));
  box.appendChild(metricCard("Queue depth", f.queue_depth));
  box.appendChild(metricCard("Stuck rooms", f.stuck_rooms));
  box.appendChild(metricCard("Explosion rate", pct(f.balance.explosion_rate), "target 30–40%"));
  box.appendChild(metricCard("Games observed", f.balance.games));
}

const pct = (x) => (x * 100).toFixed(1) + "%";

// ---- rooms --------------------------------------------------------------

function startRooms() {
  if (roomsTimer) clearInterval(roomsTimer);
  const tick = async () => {
    try {
      const rooms = await getJson("/admin/rooms");
      renderRooms(rooms);
      renderFleet(await getJson("/admin/fleet"));
    } catch (_) { /* transient */ }
  };
  tick();
  roomsTimer = setInterval(tick, 2000);
}

function renderRooms(rooms) {
  $("#rooms-count").textContent = `(${rooms.length})`;
  const tb = $("#rooms-table tbody");
  tb.innerHTML = "";
  for (const r of rooms) {
    const tr = el("tr", "selectable" + (r.stuck ? " stuck" : ""));
    const round = r.round_number ? `${r.round_number}/${r.round_total}` : "—";
    const cells = [
      r.room_code || r.room_id,
      r.phase + (r.stuck ? " ⚠" : ""),
      round,
      r.wave_number ?? "—",
      r.players ?? "—",
      (r.age_ms / 1000).toFixed(0) + "s",
    ];
    for (const c of cells) tr.appendChild(el("td", null, String(c)));
    const td = el("td");
    const btn = el("button", null, "Inspect");
    btn.onclick = () => inspectRoom(r.room_code || String(r.room_id));
    td.appendChild(btn);
    tr.appendChild(td);
    tb.appendChild(tr);
  }
}

async function inspectRoom(code) {
  const box = $("#room-detail");
  box.innerHTML = "";
  box.appendChild(el("h3", null, "Room " + code));
  const revealBtn = el("button", "warn", "🔓 Reveal hidden state");
  revealBtn.onclick = () => reveal(code, box);
  box.appendChild(revealBtn);
}

async function reveal(code, box) {
  try {
    const res = await api(`/admin/rooms/${encodeURIComponent(code)}/reveal`);
    const data = await res.json();
    const out = el("div");
    if (data.status === "no_round_in_progress") {
      out.appendChild(el("p", "muted", "No round in progress."));
    } else if (data.status === "revealed") {
      const dl = el("dl", "reveal-grid");
      const add = (k, v, secret) => {
        dl.appendChild(el("dt", null, k));
        dl.appendChild(el("dd", secret ? "secret" : null, v ?? "—"));
      };
      add("Round", data.round_number);
      add("Wave", data.wave_number);
      add("Boiling point", data.boiling_point, true);
      add("Volatility", data.volatility_total, true);
      add("Modifiers", data.modifiers);
      out.appendChild(dl);
      if (data.hands.length) {
        out.appendChild(el("h4", null, "Hands"));
        for (const h of data.hands) {
          out.appendChild(el("div", "secret", `${h.player_id}: ${h.hand}`));
        }
      }
    } else {
      out.appendChild(el("p", "muted", "No such live room."));
    }
    box.querySelectorAll(".reveal-out").forEach((n) => n.remove());
    out.classList.add("reveal-out");
    box.appendChild(out);
  } catch (e) {
    alert(e.message);
  }
}

// ---- live activity (SSE over fetch, so the bearer header is sent) -------

async function startActivity() {
  if (activityAbort) activityAbort.abort();
  activityAbort = new AbortController();
  const feed = $("#activity-feed");
  try {
    const res = await fetch("/admin/live", {
      headers: { Authorization: `Bearer ${token}` },
      signal: activityAbort.signal,
    });
    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = "";
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      let idx;
      while ((idx = buf.indexOf("\n\n")) >= 0) {
        const frame = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        const line = frame.split("\n").find((l) => l.startsWith("data:"));
        if (!line) continue;
        try {
          appendActivity(feed, JSON.parse(line.slice(5).trim()));
        } catch (_) { /* keep-alive comment */ }
      }
    }
  } catch (_) { /* aborted or stream ended */ }
}

function appendActivity(feed, ev) {
  const li = el("li", "ev-" + ev.kind);
  const room = ev.room_code ? `[${ev.room_code}] ` : "";
  const detail = describe(ev);
  li.appendChild(el("span", "tag", ev.span));
  li.appendChild(document.createTextNode(`${ev.kind} ${room}${detail}`));
  feed.prepend(li);
  while (feed.childElementCount > 200) feed.lastChild.remove();
}

function describe(ev) {
  const a = ev.attributes || {};
  if (ev.span === "score" && ev.kind === "end") return `exploded=${a["round.exploded"]} dominant=${a.dominant_color ?? ""} pot=${a["pot.value"] ?? ""}`;
  if (ev.span === "round" && ev.kind === "end") return `r${a["round.number"] ?? "?"} exploded=${a["round.exploded"]}`;
  if (ev.span === "wave") return `w${a["wave.number"] ?? "?"} ${a["wave.timed_out"] === "true" ? "(timeout)" : ""}`;
  if (ev.span === "reconnect") return `player=${a["player.id"] ?? ""}`;
  if (ev.span === "ws.message") return a["ws.message_kind"] ?? "";
  if (ev.span === "admin.command") return `${a.action ?? ""} ${a.target ?? ""} → ${a.outcome ?? ""}`;
  return Object.entries(a).map(([k, v]) => `${k}=${v}`).join(" ");
}

// ---- replay -------------------------------------------------------------

async function loadReplay() {
  const list = $("#replay-list");
  list.innerHTML = "";
  const games = await getJson("/admin/replay");
  if (!games.length) list.appendChild(el("li", "muted", "No completed games retained yet."));
  for (const g of games.reverse()) {
    const li = el("li", null, `${g.room_code || g.game_id} · ${(g.duration_ms / 1000).toFixed(1)}s · ${g.span_count} spans`);
    li.onclick = () => viewReplay(g.game_id);
    list.appendChild(li);
  }
}

async function viewReplay(gameId) {
  const box = $("#replay-view");
  box.innerHTML = "";
  const res = await api(`/admin/replay/${encodeURIComponent(gameId)}`);
  if (res.status === 404) {
    box.appendChild(el("p", "muted", "Game no longer retained in the buffer."));
    return;
  }
  const game = await res.json();
  box.appendChild(el("h3", null, "Game " + (game.room_code || game.game_id)));
  // Group spans by round, then waves, in completion order.
  const rounds = game.spans.filter((s) => s.name === "round");
  for (const r of rounds) {
    const wrap = el("div", "card");
    wrap.appendChild(el("h4", null, `Round ${r.attributes["round.number"] ?? "?"} — exploded=${r.attributes["round.exploded"]} bp=${r.attributes.boiling_point ?? "?"}`));
    const waves = game.spans.filter((s) => s.name === "wave" && s.parent_id === r.id);
    for (const w of waves) {
      const commits = game.spans.filter((s) => s.name === "commit" && s.parent_id === w.id);
      const line = `wave ${w.attributes["wave.number"] ?? "?"}: ` +
        commits.map((c) => `${c.attributes["player.id"]?.slice(0, 4)}→${c.attributes.committed_card ?? "?"}`).join(", ");
      wrap.appendChild(el("div", "secret", line || `wave ${w.attributes["wave.number"]}`));
    }
    box.appendChild(wrap);
  }
}

// ---- control ------------------------------------------------------------

function log(msg) {
  const pre = $("#control-log");
  pre.textContent = `[${new Date().toLocaleTimeString()}] ${msg}\n` + pre.textContent;
}

async function command(cmd) {
  try {
    let res;
    if (cmd === "seed") {
      res = await api("/admin/commands/rooms/seed", { method: "POST" });
    } else if (cmd === "force-start" || cmd === "kill") {
      const code = $("#room-target").value.trim();
      if (!code) return alert("enter a room code");
      res = await api(`/admin/commands/rooms/${encodeURIComponent(code)}/${cmd}`, { method: "POST" });
    } else if (cmd === "toggle") {
      const kind = $("#toggle-kind").value;
      const raw = $("#toggle-value").value.trim();
      const value = kind === "emote" ? parseInt(raw, 10) : raw;
      res = await api("/admin/commands/toggle", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ selector: { kind, value }, enabled: $("#toggle-enabled").checked }),
      });
    } else if (cmd === "reload") {
      res = await api("/admin/commands/reload", {
        method: "POST",
        headers: { "Content-Type": "text/plain" },
        body: $("#reload-toml").value,
      });
    }
    log(`${cmd}: ${res.status} ${JSON.stringify(await res.json())}`);
  } catch (e) {
    log(`${cmd}: ${e.message}`);
  }
}

// ---- balance / grafana --------------------------------------------------

async function loadBalance() {
  const b = await getJson("/admin/balance");
  const box = $("#balance-cards");
  box.innerHTML = "";
  box.appendChild(metricCard("Explosion rate", pct(b.explosion_rate), "target 30–40%"));
  box.appendChild(metricCard("Cards / round", b.cards_per_round.toFixed(2)));
  box.appendChild(metricCard("Avg round", (b.avg_round_duration_ms / 1000).toFixed(1) + "s"));
  box.appendChild(metricCard("Avg game", (b.avg_game_duration_ms / 1000).toFixed(1) + "s"));
  box.appendChild(metricCard("Wave timeout", pct(b.wave_timeout_rate)));
  box.appendChild(metricCard("Reconnect / game", b.reconnection_rate.toFixed(2)));
  box.appendChild(metricCard("Dominant-colour", pct(b.dominant_color_rate)));
  box.appendChild(metricCard("Schema", "v" + b.schema_version));
  const url = localStorage.getItem("bp_grafana_url");
  if (url) $("#grafana").src = url;
}

// ---- wiring -------------------------------------------------------------

document.querySelectorAll(".tab").forEach((tab) => {
  tab.onclick = () => {
    document.querySelectorAll(".tab").forEach((t) => t.classList.remove("active"));
    document.querySelectorAll(".panel").forEach((p) => p.classList.remove("active"));
    tab.classList.add("active");
    const id = tab.dataset.tab;
    $("#" + id).classList.add("active");
    if (id === "replay") loadReplay();
    if (id === "balance") loadBalance();
  };
});

$("#connect").onclick = connect;
$("#token").value = token;
document.querySelectorAll("[data-cmd]").forEach((b) => (b.onclick = () => command(b.dataset.cmd)));
if (token) connect();
