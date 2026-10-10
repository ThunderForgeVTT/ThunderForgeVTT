import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = path.dirname(fileURLToPath(import.meta.url));

// The sheet reader's harness (spec 048 T043): its own page and its own port,
// strict so the suite never probes whatever took it. 5190/5191 are the hero
// builder's and 5193 the demo's.
const previewPort = Number(process.env.SHEET_READER_PREVIEW_PORT ?? 5196);

export default defineConfig({
  root: path.join(here, "sheet-reader"),
  preview: { host: "127.0.0.1", port: previewPort, strictPort: true },
  build: {
    outDir: path.join(here, "sheet-reader-dist"),
    emptyOutDir: true,
  },
});
