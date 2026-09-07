import { expect, test, type Page } from "@playwright/test";
import {
  clickPlay,
  graphql,
  inviteAndJoinAsPlayer,
  openDockTab,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * The Chat panel, driven through its own UI.
 *
 * # Why this spec exists
 *
 * `sendChatMessage` is called all over this suite — the torture specs and
 * `world-event-catchup.spec.ts` post chat because it is the cheapest way to
 * make a `world_events` row appear. Every one of those calls goes straight to
 * GraphQL. Nothing had ever typed into the composer, clicked Send, or looked
 * at what a second member's browser then renders, and all five of
 * `ChatPanel.tsx`'s test ids (`chat-panel`, `chat-input`, `chat-send-button`,
 * `chat-message`, `chat-gm-only-toggle`) were unreferenced.
 *
 * # What is covered
 *
 * 1. A GM types, sends with the button, sends again with Enter (the
 *    composer's `onKeyDown` shortcut), and both render.
 * 2. Both survive a reload — the log is persisted in `world_chat_messages`,
 *    so a fresh page load must show it and not an empty panel.
 * 3. A player in the same world sees the GM's message arrive *live*, with no
 *    reload: `ChatPanel` refetches on `world_events` code 17
 *    (`engine/world/sync/playPanels.ts`).
 * 4. A GM-only message reaches the GM and not the player.
 *
 * # Why the GM-only assertion is shaped the way it is
 *
 * `mutations_chat.rs` filters `gm_only` rows out of `world_chat_messages`
 * for non-GM callers, and the broadcast payload carries only a message id
 * precisely so the body never rides the world channel past the filter. That
 * is a claim about what the player's client *has*, not about what it paints,
 * so "it isn't on screen" would be the weak half of the test: a panel that
 * received the secret and merely styled it away would pass. This asserts the
 * player's own authenticated `worldChatMessages` query does not contain the
 * body at all, and separately that the DOM does not either.
 *
 * The absence is also made non-vacuous. Before checking, the player *sends
 * their own message through the composer* — `handleSend` awaits `refresh()`,
 * so the panel's contents are the server's answer as of a moment strictly
 * after the secret was posted. Without that barrier, "the secret is absent"
 * and "the refetch has not happened yet" look identical.
 */

/** Wait for the play view to be up enough to open the dock on. */
async function waitForPlayView(page: Page): Promise<void> {
  await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });
  // `ConnectionStatus` renders nothing once the socket is live, so this is
  // "the subscription came up", not "the banner exists". Same reading as
  // world-event-catchup.spec.ts.
  await expect(page.getByTestId("live-sync-reconnecting-indicator")).toBeHidden(
    {
      timeout: 30_000,
    },
  );
}

async function openChat(page: Page): Promise<void> {
  await openDockTab(page, "chat");
  await expect(page.getByTestId("chat-panel")).toBeVisible({ timeout: 15_000 });
}

/** Type `body` into the composer and send it with the Send button. */
async function sendViaButton(page: Page, body: string): Promise<void> {
  await page.getByTestId("chat-input").fill(body);
  await page.getByTestId("chat-send-button").click();
  // The composer clears only after the mutation resolves, so an empty input
  // is the send having actually succeeded rather than merely been clicked.
  await expect(page.getByTestId("chat-input")).toHaveValue("", {
    timeout: 20_000,
  });
}

function messageWithText(page: Page, body: string) {
  return page.getByTestId("chat-message").filter({ hasText: body });
}

