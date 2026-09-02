import { describe, expect, it, vi } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { EffectHelperRow } from "../EffectHelperRow";
import type { EffectHelper } from "../effectHelpers";

/**
 * Spec 031 FR-011/FR-028 — what a Game Master sees before opening anything.
 *
 * # Why this renders to markup
 *
 * `apps/web` has neither jsdom nor testing-library, and adding either means a
 * lockfile change shared with every other workstream. The row is a pure
 * function of its props, so server-rendering it observes exactly what a
 * viewer would see; the click path is reached by invoking the handler, which
 * is what a click does. `StatusPanel`'s tests set this precedent.
 *
 * The assertions are about what a person perceives: whether a button for a
 * kind exists at all, whether a lore marker is drawn as a book, and whether
 * an effect from a subsystem this file has never heard of still gets a
 * button. That last one is the whole claim — a helper row that only works for
 * effects somebody remembered to add an icon for is a hard-coded list wearing
 * a registry's clothes.
 */

function helper(id: string, label: string): EffectHelper {
  const dot = id.indexOf(".");
  return {
    id,
    label,
    description: `${label}, at length.`,
    namespace: dot === -1 ? id : id.slice(0, dot),
  };
}

function markup(props: Partial<React.ComponentProps<typeof EffectHelperRow>>) {
  return renderToStaticMarkup(
    <EffectHelperRow
      helpers={props.helpers ?? []}
      selectedId={props.selectedId ?? null}
      onChoose={props.onChoose ?? (() => {})}
    />,
  );
}

describe("EffectHelperRow", () => {
  it("draws a lore helper as a book", () => {
    // FR-012's promise, on the surface that places it: the thing that becomes
    // a book on the map is offered as a book here. `lucide-react` renders its
    // icon name into the markup, which is the only handle a DOM-less test has
    // on which glyph was drawn — and the only thing about it worth asserting.
    const html = markup({ helpers: [helper("lore.open", "Open a lore page")] });
    expect(html).toContain("interaction-helper-lore.open");
    expect(html).toContain("book-open");
  });

  it("gives an effect from an unknown subsystem a button anyway", () => {
    // The failure this forbids: a build contributes `audio.play`, nobody
    // updates an icon map, and the GM is offered nothing — indistinguishable
    // from an effect that does not exist. Same call as the interaction
    // marker's grey badge for an uncoloured namespace.
    const html = markup({ helpers: [helper("audio.play", "Play a sound")] });
    expect(html).toContain("interaction-helper-audio.play");
    expect(html).toContain("Play a sound");
  });

  it("shows which kind is chosen", () => {
    const html = markup({
      helpers: [
        helper("lore.open", "Open a lore page"),
        helper("item.take", "Pick up"),
      ],
      selectedId: "lore.open",
    });
    // Pressed state rather than colour alone: the row has to survive being
    // read by somebody who cannot see the variant.
    expect(html.match(/aria-pressed="true"/g)).toHaveLength(1);
  });

  it("always offers a way back to scenery", () => {
    const html = markup({ helpers: [helper("lore.open", "Open a lore page")] });
    expect(html).toContain("interaction-helper-none");
  });

  it("draws nothing at all when the build contributes nothing", () => {
    // The panel says so in words, once. A second empty state here would be
    // two messages about one situation.
    expect(markup({ helpers: [] })).toBe("");
  });

  it("reports the kind that was clicked, and null for scenery", () => {
    const chosen = vi.fn();
    const row = EffectHelperRow({
      helpers: [helper("lore.open", "Open a lore page")],
      selectedId: null,
      onChoose: chosen,
    });

    // Invoking the handler the element carries is what a click does. Reached
    // through the returned tree rather than a DOM, for the reason in the
    // module note above; the markup assertions prove the same controls are
    // what a viewer is actually shown.
    click(row, "interaction-helper-lore.open");
    click(row, "interaction-helper-none");

    expect(chosen.mock.calls).toEqual([["lore.open"], [null]]);
  });
});

/** Find the element carrying `testId` anywhere in a tree and fire its click. */
function click(tree: React.ReactNode, testId: string): void {
  const found = find(tree, testId);
  if (!found) {
    throw new Error(`no element with data-testid="${testId}"`);
  }
  (found.props as { onClick?: () => void }).onClick?.();
}

function find(
  node: React.ReactNode,
  testId: string,
): React.ReactElement | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const hit = find(child, testId);
      if (hit) return hit;
    }
    return null;
  }
  if (!React.isValidElement(node)) {
    return null;
  }
  const props = node.props as {
    "data-testid"?: string;
    children?: React.ReactNode;
  };
  if (props["data-testid"] === testId) {
    return node;
  }
  return find(props.children, testId);
}
