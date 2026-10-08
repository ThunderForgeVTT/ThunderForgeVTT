/**
 * Spec 081 R7: who is signed in is the asking tab's choice, not the world's.
 * The session is read before the tab asks anything of GraphQL, so the world
 * still holds whoever asked last, or the viewer it was saved with.
 */
import { describe, expect, it } from "vitest";
import { freshWorld } from "../backend/testing/world";
import { DEMO_PLAYER, DEMO_USER, type Viewer } from "../seed/world";
import { answerRest } from "./rest";

async function sessionUser(viewer: Viewer): Promise<{ id: string }> {
  const response = await answerRest(
    "GET",
    "/api/authentication/session",
    "/demo/",
    fetch,
    async () => new Blob(),
    viewer,
  );
  const body = (await response!.json()) as {
    session: { user: { id: string } };
  };
  return body.session.user;
}

describe("the session", () => {
  it("signs a tab switched to the player in as the player, whoever asked last", async () => {
    await freshWorld("gm");
    expect((await sessionUser("player")).id).toBe(DEMO_PLAYER.id);
  });

  it("signs a Game Master's tab in as the Game Master beside a player's", async () => {
    await freshWorld("player");
    expect((await sessionUser("gm")).id).toBe(DEMO_USER.id);
  });
});
