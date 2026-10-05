import { createElement, lazy, type ComponentType, type ReactNode } from "react";
import { LazyBoundary } from "./LazyBoundary";

/**
 * A component whose code arrives the first time it is rendered (spec 068).
 *
 * What comes back is an ordinary component: it can be kept in a registry,
 * handed to `createElement`, and mounted by a caller that neither knows nor
 * cares that it was not in the bundle it started from. The wait and the
 * failure are both handled here, inside a `LazyBoundary`.
 *
 * Call it once per component, at module level. A new one per render is a new
 * component type per render, and React remounts it every time.
 */
export function onDemand<P extends object>(
  load: () => Promise<{ default: ComponentType<P> }>,
  what: string,
  fallback: ReactNode = null,
): ComponentType<P> {
  const Loaded = lazy(load) as unknown as ComponentType<P>;
  function OnDemand(props: P) {
    return createElement(LazyBoundary, {
      what,
      fallback,
      children: createElement(Loaded, props),
    });
  }
  return OnDemand;
}
