import type { ResolvedSetting } from "@/api/instanceSettings";

/**
 * Spec 088 US7 (FR-050 to FR-053, data-model.md): the mail page's one form,
 * as a pure model. It holds what was loaded and a change per key, and says
 * which keys are dirty, whether Save may run, what each save sends, and what
 * is left after a save or a reload.
 *
 * # When a key is dirty
 *
 * A plain value is dirty when its draft, trimmed, differs from what was
 * loaded, because the server trims before it compares and stores. A secret
 * is never shown, so its box starts empty: it is dirty only when something is
 * typed into it, or when it is cleared while set. A key fixed by an
 * environment variable is neither editable nor dirty, whatever is asked.
 *
 * # Refused is not invalid
 *
 * `errors` are the client's own checks, and hold Save off. `refused` are the
 * server's answers to a save; the key stays dirty with the reason beside it,
 * and the admin may change it or try again.
 */

export type Key = string;

export interface BaselineEntry {
  /** The loaded value. Always null for a secret, which is never read back. */
  value: string | null;
  secret: boolean;
  /** Fixed by an environment variable, or written elsewhere: not editable. */
  fixed: boolean;
  /** For a secret: whether one is stored. */
  set?: boolean;
}

export type Baseline = Record<Key, BaselineEntry>;

export type Change = { kind: "set"; value: string } | { kind: "clear" };

export interface FormState {
  baseline: Baseline;
  changes: Partial<Record<Key, Change>>;
  errors: Partial<Record<Key, string>>;
  refused: Partial<Record<Key, string>>;
}

export type SaveResult =
  | { key: Key; saved: true }
  | { key: Key; saved: false; error: string };

/** `mail.enabled` is sent last, so the server details land before mail is on. */
const LAST = "mail.enabled";

export function baselineOf(settings: readonly ResolvedSetting[]): Baseline {
  const baseline: Baseline = {};
  for (const setting of settings) {
    const secret = setting.secretState !== null;
    baseline[setting.key] = secret
      ? {
          value: null,
          secret: true,
          fixed: !setting.editable,
          set: setting.secretState === "SET",
        }
      : { value: setting.value, secret: false, fixed: !setting.editable };
  }
  return baseline;
}

export function emptyForm(baseline: Baseline): FormState {
  return { baseline, changes: {}, errors: {}, refused: {} };
}

function without<T>(record: Partial<Record<Key, T>>, key: Key) {
  if (!(key in record)) {
    return record;
  }
  const next = { ...record };
  delete next[key];
  return next;
}

function differs(entry: BaselineEntry, change: Change): boolean {
  if (change.kind === "clear") {
    return entry.secret ? entry.set === true : entry.value !== null;
  }
  if (entry.secret) {
    return change.value !== "";
  }
  return change.value.trim() !== (entry.value ?? "");
}

function withChange(
  state: FormState,
  key: Key,
  change: Change,
  error: string | null,
): FormState {
  const entry = state.baseline[key];
  if (!entry || entry.fixed) {
    return state;
  }
  const refused = without(state.refused, key);
  if (!differs(entry, change)) {
    return {
      ...state,
      changes: without(state.changes, key),
      errors: without(state.errors, key),
      refused,
    };
  }
  return {
    ...state,
    changes: { ...state.changes, [key]: change },
    errors: error
      ? { ...state.errors, [key]: error }
      : without(state.errors, key),
    refused,
  };
}

/** The admin typed `draft` into `key`'s box. */
export function edit(
  state: FormState,
  key: Key,
  draft: string,
  validate?: (value: string) => string | null,
): FormState {
  const entry = state.baseline[key];
  const checked = entry?.secret ? draft : draft.trim();
  return withChange(
    state,
    key,
    { kind: "set", value: draft },
    validate ? validate(checked) : null,
  );
}

/** The admin pressed Clear on `key`: the save sends null. */
export function clear(state: FormState, key: Key): FormState {
  return withChange(state, key, { kind: "clear" }, null);
}

export function discard(state: FormState): FormState {
  return emptyForm(state.baseline);
}

/** What `key`'s box shows: the change as typed, or what was loaded. */
export function draftOf(state: FormState, key: Key): string {
  const change = state.changes[key];
  if (change) {
    return change.kind === "set" ? change.value : "";
  }
  const entry = state.baseline[key];
  return entry && !entry.secret ? (entry.value ?? "") : "";
}

export function isDirty(state: FormState, key: Key): boolean {
  return key in state.changes;
}

/** The dirty keys in the order the form shows them, with `mail.enabled` last. */
export function dirtyKeys(state: FormState, order: readonly Key[]): Key[] {
  const known = new Set(order);
  const dirty = [
    ...order.filter((key) => isDirty(state, key)),
    ...Object.keys(state.changes).filter((key) => !known.has(key)),
  ];
  return [
    ...dirty.filter((key) => key !== LAST),
    ...dirty.filter((key) => key === LAST),
  ];
}

export function canSave(state: FormState): boolean {
  const dirty = Object.keys(state.changes);
  return dirty.length > 0 && dirty.every((key) => !state.errors[key]);
}

/** What one `updateInstanceSetting` for `key` sends. */
export function sendValue(state: FormState, key: Key): string | null {
  const change = state.changes[key];
  if (!change || change.kind === "clear") {
    return null;
  }
  return state.baseline[key]?.secret ? change.value : change.value.trim();
}

/** A save ended: each saved key is the new baseline, a refused one stays. */
export function applyResults(
  state: FormState,
  results: readonly SaveResult[],
): FormState {
  let next = state;
  for (const result of results) {
    const change = next.changes[result.key];
    const entry = next.baseline[result.key];
    if (!change || !entry) {
      continue;
    }
    if (!result.saved) {
      next = {
        ...next,
        refused: { ...next.refused, [result.key]: result.error },
      };
      continue;
    }
    const saved: BaselineEntry = entry.secret
      ? { ...entry, set: change.kind === "set" }
      : { ...entry, value: sendValue(next, result.key) };
    next = {
      baseline: { ...next.baseline, [result.key]: saved },
      changes: without(next.changes, result.key),
      errors: without(next.errors, result.key),
      refused: without(next.refused, result.key),
    };
  }
  return next;
}

/** The settings were read again: keep only the changes that still differ. */
export function rebase(state: FormState, fresh: Baseline): FormState {
  const next: FormState = { ...emptyForm(fresh) };
  for (const [key, change] of Object.entries(state.changes)) {
    const entry = fresh[key];
    if (!change || !entry || entry.fixed || !differs(entry, change)) {
      continue;
    }
    next.changes[key] = change;
    if (state.errors[key]) next.errors[key] = state.errors[key];
    if (state.refused[key]) next.refused[key] = state.refused[key];
  }
  return next;
}

/** "k of n saved", for the notice after a save. */
export function savedNotice(results: readonly SaveResult[]): string {
  const saved = results.filter((result) => result.saved).length;
  return `${saved} of ${results.length} saved`;
}
