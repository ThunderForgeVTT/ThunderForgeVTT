import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const web = path.resolve(__dirname, "../web");

// Spec 074: the demo is the web app's own pages behind a different door.
// Everything below `@` is `apps/web/src`, imported and not copied, so a page
// fixed there is fixed here. What the demo does differently lives in `src/`.
/**
 * Spec 074 SC-003, held by the browser as well as by the guard: the built
 * page may load from where it was served and from nowhere else. The guard in
 * `src/guard/install.ts` covers what scripts ask for; this covers what markup
 * asks for, an `<img>` pointed at another site being the case that found it.
 * Build only: the dev server's module reload needs inline scripts and a
 * socket of its own.
 */
function sealedPage(): Plugin {
  const policy = [
    "default-src 'self' data: blob:",
    "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval' blob:",
    "style-src 'self' 'unsafe-inline'",
    "connect-src 'self' data: blob:",
    "form-action 'none'",
  ].join("; ");
  return {
    name: "thunderforge-demo-sealed-page",
    apply: "build",
    transformIndexHtml: () => [
      {
        tag: "meta",
        attrs: { "http-equiv": "Content-Security-Policy", content: policy },
        injectTo: "head-prepend",
      },
    ],
  };
}

export default defineConfig({
  root: __dirname,
  // FR-003. The server mounts the built directory at `/demo`, and so can any
  // static host.
  base: "/demo/",
  resolve: {
    alias: {
      // What the demo does differently from the web app, module for module.
      // These come before "@", which would otherwise claim them.
      "@/components/seo/SEO": path.resolve(__dirname, "src/overrides/SEO.tsx"),
      "@/hooks/useAvatar": path.resolve(
        __dirname,
        "src/overrides/useAvatar.ts",
      ),
      "@": path.resolve(web, "src"),
      "@thunderforge/host": path.resolve(web, "src/host/index.ts"),
      "@thunderforge/genie": path.resolve(
        __dirname,
        "../../packs/systems/genie/web/src/index.ts",
      ),
      // One React, the web app's — see the same aliases in its own config.
      react: path.resolve(web, "node_modules/react"),
      "react-dom": path.resolve(web, "node_modules/react-dom"),
      "react/jsx-runtime": path.resolve(web, "node_modules/react/jsx-runtime"),
    },
  },
  plugins: [tailwindcss(), react(), sealedPage()],
  build: {
    outDir: path.resolve(__dirname, "../../data/demo"),
    emptyOutDir: true,
    sourcemap: true,
  },
  css: {
    preprocessorOptions: {
      scss: { loadPaths: [path.resolve(web, "src")] },
    },
  },
  server: {
    host: "127.0.0.1",
    port: Number(process.env.THUNDERFORGE_DEMO_PORT ?? 5183),
    strictPort: true,
    // The schema, the web app and the packs all sit above this directory.
    fs: { allow: [path.resolve(__dirname, "../..")] },
  },
  preview: {
    host: "127.0.0.1",
    port: Number(process.env.THUNDERFORGE_DEMO_PORT ?? 5183),
    strictPort: true,
  },
});
