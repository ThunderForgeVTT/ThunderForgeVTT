import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createHtmlPlugin } from "vite-plugin-html";
import { viteStaticCopy } from "vite-plugin-static-copy";
import tailwindcss from "@tailwindcss/vite";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  root: __dirname,
  base: "/",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
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
    port: 5173,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:30000",
        changeOrigin: true,
        ws: true,
      },
      // Native canvas authoring: imported map background images are
      // served by the backend's static /assets route (src/server/src/serve/mod.rs).
      // Bevy's default AssetServer resolves relative "assets/..." loads
      // against the page's own origin, so without this proxy rule those
      // fetches 404 in dev (the app itself is served from vite, not the backend).
      "/assets": {
        target: "http://127.0.0.1:30000",
        changeOrigin: true,
      },
    },
  },
});
