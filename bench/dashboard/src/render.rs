//! Rendering the one self-contained page (D4, task 4.2): criterion trends with
//! confidence/noise bands and sustained-level-shift highlighting, plus the
//! balance-study reports with provenance and matrix-cell outliers flagged.
//!
//! The output is a single `benches.html` with the data inlined as JSON and all
//! styles/scripts inline (hand-rolled SVG — no chart library, no CDN). It opens
//! from disk with **zero external requests** (the offline-render spec scenario).

use std::fs;
use std::path::Path;

use crate::history::HistoryRecord;

/// Render the whole dashboard to one self-contained HTML string.
///
/// `studies` are balance-study reports read as generic JSON (the versioned report
/// shape is the contract); `history` is the full criterion bench history.
pub fn render(history: &[HistoryRecord], studies: &[serde_json::Value]) -> String {
    let history_json = embed(&serde_json::to_string(history).unwrap_or_else(|_| "[]".into()));
    let studies_json = embed(&serde_json::to_string(studies).unwrap_or_else(|_| "[]".into()));

    let mut out = String::with_capacity(HEAD.len() + SCRIPT.len() + 4096);
    out.push_str(HEAD);
    out.push_str("\n<script>\nconst HISTORY = ");
    out.push_str(&history_json);
    out.push_str(";\nconst STUDIES = ");
    out.push_str(&studies_json);
    out.push_str(";\n");
    out.push_str(SCRIPT);
    out.push_str("\n</script>\n</body>\n</html>\n");
    out
}

/// Load every `*.json` balance-study report in a directory as generic JSON,
/// sorted by name. Missing dir ⇒ empty (a criterion-only dashboard still renders).
pub fn load_studies_dir(dir: &Path) -> Result<Vec<serde_json::Value>, String> {
    let mut studies = Vec::new();
    if !dir.is_dir() {
        return Ok(studies);
    }
    let mut paths: Vec<_> = fs::read_dir(dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    paths.sort();
    for path in paths {
        let json = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("{}: {e}", path.display()))?;
        studies.push(value);
    }
    Ok(studies)
}

/// Make a JSON string safe to inline inside a `<script>` element: a literal
/// `</script>` (or any `</`) inside a string would otherwise close the element.
fn embed(json: &str) -> String {
    json.replace("</", "<\\/")
}

