import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { servedTelemetry } from "@thunderforge/telemetry/vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Ports of its own: 5173 is the web app, 5180 the engine sandbox, 5190 the
// hero builder.
const devPort = Number(process.env.LANDING_PORT ?? 5200);
const previewPort = Number(process.env.LANDING_PREVIEW_PORT ?? 5201);

// The same three GitHub paths nginx serves in production (nginx.conf.template),
// for `dev` and `preview`. Stargazers need a token:
//   GITHUB_TOKEN=$(gh auth token) pnpm -F @thunderforge/landing dev
const REPO_API = "/repos/ThunderForgeVTT/ThunderForgeVTT";
const GH_PATHS: Record<string, (q: URLSearchParams) => string> = {
  "/gh/repo": () => REPO_API,
  "/gh/contributors": () => `${REPO_API}/contributors?per_page=100`,
  "/gh/stargazers": (q) => `${REPO_API}/stargazers?per_page=100&page=${Number(q.get("page")) || 1}`,
};
const ghProxy = {
  "/gh/": {
    target: "https://api.github.com",
    changeOrigin: true,
    rewrite: (url: string) => {
      const [p, query] = url.split("?");
      return GH_PATHS[p]?.(new URLSearchParams(query)) ?? "/404";
    },
    headers: {
      Accept: "application/vnd.github+json",
      ...(process.env.GITHUB_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {}),
    },
  },
};

export default defineConfig({
  root: __dirname,
  plugins: [
    react(),
    // Spec 086: `/telemetry.json` and its `connect-src`, as nginx serves them.
    // Off unless THUNDERFORGE_PREVIEW_TELEMETRY holds a config.
    servedTelemetry({ paths: ["/telemetry.json", "/demo/telemetry.json"] }),
  ],
  resolve: {
    dedupe: ["react", "react-dom"],
    alias: {
      // Spec 086 R12: the web app's redaction, not a second copy of it.
      "@thunderforge/feedback-redaction": path.resolve(
        __dirname,
        "../web/src/services/feedbackRedaction.ts",
      ),
    },
  },
  server: { host: "127.0.0.1", port: devPort, strictPort: true, proxy: ghProxy },
  preview: { host: "127.0.0.1", port: previewPort, strictPort: true, proxy: ghProxy },
  build: { outDir: "dist", emptyOutDir: true },
});
