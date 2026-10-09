/**
 * Global Privacy Control and Do Not Track (FR-018). Either limits a session
 * to errors: no events, no spans, and no sampling decision.
 */

export interface Privacy {
  gpc: boolean;
  dnt: boolean;
}

interface PrivacyNavigator {
  globalPrivacyControl?: unknown;
  doNotTrack?: unknown;
}

export function privacyOf(nav: PrivacyNavigator | undefined): Privacy {
  return {
    gpc: nav?.globalPrivacyControl === true,
    dnt: nav?.doNotTrack === "1",
  };
}

export function errorsOnly(p: Privacy): boolean {
  return p.gpc || p.dnt;
}
