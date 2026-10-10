/**
 * Spec 048 T043: the 5e sheet reader alone in a page, the way the import
 * page loads it (`sheetReader` from the pack's index, then wasm-bindgen's
 * initialiser), with no server behind it. The Playwright suite in `../e2e`
 * hands it each fixture and compares the answer with the native one.
 */
import { sheetReader } from "../src/index";

interface ReaderModule {
  default: (init?: unknown) => Promise<unknown>;
  readSheet: (bytes: Uint8Array) => string;
}

declare global {
  interface Window {
    readSheetAnswer: (bytes: number[]) => unknown;
  }
}

const status = document.getElementById("status")!;

sheetReader()
  .then(async (loaded) => {
    const reader = loaded as ReaderModule;
    await reader.default();
    window.readSheetAnswer = (bytes) =>
      JSON.parse(reader.readSheet(new Uint8Array(bytes)));
    status.textContent = "ready";
  })
  .catch((err: unknown) => {
    status.textContent = `failed: ${String(err)}`;
  });
