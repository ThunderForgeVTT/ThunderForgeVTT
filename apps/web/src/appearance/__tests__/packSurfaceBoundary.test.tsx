import { describe, expect, it } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { PackSurfaceBoundary, PackSurfaceFailed } from "../PackSurfaceBoundary";

/**
 * Spec 032 T103 — FR-016 and SC-009.
 *
 * SC-009 measures two things and they are proved in two places, on purpose.
 * *The rest of the session stays usable* is a claim about a whole running
 * application; it is proved end-to-end in a browser by navigating after an
 * injected failure, because a simulated version of that claim would be worth
 * very little. *The message names the responsible pack* is a claim about this
 * component, and is proved here.
 *
 * The message is rendered directly rather than by throwing at the boundary:
 * React's server renderer does not run `getDerivedStateFromError`, so an
 * error thrown at `renderToStaticMarkup` propagates instead of being caught.
 * This app has neither jsdom nor testing-library — see `sheetLayout.test.tsx`
 * for why that is not being changed here — so the catching half is asserted
 * through the static method that does the catching, and the message half
 * through the component that does the telling.
 */
describe("the boundary's decision to catch", () => {
  it("marks itself failed when a surface throws", () => {
    expect(PackSurfaceBoundary.getDerivedStateFromError()).toEqual({
      failed: true,
    });
  });
});

describe("what a participant reads when a pack surface fails", () => {
  const render = (packId: string, surface: string) =>
    renderToStaticMarkup(
      <PackSurfaceFailed packId={packId} surface={surface} />,
    );

  it("names the responsible pack rather than saying something went wrong", () => {
    const html = render("forged-steel", "character sheet");

    expect(html).toContain("forged-steel");
    expect(html).toContain("character sheet");
  });

  it("reports the pack it was told is rendering, not a fixed one", () => {
    // The distinction that matters when a world's chosen pack is missing: the
    // base pack has fallen back into place, and the base pack is what threw.
    const html = render("forge", "item list");

    expect(html).toContain("forge");
    expect(html).toContain("item list");
    expect(html).not.toContain("forged-steel");
  });

  it("says the rest of the session is unaffected", () => {
    expect(render("forge", "character sheet")).toContain(
      "Nothing else in this session is affected",
    );
  });

  it("blocks nothing — it is a notice, not a barrier", () => {
    // `MissingPackNotice`'s precedent: a surface that failed is not a reason
    // to make someone dismiss something before carrying on.
    expect(render("forge", "character sheet")).not.toContain("<button");
  });

  it("is announced, so a participant using a screen reader is told too", () => {
    expect(render("forge", "character sheet")).toContain('role="alert"');
  });

  it("carries the pack id as data, so a test can assert which pack failed", () => {
    expect(render("forged-steel", "character sheet")).toContain(
      'data-pack="forged-steel"',
    );
  });
});
