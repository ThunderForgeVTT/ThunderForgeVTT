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
 * Three things, and the third arrived later than the other two.
 *
 * **An actor's system data.** Reading and writing it is what every system
 * needs and no system should reimplement.
 *
 * **Presentational primitives.** The card, panel, button and badge that make a
 * pack's surfaces look like the rest of the application rather than like a
 * guest in it. That matters more than it sounds — a pack drawing its own card
 * border is how a product stops looking like one product.
 *
 * **A way to talk to the pack's own server half** — `postGraphQL` and
 * `subscribeToWorldEvents`. These are the widest exports here and they are not
 * an oversight; ADR-063 gave a pack its own tables and its own GraphQL, and a
 * pack that owns a schema it may not call is not a boundary, it is a pack
 * whose two halves have to meet in `apps/web`. Their full reasoning sits
 * beside the exports themselves, and it is worth reading before adding a
 * fourth category.
 *
 * # What does not
 *
 * Routing, authentication, the world store, the engine bridge, anything
 * holding a session credential. A pack that needs one of those is describing a
 * capability boundary, and ADR-029 is explicit that no such boundary exists in
 * this product yet. The answer is to widen a declaration format, not this file.
 *
 * **That sentence survived `032/T108`, which widened this file — so it is
 * worth saying exactly why the widening was not a capability boundary.** A
 * capability is authority a pack would not otherwise have. `postGraphQL` is
 * the same transport, endpoint and credentials every other caller in this app
 * uses, and every field it reaches is authorized on the server per request; a
 * pack gains no reach, only the ability to keep its client code next to its
 * server code. `subscribeToWorldEvents` hands over the same undifferentiated
 * NOTIFY stream every other consumer gets, which a pack filters by its own
 * event code. Neither lets a pack do something the application would refuse
 * to do on its behalf. The list above is the test: if an export would let a
 * pack act with authority the current user lacks, it belongs on that list and
 * not in this file.
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

export { Panel } from "@/components/ui/panel/Panel";
export type { PanelProps } from "@/components/ui/panel/Panel";

export { Button } from "@/components/ui/button/Button";
export type { ButtonProps } from "@/components/ui/button/Button";

export { Input } from "@/components/ui/input";

export { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
export type { StatusBadgeProps } from "@/components/ui/status-badge/StatusBadge";

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
export type { WorldRecord } from "@/types/world";
export type { WorldItemRecord } from "@/types/item";
export type { InventoryEntryRecord } from "@/types/inventory";

export { getWorldActors } from "@/api/actors";
export { getWorldItems } from "@/api/items";
export { getActorInventory } from "@/api/inventory";

export { useResetOnChange } from "@/hooks/useResetOnChange";

/**
 * The GraphQL caller, and the world-events feed.
 *
 * These two are the widest things on this list, and they are here for the
 * same reason: since ADR-063 a pack owns *tables* — Genie's six of them, its
 * models, and the queries and mutations over them all live in
 * `packs/systems/genie/server`. A pack that owns a schema and may not call it
 * is not a boundary, it is a pack whose server half and web half have to meet
 * in `apps/web`, which is precisely where every violation this app has had
 * came from.
 *
 * `postGraphQL` is the same transport every other caller in this app uses:
 * same endpoint, same credentials, same errors. It grants a pack no authority
 * it did not already have — every field it can reach is authorized on the
 * server, per request, exactly as it is for `apps/web`'s own calls. What it
 * removes is the need for a pack's client module to live outside the pack.
 *
 * `subscribeToWorldEvents` is the read half of the same story. A pack's
 * tables emit `world_events` rows under their own event code, and hearing
 * about one is how a second client at the table sees the first client's move.
 * It is *not* the engine bridge — no world store, no Bevy handle, no scene —
 * it is the NOTIFY feed, and a pack gets the same undifferentiated stream
 * every other consumer gets and filters it by its own code.
 *
 * What is still absent, and deliberately: routing, authentication, the world
 * store, and anything holding a session credential. A pack that needs the
 * current user is told who it is through its panel props (see `PanelSlot`
 * below) rather than reaching for the auth context — the host knows who is
 * looking, and a pack that could ask would be a pack that could ask on a page
 * where the answer is nobody's business.
 */
export { postGraphQL, GraphQLRequestError } from "@/api/graphqlClient";
export { subscribeToWorldEvents } from "@/engine/world/sync";
export type { WorldEventLike } from "@/engine/world/sync";

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

/**
 * # Panels a pack contributes
 *
 * A sheet needed one contract, because there is one place a sheet goes. A
 * panel needs a *vocabulary*, because there are several places a panel goes
 * and they hand it different things.
 *
 * Four pages in this app used to ask "is this world Genie?" and mount a Genie
 * component if so — the actor page's NPC shop, the staging page's session
 * loop, the system-settings page's carryover card, and the play dock's clocks
 * panel. Each was the client-side shape of exactly what FR-029 forbids: shared
 * code deciding something per game system. `check-system-registry.mjs` listed
 * all four against `032/T108`, which is this.
 *
 * The mechanism is the actor sheet's, one level deeper:
 *
 * ```
 * packs/systems/<id>/web/src/panels/<slot>.tsx
 * ```
 *
 * `apps/web/src/panels/systemPanels.ts` globs that path at build time and
 * reads the system id and the slot name out of it — the same build-time
 * discovery, and the same ADR-029 argument for why a glob is not "loading a
 * pack at runtime".
 *
 * ## A correction to ADR-066
 *
 * ADR-066 and spec 032's `T108` entry both say a filename is the wrong place
 * to encode which slot a panel fills. That is overstated, and both documents
 * are corrected where they say it. A two-level path carries a slot perfectly
 * well; what a slot actually needs, and a sheet did not, is the vocabulary
 * below — a closed set of names, and a typed props contract per name. The
 * declaration is the union, not the directory listing.
 *
 * ## Why slots are a closed union
 *
 * Because the host is the one that must know where to mount them. An open
 * string would let a pack ship `panels/wherever.tsx` and be silently never
 * rendered, which is the worst failure available: no error, no panel, and
 * nothing to grep for. Adding a slot means adding a name here *and* a mount
 * point in the page that owns it, in the same diff, on purpose.
 *
 * ## Two slots may share a component
 *
 * `world-staging` and `clocks` both show Genie's session loop, and that is
 * not a mistake to collapse — a GM sees it while staging, and reaches it
 * again mid-session from the play dock. A pack points both slot files at one
 * component and the registry resolves both keys to the same reference.
 */
export type PanelSlot =
  | "npc-detail"
  | "world-staging"
  | "world-settings"
  | "clocks";

/**
 * The actor page, below inventory and abilities, for an actor the host has
 * already determined is an NPC. Whether an actor is an NPC is a fact about
 * the actor, not a decision about a game system, so the host still makes it.
 */
export interface NpcDetailPanelProps {
  worldId: string;
  actorId: string;
  actor: import("@/types/actor").WorldActorRecord;
  /** Who is looking, so a pack never reaches for the auth context. */
  currentUserId?: string;
  isGm: boolean;
}

/** The pre-session staging page at `/world/:id/play`, below session notes. */
export interface WorldStagingPanelProps {
  worldId: string;
  world: import("@/types/world").WorldRecord | null;
  isGm: boolean;
  currentUserId?: string;
}

/**
 * The play dock's Clocks & Timers section.
 *
 * The dock draws its own empty state when no pack fills this slot, which is
 * the inversion that made this slot worth having: the panel used to ask "is
 * this Genie?" and print an empty state otherwise. It now asks "did anyone
 * contribute a clocks panel?" and prints the same empty state when nobody
 * did — the comparison is deleted rather than relocated.
 */
export interface ClocksPanelProps {
  worldId: string;
  isGm: boolean;
  currentUserId?: string;
}

/**
 * The world's System settings page, GM-only.
 *
 * `onWorldChanged` is a signal, not a value: "the world record you are
 * holding is stale, read it again". A panel that mutates the world could
 * hand back a fresh `WorldRecord` instead, but only by selecting every field
 * this app's own world query selects — which would make the shape of
 * `WorldRecord` part of the pack contract, and make adding a column to it a
 * change that breaks packs. One extra read is the cheaper half of that
 * trade, and it happens once, on a GM toggling a setting.
 */
export interface WorldSettingsPanelProps {
  worldId: string;
  world: import("@/types/world").WorldRecord;
  isGm: boolean;
  onWorldChanged: () => void;
}

/**
 * Slot name to the props that slot supplies.
 *
 * The registry is typed against this, so `resolvePanel("clocks")` hands back
 * a component the clocks dock can actually render, and a pack whose
 * `panels/clocks.tsx` takes staging's props fails to compile rather than
 * failing at a table.
 */
export interface PanelSlotProps {
  "npc-detail": NpcDetailPanelProps;
  "world-staging": WorldStagingPanelProps;
  "world-settings": WorldSettingsPanelProps;
  clocks: ClocksPanelProps;
}
