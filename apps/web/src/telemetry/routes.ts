/**
 * Page views by route template, never by path (spec 086 FR-016): a world's
 * id, an invite code or a lore slug never leaves in a `route` attribute.
 *
 * `ROUTE_TEMPLATES` mirrors every `path` in `routes/AppRoutes.tsx`, and
 * `__tests__/routes.test.ts` reads that file to prove the two agree.
 */

import { matchPath } from "react-router-dom";

export const ROUTE_TEMPLATES = [
  "/",
  "/setup/:code",
  "/setup",
  "/setup/callback",
  "/login",
  "/register",
  "/signup",
  "/oauth/callback/:providerKey",
  "/invite/:code",
  "/collection/:shareCode",
  "/legal/dmca",
  "/legal/terms",
  "/legal/privacy",
  "/legal/operator",
  "/admin",
  "/admin/welcome",
  "/admin/settings",
  "/admin/configuration",
  "/admin/instance",
  "/admin/readiness",
  "/admin/mail",
  "/admin/legal",
  "/admin/analytics",
  "/admin/storage",
  "/admin/oauth",
  "/admin/system",
  "/admin/access",
  "/admin/security",
  "/admin/moderation",
  "/admin/play-pauses",
  "/welcome",
  "/worlds",
  "/library",
  "/library/:compendiumId",
  "/worlds/create",
  "/world/:id",
  "/world/:id/paused",
  "/world/:id/staging",
  "/world/:id/actor-select",
  "/world/:id/collections",
  "/world/:id/compendium",
  "/world/:id/compendium/npc/new",
  "/world/:id/compendium/npc/:actorId/edit",
  "/world/:id/compendium/item/new",
  "/world/:id/compendium/item/:itemId/edit",
  "/world/:id/scenes",
  "/world/:id/scenes/:sceneId",
  "/world/:id/players",
  "/world/:id/actor/:actorId/view",
  "/world/:id/actor/:actorId/edit",
  "/world/:id/lore/:slug/view",
  "/world/:id/lore/:slug/edit",
  "/world/:id/lore/:slug/history",
  "/shared/actor/:code",
  "/world/:id/ability/:abilityId/view",
  "/world/:id/ability/:abilityId/edit",
  "/world/:id/item/:itemId/view",
  "/world/:id/item/:itemId/edit",
  "/shared/ability/:code",
  "/shared/item/:code",
  "/settings/storage",
  "/settings/feedback",
  "/settings/security",
  "/settings/account",
  "/settings/standing",
  "/world/:id/settings/system",
  "/world/:id/play",
  "/join/:code",
] as const;

/** What a path that matches no route reports. */
export const UNKNOWN_ROUTE = "*";

/**
 * The template a path is served by. A static segment wins over a parameter,
 * as it does in the router (`/worlds/create` is not `/world/:id`).
 */
export function routeTemplate(pathname: string): string {
  let best: string | undefined;
  let bestStatic = -1;
  for (const pattern of ROUTE_TEMPLATES) {
    if (!matchPath({ path: pattern, end: true }, pathname)) continue;
    const statics = pattern
      .split("/")
      .filter((s) => s && !s.startsWith(":")).length;
    if (statics > bestStatic) {
      best = pattern;
      bestStatic = statics;
    }
  }
  return best ?? UNKNOWN_ROUTE;
}

/**
 * Calls `onChange` with the new template whenever the router moves to a
 * different one. The router writes through `history`, so watching it keeps
 * React out of this. A change of query or of a parameter within the same
 * template is not a new page.
 */
export function watchRoutes(
  onChange: (template: string) => void,
  win: Window = window,
): void {
  let current = routeTemplate(win.location.pathname);
  const check = () => {
    const next = routeTemplate(win.location.pathname);
    if (next === current) return;
    current = next;
    onChange(next);
  };
  for (const method of ["pushState", "replaceState"] as const) {
    const original = win.history[method].bind(win.history);
    win.history[method] = (...args: Parameters<History["pushState"]>) => {
      original(...args);
      check();
    };
  }
  win.addEventListener("popstate", check);
}
