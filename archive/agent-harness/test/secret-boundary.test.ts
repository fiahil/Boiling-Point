import { test } from "node:test";
import assert from "node:assert/strict";

import { createViewModel } from "../src/net/view-model.ts";
import { assertNoSecretLeak, discloseBoilingPoint } from "../src/net/secret-boundary.ts";

test("discloseBoilingPoint accepts only legitimate sources", () => {
  const vm = createViewModel();
  discloseBoilingPoint(vm, 11, "peek");
  assert.equal(vm.self.disclosedBoilingPoint, 11);

  // @ts-expect-error — illegal source is rejected at runtime
  assert.throws(() => discloseBoilingPoint(vm, 12, "guess"), /illegal boiling-point disclosure source/);
  assert.throws(() => discloseBoilingPoint(vm, Number.NaN, "peek"), /illegal boiling-point value/);
});

test("assertNoSecretLeak catches a value without a legitimate source", () => {
  const vm = createViewModel();
  // Simulate a leak: a boiling point present with no disclosure provenance.
  vm.self.disclosedBoilingPoint = 10;
  assert.throws(() => assertNoSecretLeak(vm), /secret-boundary violation/);
});

test("assertNoSecretLeak catches a forged source", () => {
  const vm = createViewModel();
  vm.self.disclosedBoilingPoint = 10;
  // @ts-expect-error — forging an illegal source
  vm.self.boilingPointSource = "telepathy";
  assert.throws(() => assertNoSecretLeak(vm), /secret-boundary violation/);
});

test("a clean model with no disclosure passes", () => {
  const vm = createViewModel();
  assert.doesNotThrow(() => assertNoSecretLeak(vm));
});
