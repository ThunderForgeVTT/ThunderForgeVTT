import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 030, User Story 7 — a new subsystem becomes triggerable.
 *
 * Every other spec in this feature tests what an effect *does*. This one tests
 * that contributing one is a self-contained act: that the seam is a seam,
 * rather than a shape that happens to have four users.
 *
 * # What is asserted, and what a Rust test asserts instead
 *
 * Removability cannot be shown from a browser — a running server has whatever
 * was compiled into it, and this cannot rebuild one mid-test. So the halves
 * are split by what each can actually prove:
 *
 * - **Here**: a contributor that touches nothing becomes authorable through
 *   the same registry, is configured through the same form data, activates
 *   through the same mutation, and dispatches through the same message — with
 *   no special case anywhere along that path. If contributing needed *any*
 *   privileged step, it would show up as a difference here.
 * - **In `interaction_tests.rs`**, where tests execute: a registry assembled
 *   without doors and lighting offers neither, everything else is byte-identical,
 *   and authoring still works with every contributor removed.
 * - **In `check-interaction-seam.mjs`**: the interaction core names none of
 *   "light", "door" or "sound", so the coupling is greppable rather than a
 *   matter of judgement.
 *
 * Splitting it that way is the honest arrangement. A browser test claiming to
 * have removed a subsystem would be claiming something it did not do.
 */

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

test("a trivial contributor is authorable and runs, with no special case", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Seam ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  // --- it arrives through the ordinary registry ---------------------------

  const registry = await gql<{
    effectRegistry: {
      id: string;
      label: string;
      description: string;
      subjectKinds: string[];
      config: {
        key: string;
        kind: string;
        required: boolean;
        options: { value: string; label: string }[] | null;
      }[];
    }[];
  }>(
    page,
    `query {
      effectRegistry {
        id
        label
        description
        subjectKinds
        config { key kind required options { value label } }
      }
    }`,
    {},
  );

  const probe = registry.effectRegistry.find((d) => d.id === "probe.echo");
  expect(probe, "the probe is offered like anything else").toBeTruthy();

  // Every field a real contributor fills in, filled in — nothing optional,
  // nothing defaulted, nothing marked as a special kind of effect. There is no
  // "isBuiltIn", no "isTest", no flag distinguishing it, and that absence is
  // the assertion: the registry has one sort of member.
  const keys = Object.keys(probe!).sort();
  const anyOther = registry.effectRegistry.find((d) => d.id !== "probe.echo")!;
  expect(
    keys,
    "the probe is described by exactly the fields every contributor is",
  ).toEqual(Object.keys(anyOther).sort());
  expect(probe!.label.length).toBeGreaterThan(0);
  expect(probe!.description.length).toBeGreaterThan(0);
  expect(probe!.config[0].kind).toBe("choice");
  expect(probe!.config[0].options?.length).toBeGreaterThan(0);

  // Namespaced like the rest, which is what makes collision detection a prefix
  // check rather than a coordination problem.
  for (const declaration of registry.effectRegistry) {
    expect(
      declaration.id,
      `${declaration.id} must be namespaced by its contributor`,
    ).toContain(".");
  }

  // --- it is authored through the same mutation ---------------------------

  const prop = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: 0, y: 0, tokenType: "object" } },
  );

  const created = await gql<{
    createInteractive: { interactiveId: string; available: boolean };
  }>(
    page,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId available }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: prop.createToken.tokenId,
        effectId: "probe.echo",
        effectConfig: { note: "first" },
        trigger: "click",
        activation: "anyone",
      },
    },
  );
  const interactiveId = created.createInteractive.interactiveId;
  expect(created.createInteractive.available).toBe(true);

  // The same validation applies to it as to anything else. A contributor that
  // could smuggle unvalidated configuration through would be a contributor
  // with a privileged path.
  const badNote = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($input: GraphQLCreateInteractiveInput!) {
        createInteractive(input: $input) {
          interactiveId
        }
      }
    `,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: prop.createToken.tokenId,
        effectId: "probe.echo",
        effectConfig: { note: "third" },
        trigger: "click",
        activation: "anyone",
      },
    },
  );
  expect(
    badNote.errors?.length,
    "a value outside the declared choices is refused, as for any contributor",
  ).toBeGreaterThan(0);

  // --- and it runs through the same message -------------------------------

  await page.goto(`/world/${worldId}/play`);
  await page.evaluate((id) => {
    (window as unknown as { __probe: string }).__probe = id;
  }, interactiveId);
  await waitForEngineReady(page);

  const ran = await page.evaluate(async (scene: string) => {
    const sync = (await import(
      /* @vite-ignore */ "/src/engine/world/sync/interactives.ts"
    )) as typeof import("../src/engine/world/sync/interactives");
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/interactionProbe.ts"
    )) as typeof import("../src/engine/bevy/interactionProbe");

    const store = bevy.getBoundWorldStore();
    if (!store) return { outcome: "no-store", dispatched: [] };

    await sync.refreshInteractives(store, scene);
    const result = await sync.activateAndApply(
      store,
      (window as unknown as { __probe: string }).__probe,
    );
    // A frame for the engine's Update schedule to read the message.
    await new Promise((resolve) => setTimeout(resolve, 400));

    return {
      outcome: result.outcome,
      dispatched: await probe.dispatchedEffects(),
    };
  }, sceneId);

  expect(ran.outcome).toBe("performed");
  expect(
    ran.dispatched.some(
      (entry) =>
        entry.effectId === "probe.echo" &&
        entry.interactiveId === interactiveId,
    ),
    "the effect reached the engine through the same message every other one uses",
  ).toBe(true);

  console.log(
    `[seam] contributors=${registry.effectRegistry.length} probeAuthorable=true probeDispatched=true specialCases=0`,
  );
});
