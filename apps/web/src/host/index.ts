/**
 * `@thunderforge/host` — the surface a bundled system pack's web code may
 * import from this application.
 *
 * # Why this file exists
 *
 * ADR-029 permits a **bundled** pack to contribute behaviour: its code is in
 * this repository, reviewed here, and compiled into the product, so it carries
 * the same trust as any other file. What that ADR does not settle is the
 * narrower question this file answers — *what may a pack reach for?*
 *
 * Without an answer, the honest options were both bad. A pack could import
 * `@/anything`, which makes every internal module a de-facto public API that
 * cannot be renamed without breaking packs. Or a pack could import nothing, in
 * which case its data-connected containers have to live in `apps/web`, which is
 * how `systemActorSheets.ts` ended up holding `{ genie: GenieActorSheet }` —
 * the last place shared web code named a game system.
 *
 * So: an explicit, deliberately small list. A pack imports from here or it
 * imports from nowhere. Adding to it is a decision someone makes on purpose,
 * in a diff, rather than something that happens by autocomplete.
 *
 * # What belongs here
 *
 * Things every system needs and no system should reimplement: reading and
 * writing an actor's system data, and the presentational primitives that make
 * a pack's sheet look like the rest of the application rather than like a
 * guest in it. That second half matters more than it sounds — a pack drawing
 * its own card border is how a product stops looking like one product.
 *
 * # What does not
 *
 * Routing, authentication, the world store, the engine bridge, anything
 * holding a session credential. A pack that needs one of those is describing a
 * capability boundary, and ADR-029 is explicit that no such boundary exists in
 * this product yet. The answer is to widen a declaration format, not this file.
 *
 * # Stability
 *
 * Treat every export here as public API. Renaming one means updating the packs
 * that use it in the same commit — which is possible precisely because they all
 * live in this repository, and is exactly the cost ADR-029 accepted when it
 * ruled that only bundled packs may contribute behaviour.
 */

export { Card } from "@/components/ui/card/Card";
export type { CardProps } from "@/components/ui/card/Card";

export { useActorSystemData } from "@/hooks/useActorSystemData";
export type {
  ActorSystemData,
  UseActorSystemDataResult,
} from "@/hooks/useActorSystemData";

export {
  useUpdateActorData,
  useUpdateAbilityData,
  useUpdateProficiencyData,
  useUpdateResourceData,
  useUpdateTraitData,
  useUpdateSpellData,
} from "@/hooks/useUpdateActorData";
export type { UseUpdateActorDataResult } from "@/hooks/useUpdateActorData";

export {
  fetchActorSystemData,
  updateActorSystemData,
} from "@/api/actorSystemData";
export type {
  ActorSystemDataRecord,
  ActorSystemDataType,
} from "@/api/actorSystemData";

export type { WorldActorRecord } from "@/types/actor";

/**
 * The props every pack-contributed actor sheet receives.
 *
 * Declared here rather than in `systemActorSheets.ts` because it is the
 * contract *between* the host and a pack, and a contract that lives on only
 * one side of a boundary drifts toward that side.
 */
export interface ActorSheetProps {
  actor: import("@/types/actor").WorldActorRecord;
  canEdit: boolean;
}
