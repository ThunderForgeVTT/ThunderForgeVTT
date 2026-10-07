/**
 * Session notes are the Game Master's to write and every member's to read,
 * and the demo keeps them in this browser instead of refusing the save.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { ask, freshWorld, must, refused, viewAs } from "../testing/world";
import { demoState } from "../state";

const SAVE = `mutation ($input: UpdateWorldSessionNotesInput!) {
  updateWorldSessionNotes(input: $input) { id sessionNotes }
}`;
const READ = `query ($id: UUID!) { world(id: $id) { sessionNotes } }`;

beforeEach(async () => {
  await freshWorld("gm");
});

describe("session notes", () => {
  it("are saved by the Game Master and read back by a player", async () => {
    const worldId = demoState().world.id;
    const saved = await must(SAVE, {
      input: { worldId, notes: "Goblins owe the party a cart." },
    });
    expect(saved.updateWorldSessionNotes.sessionNotes).toBe(
      "Goblins owe the party a cart.",
    );

    viewAs("player");
    const read = await must(READ, { id: worldId });
    expect(read.world.sessionNotes).toBe("Goblins owe the party a cart.");
    expect(refused).toEqual([]);
  });

  it("take an empty string as a real save", async () => {
    const worldId = demoState().world.id;
    await must(SAVE, { input: { worldId, notes: "Something" } });
    const saved = await must(SAVE, { input: { worldId, notes: "" } });
    expect(saved.updateWorldSessionNotes.sessionNotes).toBe("");
  });

  it("are not a player's to write", async () => {
    viewAs("player");
    const answer = await ask(SAVE, {
      input: { worldId: demoState().world.id, notes: "Mine now" },
    });
    expect(answer.errors?.[0]?.message).toBe(
      "Only the DM (Owner or GM) may update session notes",
    );
    expect(demoState().world.sessionNotes).toBeNull();
    expect(refused).toEqual([]);
  });
});
