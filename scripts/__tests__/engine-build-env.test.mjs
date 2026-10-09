import assert from "node:assert/strict";
import { test } from "node:test";

import { engineBuildEnv, getEngineInputsHash } from "../shared.mjs";

// Spec 087: the dev engine keeps line tables and drops the rest of the debug
// info. Only the wasm dev build is touched: the host's `cargo` profiles and
// the release engine are left as the workspace sets them.

test("a dev engine build keeps only line tables", () => {
  assert.deepEqual(engineBuildEnv("dev"), {
    CARGO_PROFILE_DEV_DEBUG: "line-tables-only",
  });
});

test("a release engine build changes nothing", () => {
  assert.deepEqual(engineBuildEnv("release"), {});
});

test("the debug setting is part of the engine's cache key", () => {
  // A dev wasm built before the setting must not pass as current: the hash
  // has to move when the setting does, or pkg.sum would keep serving it.
  const withSetting = getEngineInputsHash("dev");
  const withoutSetting = getEngineInputsHash("dev", { buildEnv: {} });
  assert.notEqual(withSetting, withoutSetting);
});
