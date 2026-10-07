/**
 * Chat and launching a scene answer a player as the server does
 * (`mutations_chat.rs`, `mutations_scenes.rs`): a player is named as
 * themselves, never sees a GM-only message, and may not launch a scene.
 */
import { beforeEach, describe, expect, it } from "vitest";
import type { DemoState } from "./state";
import { ask, freshWorld, must, viewAs } from "./testing/world";

let state: DemoState;

const SEND = `mutation ($input: SendChatMessageInput!) {
  sendChatMessage(input: $input) { authorUserId authorLabel body gmOnly }
}`;
const HISTORY = `query ($w: UUID!, $l: Int) {
  worldChatMessages(worldId: $w, limit: $l) { body gmOnly }
}`;
const LAUNCH = `mutation ($w: UUID!, $s: UUID!) {
  launchScene(worldId: $w, sceneId: $s) { id }
}`;

beforeEach(async () => {
  state = await freshWorld();
  state.chat = [];
});

function send(body: string, gmOnly?: boolean) {
  return ask(SEND, { input: { worldId: state.world.id, body, gmOnly } });
}

describe("sendChatMessage", () => {
  it("names the sender, and trims what they wrote", async () => {
    viewAs("player");
    const answer = await send("  hello  ");
    expect(answer.errors).toBeUndefined();
    expect(answer.data?.sendChatMessage).toMatchObject({
      authorLabel: "player",
      body: "hello",
      gmOnly: false,
    });
  });

  it("refuses an empty message, a long one, and a player's whisper", async () => {
    expect((await send("   ")).errors?.[0]?.message).toBe(
      "Message cannot be empty",
    );
    expect((await send("x".repeat(4001))).errors?.[0]?.message).toBe(
      "Message cannot exceed 4000 characters",
    );
    viewAs("player");
    expect((await send("psst", true)).errors?.[0]?.message).toBe(
      "Only the GM may send a GM-only message",
    );
  });
});

describe("worldChatMessages", () => {
  it("keeps the Game Master's whispers from a player", async () => {
    await send("to all");
    await send("for the GM", true);
    const gm = await must(HISTORY, { w: state.world.id });
    expect(gm.worldChatMessages).toHaveLength(2);
    viewAs("player");
    const player = await must(HISTORY, { w: state.world.id });
    expect(player.worldChatMessages).toEqual([
      { body: "to all", gmOnly: false },
    ]);
  });

  it("answers the newest messages, oldest first", async () => {
    for (const body of ["one", "two", "three"]) await send(body);
    const answer = await must(HISTORY, { w: state.world.id, l: 2 });
    expect(
      answer.worldChatMessages.map((m: { body: string }) => m.body),
    ).toEqual(["two", "three"]);
  });
});

describe("launchScene", () => {
  it("is the Game Master's alone", async () => {
    const sceneId = state.scenes[0].sceneId as string;
    viewAs("player");
    expect(
      (await ask(LAUNCH, { w: state.world.id, s: sceneId })).errors?.[0]
        ?.message,
    ).toBe("Only the DM (Owner or GM) may launch a scene");
    viewAs("gm");
    expect(
      (await ask(LAUNCH, { w: state.world.id, s: sceneId })).errors,
    ).toBeUndefined();
  });
});
