// ts-rs codegen seam (task 1.2; design D6).
//
// The source of truth for the wire protocol is the Rust `protocol/` crate. Once it derives
// `ts-rs::TS` with `#[ts(export)]`, `cargo test -p protocol` writes TypeScript bindings,
// which we copy over `src/protocol/messages.ts` (replacing the PROVISIONAL hand-authored
// types). This script wires that step into `npm run gen:protocol`.
//
// `server-release-1` (which owns `protocol/`) is being built in parallel and is not yet
// committed, so today this script detects the crate's absence and exits with guidance
// rather than failing. Run it again after the crate lands.

import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const harnessRoot = resolve(here, "..");
const workspaceRoot = resolve(harnessRoot, ".."); // agent-harness lives beside the cargo workspace
const protocolCrate = join(workspaceRoot, "protocol");
const cargoToml = join(protocolCrate, "Cargo.toml");

if (!existsSync(cargoToml)) {
  console.error(`[gen:protocol] Rust protocol crate not found at ${protocolCrate}.`);
  console.error("[gen:protocol] server-release-1 is not committed yet — using the PROVISIONAL");
  console.error("[gen:protocol] src/protocol/messages.ts. Re-run this once `protocol/` exists.");
  process.exit(0);
}

console.error(`[gen:protocol] exporting ts-rs bindings from ${protocolCrate} ...`);
const result = spawnSync("cargo", ["test", "-p", "protocol", "--features", "ts-rs", "export_bindings"], {
  cwd: workspaceRoot,
  stdio: "inherit",
});

if (result.status !== 0) {
  console.error("[gen:protocol] cargo ts-rs export failed — see output above.");
  process.exit(result.status ?? 1);
}

// ts-rs writes bindings under the crate (default `bindings/`); the integration step copies
// the combined message module over src/protocol/messages.ts. Left as the final wiring once
// the crate's exact binding layout is known.
console.error("[gen:protocol] bindings exported. Copy them over src/protocol/messages.ts (see README).");
