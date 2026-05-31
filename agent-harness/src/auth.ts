// Subscription (OAuth) auth for the Agent SDK (design D2). The credential resolution
// order puts ANTHROPIC_API_KEY (and ANTHROPIC_AUTH_TOKEN) AHEAD of subscription auth, so a
// stray exported key silently bills pay-as-you-go API credits while appearing to use the
// subscription. This neutralizes those unless the operator explicitly opts into API-key
// billing. Subscription auth then comes from either CLAUDE_CODE_OAUTH_TOKEN or the Claude
// Code CLI's stored login (`claude setup-token` / interactive). We log which path is active.

export type AuthPath = "subscription" | "api-key";

export interface AuthResult {
  path: AuthPath;
  notes: string[];
}

export function configureAuth(log: (msg: string) => void = console.error): AuthResult {
  const notes: string[] = [];

  if (process.env.BP_ALLOW_API_KEY === "1" && process.env.ANTHROPIC_API_KEY) {
    log("[auth] BP_ALLOW_API_KEY=1 — billing pay-as-you-go API credits (NOT the subscription).");
    return { path: "api-key", notes };
  }

  // Neutralize the higher-precedence credentials so subscription auth wins.
  for (const key of ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"] as const) {
    if (process.env[key]) {
      delete process.env[key];
      notes.push(`Unset ${key} to keep subscription billing (set BP_ALLOW_API_KEY=1 to opt into API billing).`);
    }
  }

  if (process.env.CLAUDE_CODE_OAUTH_TOKEN) {
    log("[auth] Using Claude subscription via CLAUDE_CODE_OAUTH_TOKEN.");
  } else {
    log("[auth] Using Claude subscription via the Claude Code CLI's stored login (run `claude setup-token` if this fails).");
  }
  for (const n of notes) log(`[auth] ${n}`);
  return { path: "subscription", notes };
}
