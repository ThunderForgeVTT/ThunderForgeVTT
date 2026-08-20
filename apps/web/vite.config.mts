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
      // Backend-served imported assets (e.g. map-import background images
      // at scenes.background_image_path), mounted at /assets by
      // src/server/src/serve/mod.rs's `ServeDir::new(&directories.asset_directory)`.
      // Bevy's default `AssetPlugin` root is the literal string "assets",
      // resolved against the page origin on wasm32, so this proxy is what
      // makes `AssetServer::load("map-imports/.../uuid.png")` resolve in dev.
      "/assets": {
        target: "http://127.0.0.1:30000",
        changeOrigin: true,
      },
    },
  },
});
