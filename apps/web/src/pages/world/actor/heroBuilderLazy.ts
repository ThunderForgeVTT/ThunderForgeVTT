import { lazy } from "react";

/**
 * Spec 044 FR-022: the builder's dialogs, loaded when one is opened.
 *
 * The only way a host reaches them. A static import of either dialog from a
 * page would put every part's drawing in that page's chunk, and the imagery
 * panel and the compendium list are on pages most visits never build from.
 */
export const LazyHeroBuilderDialog = lazy(() => import("./HeroBuilderDialog"));

export const LazyQuickNpcDialog = lazy(
  () => import("@/pages/world/compendium/QuickNpcDialog"),
);
