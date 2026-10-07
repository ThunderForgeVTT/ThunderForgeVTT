import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.mts";

/**
 * The demo's unit tests: the page's own aliases, `src/` only. Playwright owns
 * `e2e/`, and its specs fail under Vitest.
 */
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      include: ["src/**/*.test.ts"],
      environment: "node",
    },
  }),
);
