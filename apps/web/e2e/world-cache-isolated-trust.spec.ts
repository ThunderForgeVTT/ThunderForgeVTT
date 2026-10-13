import { expect, test } from "./fixtures/test";
import { uniqueSuffix } from "./fixtures/helpers";
import {
  closeTrustTable,
  moveCommand,
  newToken,
  ownToken,
  reconcile,
  rollAndRecord,
  serverToken,
  trustTable,
  type QueuedChange,
} from "./fixtures/world-cache-trust";

/**
 * Peer-adjudicated play's trust model (spec 028 Phase 10a, T108–T108c,
 * ADR-052 §4): who may attribute a change to whom, and when a reported
 * outcome counts as a discrepancy. Why these submit to
 * `reconcileQueuedChanges` directly is in `fixtures/world-cache-trust.ts`.
 *
 * `attributedToUserId`, `reportedOutcome` and `discrepancy` are schema, so a
 * server built before T102/T102c refuses the query outright — which is at
 * least an honest error, and the one these tests report.
 */

test.describe("Client world cache — peer-adjudicated play (US7, Phase 10a)", () => {
  test.setTimeout(600_000);

  /**
   * SC-021 / FR-061a. The one thing the trust model refuses.
   *
   * Attribution is made the *only* difference between the accepted and the
   * refused submission, because anything else would prove something weaker.
   * The same command, attributed to the same third person, is refused from a
   * player and accepted from a Game Master — and, so that "refused" cannot be
   * read as "that change was never going to work", the same player's own
   * unattributed submission of the same move is accepted immediately before.
   *
   * That middle step is what makes the negative mean something: without it a
   * server that rejected every replay from a player would pass.
   */
  test("a non-GM may not submit a change attributed to another player, and a GM may (SC-021, T108)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108", 2);
    const [alice, bob] = table.players;
    const [aliceId, bobId] = table.playerIds;

    // Alice owns both tokens, so nothing below can be refused for want of
    // permission to move the thing — only for attribution.
    const own = await newToken(table.gm, table.sceneId);
    const relayed = await newToken(table.gm, table.sceneId);
    await ownToken(table.gm, own, aliceId);
    await ownToken(table.gm, relayed, aliceId);

    // 1. Alice, claiming to relay Bob's change. The whole of FR-061a.
    const impersonation = await reconcile(alice, table.worldId, [
      {
        localId: "as-bob",
        command: moveCommand(relayed, 210, -140),
        attributedToUserId: bobId,
      },
    ]);
    expect(
      impersonation[0].applied,
      "a player may never submit on someone else's behalf",
    ).toBe(false);
    expect(impersonation[0].reason).toBe("PERMISSION_DENIED");
    expect(
      await serverToken(table.gm, table.sceneId, relayed),
      "a refused attribution must leave the token untouched",
    ).toMatchObject({ x: 0, y: 0 });

    // 2. The same player, the same kind of change, her own name on it. This
    //    is the control: the refusal above is about *whose* change it is, not
    //    about the change or the submitter.
    const ownSubmission = await reconcile(alice, table.worldId, [
      { localId: "as-self", command: moveCommand(own, 90, 60) },
    ]);
    expect(
      ownSubmission[0].applied,
      "a player replaying their own change must still be accepted",
    ).toBe(true);
    expect(await serverToken(table.gm, table.sceneId, own)).toMatchObject({
      x: 90,
      y: 60,
    });

    // 3. The identical submission from the Game Master. Same command, same
    //    attribution, different submitter — and that difference is the
    //    entire trust model (ADR-052, "The trust model, stated plainly").
    const relayedByGm = await reconcile(table.gm, table.worldId, [
      {
        localId: "gm-as-bob",
        command: moveCommand(relayed, 210, -140),
        attributedToUserId: bobId,
      },
    ]);
    expect(
      relayedByGm[0].applied,
      "a Game Master relaying a player's peer-adjudicated change is the ordinary case",
    ).toBe(true);
    expect(
      await serverToken(table.gm, table.sceneId, relayed),
      "the relayed change must actually take effect",
    ).toMatchObject({ x: 210, y: -140 });

    await bob.context().close();
    await closeTrustTable(table);
  });

  /**
   * SC-021a / SC-021b / FR-064 to FR-067. A client reports a number the
   * server determined differently.
   *
   * Four things have to be true at once, and three of them are absences:
   * the change is **applied**, the outcome is **unaltered**, both numbers are
   * **inspectable by the GM**, and **nobody else is told**. The last is a
   * requirement rather than a detail — a mark against a player visible to the
   * table is the harm the whole design is arranged to avoid — so it is
   * asserted from the player's own session, where the identical submission
   * must come back with nothing attached.
   */
  test("a reported dice value the server determined differently is applied and disclosed to the GM alone (SC-021a, SC-021b, T108a)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108a", 1);
    const [alice] = table.players;
    const [aliceId] = table.playerIds;

    // A real roll, resolved by the server, which is what makes there be
    // something to disagree with (ADR-044).
    const { recordId, determined } = await rollAndRecord(
      alice,
      table.gm,
      table.worldId,
    );
    const claimed = determined + 13;

    const moved = await newToken(table.gm, table.sceneId);
    const outcome = (
      await reconcile(table.gm, table.worldId, [
        {
          localId: "disputed",
          command: moveCommand(moved, 175, -95),
          attributedToUserId: aliceId,
          reportedOutcome: {
            kind: "dice",
            version: 1,
            recordId,
            value: claimed,
          },
        },
      ])
    )[0];

    // Never auto-rejected, never interrupted (FR-066).
    expect(outcome.applied, "a discrepancy must not reject the change").toBe(
      true,
    );
    expect(
      outcome.reason,
      "nor turn it into a failure of some other kind",
    ).toBeNull();
    // Never altered: the move lands exactly where it was asked to.
    expect(
      await serverToken(table.gm, table.sceneId, moved),
      "the outcome stands as reported",
    ).toMatchObject({ x: 175, y: -95 });

    // Both numbers, inspectable, attributed to the originator rather than to
    // the Game Master who relayed it.
    expect(
      outcome.discrepancy,
      "the GM must be shown the difference",
    ).not.toBeNull();
    expect(outcome.discrepancy!.reportedValue).toBe(claimed);
    expect(outcome.discrepancy!.determinedValue).toBe(determined);
    expect(outcome.discrepancy!.recordId).toBe(recordId);
    expect(
      outcome.discrepancy!.userId,
      "the disclosure names whose outcome it was, not who relayed it",
    ).toBe(aliceId);

    // FR-067, from the other side. The identical submission made by the
    // player herself comes back with nothing attached — the disclosure is
    // computed for a Game Master submitter and for nobody else, so there is
    // no session in which a player can be shown a mark against a player.
    const asPlayer = (
      await reconcile(alice, table.worldId, [
        {
          localId: "disputed-by-player",
          command: moveCommand(moved, 176, -96),
          reportedOutcome: {
            kind: "dice",
            version: 1,
            recordId,
            value: claimed,
          },
        },
      ])
    )[0];
    expect(
      asPlayer.discrepancy,
      "no discrepancy display may ever reach anyone but the Game Master",
    ).toBeNull();

    await closeTrustTable(table);
  });

  /**
   * FR-061b. A Game Master acting on a player's behalf produces no flag and
   * no notification to anyone.
   *
   * The absence is the requirement, so the same test also makes the *presence*
   * happen: an identical relayed change, differing only in that its reported
   * outcome genuinely disagrees with the server, does produce a disclosure.
   * Without that half, a server that had never learned to flag anything at
   * all would pass this test.
   */
  test("a Game Master acting on a player's behalf produces no flag and no notice (FR-061b, T108b)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108b", 1);
    const [alice] = table.players;
    const [aliceId] = table.playerIds;

    const quiet = await newToken(table.gm, table.sceneId);
    const flagged = await newToken(table.gm, table.sceneId);

    // Exactly what a GM adjudicating for a player produces: an attributed
    // token move, with no outcome the server could have determined for itself
    // (FR-068 — most of what this mutation carries).
    const onBehalf = (
      await reconcile(table.gm, table.worldId, [
        {
          localId: "on-behalf",
          command: moveCommand(quiet, 120, 45),
          attributedToUserId: aliceId,
        },
      ])
    )[0];
    expect(
      onBehalf.applied,
      "acting for a player is ordinary and unremarkable",
    ).toBe(true);
    expect(onBehalf.reason).toBeNull();
    expect(
      onBehalf.discrepancy,
      "a GM acting on a player's behalf is not something to flag",
    ).toBeNull();

    // Nor is the player told. The change reaches them as an ordinary token
    // event and nothing else: their own reconcile call — the only channel
    // that carries a verdict — has nothing to report.
    const playerSees = await reconcile(alice, table.worldId, []);
    expect(
      playerSees,
      "the player has nothing owing and hears nothing",
    ).toHaveLength(0);
    expect(
      await serverToken(alice, table.sceneId, quiet),
      "the player simply sees the token where the table put it",
    ).toMatchObject({ x: 120, y: 45 });

    // And the control: the machinery is running, and does flag a genuine
    // mismatch on an otherwise identical relayed change.
    const { recordId, determined } = await rollAndRecord(
      alice,
      table.gm,
      table.worldId,
    );
    const control = (
      await reconcile(table.gm, table.worldId, [
        {
          localId: "on-behalf-mismatch",
          command: moveCommand(flagged, 121, 46),
          attributedToUserId: aliceId,
          reportedOutcome: {
            kind: "dice",
            version: 1,
            recordId,
            value: determined + 5,
          },
        },
      ])
    )[0];
    expect(
      control.discrepancy,
      "the same relay with a genuinely different number is disclosed — so the silence above is a decision, not a gap",
    ).not.toBeNull();

    await closeTrustTable(table);
  });

  /**
   * SC-021c / FR-067a. Every ambiguity is silence.
   *
   * This is the test that matters more than T108a, and the reason is not
   * symmetry: a missed discrepancy costs nothing, because the GM runs their
   * table either way, while a false one puts an innocent player under
   * suspicion in front of the only person who can act on it. So each of these
   * cases must produce *no* disclosure rather than a plausible-looking one.
   *
   * The genuine mismatch is submitted first, from the same session against the
   * same record, so every `toBeNull` below is measured against a detector that
   * has just been seen working.
   *
   * One ambiguity is not reachable from here: a **database failure or
   * statement timeout** while reading the determined value. There is no way to
   * induce one through a signed-in session, and contriving one would mean
   * building a failure mode into the server to test it. It is covered where it
   * can be — `determination_from_lookup` in `mutations_reconcile.rs` maps an
   * `Err` and an absent row to the same `None`, which is the same code path
   * the "unknown record" case below takes.
   */
  test("timeout, parse failure, version mismatch and missing determination each produce no discrepancy (SC-021c, T108c)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108c", 1);
    const [alice] = table.players;
    const [aliceId] = table.playerIds;

    const { recordId, determined } = await rollAndRecord(
      alice,
      table.gm,
      table.worldId,
    );
    const claimed = determined + 11;

    // A separate world the Game Master also owns, for the "the record is not
    // one of ours" case — the lookup is scoped by world, so a real record id
    // from elsewhere must read as no such row rather than as a value to
    // compare against.
    const otherWorldId = await (async () => {
      await table.gm.goto("/worlds/create");
      await table.gm
        .locator("#world-name")
        .fill(`E2E Trust Other ${uniqueSuffix()}`);
      await table.gm.getByRole("button", { name: /create world/i }).click();
      await table.gm.waitForURL(/\/world\/[^/]+\/staging$/, {
        timeout: 20_000,
      });
      const match = /\/world\/([^/]+)\/staging$/.exec(
        new URL(table.gm.url()).pathname,
      );
      return match![1];
    })();

    const token = async () => newToken(table.gm, table.sceneId);
    const submit = async (
      localId: string,
      reportedOutcome: QueuedChange["reportedOutcome"],
    ) =>
      (
        await reconcile(table.gm, table.worldId, [
          {
            localId,
            command: moveCommand(await token(), 30, 30),
            attributedToUserId: aliceId,
            reportedOutcome,
          },
        ])
      )[0];

    // The positive control, first. Everything after this is the same session,
    // the same record and the same numbers, changed one field at a time.
    const genuine = await submit("genuine", {
      kind: "dice",
      version: 1,
      recordId,
      value: claimed,
    });
    expect(
      genuine.discrepancy,
      "a genuine determined-value mismatch is the one case that speaks",
    ).not.toBeNull();

    const ambiguities: [string, QueuedChange["reportedOutcome"], string][] = [
      [
        "version",
        { kind: "dice", version: 2, recordId, value: claimed },
        "a client one release ahead is telling us something we cannot read",
      ],
      [
        "kind",
        { kind: "initiative", version: 1, recordId, value: claimed },
        "an outcome the server has no independent basis for is not a disagreement",
      ],
      [
        "no-value",
        { kind: "dice", version: 1, recordId, value: null },
        "half a report is not a report",
      ],
      [
        "no-record",
        { kind: "dice", version: 1, recordId: null, value: claimed },
        "a value with nothing to compare it against determines nothing",
      ],
      [
        "unknown-record",
        {
          kind: "dice",
          version: 1,
          recordId: "00000000-0000-4000-8000-000000000000",
          value: claimed,
        },
        "a record that is not there is an absent determination, not a differing one",
      ],
    ];

    for (const [name, reported, why] of ambiguities) {
      const outcome = await submit(`ambiguous-${name}`, reported);
      expect(outcome.discrepancy, why).toBeNull();
      expect(
        outcome.applied,
        "and an ambiguity must not cost the change either — silence is not refusal",
      ).toBe(true);
    }

    // The same shape again, with a record that exists but belongs to another
    // world. The lookup is scoped, so this is an absent determination — and
    // it is the case a scoping mistake would turn into a fabricated mismatch.
    const elsewhere = await rollAndRecord(table.gm, table.gm, otherWorldId);
    const crossWorld = await submit("ambiguous-other-world", {
      kind: "dice",
      version: 1,
      recordId: elsewhere.recordId,
      value: claimed,
    });
    expect(
      crossWorld.discrepancy,
      "a record from another world must read as no such row",
    ).toBeNull();

    await closeTrustTable(table);
  });
});
