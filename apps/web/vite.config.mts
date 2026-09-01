import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createHtmlPlugin } from "vite-plugin-html";
import { viteStaticCopy } from "vite-plugin-static-copy";
import tailwindcss from "@tailwindcss/vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// One stack per shard, so the ports and the proxy target cannot be constants.
//
// `scripts/e2e-parallel.mjs` runs several complete stacks side by side — each
// its own database, bucket, backend and frontend — so that the e2e suite can
// be split without the shards sharing mutable state. Every value below keeps
// its previous literal as the default, so a plain `pnpm dev` is unchanged and
// this is invisible to anyone not sharding.
const devPort = Number(process.env.THUNDERFORGE_WEB_PORT ?? 5173);
const backendOrigin =
  process.env.THUNDERFORGE_BACKEND_ORIGIN ?? "http://127.0.0.1:30000";

export default defineConfig({
  root: __dirname,
  base: "/",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      // packs/systems/genie/web has never actually been built as a
      // library (no vite lib config, tsconfig has noEmit: true) — aliased
      // straight to its TS source rather than fixing that unrelated,
      // pre-existing build gap just to consume it here.
      "@thunderforge/genie": path.resolve(
        __dirname,
        "../../packs/systems/genie/web/src/index.ts",
      ),
      // genie/web has its own node_modules with React 18.3.1 (this app is
      // on React 19); without forcing it to resolve here, its aliased
      // source pulls in a second React copy → "Invalid hook call".
      react: path.resolve(__dirname, "node_modules/react"),
      "react-dom": path.resolve(__dirname, "node_modules/react-dom"),
      "react/jsx-runtime": path.resolve(
        __dirname,
        "node_modules/react/jsx-runtime",
      ),
    },
  },
  plugins: [
    tailwindcss(),
    react(),
    createHtmlPlugin({
      minify: true,
      inject: {
        data: {
          pageTitle: "ThunderForge VTT",
          pageDescription:
            "ThunderForge VTT blends real-time world building, secure instance setup, and collaborative tabletop play.",
        },
      },
    }),
    viteStaticCopy({
      targets: [
        {
          src: path.resolve(__dirname, "../../assets/**/*"),
          dest: "../assets",
        },
      ],
    }),
  ],
  build: {
    outDir: path.resolve(__dirname, "../../data/client"),
    emptyOutDir: true,
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (
            id.includes("react-helmet-async") ||
            id.includes("react-router-dom") ||
            id.includes("/react/") ||
            id.includes("/react-dom/")
          ) {
            return "react";
          }

          if (
            id.includes("/tldraw/") ||
            id.includes("/rxdb/") ||
            id.includes("/rxjs/")
          ) {
            return "collaboration";
          }

          return undefined;
        },
        entryFileNames: "assets/entry/[name]-[hash].js",
        chunkFileNames: "assets/chunks/[name]-[hash].js",
        assetFileNames: "assets/static/[name]-[hash][extname]",
      },
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        loadPaths: [path.resolve(__dirname, "./src")],
      },
    },
  },
  server: {
    host: "127.0.0.1",
    port: devPort,
    // Fail rather than drift. Vite's default is to take the next free port
    // when 5173 is busy, which is convenient alone and wrong here: the
    // backend proxy, the e2e baseURL and a tunnel's ingress all name 5173,
    // so a silent move to 5174 leaves the tunnel forwarding to nothing and
    // surfaces later as an unrelated-looking error from whichever process
    // notices first. Observed exactly that: a stale dev server held 5173,
    // Vite moved, and the failure that reached the console was the backend
    // panicking on its own port being in use.
    // Still strict, and now more so rather than less: a shard whose frontend
    // silently moved would proxy to another shard's backend and produce
    // cross-shard failures that look like product bugs.
    strictPort: true,
    // Vite's DNS-rebinding protection rejects any request whose Host
    // header it doesn't recognize — including one forwarded through a
    // `cloudflared tunnel --url http://localhost:5173` quick tunnel
    // (scripts/dev.mjs's `--tunnel` flag), where the browser's Host
    // header is the tunnel's own random *.trycloudflare.com subdomain,
    // not localhost. Allowing that suffix (not disabling the check
    // entirely) keeps the protection for every other unrecognized host.
    // `.trycloudflare.com` covers a quick tunnel, whose hostname is random
    // every run. A named tunnel has a stable hostname that this cannot
    // predict, so it is named explicitly — without it Vite answers 403 to a
    // tunnel that is working perfectly, which looks exactly like the tunnel
    // being broken.
    allowedHosts: [
      ".trycloudflare.com",
      ...(process.env.TUNNEL_HOSTNAME ? [process.env.TUNNEL_HOSTNAME] : []),
    ],
    proxy: {
      "/api": {
        target: backendOrigin,
        changeOrigin: true,
        ws: true,
      },
      // Backend-served imported assets (e.g. map-import background images
      // at scenes.background_image_path), mounted at /assets by
      // src/server/src/serve/mod.rs's `ServeDir::new(&directories.asset_directory)`.
      // Bevy's default `AssetPlugin` root is the literal string "assets",
      // resolved against the page origin on wasm32, so this proxy is what
      // makes `AssetServer::load("map-imports/.../uuid.png")` resolve in dev.
      "/assets": {
        target: backendOrigin,
        changeOrigin: true,
      },
    },
  },
});
