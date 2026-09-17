import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Ports of its own, strict for the web app's reason: a builder that silently
// moved to the next port would leave its e2e probing whatever took this one.
// 5173 is the web app and 5180 the engine sandbox.
const devPort = Number(process.env.HERO_BUILDER_PORT ?? 5190);
const previewPort = Number(process.env.HERO_BUILDER_PREVIEW_PORT ?? 5191);

export default defineConfig({
  root: __dirname,
  plugins: [react()],
  // One React. The library takes React as a peer, and a second copy resolved
  // through it would fail as "Invalid hook call".
  resolve: { dedupe: ["react", "react-dom"] },
  server: { host: "127.0.0.1", port: devPort, strictPort: true },
  preview: { host: "127.0.0.1", port: previewPort, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true },
});
