import path from "node:path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteStaticCopy } from "vite-plugin-static-copy";

export default defineConfig({
  root: path.resolve(__dirname, "src/client"),
  base: "/",
  plugins: [
    react(),
    viteStaticCopy({
      targets: [
        {
          src: path.resolve(__dirname, "assets/**/*"),
          dest: "../assets",
        },
      ],
    }),
  ],
  build: {
    outDir: path.resolve(__dirname, "data/client"),
    emptyOutDir: true,
    sourcemap: true,
  },
  server: {
    host: "127.0.0.1",
    port: 5173,
  },
});