import { expect, test, type Page } from "@playwright/test";
import {
  openAdminPage,
  readSettings,
  writeSetting,
  writeSettingOrThrow,
  type GqlResult,
} from "./fixtures/admin";
import {
  freshCredentials,
  graphql,
  register,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 040 FR-026: an instance with nobody to serve a copyright notice on
 * refuses to publish content beyond a world, and says why.
 *
 * # Why this needs to be end to end
 *
 * `graphql/publishing_gate.rs` proves the predicate and proves that every
 * `create*ShareLink` impl consults it — by reading the source. Neither of
 * those touches a running instance: they cannot tell you whether the refusal
 * reaches a caller through the schema, whether an operator can actually clear
 * the gate from the administrator's surface, or — the one that matters most —
 * whether a link minted while the instance was configured keeps working after
 * the contact goes away.
 *
 * # Why the e2e stack can be made to lack a notice contact
 *
 * `scripts/e2e-parallel.mjs` sets no `THUNDERFORGE_NOTICE_CONTACT_*`
 * variable, so all three declarations are `Backing::Row` with nothing fixing
 * them from the environment — which is exactly the condition
 * `settings::changes::write_setting` requires before it will accept a write.
 * The instance is therefore configured and unconfigured here through the real
 * administrator mutation rather than by restarting anything, which is also
 * how an operator would do it.
 *
 * Should an operator's own environment happen to fix one of them (the
 * variables are read from `.env` too), the writes below would be refused and
 * these tests would be proving nothing. So they check first and say so.
 *
 * # Housekeeping
 *
 * Instance settings are global to the stack, and a shard runs `--workers=1`,
 * so these tests are serial and put back exactly what they found.
 */

const NOTICE_KEYS = [
  "notice.contact_name",
  "notice.contact_email",
  "notice.contact_postal_address",
] as const;

/**
 * A notice contact the declarations' own validators accept.
 *
 * `realdomain.org` rather than anything `.test`, `.local` or `example.*`:
 * `settings::validate` refuses a reserved TLD, so a plausible-looking address
 * would leave the gate shut for a reason no test here intended.
 */
const CONFIGURED: Record<(typeof NOTICE_KEYS)[number], string> = {
  "notice.contact_name": "The Steward",
  "notice.contact_email": "notices@realdomain.org",
  "notice.contact_postal_address": "1 Anvil Road, Forgeton",
};

const CREATE_ITEM_SHARE = `
  mutation ShareItem($id: UUID!) {
    createItemShareLink(itemId: $id) { id shareCode }
  }
`;

const CREATE_ABILITY_SHARE = `
  mutation ShareAbility($id: UUID!) {
    createAbilityShareLink(abilityId: $id) { id shareCode }
  }
`;

const SHARED_ITEM = `
  query SharedItem($code: String!) {
    sharedItem(shareCode: $code) { name description }
  }
`;

type ShareResult = GqlResult<{
  createItemShareLink?: { id: string; shareCode: string };
  createAbilityShareLink?: { id: string; shareCode: string };
}>;

let admin: Page;
/** What the stack was configured with before this file touched it. */
const original: Record<string, string | null> = {};
/** Non-empty when the environment fixes a notice setting; see the note above. */
let fixedByEnvironment: string[] = [];

async function configure(): Promise<void> {
  for (const key of NOTICE_KEYS) {
    await writeSettingOrThrow(admin, key, CONFIGURED[key]);
  }
}

async function unconfigure(): Promise<void> {
  for (const key of NOTICE_KEYS) {
    await writeSettingOrThrow(admin, key, null);
  }
}

/** A registered Game Master with a world of their own, over the API. */
async function aGameMaster(
  page: Page,
): Promise<{ worldId: string; worldName: string }> {
  await register(page, freshCredentials("e2egate"));
  const worldName = `Gate World ${uniqueSuffix()}`;
  const created = await graphql<GqlResult<{ createWorld: { id: string } }>>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name: worldName } },
  );
  const worldId = created.data?.createWorld?.id;
  if (!worldId) {
    throw new Error(
      `could not create a world: ${JSON.stringify(created.errors ?? created)}`,
    );
  }
  return { worldId, worldName };
}

async function anItem(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const created = await graphql<GqlResult<{ createItem: { id: string } }>>(
    page,
    `
      mutation CI($input: CreateItemInput!) {
        createItem(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, name, description: "It never gutters." } },
  );
  const id = created.data?.createItem?.id;
  if (!id) {
    throw new Error(
      `could not create an item: ${JSON.stringify(created.errors ?? created)}`,
    );
  }
  return id;
}

async function anAbility(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const created = await graphql<GqlResult<{ createAbility: { id: string } }>>(
    page,
    `
      mutation CA($input: CreateAbilityInput!) {
        createAbility(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        worldId,
        name,
        description: "A crack of nearby thunder.",
        classification: "SPELL",
        // Not GM-only: the share path refuses one of those for its own
        // reasons, and a refusal from the wrong guard would look like this
        // one.
        gmOnly: false,
      },
    },
  );
  const id = created.data?.createAbility?.id;
  if (!id) {
    throw new Error(
      `could not create an ability: ${JSON.stringify(created.errors ?? created)}`,
    );
  }
  return id;
}

