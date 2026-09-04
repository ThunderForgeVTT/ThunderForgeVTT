import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import tsconfigPaths from "vite-tsconfig-paths";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

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
 *
 * The aliases below duplicate `vite.config.mts` rather than being imported
 * from it, and that is worth a sentence. `vite-tsconfig-paths` applies this
 * app's tsconfig `paths` only to files that tsconfig `include`s — `src` and
 * `e2e`. Since `systemActorSheets.ts` discovers sheet containers that live
 * under `packs/`, a *pack's* file importing `@thunderforge/host` is outside
 * that reach and resolves to nothing. Stating the aliases here is what makes
 * pack code testable from this app at all.
 */
export default defineConfig({
  plugins: [tsconfigPaths()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@thunderforge/host": path.resolve(__dirname, "./src/host/index.ts"),
      "@thunderforge/genie": path.resolve(
        __dirname,
        "../../packs/systems/genie/web/src/index.ts",
      ),
      // Same reason as `vite.config.mts`: genie/web carries its own React
      // 18 in `node_modules`, and a second React copy is an "Invalid hook
      // call" waiting to happen the moment one of these is rendered.
      react: path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
      "react/jsx-runtime": path.resolve(
        __dirname,
        "node_modules/react/jsx-runtime",
      ),
    },
  },
  test: {
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["e2e/**", "node_modules/**", "dist/**"],
    environment: "node",
  },
});