/// The document head + opening body: title, the inline stylesheet, and the static
/// section scaffolding the script fills in.
const HEAD: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Boiling Point — Benchmarks</title>
<style>
:root {
  --ink: #14110d; --paper: #f5efe2; --line: #d9cdb4;
  --muted: #6b6457; --accent: #8a5a2b; --good: #2f7d4f; --warn: #b4451f; --band: #d8c79a;
}
* { box-sizing: border-box; }
body { margin: 0; font: 15px/1.5 ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  color: var(--ink); background: var(--paper); padding: 2rem; }
h1 { font-size: 1.4rem; margin: 0 0 .25rem; letter-spacing: .02em; }
h2 { font-size: 1.05rem; margin: 2rem 0 .75rem; border-bottom: 2px solid var(--ink); padding-bottom: .25rem; }
h3 { font-size: .95rem; margin: .25rem 0; }
.sub { color: var(--muted); margin: 0 0 1rem; }
.card { background: #fffdf7; border: 1px solid var(--line); border-radius: 6px; padding: 1rem 1.25rem; margin: 0 0 1rem; }
.row { display: flex; flex-wrap: wrap; gap: .5rem 1.5rem; align-items: baseline; }
.k { color: var(--muted); }
.v { font-weight: 600; }
table { border-collapse: collapse; width: 100%; margin: .5rem 0; font-size: .9rem; }
th, td { text-align: left; padding: .25rem .6rem; border-bottom: 1px solid var(--line); }
th { color: var(--muted); font-weight: 600; }
td.num { text-align: right; font-variant-numeric: tabular-nums; }
.pill { display: inline-block; padding: .05rem .5rem; border-radius: 10px; font-size: .8rem; }
.pill.good { background: #e3f1e7; color: var(--good); }
.pill.warn { background: #f6e2da; color: var(--warn); }
.flag { color: var(--warn); }
.outlier { background: #f6e2da; }
.shift { color: var(--warn); font-weight: 600; }
.empty { color: var(--muted); font-style: italic; }
svg { display: block; width: 100%; height: auto; }
.legend { color: var(--muted); font-size: .8rem; margin-top: .25rem; }
footer { color: var(--muted); font-size: .8rem; margin-top: 2rem; border-top: 1px solid var(--line); padding-top: .75rem; }
</style>
</head>
<body>
<h1>Boiling Point — Benchmarks</h1>
<p class="sub">One self-contained page · benchmarks <em>measure</em>, tests gate · read trends, not single runs (rerun noise ~6–12%).</p>
<div id="app"></div>
<footer id="foot"></footer>"#;

/// The rendering script: builds the criterion trends (banded SVG + level-shift
/// flag) and the study cards (provenance, §IV metrics, matrix-cell outliers).
const SCRIPT: &str = r#"
const app = document.getElementById('app');
const el = (tag, cls, html) => { const e = document.createElement(tag); if (cls) e.className = cls; if (html != null) e.innerHTML = html; return e; };
const esc = s => String(s).replace(/[&<>]/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));

function fmtNs(v) {
  if (v == null) return '—';
  if (v < 1e3) return v.toFixed(1) + ' ns';
  if (v < 1e6) return (v / 1e3).toFixed(2) + ' µs';
  if (v < 1e9) return (v / 1e6).toFixed(3) + ' ms';
  return (v / 1e9).toFixed(3) + ' s';
}
function fmtDate(unix) { return unix ? new Date(unix * 1000).toISOString().slice(0, 10) : '—'; }
function median(xs) { const s = [...xs].sort((a, b) => a - b); const n = s.length; return n ? (n % 2 ? s[(n-1)/2] : (s[n/2-1] + s[n/2]) / 2) : 0; }

// --- criterion: id -> [{t, commit, point, lower, upper}] in record order ---
function benchSeries(history) {
  const map = new Map();
  history.forEach(rec => (rec.benches || []).forEach(b => {
    if (!map.has(b.id)) map.set(b.id, []);
    map.get(b.id).push({ t: rec.timestamp_unix, commit: rec.commit, point: b.point_ns, lower: b.lower_ns, upper: b.upper_ns });
  }));
  return new Map([...map.entries()].sort((a, b) => a[0].localeCompare(b[0])));
}

// A sustained level shift: the last 3 points' median sits beyond the prior
// median by more than the typical band half-width — a probable regression to
// investigate (never a single-run delta).
function levelShift(pts) {
  if (pts.length < 4) return null;
  const tail = pts.slice(-3), prior = pts.slice(0, -3);
  const tailMed = median(tail.map(p => p.point)), priorMed = median(prior.map(p => p.point));
  const halfBand = median(tail.map(p => (p.upper - p.lower) / 2));
  const delta = tailMed - priorMed;
  if (Math.abs(delta) > halfBand && halfBand > 0) {
    const dir = delta > 0 ? 'slower' : 'faster';
    const pct = priorMed ? (delta / priorMed * 100).toFixed(1) : '∞';
    return `possible sustained shift: recent runs ${dir} (${pct}% vs prior), beyond the bands`;
  }
  return null;
}

function trendSvg(pts) {
  const W = 760, H = 200, m = { l: 64, r: 12, t: 12, b: 24 };
  const iw = W - m.l - m.r, ih = H - m.t - m.b;
  const lo = Math.min(...pts.map(p => p.lower)), hi = Math.max(...pts.map(p => p.upper));
  const pad = (hi - lo) * 0.1 || 1;
  const y0 = lo - pad, y1 = hi + pad;
  const x = i => m.l + (pts.length === 1 ? iw / 2 : iw * i / (pts.length - 1));
  const y = v => m.t + ih * (1 - (v - y0) / (y1 - y0));
  const up = pts.map((p, i) => `${x(i).toFixed(1)},${y(p.upper).toFixed(1)}`);
  const dn = pts.map((p, i) => `${x(i).toFixed(1)},${y(p.lower).toFixed(1)}`).reverse();
  const band = `<polygon points="${up.concat(dn).join(' ')}" fill="var(--band)" opacity="0.55"/>`;
  const line = `<polyline points="${pts.map((p, i) => `${x(i).toFixed(1)},${y(p.point).toFixed(1)}`).join(' ')}" fill="none" stroke="var(--accent)" stroke-width="2"/>`;
  const dots = pts.map((p, i) => `<circle cx="${x(i).toFixed(1)}" cy="${y(p.point).toFixed(1)}" r="3" fill="var(--accent)"><title>${esc(p.commit)} · ${fmtNs(p.point)} [${fmtNs(p.lower)}, ${fmtNs(p.upper)}]</title></circle>`).join('');
  const ax = `<line x1="${m.l}" y1="${m.t}" x2="${m.l}" y2="${m.t + ih}" stroke="var(--line)"/><line x1="${m.l}" y1="${m.t + ih}" x2="${W - m.r}" y2="${m.t + ih}" stroke="var(--line)"/>`;
  const yl = `<text x="4" y="${(m.t + 10).toFixed(0)}" fill="var(--muted)" font-size="11">${fmtNs(y1)}</text><text x="4" y="${(m.t + ih).toFixed(0)}" fill="var(--muted)" font-size="11">${fmtNs(y0)}</text>`;
  return `<svg viewBox="0 0 ${W} ${H}" role="img">${ax}${band}${line}${dots}${yl}</svg>`;
}

function renderCriterion() {
  const sec = el('section');
  sec.appendChild(el('h2', null, 'Engine micro-benchmarks (per merge to main)'));
  const series = benchSeries(HISTORY);
  if (series.size === 0) { sec.appendChild(el('p', 'empty', 'No bench records yet — the first merge to main seeds the history.')); return sec; }
  for (const [id, pts] of series) {
    const card = el('div', 'card');
    const last = pts[pts.length - 1];
    card.appendChild(el('div', 'row', `<h3>${esc(id)}</h3><span class="k">latest</span> <span class="v">${fmtNs(last.point)}</span> <span class="k">band</span> <span>[${fmtNs(last.lower)}, ${fmtNs(last.upper)}]</span> <span class="k">runs</span> <span>${pts.length}</span>`));
    card.insertAdjacentHTML('beforeend', trendSvg(pts));
    const shift = levelShift(pts);
    card.appendChild(el('div', 'legend' + (shift ? ' shift' : ''), shift ? '▲ ' + esc(shift) : 'shaded = confidence band · within-band wobble is noise, not a regression'));
    sec.appendChild(card);
  }
  return sec;
}

// --- balance studies ---
function metricRow(m) {
  let target = '—', cls = '';
  if (m.target) {
    if (m.target.kind === 'point') { target = m.target.value.toFixed(3); if (m.value != null) cls = ''; }
    else if (m.target.kind === 'band') { target = `[${m.target.lo}, ${m.target.hi}]`; if (m.value != null && (m.value < m.target.lo || m.value > m.target.hi)) cls = 'flag'; }
    if (m.target_status) target += ` <span class="k">(${esc(m.target_status)})</span>`;
  }
  const v = m.value == null ? '<span class="k">no data</span>' : m.value.toFixed(3);
  return `<tr class="${cls}"><td>${esc(m.label || m.id)}</td><td class="num">${v}</td><td>${target}</td></tr>`;
}

function matrixTables(harness) {
  let html = '';
  (harness.cells || []).forEach(cell => {
    const s = cell.stats || {};
    [['brewer_matrix', 'Brewer'], ['archetype_matrix', 'Deck archetype']].forEach(([key, axis]) => {
      const matrix = s[key];
      if (!matrix || Object.keys(matrix).length === 0) return;
      // Aggregate per axis key across personas (the outlier signal).
      const totals = {};
      Object.values(matrix).forEach(byKey => Object.entries(byKey).forEach(([k, c]) => {
        const t = totals[k] || (totals[k] = { games: 0, wins: 0 });
        t.games += c.games; t.wins += c.wins;
      }));
      const rows = Object.entries(totals).sort((a, b) => a[0].localeCompare(b[0])).map(([k, t]) => {
        const wr = t.games ? t.wins / t.games : 0;
        const out = t.games >= 200 && (wr < 0.15 || wr > 0.35);
        return `<tr class="${out ? 'outlier' : ''}"><td>${esc(k)}</td><td class="num">${t.games}</td><td class="num">${(wr * 100).toFixed(1)}%</td>${out ? '<td class="flag">outlier</td>' : '<td></td>'}</tr>`;
      }).join('');
      html += `<h3>${esc(cell.name)} · persona × ${axis} (vs 25% seat baseline)</h3><table><tr><th>${axis}</th><th class="num">games</th><th class="num">win rate</th><th></th></tr>${rows}</table>`;
    });
  });
  return html;
}

function renderStudies() {
  const sec = el('section');
  sec.appendChild(el('h2', null, 'Balance studies (on demand, observational)'));
  if (!STUDIES.length) { sec.appendChild(el('p', 'empty', 'No study reports present — run `make bench-study` and regenerate.')); return sec; }
  STUDIES.forEach(st => {
    const card = el('div', 'card');
    const p = st.provenance || {};
    const repro = p.reproducible ? '<span class="pill good">reproducible</span>' : '<span class="pill warn">not reproducible</span>';
    card.appendChild(el('div', null, `<h3>${esc(st.study?.name || 'study')}</h3>` + (st.study?.question ? `<p class="sub">${esc(st.study.question)}</p>` : '')));
    card.appendChild(el('div', 'row', `<span class="k">seed</span> <span>${esc(p.root_seed)}</span> <span class="k">games</span> <span>${esc(p.total_games)}</span> <span class="k">config</span> <span>${esc(p.config_fingerprint)}</span> <span class="k">engine</span> <span>${esc(p.engine_commit)}</span> <span class="k">${fmtDate(p.generated_unix)}</span> ${repro}`));
    const metrics = (st.metrics || []).map(metricRow).join('');
    card.insertAdjacentHTML('beforeend', `<table><tr><th>§IV metric</th><th class="num">value</th><th>target</th></tr>${metrics}</table>`);
    if (st.harness) card.insertAdjacentHTML('beforeend', matrixTables(st.harness));
    const flags = st.flags || [];
    if (flags.length) card.insertAdjacentHTML('beforeend', '<h3>Flags</h3><ul>' + flags.map(f => `<li class="flag">${esc(f.cell)} · ${esc(f.kind)} — ${esc(f.detail)}</li>`).join('') + '</ul>');
    else card.appendChild(el('p', 'empty', 'No flags — every cell within threshold.'));
    sec.appendChild(card);
  });
  return sec;
}

app.appendChild(renderCriterion());
app.appendChild(renderStudies());
document.getElementById('foot').textContent =
  `${HISTORY.length} bench record(s) · ${STUDIES.length} study report(s) · generated offline by bench_dashboard`;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::criterion::BenchEstimate;
    use crate::history::SCHEMA_VERSION;

    fn record() -> HistoryRecord {
        HistoryRecord {
            schema_version: SCHEMA_VERSION,
            commit: "abc123".into(),
            timestamp_unix: 1_700_000_000,
            benches: vec![BenchEstimate {
                id: "engine/wave_resolution".into(),
                point_ns: 120.0,
                lower_ns: 112.0,
                upper_ns: 128.0,
            }],
        }
    }

    /// The page is self-contained: it inlines the data and makes no external
    /// request (the offline-render spec scenario).
    #[test]
    fn renders_a_self_contained_offline_page() {
        let html = render(&[record()], &[]);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("const HISTORY ="));
        assert!(html.contains("engine/wave_resolution"));
        // No external references of any kind.
        assert!(!html.contains("http://"), "no external http refs");
        assert!(!html.contains("https://"), "no external https refs");
        assert!(!html.contains("src=\"http"), "no remote scripts");
    }

    /// A study report (generic JSON, the versioned shape) folds onto the page.
    #[test]
    fn folds_in_a_study_report() {
        let study = serde_json::json!({
            "schema_version": 1,
            "study": { "name": "explosion-band", "question": "holds ~45%?" },
            "provenance": { "root_seed": 7, "total_games": 200, "config_fingerprint": "deadbeef",
                "engine_commit": "abc123", "transport": "in-process", "reproducible": true, "generated_unix": 1700000000 },
            "metrics": [{ "id": "boom_rate", "label": "Boom rate", "unit": "ratio", "value": 0.448,
                "target": { "kind": "point", "value": 0.45 }, "target_status": "needs playtesting" }],
            "flags": [],
            "harness": { "cells": [] }
        });
        let html = render(&[], &[study]);
        assert!(html.contains("explosion-band"));
        assert!(html.contains("boom_rate") || html.contains("Boom rate"));
        assert!(!html.contains("https://"));
    }

    /// A literal `</script>` inside study text cannot break out of the inline data.
    #[test]
    fn escapes_script_close_in_embedded_data() {
        let study =
            serde_json::json!({ "study": { "name": "x</script><b>" }, "metrics": [], "flags": [] });
        let html = render(&[], &[study]);
        assert!(
            !html.contains("</script><b>"),
            "the close tag must be neutralised"
        );
    }
}