test.describe("Play view chat panel", () => {
  test("a GM sends messages from the composer and they survive a reload", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    const suffix = uniqueSuffix();
    const typed = `Typed and clicked ${suffix}`;
    const pressed = `Sent with Enter ${suffix}`;

    await registerAndCreateWorld(page, `E2E Chat ${suffix}`, "e2echatgm");
    await clickPlay(page);
    await waitForPlayView(page);
    await openChat(page);

    // An empty world's chat says so — this is the state the first message has
    // to replace, and asserting it here means the assertions below cannot be
    // satisfied by a panel that renders everything it is ever given.
    await expect(page.getByText("No messages yet.")).toBeVisible({
      timeout: 15_000,
    });

    await sendViaButton(page, typed);
    await expect(messageWithText(page, typed)).toHaveCount(1, {
      timeout: 20_000,
    });

    // Enter sends, per the composer's own `onKeyDown` (Shift+Enter would
    // insert a newline instead).
    await page.getByTestId("chat-input").fill(pressed);
    await page.getByTestId("chat-input").press("Enter");
    await expect(page.getByTestId("chat-input")).toHaveValue("", {
      timeout: 20_000,
    });
    await expect(messageWithText(page, pressed)).toHaveCount(1, {
      timeout: 20_000,
    });

    // Persisted, not just held in React state: `world_chat_messages` is a
    // table and `getWorldChatMessages` is the panel's first effect.
    await page.reload();
    await waitForPlayView(page);
    await openChat(page);

    await expect(messageWithText(page, typed)).toHaveCount(1, {
      timeout: 20_000,
    });
    await expect(messageWithText(page, pressed)).toHaveCount(1, {
      timeout: 20_000,
    });

    // Oldest-first, as `world_chat_messages_impl` reverses its newest-first
    // query to produce. A reload that restored the log backwards would
    // otherwise pass everything above.
    const bodies = await page.getByTestId("chat-message").allInnerTexts();
    const typedIndex = bodies.findIndex((text) => text.includes(typed));
    const pressedIndex = bodies.findIndex((text) => text.includes(pressed));
    expect(
      typedIndex,
      "the backscroll should be oldest-first after a reload",
    ).toBeLessThan(pressedIndex);
  });

  test("a player sees a GM's message live, and never receives a GM-only one", async ({
    page,
    browser,
  }) => {
    test.setTimeout(240_000);

    const suffix = uniqueSuffix();
    const publicBody = `Everyone hears this ${suffix}`;
    const secretBody = `Only the GM hears this ${suffix}`;
    const playerBody = `The player answers ${suffix}`;

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Chat Two Clients ${suffix}`,
      "e2echatgm",
    );
    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2echatplayer",
    );

    try {
      await clickPlay(page);
      await waitForPlayView(page);
      await openChat(page);

      await playerPage.goto(`/world/${worldId}/play`);
      await waitForPlayView(playerPage);
      await openChat(playerPage);

      // Only the GM composes GM-only messages, and the server agrees: a
      // non-GM asking for `gmOnly: true` is rejected outright rather than
      // downgraded (see `send_chat_message_impl`). The absent control is the
      // UI half of that rule.
      await expect(page.getByTestId("chat-gm-only-toggle")).toBeVisible({
        timeout: 15_000,
      });
      await expect(playerPage.getByTestId("chat-gm-only-toggle")).toHaveCount(
        0,
      );

      // The socket being live is not the same as the panel's subscription
      // being open — the play view mounts several of them and each opens as
      // it renders. An event published in that window reaches nobody, which
      // would look exactly like live delivery being broken. Same measured
      // settle as world-event-catchup.spec.ts.
      await playerPage.waitForTimeout(6_000);

      // --- live delivery, no reload ---
      await sendViaButton(page, publicBody);
      await expect(messageWithText(page, publicBody)).toHaveCount(1, {
        timeout: 20_000,
      });
      await expect(messageWithText(playerPage, publicBody)).toHaveCount(1, {
        timeout: 60_000,
      });

      // --- the GM-only message ---
      await page.getByTestId("chat-gm-only-toggle").check();
      await sendViaButton(page, secretBody);

      const secretOnGm = messageWithText(page, secretBody);
      await expect(secretOnGm).toHaveCount(1, { timeout: 20_000 });
      // The badge the server's `gmOnly` flag drives — the GM must be able to
      // tell a whisper from something the table heard.
      await expect(secretOnGm).toContainText("GM only");

      // The toggle is sticky by design (nothing resets it), so it is cleared
      // by hand before the next public message.
      await page.getByTestId("chat-gm-only-toggle").uncheck();

      // --- the barrier ---
      //
      // The player sends through the composer; `handleSend` awaits its own
      // `refresh()`, so once this message is on screen the panel is showing
      // the server's answer from strictly after the secret was written. Any
      // absence observed below is therefore a real absence and not a refetch
      // that has yet to happen.
      await sendViaButton(playerPage, playerBody);
      await expect(messageWithText(playerPage, playerBody)).toHaveCount(1, {
        timeout: 20_000,
      });

      await expect(messageWithText(playerPage, secretBody)).toHaveCount(0);
      await expect(playerPage.getByTestId("chat-panel")).not.toContainText(
        secretBody,
      );

      // And the stronger claim: the player's client never had it. This is the
      // same authenticated session the panel fetches with, asked directly, so
      // a panel that received the secret and merely hid it would fail here.
      const asPlayer = await graphql<{
        data?: { worldChatMessages?: { body: string; gmOnly: boolean }[] };
        errors?: { message: string }[];
      }>(
        playerPage,
        `
          query ($worldId: UUID!) {
            worldChatMessages(worldId: $worldId) {
              body
              gmOnly
            }
          }
        `,
        { worldId },
      );
      expect(
        asPlayer.errors,
        `worldChatMessages failed for the player: ${JSON.stringify(asPlayer.errors)}`,
      ).toBe(undefined);
      const playerBodies = (asPlayer.data?.worldChatMessages ?? []).map(
        (message) => message.body,
      );
      expect(
        playerBodies,
        "a player's own chat query must not return a GM-only message",
      ).not.toContain(secretBody);
      expect(
        playerBodies,
        "the player should still have the public messages",
      ).toContain(publicBody);
      expect(
        (asPlayer.data?.worldChatMessages ?? []).some(
          (message) => message.gmOnly,
        ),
        "no message a player receives may be flagged GM-only",
      ).toBe(false);

      // A non-GM cannot post one either — the server rejects rather than
      // silently downgrading, so a confused client never publishes to the
      // table something its sender believed was private.
      const attempt = await graphql<{
        data?: { sendChatMessage?: { id: string } };
        errors?: { message: string }[];
      }>(
        playerPage,
        `
          mutation ($input: SendChatMessageInput!) {
            sendChatMessage(input: $input) {
              id
            }
          }
        `,
        {
          input: {
            worldId,
            body: `A player should not be able to whisper ${suffix}`,
            gmOnly: true,
          },
        },
      );
      expect(
        attempt.errors?.[0]?.message,
        "a non-GM asking for gmOnly must be refused, not downgraded",
      ).toContain("Only the GM");
      expect(attempt.data?.sendChatMessage).toBeFalsy();

      // The GM's own client, meanwhile, hears the player live.
      await expect(messageWithText(page, playerBody)).toHaveCount(1, {
        timeout: 60_000,
      });
    } finally {
      await playerPage.context().close();
    }
  });
});
