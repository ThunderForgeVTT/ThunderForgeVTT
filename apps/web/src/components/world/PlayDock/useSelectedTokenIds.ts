import { useEffect, useState } from "react";
import { getBoundWorldStore } from "@/engine/bevy";
import { sameTokenIds } from "./combatRoster";

/**
 * What the engine currently has selected, for chrome to read.
 *
 * # Why this reads the bound store instead of taking a prop
 *
 * Constitution Principle I: selection is engine state. The world store is the
 * mirror the engine already writes it into (`select_token` / `select_tokens`
 * from Bevy, via `bindWorldStore`), so observing it here adds no second
 * authority — nothing in this hook can change what is selected, only report
 * it. Threading the selection down as a prop from the world page would have
 * been the other option; it would put a value that changes on every click
 * through a component that re-renders the whole map dock, and would tie this
 * panel to one page's plumbing.
 *
 * # Why it waits for a store rather than assuming one
 *
 * `bindWorldStore` finishes asynchronously, after the WASM module loads, and
 * the dock can mount first. A single read at mount would therefore report "no
 * selection" for the whole session on a slow load. Polling until the store
 * exists is unattractive, but it is honest and it stops the moment it
 * succeeds; the alternative — an event the engine bridge would have to publish
 * — means editing the bridge for a panel's benefit.
 *
 * Selection changes on the same store as token drags, so identical selections
 * are filtered out. Without that, dragging a token would re-render this panel
 * on every frame the store hears about.
 */

/** How often to look for the store before it has finished binding. */
const BIND_POLL_MS = 500;

export function useSelectedTokenIds(): string[] {
  const [selected, setSelected] = useState<string[]>([]);

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    let poll: ReturnType<typeof setInterval> | undefined;

    const adopt = (ids: string[]) =>
      setSelected((current) => (sameTokenIds(current, ids) ? current : ids));

    const attach = (): boolean => {
      const store = getBoundWorldStore();
      if (!store) return false;
      adopt(store.getState().selectedTokenIds);
      unsubscribe = store.subscribe((event) =>
        adopt(event.state.selectedTokenIds),
      );
      return true;
    };

    if (!attach()) {
      poll = setInterval(() => {
        if (attach() && poll) clearInterval(poll);
      }, BIND_POLL_MS);
    }

    return () => {
      unsubscribe?.();
      if (poll) clearInterval(poll);
    };
  }, []);

  return selected;
}
