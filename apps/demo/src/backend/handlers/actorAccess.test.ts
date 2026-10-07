/**
 * Opening a character asks for the world's lore and the actor's ownership
 * block. Both are answered, by the server's rules, and neither is refused as
 * "not part of the demo".
 */
import { beforeEach, describe, expect, it } from "vitest";
import { ask, freshWorld, must, refused, viewAs } from "../testing/world";
import { DEMO_PLAYER } from "../../seed/world";
import { demoState } from "../state";

const PERMISSIONS = `query ($actorId: UUID!) {
  actorPermissions(actorId: $actorId) { actorId userId level updatedAt }
}`;
const SET = `mutation ($input: SetActorPermissionInput!) {
  setActorPermission(input: $input) { actorId userId level }
}`;
const REMOVE = `mutation ($actorId: UUID!, $userId: UUID!) {
  removeActorPermission(actorId: $actorId, userId: $userId)
}`;
const ACTORS = `query ($worldId: UUID!) {
  worldActors(worldId: $worldId) { id label myPermissionLevel loreLinkedFrom { id title } }
}`;
const LORE = `query ($worldId: UUID!) {
  worldLoreEntries(worldId: $worldId) { id title slug renderedHtml myPermissionLevel linkedFrom { id } }
}`;

const goblin = () =>
  demoState().actors.find((a) => a.castKey === "goblin")!.id as string;

beforeEach(async () => {
  await freshWorld("gm");
});

describe("the ownership block", () => {
  it("is empty for the Game Master until a grant is made, then lists it", async () => {
    const actorId = goblin();
    expect((await must(PERMISSIONS, { actorId })).actorPermissions).toEqual([]);

    await must(SET, {
      input: { actorId, userId: DEMO_PLAYER.id, level: "EDITOR" },
    });
    const rows = (await must(PERMISSIONS, { actorId })).actorPermissions;
    expect(rows).toMatchObject([
      { actorId, userId: DEMO_PLAYER.id, level: "EDITOR" },
    ]);

    expect((await must(REMOVE, { actorId, userId: DEMO_PLAYER.id }))
      .removeActorPermission).toBe(true);
    expect((await must(REMOVE, { actorId, userId: DEMO_PLAYER.id }))
      .removeActorPermission).toBe(false);
    expect(refused).toEqual([]);
  });

  it("gives the player what the grant says", async () => {
    const actorId = demoState().actors.find((a) => a.castKey === "hobgoblin")!
      .id as string;
    demoState().actors.find((a) => a.id === actorId)!.visibleToPlayers = true;
    await must(SET, {
      input: { actorId, userId: DEMO_PLAYER.id, level: "EDITOR" },
    });
    viewAs("player");
    const { worldActors } = await must(ACTORS, {
      worldId: demoState().world.id,
    });
    const row = worldActors.find((a: { id: string }) => a.id === actorId);
    expect(row.myPermissionLevel).toBe("EDITOR");
  });

  it("is the Game Master's alone, as on the server", async () => {
    viewAs("player");
    const answer = await ask(PERMISSIONS, {
      actorId: demoState().claimedActorId,
    });
    expect(answer.errors?.[0]?.message).toBe(
      "Only the DM (Owner or GM) may view or change an actor's ownership block",
    );
    expect(refused).toEqual([]);
  });
});

describe("the world's lore, read from a character", () => {
  it("is answered for every member, and backlinks reach the actor", async () => {
    const worldId = demoState().world.id;
    const state = demoState();
    state.lore.push({
      id: "d0000000-0000-4000-00aa-000000000001",
      worldId,
      title: "Road Notes",
      slug: "road-notes",
      content: "Watch for [[Brannoc Stoneward]].",
      createdBy: state.world.createdBy,
      createdAt: state.world.createdAt,
      updatedAt: state.world.createdAt,
    });
    for (const viewer of ["gm", "player"] as const) {
      viewAs(viewer);
      const { worldLoreEntries } = await must(LORE, { worldId });
      const entry = worldLoreEntries.find(
        (e: { slug: string }) => e.slug === "road-notes",
      );
      expect(entry.myPermissionLevel).toBe(viewer === "gm" ? "OWNER" : "VIEWER");
      expect(entry.renderedHtml).toContain(
        '<a class="lore-link" href="/world/',
      );
      const { worldActors } = await must(ACTORS, { worldId });
      const brannoc = worldActors.find(
        (a: { label: string }) => a.label === "Brannoc Stoneward",
      );
      expect(brannoc.loreLinkedFrom.map((e: { title: string }) => e.title))
        .toContain("Road Notes");
    }
    expect(refused).toEqual([]);
  });
});
