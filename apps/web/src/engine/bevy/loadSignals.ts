/**
 * What the engine's loader says about itself, for whoever listens (spec 086
 * FR-018, T060).
 *
 * `mountEngine` emits here and imports nothing else for it: the engine code
 * calls no telemetry API, and this module has no dependencies, so the
 * telemetry chunk can subscribe without pulling the engine's bridge in.
 *
 * Signals are kept, a few at most, and replayed to a late subscriber. The
 * telemetry chunk arrives after the page has loaded; a browser with no WebGL2
 * fails before that, and would otherwise never be heard from.
 */
import type { EngineLoadStage } from "./index";

/** The closed set of reasons the board cannot load before it downloads. */
export type EngineUnavailableReason = "no_webgl2";

export type EngineLoadSignal =
  | { stage: EngineLoadStage }
  | { stage: "unavailable"; reason: EngineUnavailableReason };

export type EngineLoadSubscribe = (
  listener: (signal: EngineLoadSignal) => void,
) => () => void;

export function createLoadSignals(keep = 8) {
  const history: EngineLoadSignal[] = [];
  const listeners = new Set<(signal: EngineLoadSignal) => void>();
  const subscribe: EngineLoadSubscribe = (listener) => {
    for (const signal of history) listener(signal);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };
  const emit = (signal: EngineLoadSignal) => {
    history.push(signal);
    if (history.length > keep) history.shift();
    for (const listener of listeners) {
      try {
        listener(signal);
      } catch {
        // A listener that throws never breaks the load it is watching.
      }
    }
  };
  return { emit, subscribe };
}

const signals = createLoadSignals();

export const emitEngineLoad = signals.emit;
export const onEngineLoad = signals.subscribe;
