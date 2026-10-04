/**
 * The table's standing difficulty, kept current for whoever is looking.
 *
 * One hook for the three places it is shown — the sheet, the play dock's
 * panel, and the staging page — so all three read it the same way, hear the
 * same event, and cannot drift into showing different numbers.
 */
import { useCallback, useEffect, useState } from "react";
import { GraphQLRequestError } from "@thunderforge/host";

import {
  NO_TABLE_DIFFICULTY,
  type Band,
  type TableDifficulty,
} from "./game.ts";
import {
  clearTableDifficulty,
  fetchTableDifficulty,
  setTableDifficulty,
  watchTableDifficulty,
} from "./tableDifficulty.ts";

export interface TableDifficultyHandle {
  difficulty: TableDifficulty;
  /**
   * `failed` when the last read was refused. A sheet in that state does not
   * know whether the Game Master has set a number, and must not let its
   * player type one as though they had not.
   */
  state: "loading" | "ready" | "failed";
  /** A write in flight. */
  busy: boolean;
  /** Why the last write was refused, in words the Game Master can act on. */
  refusal: string | null;
  /**
   * Read again, now, and answer with what was read. Throws when the read is
   * refused — a roll calls this and must not go ahead on a guess.
   */
  refresh: () => Promise<TableDifficulty>;
  set: (difficulty: { target: number } | { band: Band }) => Promise<void>;
  clear: () => Promise<void>;
}

function reasonOf(thrown: unknown): string {
  if (thrown instanceof GraphQLRequestError || thrown instanceof Error) {
    return thrown.message;
  }
  return "Something went wrong.";
}

export function useTableDifficulty(worldId: string): TableDifficultyHandle {
  const [difficulty, setDifficulty] =
    useState<TableDifficulty>(NO_TABLE_DIFFICULTY);
  const [state, setState] = useState<"loading" | "ready" | "failed">("loading");
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);

  const refresh = useCallback(async (): Promise<TableDifficulty> => {
    try {
      const read = await fetchTableDifficulty(worldId);
      setDifficulty(read);
      setState("ready");
      return read;
    } catch (thrown) {
      setState("failed");
      throw thrown;
    }
  }, [worldId]);

  useEffect(() => {
    // The first read, and one more each time the Game Master changes it. A
    // refused read is recorded by `refresh` itself; there is nobody here to
    // hand the error to.
    void refresh().catch(() => undefined);
    return watchTableDifficulty(
      worldId,
      () => void refresh().catch(() => undefined),
    );
  }, [worldId, refresh]);

  const write = useCallback(
    async (act: () => Promise<TableDifficulty>): Promise<void> => {
      setRefusal(null);
      setBusy(true);
      try {
        setDifficulty(await act());
        setState("ready");
      } catch (thrown) {
        setRefusal(reasonOf(thrown));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const set = useCallback(
    (next: { target: number } | { band: Band }) =>
      write(() => setTableDifficulty(worldId, next)),
    [worldId, write],
  );
  const clear = useCallback(
    () => write(() => clearTableDifficulty(worldId)),
    [worldId, write],
  );

  return { difficulty, state, busy, refusal, refresh, set, clear };
}
