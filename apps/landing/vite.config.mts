import { createReadStream, existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
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

// Spec 090 T036: the demo's build at `/demo/`, as the image serves it beside
// the landing, so `vite preview` (and the e2e run against it) can load the
// engine the frame boots. Preview only: the production build never sees it,
// and with no demo build the path is simply not found.
const DEMO_DIR = path.resolve(__dirname, "../../data/demo");
const TYPES: Record<string, string> = {
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".css": "text/css",
  ".html": "text/html",
  ".svg": "image/svg+xml",
  ".webp": "image/webp",
  ".png": "image/png",
  ".woff2": "font/woff2",
};
function servedDemo(): Plugin {
  return {
    name: "thunderforge-served-demo",
    configurePreviewServer(server) {
      server.middlewares.use("/demo/", (req, res, next) => {
        const rel = decodeURIComponent((req.url ?? "/").split("?")[0]);
        const file = path.join(DEMO_DIR, rel);
        if (!file.startsWith(DEMO_DIR + path.sep)) return next();
        if (!existsSync(file) || !statSync(file).isFile()) return next();
        res.setHeader("Content-Type", TYPES[path.extname(file)] ?? "application/octet-stream");
        res.setHeader("Content-Length", statSync(file).size);
        res.setHeader("Cache-Control", "no-cache");
        createReadStream(file).pipe(res);
      });
    },
  };
}

export default defineConfig({
  root: __dirname,
  // One page, and nginx answers 404 for any other path. The SPA fallback
  // would hand the engine `index.html` for the `.meta` file Bevy asks for
  // before every image, and the image would never load (spec 091 T031).
  appType: "mpa",
  plugins: [
    react(),
    // Spec 086: `/telemetry.json` and its `connect-src`, as nginx serves them.
    // Off unless THUNDERFORGE_PREVIEW_TELEMETRY holds a config.
    servedTelemetry({ paths: ["/telemetry.json", "/demo/telemetry.json"] }),
    servedDemo(),
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
