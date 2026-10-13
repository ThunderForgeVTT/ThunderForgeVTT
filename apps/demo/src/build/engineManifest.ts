import { createHash } from "node:crypto";
import type { Plugin } from "vite";

/**
 * Spec 090 FR-012: where the demo's engine is, for a page that is not the
 * demo. The landing throws its dice on this engine and loads it from these
 * hashed files, so a visitor who goes on to the demo downloads nothing
 * twice.
 *
 * Paths are relative to `/demo/`. `bytes.br` and `bytes.gz` are filled in
 * after the build by `apps/landing/scripts/compress.mjs`, which is what
 * compresses them.
 */
export interface EngineManifest {
  js: string;
  wasm: string;
  bytes: { raw: number; br?: number; gz?: number };
  version: string;
}

/** What `generateBundle` hands a plugin, narrowed to what is read here. */
type Output =
  | { type: "chunk"; fileName: string; isDynamicEntry: boolean }
  | { type: "asset"; fileName: string; source: string | Uint8Array };

const ENGINE_JS = /^assets\/engine-[\w-]+\.js$/;
const ENGINE_WASM = /^assets\/engine_bg-[\w-]+\.wasm$/;

function only<T extends Output>(found: T[], what: string): T {
  if (found.length !== 1) {
    throw new Error(
      `engine.json: expected one ${what} in the demo build, found ${found.length}`,
    );
  }
  return found[0];
}

export function engineManifest(
  bundle: Record<string, Output>,
): EngineManifest {
  const files = Object.values(bundle);
  const js = only(
    files.filter((f) => f.type === "chunk" && ENGINE_JS.test(f.fileName)),
    "engine glue chunk",
  );
  const wasm = only(
    files.filter(
      (f): f is Extract<Output, { type: "asset" }> =>
        f.type === "asset" && ENGINE_WASM.test(f.fileName),
    ),
    "engine wasm",
  );
  const bytes =
    typeof wasm.source === "string"
      ? Buffer.from(wasm.source)
      : Buffer.from(wasm.source);
  return {
    js: js.fileName,
    wasm: wasm.fileName,
    bytes: { raw: bytes.byteLength },
    version: createHash("sha256").update(bytes).digest("hex").slice(0, 16),
  };
}

/** Writes `engine.json` beside the demo's `index.html`. */
export function engineManifestPlugin(): Plugin {
  return {
    name: "thunderforge-demo-engine-manifest",
    apply: "build",
    generateBundle(_options, bundle) {
      this.emitFile({
        type: "asset",
        fileName: "engine.json",
        source: `${JSON.stringify(engineManifest(bundle as Record<string, Output>), null, 2)}\n`,
      });
    },
  };
}
