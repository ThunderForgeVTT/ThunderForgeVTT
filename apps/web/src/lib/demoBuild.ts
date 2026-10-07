/**
 * Spec 074 FR-013: whether this bundle is the in-browser demo.
 *
 * Always false here. The demo build (`apps/demo/vite.config.mts`) aliases this
 * module to one that says true, so a control whose whole purpose is a real
 * instance (an invite link, signing out, deleting an account) can step aside
 * in the demo instead of being offered and then refused. It is a build-time
 * fact about the bundle, not a feature flag: the app never runs as both.
 */
export const IN_DEMO = false;
