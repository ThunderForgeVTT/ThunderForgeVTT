/**
 * The pictures a visitor gives the demo: the server's object store, in the
 * browser.
 *
 * A server transcodes an upload to WebP, keeps the bytes in RustFS and a row
 * in `canvas_image_assets` (`storage/transcode.rs`, `graphql/mutations_assets.rs`).
 * The demo keeps the row with the rest of the world (`state.assets`) and the
 * bytes here, in IndexedDB: a map is megabytes, and the world's own storage
 * is a few. A browser without IndexedDB (or a test) keeps them in memory for
 * the life of the page.
 */

/** `MAX_UPLOAD_BYTES`, `storage/transcode.rs`. */
export const MAX_UPLOAD_BYTES = 50 * 1024 * 1024;
/** `MAX_CANVAS_TEXTURE_DIMENSION`, `storage/transcode.rs`. */
export const MAX_CANVAS_TEXTURE_DIMENSION = 4096;

export interface Transcoded {
  bytes: Blob;
  width: number;
  height: number;
}

/** What a picture is resized to: its own size, or a smaller one. */
export type Fit = (width: number, height: number) => [number, number];

/** `resize_to_max_dimension`: the longest edge at most the cap, in proportion. */
export const fitToCap: Fit = (width, height) => {
  const cap = MAX_CANVAS_TEXTURE_DIMENSION;
  if (width <= cap && height <= cap) return [width, height];
  const scale = Math.min(cap / width, cap / height);
  return [
    Math.max(1, Math.round(width * scale)),
    Math.max(1, Math.round(height * scale)),
  ];
};

/** Decodes any picture the browser can read and encodes it as WebP. */
async function browserTranscode(bytes: Blob, fit: Fit): Promise<Transcoded> {
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(bytes);
  } catch (error) {
    throw new Error(`failed to decode/transcode image: ${String(error)}`);
  }
  const [width, height] = fit(bitmap.width, bitmap.height);
  const canvas = new OffscreenCanvas(width, height);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("failed to decode/transcode image: no canvas");
  context.drawImage(bitmap, 0, 0, width, height);
  bitmap.close();
  const webp = await canvas.convertToBlob({ type: "image/webp", quality: 0.9 });
  return { bytes: webp, width, height };
}

/** Replaceable, so a test can stand in for the browser's image codecs. */
export const imaging = { transcode: browserTranscode };

const DATABASE = "thunderforge-demo-uploads";
const STORE = "bytes";

const memory = new Map<string, Blob>();

function open(): Promise<IDBDatabase> | null {
  if (typeof indexedDB === "undefined") return null;
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function inStore<T>(
  mode: IDBTransactionMode,
  act: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T | undefined> {
  const opening = open();
  if (!opening) return undefined;
  const db = await opening;
  try {
    return await new Promise<T>((resolve, reject) => {
      const request = act(db.transaction(STORE, mode).objectStore(STORE));
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
  } finally {
    db.close();
  }
}

/** Keeps an asset's bytes. Memory always; IndexedDB when there is one. */
export async function keepBytes(assetId: string, bytes: Blob): Promise<void> {
  memory.set(assetId, bytes);
  try {
    await inStore("readwrite", (store) => store.put(bytes, assetId));
  } catch {
    // A full or forbidden store costs the picture on reload, as a full
    // localStorage costs the world; it is still here for this page.
  }
}

/** An asset's bytes, or `null` if this browser does not have them. */
export async function bytesOf(assetId: string): Promise<Blob | null> {
  const held = memory.get(assetId);
  if (held) return held;
  try {
    const stored = await inStore<Blob | undefined>("readonly", (store) =>
      store.get(assetId),
    );
    if (stored) memory.set(assetId, stored);
    return stored ?? null;
  } catch {
    return null;
  }
}

/** FR-007: the pictures go with the world they were part of. */
export function forgetUploads(): void {
  memory.clear();
  void inStore("readwrite", (store) => store.clear()).catch(() => undefined);
}
