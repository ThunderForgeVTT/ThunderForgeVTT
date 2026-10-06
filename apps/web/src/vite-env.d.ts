/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SITE_URL?: string;
  readonly VITE_DEFAULT_OG_IMAGE?: string;
  /**
   * Spec 037 (FR-009): the build a feedback submission was sent from.
   *
   * Read rather than `define`d so that no build-configuration change is needed
   * for the value to be *readable*; until something sets it, the client
   * honestly reports "unknown build" instead of inventing one. research.md
   * § R12 wants the server's own version recorded beside it.
   */
  readonly VITE_APP_VERSION?: string;
  /**
   * Spec 076 FR-014: "1" installs `window.__engineProbe` in a production
   * build, for the demo's e2e, which has no development build to test. Not
   * set by any shipped build.
   */
  readonly VITE_ENGINE_PROBE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare module "*.module.scss" {
  const classes: Record<string, string>;
  export default classes;
}

declare module "*.svg" {
  const source: string;
  export default source;
}
