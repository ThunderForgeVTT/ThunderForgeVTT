/**
 * The instance's own links, in one list.
 *
 * Two surfaces render these: the footer, on every shell that has room for one,
 * and the About dialog on the play field, which has none. Declaring them once
 * is what stops the play field's copy from drifting into a shorter, staler
 * version of the footer's — the failure spec 031 fixed for the admin nav by
 * making its sections data rather than markup.
 *
 * Spec 039 FR-056 is why the legal group is not optional anywhere: the contact
 * for copyright notices must be discoverable by anyone who needs to file one,
 * without an account. A person at a table is as entitled to find it as a
 * person on the marketing page.
 */

export interface SiteLink {
  label: string;
  to: string;
}

export interface SiteLinkGroup {
  heading: string;
  testId: string;
  links: SiteLink[];
}

export const SITE_LINK_GROUPS: readonly SiteLinkGroup[] = [
  {
    heading: "This instance",
    testId: "footer-instance-links",
    links: [{ label: "System status", to: "/status" }],
  },
  {
    heading: "Legal",
    testId: "footer-legal-links",
    links: [
      { label: "Terms of service", to: "/legal/terms" },
      { label: "Privacy policy", to: "/legal/privacy" },
      // Named for what a person is trying to do, not for the statute. Somebody
      // reporting stolen artwork is not searching for "DMCA".
      { label: "Copyright notices", to: "/legal/dmca" },
      // Spec 039 FR-045/FR-046: who runs this instance, and what that makes
      // them responsible for.
      { label: "Operator responsibilities", to: "/legal/operator" },
    ],
  },
];

/**
 * The demo (spec 074): a world anybody can open and run, kept in their own
 * browser.
 *
 * Not in the list above, for two reasons. It is drawn only on an instance
 * that offers it (`FEATURE_DEMO`), which a constant cannot know. And it is
 * not a route of this app: `/demo/` is a second client served beside this
 * one, so it is an `href` and a page load, never a router `to`.
 */
export const DEMO_LINK: SiteLink = {
  label: "Enter demo workspace",
  to: "/demo/",
};

/**
 * The groups worth showing to somebody who is mid-session.
 *
 * The demo is never among them: offering it to a person already at a table
 * is at best noise and at worst a way to lose the game they are in.
 */
export const IN_SESSION_LINK_GROUPS: readonly SiteLinkGroup[] =
  SITE_LINK_GROUPS;
