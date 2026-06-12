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
let groupsTimer = null;

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
    const me = await getJson("/admin/me");
    $("#role").textContent = me.role;
    $("#role").className = "pill " + (me.role === "elevated" ? "warn" : "ok");
    renderFleet(await getJson("/admin/fleet"));
    startGroups();
    startActivity();
    loadBalance();
  } catch (e) {
    $("#role").textContent = "denied";
    $("#role").className = "pill";
    alert(e.message);
  }
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
  box.appendChild(metricCard("Live groups", f.groups));
  box.appendChild(metricCard("In-flight games", f.games));
  box.appendChild(metricCard("Queue depth", f.queue_depth));
  box.appendChild(metricCard("Stuck groups", f.stuck_groups));
  const boom = f.balance.metrics.find((m) => m.id === "boom_rate");
  box.appendChild(metricCard("Boom rate", fmtMetric(boom), targetText(boom)));
  box.appendChild(metricCard("Games observed", f.balance.games));
}

const pct = (x) => (x * 100).toFixed(1) + "%";

// Render one evaluated boom-balance-metrics value ("—" until its population exists).
function fmtMetric(m) {
  if (!m || m.value == null) return "—";
  if (m.unit === "ratio") return pct(m.value);
  if (m.unit === "seconds") return m.value.toFixed(1) + "s";
  return m.value.toFixed(2);
}

// The metric's [needs playtesting] target band, or null when no target is seeded
// (the observed value renders with no band).
function targetText(m) {
  if (!m || !m.target) return null;
  const v = (x) => (m.unit === "ratio" ? pct(x) : m.unit === "seconds" ? x.toFixed(0) + "s" : x);
  const band = m.target.kind === "point"
    ? `target ~${v(m.target.value)}`
    : `target ${v(m.target.lo)}–${v(m.target.hi)}`;
  return `${band} [${m.target_status}]`;
}

// ---- groups --------------------------------------------------------------

function startGroups() {
  if (groupsTimer) clearInterval(groupsTimer);
  const tick = async () => {
    try {
      const groups = await getJson("/admin/groups");
      renderGroups(groups);
      renderFleet(await getJson("/admin/fleet"));
    } catch (_) { /* transient */ }
  };
  tick();
  groupsTimer = setInterval(tick, 2000);
}

function renderGroups(groups) {
  $("#groups-count").textContent = `(${groups.length})`;
  const tb = $("#groups-table tbody");
  tb.innerHTML = "";
  for (const r of groups) {
    const tr = el("tr", "selectable" + (r.stuck ? " stuck" : ""));
    const round = r.round_number ? `${r.round_number}/${r.round_total}` : "—";
    const cells = [
      r.group_code || r.group_id,
      r.phase + (r.stuck ? " ⚠" : ""),
      round,
      r.wave_number ?? "—",
      r.players ?? "—",
      (r.age_ms / 1000).toFixed(0) + "s",
    ];
    for (const c of cells) tr.appendChild(el("td", null, String(c)));
    const td = el("td");
    const btn = el("button", null, "Inspect");
    btn.onclick = () => inspectGroup(r.group_code || String(r.group_id));
    td.appendChild(btn);
    tr.appendChild(td);
    tb.appendChild(tr);
  }
}

async function inspectGroup(code) {
  const box = $("#group-detail");
  box.innerHTML = "";
  box.appendChild(el("h3", null, "Group " + code));
  const revealBtn = el("button", "warn", "🔓 Reveal hidden state");
  revealBtn.onclick = () => reveal(code, box);
  box.appendChild(revealBtn);
}

