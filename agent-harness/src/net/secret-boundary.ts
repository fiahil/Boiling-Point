// The harness is a player, not the server (Constitution I). Its view model is built
// only from received messages and structurally has no field for opponents' hands or the
// draw deck. The one secret a player can ever legitimately learn — the boiling point — is
// disclosed only by the player's own Peek or by an explosion depile. This module is the
// single gate through which that value may enter the model, plus a runtime assertion that
// the boundary never broke. (In the Rust bot-harness this is compile-enforced; in TS we
// uphold it by discipline + this assertion — design D6.)

import type { ViewModel } from "./view-model.ts";

export type DisclosureSource = "peek" | "explosion";

const VALID_SOURCES: ReadonlySet<DisclosureSource> = new Set(["peek", "explosion"]);

/** The ONLY way the boiling point may enter the view model. */
export function discloseBoilingPoint(
  vm: ViewModel,
  value: number,
  source: DisclosureSource,
): void {
  if (!VALID_SOURCES.has(source)) {
    throw new Error(`illegal boiling-point disclosure source: ${String(source)}`);
  }
  if (!Number.isFinite(value)) {
    throw new Error(`illegal boiling-point value: ${String(value)}`);
  }
  vm.self.disclosedBoilingPoint = value;
  vm.self.boilingPointSource = source;
}

/**
 * Fails if the secret boundary was violated. Run after applying each message.
 * The model has no opponents'-hand or draw-deck fields at all, so the only thing to
 * verify is that any disclosed boiling point carries a legitimate provenance.
 */
export function assertNoSecretLeak(vm: ViewModel): void {
  const { disclosedBoilingPoint, boilingPointSource } = vm.self;
  const hasValue = disclosedBoilingPoint !== undefined;
  const hasSource = boilingPointSource !== undefined;
  if (hasValue !== hasSource) {
    throw new Error(
      "secret-boundary violation: boiling point present without a legitimate disclosure source",
    );
  }
  if (hasSource && !VALID_SOURCES.has(boilingPointSource)) {
    throw new Error(
      `secret-boundary violation: illegal disclosure source ${String(boilingPointSource)}`,
    );
  }
}
