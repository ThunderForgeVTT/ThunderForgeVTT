import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * Returns 404 for `*.meta` instead of letting the SPA fallback answer with
 * index.html.
 *
 * Bevy's `AssetServer` runs with `AssetMetaCheck::Always`, so every
 * `asset_server.load("maps/x.webp")` first requests `maps/x.webp.meta`. A dev
 * server that answers that with HTML hands Bevy a "meta file" it then fails
 * to parse as RON — and the *asset load fails with it*, which reads as "the
 * image is broken" when the image was never the problem. A 404 is what makes
 * Bevy fall back to the default meta and load the image normally. The real
 * server has the same rule; see `canvas_assets_serve::parse_asset_id`.
 */
function fourOhFourForAssetMeta() {
  return {
    name: "sandbox-404-asset-meta",
    configureServer(server: { middlewares: { use: (fn: (req: { url?: string }, res: { statusCode: number; end: () => void }, next: () => void) => void) => void } }) {
      server.middlewares.use((req, res, next) => {
        if (req.url?.split("?")[0].endsWith(".meta")) {
          res.statusCode = 404;
          res.end();
          return;
        }
        next();
      });
    },
  };
}

export default defineConfig({
  root: __dirname,
  plugins: [fourOhFourForAssetMeta()],
  server: { port: 5180 },
  // The engine is a ~190MB dev wasm; excluding it from pre-bundling keeps
  // dev startup from trying to optimise it on every boot.
  optimizeDeps: { exclude: ["@thunderforge/engine"] },
  resolve: {
    alias: {
      "@thunderforge/engine": path.resolve(__dirname, "../../dist/engine/engine.js"),
    },
  },
});