async function reveal(code, box) {
  try {
    const res = await api(`/admin/groups/${encodeURIComponent(code)}/reveal`);
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
      add("Pot volatility", data.volatility_total, true);
      add("Modifiers", data.modifiers);
      add("Active effects", data.active_effects, true);
      out.appendChild(dl);
      if (data.hands.length) {
        out.appendChild(el("h4", null, "Hands"));
        for (const h of data.hands) {
          out.appendChild(el("div", "secret", `${h.player_id}: [${h.pantry}] grimoire[${h.spells}]`));
        }
      }
      if (data.committed.length) {
        out.appendChild(el("h4", null, "Committed (unrevealed)"));
        for (const c of data.committed) {
          out.appendChild(el("div", "secret", `${c.player_id}: ${c.card} vote=${c.vote_color}`));
        }
      }
    } else {
      out.appendChild(el("p", "muted", "No such live group."));
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
  const group = ev.group_code ? `[${ev.group_code}] ` : "";
  const detail = describe(ev);
  li.appendChild(el("span", "tag", ev.span));
  li.appendChild(document.createTextNode(`${ev.kind} ${group}${detail}`));
  feed.prepend(li);
  while (feed.childElementCount > 200) feed.lastChild.remove();
}

function describe(ev) {
  const a = ev.attributes || {};
  if (ev.span === "score" && ev.kind === "end") return `boomed=${a["round.boomed"]} pot=${a["pot.value"] ?? ""}${a.detonators ? ` detonators=${a.detonators}` : ""}`;
  if (ev.span === "round" && ev.kind === "end") return `r${a["round.number"] ?? "?"} boomed=${a["round.boomed"]}`;
  if (ev.span === "depile") return `bp=${a.boiling_point ?? "?"}${a.crossing_index != null ? ` crossed@${a.crossing_index}` : ""}`;
  if (ev.span === "spell.cast") return `${(a["player.id"] ?? "").slice(0, 4)} cast ${a["spell.kind"] ?? ""}${a["spell.target"] ? "→" + a["spell.target"] : ""}`;
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
    const li = el("li", null, `${g.group_code || g.game_id} · ${(g.duration_ms / 1000).toFixed(1)}s · ${g.span_count} spans`);
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
  box.appendChild(el("h3", null, "Game " + (game.group_code || game.game_id)));
  // Group spans by round, then waves (commits + spell casts), then the depile —
  // the preserved v2 tree, replayed wave by wave in completion order.
  const rounds = game.spans.filter((s) => s.name === "round");
  for (const r of rounds) {
    const wrap = el("div", "card");
    wrap.appendChild(el("h4", null, `Round ${r.attributes["round.number"] ?? "?"} — boomed=${r.attributes["round.boomed"]} bp=${r.attributes.boiling_point ?? "?"}`));
    const waves = game.spans.filter((s) => s.name === "wave" && s.parent_id === r.id);
    for (const w of waves) {
      const commits = game.spans.filter((s) => s.name === "commit" && s.parent_id === w.id);
      const casts = game.spans.filter((s) => s.name === "spell.cast" && s.parent_id === w.id);
      const parts = commits
        .map((c) => `${c.attributes["player.id"]?.slice(0, 4)}→${c.attributes.committed_card ?? "?"} (${c.attributes["vote.color"] ?? "?"})`)
        .concat(casts.map((c) => `${c.attributes["player.id"]?.slice(0, 4)} cast ${c.attributes["spell.kind"] ?? "?"}${c.attributes["spell.target"] ? "→" + c.attributes["spell.target"] : ""}`));
      const line = `wave ${w.attributes["wave.number"] ?? "?"}: ` + parts.join(", ");
      wrap.appendChild(el("div", "secret", line));
    }
    const depile = game.spans.find((s) => s.name === "depile" && s.parent_id === r.id);
    if (depile) {
      wrap.appendChild(el("div", null, `depile bp=${depile.attributes.boiling_point ?? "?"}${depile.attributes.crossing_index != null ? ` crossed@${depile.attributes.crossing_index}` : ""}: ${depile.attributes.reveals ?? ""}`));
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
      res = await api("/admin/commands/groups/seed", { method: "POST" });
    } else if (cmd === "force-start" || cmd === "kill") {
      const code = $("#group-target").value.trim();
      if (!code) return alert("enter a group code");
      res = await api(`/admin/commands/groups/${encodeURIComponent(code)}/${cmd}`, { method: "POST" });
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

// ---- popularity (historical, from post-game persistence) -----------------

// A no-dependency bar chart: one flex column per day, height scaled to the
// window's max. `segments(d)` returns [{value, cls}] stacked bottom-up.
function barChart(title, daily, max, segments) {
  const wrap = el("div", "card chart-card");
  wrap.appendChild(el("h3", null, title));
  const chart = el("div", "chart");
  for (const d of daily) {
    const col = el("div", "chart-col");
    const stack = el("div", "chart-stack");
    for (const s of segments(d)) {
      const bar = el("div", "chart-bar " + s.cls);
      bar.style.height = max > 0 ? (s.value / max) * 100 + "%" : "0";
      stack.appendChild(bar);
    }
    col.title = `${d.day} — ${d.games} games, ${d.players} players (${d.new_players} new)`;
    col.appendChild(stack);
    // Light date ticks: the 1st of a month and the newest day.
    const tick = d === daily[daily.length - 1] || d.day.endsWith("-01");
    col.appendChild(el("div", "chart-tick", tick ? d.day.slice(5) : ""));
    chart.appendChild(col);
  }
  wrap.appendChild(chart);
  return wrap;
}

async function loadPopularity() {
  const days = $("#popularity-days").value;
  const data = await getJson(`/admin/stats/popularity?days=${days}`);
  const cards = $("#popularity-cards");
  const charts = $("#popularity-charts");
  cards.innerHTML = "";
  charts.innerHTML = "";
  if (!data.available) {
    cards.appendChild(metricCard("Popularity", "—", data.reason || "unavailable"));
    charts.appendChild(el("p", "muted", "Historical stats need post-game persistence (start the server with a database URL)."));
    return;
  }
  const s = data.stats;
  cards.appendChild(metricCard(`Games (${s.window_days}d)`, s.window_games));
  cards.appendChild(metricCard(`Players (${s.window_days}d)`, s.window_players));
  cards.appendChild(metricCard(`New players (${s.window_days}d)`, s.window_new_players));
  cards.appendChild(metricCard("Games (all time)", s.total_games));
  cards.appendChild(metricCard("Players (all time)", s.total_players));

  const maxGames = Math.max(...s.daily.map((d) => d.games));
  charts.appendChild(barChart("Games per day", s.daily, maxGames, (d) => [
    { value: d.games, cls: "games" },
  ]));
  const maxPlayers = Math.max(...s.daily.map((d) => d.players));
  // Stacked: new players (bright) under returning players (dim) = daily total.
  charts.appendChild(barChart("Unique players per day (bright = first-ever game)", s.daily, maxPlayers, (d) => [
    { value: d.players - d.new_players, cls: "returning" },
    { value: d.new_players, cls: "new" },
  ]));
}

// ---- balance / grafana --------------------------------------------------

async function loadBalance() {
  const b = await getJson("/admin/balance");
  const box = $("#balance-cards");
  box.innerHTML = "";
  // Every card is one boom-balance-metrics definition: observed value plus its
  // [needs playtesting] target band when one is seeded.
  for (const m of b.metrics) {
    box.appendChild(metricCard(m.label, fmtMetric(m), targetText(m)));
  }
  if (b.per_spell_cast_rates.length) {
    const c = el("div", "card");
    c.appendChild(el("h3", null, "Casts / round by spell"));
    for (const [kind, rate] of b.per_spell_cast_rates) {
      c.appendChild(el("div", null, `${kind}: ${rate.toFixed(2)}`));
    }
    box.appendChild(c);
  }
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
    if (id === "popularity") loadPopularity().catch((e) => alert(e.message));
  };
});

$("#popularity-days").onchange = () => loadPopularity().catch((e) => alert(e.message));

$("#connect").onclick = connect;
$("#token").value = token;
document.querySelectorAll("[data-cmd]").forEach((b) => (b.onclick = () => command(b.dataset.cmd)));
if (token) connect();
