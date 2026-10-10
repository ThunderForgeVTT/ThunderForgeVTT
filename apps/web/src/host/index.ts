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
 * **Contracts, as types.** `ActorSheetProps`, the panel slots, and
 * `StatBlockSource` are the shapes a pack's default exports must have for the
 * host to find and use them. A type hands a pack nothing it can call, so these
 * are not a fourth category of reach: they are the host saying what it will
 * ask a pack for. `StatBlockSource` is the newest, and it is deliberately the
 * narrowest kind of contribution there is. A pack that knows its game's
 * creatures answers with *data* (which slots to write, which attacks to make)
 * and the host does the writing through its own API with the current user's
 * permissions. The pack is never handed the ability API to do it itself,
 * which is why applying a stat block needed a type here and no new function.
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
 * The actor's links to staged pieces (spec 048), which the ability and
 * inventory reads withhold, so a sheet can mark them "awaiting the GM".
 */
export { useActorStagedLinks } from "@/hooks/useActorStagedLinks";
export type { ActorStagedLink } from "@/api/sheetImport";

/**
 * A world's answers to the settings its system declares (spec 067).
 *
 * A pack declares a setting in its manifest's `settings` block and the host
 * stores it, draws its control and announces a change. This is how the pack's
 * own surfaces read it back: `valueOf("key")`, kept current while the page is
 * open. It reads what any member of the world may read.
 */
export { useWorldSystemSettings } from "@/hooks/useWorldSystemSettings";
export type { WorldSystemSettingsHandle } from "@/hooks/useWorldSystemSettings";
export type {
  WorldSystemSetting,
  WorldSystemSettingValue,
} from "@/api/worldSystemSettings";

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
import {
  subscribeToWorldEvents as subscribeToWorldEventsWithOptions,
  type WorldEventLike,
} from "@/engine/world/sync";

export { postGraphQL, GraphQLRequestError } from "@/api/graphqlClient";
export type { WorldEventLike };

/**
 * A world's event stream, for a pack: one that never routes the page.
 *
 * A pack's panels sit on the pages *around* play as well as in it — Genie's
 * session panel is on the staging page — and those pages stay open to a
 * world's members while an operator has paused its play (spec 051 FR-024).
 * A pack stream that announced the pause took anyone who merely opened a
 * paused world's staging page to the notice. So a pack's stream ends quietly
 * on a pause. On the playfield nothing is lost: its own streams, heartbeat
 * and play-field claim announce the pause, and routing was never a pack's to
 * do (see "What is still absent" above).
 */
export function subscribeToWorldEvents(
  worldId: string,
): AsyncIterable<WorldEventLike> {
  return subscribeToWorldEventsWithOptions(worldId, { announcePause: false });
}

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
 * # Stat blocks a pack contributes
 *
 * A system that prints creatures (a goblin with an armour class, hit points
 * and a scimitar) may put a `StatBlockSource` at
 * `packs/systems/<id>/web/src/StatBlocks.ts`. `systemStatBlocks.ts` finds it.
 *
 * The pack never writes anything. It says what applying a block *would*
 * write, and `applyStatBlock.ts` does it: system-data slots through
 * `updateActorSystemData`, attacks as world abilities attached to the actor.
 * So the host needs to know nothing about what a creature is in any game,
 * and the pack needs no authority it does not already lack.
 */

/** The four slots a stat block may write, whole, keyed as the write API is. */
export type StatBlockSlots = Partial<
  Record<
    "ability_data" | "resource_data" | "proficiency_data" | "trait_data",
    Record<string, unknown>
  >
>;

/** One attack a creature can make, as the ability the host should create. */
export interface StatBlockAttackPlan {
  /** Unique within the plan; what another entry's `parts` refers to. */
  key: string;
  /** The world ability's name. */
  name: string;
  description: string;
  /** A type in the world's ability vocabulary. */
  classification: string;
  /** Dice for the attack roll, or null for an entry that only groups others. */
  attackRoll: string | null;
  /** Dice for the damage, or null likewise. */
  damage: string | null;
  /** In the system's own distance unit. */
  reach: number | null;
  rangeNormal: number | null;
  rangeLong: number | null;
  /** Keys of the entries one use of this makes, in order: a multiattack. */
  parts: string[];
}

export interface StatBlockPlan {
  /** The block's own name, for messages. */
  name: string;
  slots: StatBlockSlots;
  attacks: StatBlockAttackPlan[];
}

export interface StatBlockSummary {
  id: string;
  name: string;
  /** The bestiary creature this block is for, by slug, when there is one. */
  bestiary: string | null;
  /** One line of the numbers a Game Master picks by. */
  summary: string;
}

export interface StatBlockSource {
  blocks: StatBlockSummary[];
  /**
   * What applying a block would write. `current` is what the actor holds
   * now, so a pack can keep what a block has no opinion on (a Game Master's
   * notes) rather than erase it. Null for a block this source does not have.
   */
  plan(id: string, current: StatBlockSlots | null): StatBlockPlan | null;
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
 * panel (the slot now named `dock`). Each was the client-side shape of exactly what FR-029 forbids: shared
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
 * `world-staging` and `dock` both show Genie's session loop, and that is
 * not a mistake to collapse — a GM sees it while staging, and reaches it
 * again mid-session from the play dock. A pack points both slot files at one
 * component and the registry resolves both keys to the same reference.
 */
export type PanelSlot =
  | "npc-detail"
  | "world-staging"
  | "world-settings"
  | "dock";

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
 * The play dock's section for the world's game system (spec 067 Story 2).
 *
 * Named for where it mounts, not for what the first pack put there: it was
 * `clocks` until a pack filled it with something that was not a clock. The
 * module may also export `title`, a string, which is what the dock tab
 * reads. The dock shows no tab at all for a system that fills no `dock`
 * slot, so there is no empty state for a pack to be mistaken for.
 */
export interface DockPanelProps {
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
 * The registry is typed against this, so `resolvePanel("dock")` hands back
 * a component the play dock can actually render, and a pack whose
 * `panels/dock.tsx` takes staging's props fails to compile rather than
 * failing at a table.
 */
export interface PanelSlotProps {
  "npc-detail": NpcDetailPanelProps;
  "world-staging": WorldStagingPanelProps;
  "world-settings": WorldSettingsPanelProps;
  dock: DockPanelProps;
}
