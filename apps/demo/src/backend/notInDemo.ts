/**
 * Spec 074 FR-009: the one answer for anything the demo does not do.
 *
 * Whatever asked — an operation with no handler, an address nothing here
 * serves, a socket to somewhere — is told so here, and the page's standing
 * notice turns it into something the visitor can read.
 */
export const NOT_IN_DEMO_EVENT = "thunderforge-demo:not-in-demo";
export const NOT_IN_DEMO_CODE = "NOT_IN_DEMO";

/** Everything refused so far, for a test to read. */
export const refused: string[] = [];

export function reportNotInDemo(what: string): void {
  refused.push(what);
  window.dispatchEvent(new CustomEvent(NOT_IN_DEMO_EVENT, { detail: what }));
}

declare global {
  interface Window {
    __thunderforgeDemoRefused?: string[];
  }
}
window.__thunderforgeDemoRefused = refused;
