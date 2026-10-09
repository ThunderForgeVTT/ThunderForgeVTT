/**
 * The session, in `sessionStorage` under `thunderforge.telemetry`.
 *
 * It survives a reload and the viewer switch, and dies with the tab. It is
 * never written to `localStorage`, and no cookie is set. When storage throws
 * (a sandboxed frame, a locked-down browser), the session lives in memory.
 */

import { FUNNEL_STEPS, type FunnelStep } from "./allowList.ts";

export const SESSION_KEY = "thunderforge.telemetry";

export interface Session {
  id: string;
  start: number;
  steps: FunnelStep[];
  sampled: boolean;
}

export type SessionStorage = Pick<Storage, "getItem" | "setItem">;

const HEX32 = /^[0-9a-f]{32}$/;

export function randomHex(bytes: number, random: () => number): string {
  let out = "";
  const cryptoObj = (globalThis as { crypto?: Crypto }).crypto;
  if (cryptoObj?.getRandomValues) {
    const buf = new Uint8Array(bytes);
    cryptoObj.getRandomValues(buf);
    for (const b of buf) out += b.toString(16).padStart(2, "0");
    return out;
  }
  for (let i = 0; i < bytes; i++) {
    out += Math.floor(random() * 256)
      .toString(16)
      .padStart(2, "0");
  }
  return out;
}

function read(storage: SessionStorage): Session | null {
  try {
    const raw = storage.getItem(SESSION_KEY);
    if (!raw) return null;
    const s = JSON.parse(raw) as Partial<Session>;
    if (
      typeof s.id !== "string" ||
      !HEX32.test(s.id) ||
      typeof s.start !== "number" ||
      typeof s.sampled !== "boolean" ||
      !Array.isArray(s.steps)
    ) {
      return null;
    }
    return {
      id: s.id,
      start: s.start,
      sampled: s.sampled,
      steps: s.steps.filter((x): x is FunnelStep => typeof x === "string"),
    };
  } catch {
    return null;
  }
}

export function saveSession(storage: SessionStorage, s: Session): void {
  try {
    storage.setItem(SESSION_KEY, JSON.stringify(s));
  } catch {
    // In-memory only, as when there is no storage at all.
  }
}

/**
 * The tab's session, created on first use. `sampled` is decided here, once,
 * and an existing session keeps its decision whatever the rate says now.
 */
export function loadSession(opts: {
  storage: SessionStorage;
  now: () => number;
  random: () => number;
  sampleRate: number;
  decideSampling: boolean;
}): Session {
  const existing = read(opts.storage);
  if (existing) return existing;
  const s: Session = {
    id: randomHex(16, opts.random),
    start: opts.now(),
    steps: [],
    sampled: opts.decideSampling && opts.random() < opts.sampleRate,
  };
  saveSession(opts.storage, s);
  return s;
}

/** A storage that keeps nothing beyond the page, for when the real one throws. */
export function memoryStorage(): SessionStorage {
  const m = new Map<string, string>();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => {
      m.set(k, v);
    },
  };
}

/** Funnel steps reached before the chunk arrived (contracts/browser-events.md). */
export const PENDING_KEY = "thunderforge.telemetry.pending";

export function addPendingStep(
  storage: SessionStorage,
  step: FunnelStep,
): void {
  try {
    const raw = storage.getItem(PENDING_KEY);
    const list: unknown = raw ? JSON.parse(raw) : [];
    const steps = Array.isArray(list)
      ? list.filter((x) => typeof x === "string")
      : [];
    if (!steps.includes(step)) steps.push(step);
    storage.setItem(PENDING_KEY, JSON.stringify(steps));
  } catch {
    // No storage: the step is lost, as any event before the chunk is.
  }
}

export function takePendingSteps(storage: SessionStorage): FunnelStep[] {
  try {
    const raw = storage.getItem(PENDING_KEY);
    if (!raw) return [];
    storage.setItem(PENDING_KEY, "[]");
    const list: unknown = JSON.parse(raw);
    return Array.isArray(list)
      ? list.filter((x): x is FunnelStep =>
          (FUNNEL_STEPS as readonly unknown[]).includes(x),
        )
      : [];
  } catch {
    return [];
  }
}
