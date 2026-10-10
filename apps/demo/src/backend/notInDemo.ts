/**
 * Spec 074 FR-009: the one answer for anything the demo does not do.
 *
 * Whatever asked — an operation with no handler, an address nothing here
 * serves, a socket to somewhere — is told so here, and the page's standing
 * notice turns it into something the visitor can read.
 */
import { refusalArea } from "./refusalNames";
import { tapNotInDemo } from "./telemetryTap";

export const NOT_IN_DEMO_EVENT = "thunderforge-demo:not-in-demo";
export const NOT_IN_DEMO_CODE = "NOT_IN_DEMO";

/** Everything refused so far, for a test to read. */
export const refused: string[] = [];

/**
 * The error message a refusal carries, which a page may show in its own
 * status line: the area by name, never the field that asked for it.
 */
export function notInDemoMessage(what: string): string {
  return `${refusalArea(what)} is not part of the demo.`;
}

export function reportNotInDemo(what: string): void {
  refused.push(what);
  tapNotInDemo(what);
  window.dispatchEvent(new CustomEvent(NOT_IN_DEMO_EVENT, { detail: what }));
}

declare global {
  interface Window {
    __thunderforgeDemoRefused?: string[];
  }
}
window.__thunderforgeDemoRefused = refused;
