import { defineConfig } from "vitest/config";
import tsconfigPaths from "vite-tsconfig-paths";

/**
 * Vitest config for `apps/web` unit tests.
 *
 * Deliberately separate from `vite.config.mts` and narrowly scoped to
 * `src/**`: Vitest's default `include` would otherwise sweep up
 * `apps/web/e2e/*.spec.ts`, which are Playwright specs and fail immediately
 * ("You are calling test.describe() from an async test.describe() block").
 * Playwright owns `e2e/`, Vitest owns `src/`.
 *
 * Added by spec 025 (T019). Before this, `vitest` was not installed and no
 * `test` script existed, so `src/utils/__tests__/sizeCategory.test.ts` — which
 * has imported `vitest` since spec 018 — had never actually run. That file
 * passes now.
 */
export default defineConfig({
  plugins: [tsconfigPaths()],
  test: {
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["e2e/**", "node_modules/**", "dist/**"],
    environment: "node",
  },
});
