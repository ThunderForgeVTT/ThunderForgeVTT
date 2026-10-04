/**
 * What the Game Master has said has to be beaten, read from and written to
 * the server.
 *
 * The number used to live on each player's sheet: they typed it, or in a
 * world that rolls for difficulty they rolled the Game Master's dice
 * themselves. It now lives in a table this pack owns, written by the Game
 * Master and read by everyone, so the whole table rolls against the same
 * number and nobody can re-roll the Game Master's dice but the Game Master.
 *
 * Three fields and one event code, all through `@thunderforge/host`, as the
 * rest of the pack's network surface is.
 */
import {
  postGraphQL,
  subscribeToWorldEvents,
  type WorldEventLike,
} from "@thunderforge/host";

import {
  BANDS,
  NO_TABLE_DIFFICULTY,
  type Band,
  type TableDifficulty,
} from "./game.ts";

const DIFFICULTY_FIELDS = `
  worldId
  target
  band
  gmDice
  canSet
`;

const TABLE_DIFFICULTY_QUERY = `
  query RollForShoesTableDifficulty($worldId: UUID!) {
    rollForShoesTableDifficulty(worldId: $worldId) {
      ${DIFFICULTY_FIELDS}
    }
  }
`;

const SET_TABLE_DIFFICULTY_MUTATION = `
  mutation SetRollForShoesTableDifficulty(
    $input: SetRollForShoesTableDifficultyInput!
  ) {
    setRollForShoesTableDifficulty(input: $input) {
      ${DIFFICULTY_FIELDS}
    }
  }
`;

const CLEAR_TABLE_DIFFICULTY_MUTATION = `
  mutation ClearRollForShoesTableDifficulty($worldId: UUID!) {
    clearRollForShoesTableDifficulty(worldId: $worldId) {
      ${DIFFICULTY_FIELDS}
    }
  }
`;

interface TableDifficultyPayload {
  worldId: string;
  target: number | null;
  band: string | null;
  gmDice: number[] | null;
  canSet: boolean;
}

/**
 * The wire shape as the rules see it.
 *
 * A band this build does not know — a newer server — reads as no band. The
 * number is what a roll needs and it is still there.
 */
function toDifficulty(payload: TableDifficultyPayload): TableDifficulty {
  const band = BANDS.find((known) => known === payload.band) ?? null;
  return {
    target: typeof payload.target === "number" ? payload.target : null,
    band,
    gmDice: Array.isArray(payload.gmDice) ? payload.gmDice : null,
    canSet: payload.canSet === true,
  };
}

/** Throws on refusal; the caller decides what a failed read means. */
export async function fetchTableDifficulty(
  worldId: string,
): Promise<TableDifficulty> {
  const { rollForShoesTableDifficulty } = await postGraphQL<{
    rollForShoesTableDifficulty: TableDifficultyPayload | null;
  }>(TABLE_DIFFICULTY_QUERY, { worldId });
  return rollForShoesTableDifficulty
    ? toDifficulty(rollForShoesTableDifficulty)
    : NO_TABLE_DIFFICULTY;
}

/**
 * Say what has to be beaten: a number, or a band.
 *
 * Exactly one. In a world that rolls for difficulty a band is rolled by the
 * server, once — the answer carries the dice.
 */
export async function setTableDifficulty(
  worldId: string,
  difficulty: { target: number } | { band: Band },
): Promise<TableDifficulty> {
  const { setRollForShoesTableDifficulty } = await postGraphQL<{
    setRollForShoesTableDifficulty: TableDifficultyPayload;
  }>(SET_TABLE_DIFFICULTY_MUTATION, { input: { worldId, ...difficulty } });
  return toDifficulty(setRollForShoesTableDifficulty);
}

export async function clearTableDifficulty(
  worldId: string,
): Promise<TableDifficulty> {
  const { clearRollForShoesTableDifficulty } = await postGraphQL<{
    clearRollForShoesTableDifficulty: TableDifficultyPayload;
  }>(CLEAR_TABLE_DIFFICULTY_MUTATION, { worldId });
  return toDifficulty(clearRollForShoesTableDifficulty);
}

/**
 * The server's code for "the table's difficulty changed". This pack's own,
 * reserved beside every other code in the server's `world_events.rs`.
 */
const TABLE_DIFFICULTY_CHANGED = 32;

function codeOf(event: WorldEventLike): number | undefined {
  return event.event_code ?? event.eventCode;
}

/**
 * Call `onChanged` whenever the Game Master sets or clears the difficulty.
 * Returns the function that stops watching.
 *
 * The event says only that something changed; the listener reads again. The
 * iterator is held and returned explicitly, for the reason `watchSheet`
 * gives.
 */
export function watchTableDifficulty(
  worldId: string,
  onChanged: () => void,
): () => void {
  const iterator = subscribeToWorldEvents(worldId)[Symbol.asyncIterator]();
  let stopped = false;

  void (async () => {
    try {
      while (!stopped) {
        const { value: event, done } = await iterator.next();
        if (done || stopped || !event) {
          break;
        }
        if (codeOf(event) === TABLE_DIFFICULTY_CHANGED) {
          onChanged();
        }
      }
    } catch (error) {
      console.error("Roll for Shoes difficulty sync error:", error);
    }
  })();

  return () => {
    stopped = true;
    void iterator.return?.();
  };
}
