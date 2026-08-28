import { useState } from "react";

/**
 * React's "adjusting some state when a prop changes" pattern, as a hook:
 * https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
 *
 * Many components here fetch on `[worldId, …]` and open the effect by
 * resetting their own state ("clear the previous world's rows, show the
 * loader again") before the request goes out. That reset is not
 * synchronisation with anything external — it is state *derived* from the
 * arguments — so doing it in the effect costs a committed render showing the
 * previous key's data next to the new key, and `react-hooks/set-state-in-effect`
 * is right to flag it.
 *
 * Doing it during render instead lets React discard that render and re-run
 * the component immediately, before anything is committed or any child
 * renders. The effect that follows then only ever reports results.
 *
 * `key` is compared with `Object.is`, so pass a primitive — a template string
 * when several arguments make up the identity. Never pass a function: the
 * `useState` initialiser would call it.
 *
 * `reset` runs *during render* and must therefore only call `setState` on the
 * calling component. No refs, no DOM, no side effects of any kind.
 */
export function useResetOnChange(key: unknown, reset: () => void): void {
  const [previousKey, setPreviousKey] = useState(key);
  if (!Object.is(previousKey, key)) {
    setPreviousKey(key);
    reset();
  }
}
