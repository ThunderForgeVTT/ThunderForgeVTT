/**
 * A real HTTP server for the standalone e2e tests (spec 080 T017, T027):
 * one file, single byte ranges, a strong `ETag` and `If-Range`, the way the
 * ThunderForge server answers. Each test steers it through `cut` and `swap`.
 */
import { createHash } from "node:crypto";
import { createServer, type IncomingMessage, type Server } from "node:http";
import type { AddressInfo } from "node:net";

export interface Served {
  range: string | null;
  ifRange: string | null;
  status: number;
  /** Bytes written to the socket for this answer. */
  written: number;
}

export interface FileServer {
  url: string;
  requests: Served[];
  /** Replace the file; its tag changes with it. */
  swap(bytes: Uint8Array): void;
  /**
   * Called before each answer. Return a byte count to write that many bytes
   * and then destroy the socket.
   */
  cut: ((request: IncomingMessage, start: number) => number | null) | null;
  /** Called after each answer starts, with how many answers came before. */
  onAnswer: ((index: number) => void) | null;
  close(): Promise<void>;
}

export const sha256 = (bytes: Uint8Array) =>
  createHash("sha256").update(bytes).digest("hex");

/** Bytes that are not all alike, so a byte out of place changes the hash. */
export function generated(size: number, seed: number): Uint8Array {
  const out = new Uint8Array(size);
  let x = seed >>> 0 || 1;
  for (let i = 0; i < size; i++) {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    out[i] = x & 0xff;
  }
  return out;
}

const CHUNK = 64 * 1024;

export async function fileServer(initial: Uint8Array): Promise<FileServer> {
  let file = initial;
  let tag = `"${sha256(file).slice(0, 16)}"`;
  const requests: Served[] = [];

  const state: FileServer = {
    url: "",
    requests,
    swap(bytes) {
      file = bytes;
      tag = `"${sha256(file).slice(0, 16)}"`;
    },
    cut: null,
    onAnswer: null,
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections();
        server.close(() => resolve());
      }),
  };

  const server: Server = createServer((request, response) => {
    const bytes = file;
    const etag = tag;
    const rangeHeader = request.headers.range ?? null;
    const ifRange = (request.headers["if-range"] as string | undefined) ?? null;
    let start = 0;
    let end = bytes.length - 1;
    let partial = false;
    const match = rangeHeader?.match(/^bytes=(\d+)-(\d*)$/);
    if (match && (ifRange === null || ifRange === etag)) {
      start = Number(match[1]);
      if (match[2] !== "") end = Math.min(Number(match[2]), bytes.length - 1);
      if (start >= bytes.length) {
        response.writeHead(416, { "Content-Range": `bytes */${bytes.length}` });
        response.end();
        requests.push({ range: rangeHeader, ifRange, status: 416, written: 0 });
        return;
      }
      partial = true;
    }
    const status = partial ? 206 : 200;
    const served: Served = { range: rangeHeader, ifRange, status, written: 0 };
    requests.push(served);
    response.writeHead(status, {
      "Accept-Ranges": "bytes",
      ETag: etag,
      "Content-Length": String(end - start + 1),
      ...(partial
        ? { "Content-Range": `bytes ${start}-${end}/${bytes.length}` }
        : {}),
    });
    state.onAnswer?.(requests.length - 1);

    const limit = state.cut?.(request, start) ?? null;
    let at = start;
    const write = () => {
      while (at <= end) {
        if (limit !== null && served.written >= limit) {
          response.socket?.destroy();
          return;
        }
        const room = limit === null ? CHUNK : Math.min(CHUNK, limit - served.written);
        const slice = bytes.subarray(at, Math.min(at + room, end + 1));
        at += slice.length;
        served.written += slice.length;
        if (!response.write(slice)) {
          response.once("drain", write);
          return;
        }
      }
      response.end();
    };
    write();
  });

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  state.url = `http://127.0.0.1:${port}/file.bin`;
  return state;
}