test.describe.configure({ mode: "serial" });

test.describe("Spec 040 FR-026: the publishing gate", () => {
  test.beforeAll(async ({ browser }) => {
    admin = await openAdminPage(browser);
    const settings = await readSettings(admin);
    fixedByEnvironment = [];
    for (const key of NOTICE_KEYS) {
      const setting = settings.find((s) => s.key === key);
      if (!setting) {
        throw new Error(`\`${key}\` is not declared — the gate has moved`);
      }
      original[key] = setting.value;
      if (setting.source === "ENVIRONMENT") {
        fixedByEnvironment.push(setting.fixedBy ?? key);
      }
    }
  });

  test.afterAll(async () => {
    if (!admin) {
      return;
    }
    for (const key of NOTICE_KEYS) {
      // Not `writeSettingOrThrow`: if the environment fixed these, every
      // write here is refused and that is not a failure of the restore.
      await writeSetting(admin, key, original[key]);
    }
    await admin.context().close();
  });

  test("an instance with no notice contact refuses to mint a share link, and says why", async ({
    page,
  }) => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment, so the notice contact cannot be taken away from a test`,
    );

    const { worldId } = await aGameMaster(page);
    const suffix = uniqueSuffix();
    const itemId = await anItem(page, worldId, `Storm Lantern ${suffix}`);
    const abilityId = await anAbility(page, worldId, `Thunderclap ${suffix}`);

    await unconfigure();

    for (const [what, mutation, id] of [
      ["an item", CREATE_ITEM_SHARE, itemId],
      ["an ability", CREATE_ABILITY_SHARE, abilityId],
    ] as const) {
      const refused = await graphql<ShareResult>(page, mutation, { id });
      expect(
        refused.data?.createItemShareLink ??
          refused.data?.createAbilityShareLink,
        `${what} must not be given a share code by an instance nobody can serve a notice on`,
      ).toBeFalsy();

      const message = refused.errors?.[0]?.message ?? "";
      // Why: the refusal is about copyright notices and not about the item.
      expect(message).toContain("copyright");
      // Where it is fixed, which is the half an operator acts on.
      expect(message).toContain("Admin");
      // And it says what still works, so a refusal is not read as an outage.
      expect(message).toContain("inside a world are unaffected");
    }

    // The other half of the same test, in the same session and against the
    // same two artifacts: with a contact set, both mint. Without this, the
    // refusals above would be equally satisfied by an unrelated failure —
    // a broken session, an item the caller may not share, a mutation that
    // errors for everybody.
    await configure();

    const item = await graphql<ShareResult>(page, CREATE_ITEM_SHARE, {
      id: itemId,
    });
    expect(
      item.errors,
      "a configured instance must mint an item share",
    ).toBeFalsy();
    expect(item.data?.createItemShareLink?.shareCode).toBeTruthy();

    const ability = await graphql<ShareResult>(page, CREATE_ABILITY_SHARE, {
      id: abilityId,
    });
    expect(ability.errors).toBeFalsy();
    expect(ability.data?.createAbilityShareLink?.shareCode).toBeTruthy();
  });

  /**
   * The property most likely to be broken later, and the reason this file
   * exists at all.
   *
   * Taking the notice contact away is a configuration change, not a takedown.
   * A reader holding a code that already worked must keep it: gating the
   * *read* would turn an operator editing a setting into a silent data-loss
   * event for everybody they had ever shared with, with no notice to anyone.
   */
  test("a share minted while configured keeps resolving after the contact goes away", async ({
    page,
    browser,
  }) => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment, so the notice contact cannot be taken away from a test`,
    );

    await configure();

    const { worldId } = await aGameMaster(page);
    const itemName = `Kept Lantern ${uniqueSuffix()}`;
    const itemId = await anItem(page, worldId, itemName);

    const minted = await graphql<ShareResult>(page, CREATE_ITEM_SHARE, {
      id: itemId,
    });
    const shareCode = minted.data?.createItemShareLink?.shareCode;
    expect(
      shareCode,
      "the share has to be minted while the instance may publish, or the test is about something else",
    ).toBeTruthy();

    await unconfigure();

    // First prove the instance really did stop being able to publish.
    // Without this, "the old link still works" would also pass on an
    // instance where the settings write silently did nothing.
    const secondItemId = await anItem(
      page,
      worldId,
      `Refused ${uniqueSuffix()}`,
    );
    const refused = await graphql<ShareResult>(page, CREATE_ITEM_SHARE, {
      id: secondItemId,
    });
    expect(refused.data?.createItemShareLink).toBeFalsy();
    expect(refused.errors?.[0]?.message ?? "").toContain("copyright");

    // And now the point: the code minted a moment ago still resolves, with
    // the real content behind it rather than an "unavailable" placeholder.
    const read = await graphql<GqlResult<{ sharedItem: { name: string } }>>(
      page,
      SHARED_ITEM,
      { code: shareCode },
    );
    expect(
      read.errors,
      "removing the notice contact must not revoke links already issued",
    ).toBeFalsy();
    expect(read.data?.sharedItem?.name).toBe(itemName);

    // Including for the visitor ADR-071 wrote the share read for: somebody
    // with no account at all, arriving at the page rather than the resolver.
    const visitorContext = await browser.newContext();
    const visitor = await visitorContext.newPage();
    try {
      await visitor.goto(`/shared/item/${shareCode}`);
      await expect(
        visitor.getByRole("heading", { name: itemName }),
        "a stranger's existing link must survive the contact being removed",
      ).toBeVisible({ timeout: 30_000 });
    } finally {
      await visitorContext.close();
    }
  });
  test("a world is fully playable while the instance has nobody to serve a notice on", async ({
    page,
    browser,
  }) => {
    test.skip(
      fixedByEnvironment.length > 0,
      `this stack fixes ${fixedByEnvironment.join(", ")} in the environment`,
    );

    // The refusal message promises that "things inside a world are
    // unaffected", and until now that promise was tested by asserting the
    // message *says* it. That is a test of the wording, not of the claim —
    // it would pass unchanged if the gate had shut a world down, so long as
    // the sentence survived. This plays a world instead.
    await unconfigure();

    const { worldId } = await aGameMaster(page);

    // Content authored inside the world. Lore is the pointed case: the same
    // entry becomes publishable-beyond-the-world material the moment somebody
    // shares a collection, so if the gate were drawn in the wrong place this
    // is where it would bite.
    const lore = await graphql<GqlResult<{ createLoreEntry: { id: string } }>>(
      page,
      `
        mutation CL($input: CreateLoreEntryInput!) {
          createLoreEntry(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId,
          title: `Unserved ${uniqueSuffix()}`,
          content: "Written while nobody could be served a notice.",
        },
      },
    );
    expect(
      lore.errors,
      "authoring inside a world must not need a notice contact",
    ).toBeFalsy();
    expect(lore.data?.createLoreEntry?.id).toBeTruthy();

    // And a second person can be invited and can play. Sharing *within* a
    // world is the capability the gate deliberately does not cover — spec 040
    // gates `PublishBeyondWorld`, and a table is not beyond the world.
    const invite = await graphql<
      GqlResult<{ generateInviteCode: { inviteCode: string } }>
    >(
      page,
      `
        mutation GI($input: GenerateInviteCodeInput!) {
          generateInviteCode(input: $input) {
            inviteCode
          }
        }
      `,
      { input: { worldId, maxUses: 5 } },
    );
    expect(
      invite.errors,
      "inviting somebody to a table must not need a notice contact",
    ).toBeFalsy();
    const code = invite.data?.generateInviteCode?.inviteCode;
    expect(code).toBeTruthy();

    const playerContext = await browser.newContext();
    const player = await playerContext.newPage();
    try {
      await register(player, freshCredentials("e2egateplay"));
      const joined = await graphql<
        GqlResult<{ joinWorld: { worldId: string } }>
      >(
        player,
        `
          mutation JW($input: JoinWorldInput!) {
            joinWorld(input: $input) {
              worldId
            }
          }
        `,
        { input: { inviteCode: code } },
      );
      expect(
        joined.errors,
        "joining a table must not need a notice contact",
      ).toBeFalsy();
      expect(joined.data?.joinWorld?.worldId).toBe(worldId);

      // Present at the table, which is the plainest statement of "playable".
      const beat = await graphql<GqlResult<{ heartbeat: unknown }>>(
        player,
        `
          mutation HB($worldId: UUID!) {
            heartbeat(worldId: $worldId)
          }
        `,
        { worldId },
      );
      expect(
        beat.errors,
        "being present at a table must not need a notice contact",
      ).toBeFalsy();

      const presence = await graphql<
        GqlResult<{ worldPresence: { userId: string }[] }>
      >(
        page,
        `
          query WP($worldId: UUID!) {
            worldPresence(worldId: $worldId) {
              userId
            }
          }
        `,
        { worldId },
      );
      expect(presence.errors).toBeFalsy();
      expect(
        presence.data?.worldPresence?.length,
        "the invited player must actually be at the table",
      ).toBeGreaterThan(0);
    } finally {
      await playerContext.close();
    }

    await configure();
  });
});
