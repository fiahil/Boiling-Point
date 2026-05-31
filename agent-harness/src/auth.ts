// Subscription (OAuth) auth for the Agent SDK (design D2). The credential resolution
// order puts ANTHROPIC_API_KEY (and ANTHROPIC_AUTH_TOKEN) AHEAD of the OAuth token, so a
// stray exported key silently bills pay-as-you-go API credits while appearing to use the
// subscription. This neutralizes those unless the operator explicitly opts into API-key
// billing, and logs which path is active so it is never a silent surprise.

export type AuthPath = "subscription" | "api-key" | "none";

export interface AuthResult {
  path: AuthPath;
  notes: string[];
}

export function configureAuth(log: (msg: string) => void = console.error): AuthResult {
  const notes: string[] = [];
  const allowApiKey = process.env.BP_ALLOW_API_KEY === "1";

  if (allowApiKey) {
    if (process.env.ANTHROPIC_API_KEY) {
      log("[auth] BP_ALLOW_API_KEY=1 — billing pay-as-you-go API credits (NOT the subscription).");
      return { path: "api-key", notes };
    }
    notes.push("BP_ALLOW_API_KEY set but no ANTHROPIC_API_KEY present.");
  }

  // Neutralize the higher-precedence credentials so the OAuth token wins.
  for (const key of ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"] as const) {
    if (process.env[key]) {
      delete process.env[key];
      notes.push(`Unset ${key} to keep subscription billing (set BP_ALLOW_API_KEY=1 to opt into API billing).`);
    }
  }

  if (process.env.CLAUDE_CODE_OAUTH_TOKEN) {
    log("[auth] Using Claude subscription (CLAUDE_CODE_OAUTH_TOKEN).");
    for (const n of notes) log(`[auth] ${n}`);
    return { path: "subscription", notes };
  }

  log("[auth] No CLAUDE_CODE_OAUTH_TOKEN found. Run `claude setup-token`, or set BP_ALLOW_API_KEY=1 to use an API key.");
  return { path: "none", notes };
}
