import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Phase 8 — a token carries the attribute scores its own system declares, and
 * nothing anywhere names them.
 *
 * # Why this is four systems and not one
 *
 * The engine used to hold `TokenAbilities { strength, dexterity,
 * constitution, intelligence, wisdom, charisma }`. That passes a single-system
 * test for D&D 5e and Pathfinder 2e forever, while storing six `None`s for
 * every Genie and Blades character — and both of those manifests already
 * declared their own three-attribute sets, so the struct disagreed with data
 * sitting in the repository.
 *
 * A test that exercised one ruleset could not tell the difference. These four
 * can: two declare six attributes, two declare three, and the two threes have
 * no attribute in common with the sixes. Nothing in the pipeline may contain
 * the word "dexterity" for this to pass.
 */

interface SystemCase {
  systemId: string;
  /** Stored `ability_data`, in that system's own field names. */
  stored: Record<string, number>;
  /** Attribute ids expected back, in declared order. */
  expect: string[];
}

const SYSTEMS: SystemCase[] = [
  {
    systemId: "genie",
    stored: { might: 5, cunning: 3, spirit: 4 },
    expect: ["might", "cunning", "spirit"],
  },
  {
    systemId: "dnd5e",
    stored: {
      strength: 16,
      dexterity: 12,
      constitution: 14,
      intelligence: 8,
      wisdom: 13,
      charisma: 10,
    },
    expect: [
      "strength",
      "dexterity",
      "constitution",
      "intelligence",
      "wisdom",
      "charisma",
    ],
  },
  {
    systemId: "pathfinder2e",
    // Pathfinder 2e stores *modifiers*, not raw scores — its own pack
    // validates them into -5..=10 (packs/systems/pathfinder2e/server/src/
    // validators.rs). D&D 5e, which declares the same six attribute names,
    // stores raw scores. Same identifiers, different scales: another reason
    // nothing above the manifest may assume what a number means.
    stored: {
      strength: 4,
      dexterity: 2,
      constitution: 1,
      intelligence: 0,
      wisdom: 3,
      charisma: -1,
    },
    expect: [
      "strength",
      "dexterity",
      "constitution",
      "intelligence",
      "wisdom",
      "charisma",
    ],
  },
  {
    systemId: "blades_in_the_dark",
    // A zero is a real and common action rating, and the obvious
    // "treat falsy as unset" implementation deletes it.
    stored: { insight: 2, prowess: 0, resolve: 3 },
    expect: ["insight", "prowess", "resolve"],
  },
];

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

for (const system of SYSTEMS) {
  test(`${system.systemId} attributes reach the token under their own names`, async ({
    page,
  }) => {
    test.setTimeout(3 * 60_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `Attr ${system.systemId} ${suffix}`,
    );

    const active = await gql<{ world?: { activeSceneId: string | null } }>(
      page,
      `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
      { id: worldId },
    );
    const [firstScene] = await sceneIds(page, worldId);
    const sceneId = active.world?.activeSceneId ?? firstScene;

    await gql(
      page,
      `mutation ($input: UpdateWorldGameSystemInput!) {
        updateWorldGameSystem(input: $input) { id }
      }`,
      { input: { worldId, gameSystemId: system.systemId } },
    );

    const actor = await gql<{ createActor: { id: string } }>(
      page,
      `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      {
        input: {
          worldId,
          label: `Hero ${suffix}`,
          isNpc: false,
          gameSystemId: system.systemId,
        },
      },
    );

    await gql(
      page,
      `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) { id }
      }`,
      {
        input: {
          actorId: actor.createActor.id,
          gameSystemId: system.systemId,
          dataType: "ability_data",
          data: system.stored,
        },
      },
    );

    const created = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      {
        input: {
          sceneId,
          x: 0,
          y: 0,
          actorId: actor.createActor.id,
          tokenType: "character",
        },
      },
    );
    const tokenId = created.createToken.tokenId;

    // Observed on the wire, which is what the client is actually sent.
    const resolved = await gql<{
      tokenAttributes: {
        tokenId: string;
        attributes: { id: string; label: string; value: number }[];
      }[];
    }>(
      page,
      `query ($sceneId: UUID!) {
        tokenAttributes(sceneId: $sceneId) {
          tokenId
          attributes { id label value }
        }
      }`,
      { sceneId },
    );

    const row = resolved.tokenAttributes.find((r) => r.tokenId === tokenId);
    expect(
      row,
      `${system.systemId} must resolve attributes for its token`,
    ).toBeTruthy();

    expect(
      row!.attributes.map((a) => a.id),
      "the system's own attributes, in the order it declared them",
    ).toEqual(system.expect);

    const values = Object.fromEntries(
      row!.attributes.map((a) => [a.id, a.value]),
    );
    expect(values).toEqual(system.stored);

    // Every attribute carries a label somebody can read.
    for (const attribute of row!.attributes) {
      expect(
        attribute.label.length,
        `${attribute.id} needs a label`,
      ).toBeGreaterThan(0);
    }

     
    console.log(
      `[attributes] ${system.systemId} count=${row!.attributes.length} ids=${row!.attributes
        .map((a) => a.id)
        .join(",")}`,
    );
  });
}
