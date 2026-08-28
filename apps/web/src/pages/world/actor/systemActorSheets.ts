/**
 * apps/web/src/pages/world/actor/systemActorSheets.ts
 *
 * `systemId -> ActorSheet` dispatch registry, so `ActorDetailPage.tsx`
 * doesn't need a new hardcoded `actor.gameSystemId === "<pack>"` branch
 * every time a system pack ships sheet UI — the whole point of a system
 * pack being a pure extension on a shared foundation is that adding one
 * doesn't require editing this app's core pages.
 *
 * Deliberately NOT routed through `GameSystemContext`'s manifest loader:
 * that context loads a manifest's *presentational* `components.CharacterSheet`
 * (a plain, props-in component with no data-fetching of its own) plus
 * calculators — it has no concept of a data-connected container. Each
 * entry here IS that container: it owns the system-specific data-fetching/
 * mutation wiring (e.g. `GenieActorSheet`'s `useActorSystemData`/
 * `useUpdateTraitData` calls, `trait_data.level`/`calculateMaxWishPoints`
 * handling) and renders the manifest's presentational component itself.
 * Only the *mounting decision* — which container to render for a given
 * actor — is generic; each container's internals are, and should stay,
 * system-specific.
 */

import type { ComponentType } from "react";
import { GenieActorSheet } from "./GenieActorSheet";
import type { WorldActorRecord } from "@/types/actor";

export interface ActorSheetProps {
  actor: WorldActorRecord;
  canEdit: boolean;
}

export const SYSTEM_ACTOR_SHEETS: Record<
  string,
  ComponentType<ActorSheetProps>
> = {
  genie: GenieActorSheet,
};
