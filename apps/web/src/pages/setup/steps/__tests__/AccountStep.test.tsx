import { describe, expect, it } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { AccountStep } from "../AccountStep";

/**
 * The provider route into the first administrator is offered only where it
 * can be taken. The e2e stack always starts with providers configured, so
 * the instance without any is proved here.
 */
function render(providers: { provider_key: string; display_name: string }[]) {
  return renderToStaticMarkup(
    <AccountStep
      providers={providers}
      adminCode="CODE"
      created={false}
      onCreated={() => {}}
    />,
  );
}

describe("the account step's provider route", () => {
  it("is not shown when no provider is configured", () => {
    const html = render([]);
    expect(html).toContain("setup-account-submit");
    expect(html).not.toContain("setup-oauth");
    expect(html).not.toContain("provider");
  });

  it("offers each configured provider", () => {
    const html = render([{ provider_key: "discord", display_name: "Discord" }]);
    expect(html).toContain("setup-oauth-username");
    expect(html).toContain("setup-oauth-start-discord");
    expect(html).toContain("Continue with Discord");
  });
});
