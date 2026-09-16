import { useEffect, useMemo, useRef, useState } from "react";
import {
  beginTokenPlacement,
  cancelTokenPlacement,
  onPlacementCancelled,
  onPlacementConfirmed,
} from "@/engine/bevy";
import { getMyActorClaim } from "@/api/actorClaims";
import {
  getWorldActorImages,
  getWorldActors,
  setActorVisibleToPlayers,
  type ActorImageRecord,
} from "@/api/actors";
import { getTokens } from "@/api/tokens";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { mayLookAt } from "@/engine/lookAt";
import { cn } from "@/lib/utils";
import { portraitOf, tokenImageOf } from "@/pages/world/actor/actorImagery";
import type { WorldActorRecord } from "@/types/actor";
import type { TokenRecord } from "@/types/token";
import { InPaneCharacterSheet } from "./InPaneCharacterSheet";
import { LookAtButton } from "./LookAtButton";

export interface ActorsPanelProps {
  worldId: string;
  /** Spec 046: the scene in play, for attacks made from a character's sheet. */
  sceneId?: string | null;
  /**
   * Whether the viewer runs the world. A Game Master gets a switch on each
   * NPC for whether players see it (owner decision 2026-09-15). A player is
   * never sent a hidden NPC, so there is nothing for them to switch.
   */
  isGm?: boolean;
}

interface FolderProps {
  label: string;
  count: number;
  open: boolean;
  onToggle: () => void;
  children: React.ReactNode;
  emptyLabel: string;
}

function Folder({
  label,
  count,
  open,
  onToggle,
  children,
  emptyLabel,
}: FolderProps) {
  return (
    <section>
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        data-testid={`actor-folder-${label.toLowerCase()}`}
        className="flex w-full items-center gap-2 rounded-md px-1 py-1.5 text-xs font-semibold tracking-widest text-muted-foreground uppercase transition-colors hover:bg-muted hover:text-foreground"
      >
        <span
          className={
            open ? "rotate-90 transition-transform" : "transition-transform"
          }
        >
          ›
        </span>
        {label}
        <span className="ml-auto tabular-nums">{count}</span>
      </button>
      {open ? (
        count === 0 ? (
          <p className="px-2 py-1 text-sm text-muted-foreground">
            {emptyLabel}
          </p>
        ) : (
          <ul className="grid gap-1 py-1">{children}</ul>
        )
      ) : null}
    </section>
  );
}

/**
 * The world's cast, foldered into PCs and NPCs with a search box.
 *
 * Search filters both folders at once and auto-opens them, so a query that
 * only matches NPCs doesn't look like it matched nothing because the NPC
 * folder happened to be collapsed. Filtering is client-side over the
 * already-loaded roster — `searchActors` exists server-side, but round
 * -tripping per keystroke would be slower and noisier than filtering a list
 * this size in memory.
 */
export function ActorsPanel({
  worldId,
  sceneId = null,
  isGm = false,
}: ActorsPanelProps) {
  const [actors, setActors] = useState<WorldActorRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [pcsOpen, setPcsOpen] = useState(true);
  const [npcsOpen, setNpcsOpen] = useState(true);
  /**
   * The character the viewer is playing, if any.
   *
   * `getMyActorClaim` is the world's one answer to "who am I at this table"
   * (spec 017) and already drives the actor-selection gate and the pickup
   * prompt. Asking it again here rather than inventing a dock-local notion of
   * ownership keeps one mechanism: `null` for a Game Master, an Owner, or a
   * member who has not claimed anybody, and every one of those cases wants the
   * Game Master's new-tab View.
   *
   * A claim that cannot be read is the same as no claim — View opens a tab,
   * which is what it did before this existed. Nothing is worth failing the
   * roster over.
   */
  const [claimedActorId, setClaimedActorId] = useState<string | null>(null);
  /**
   * Spec 031 US2 #3: the pane's previous content is the roster, so "dismiss"
   * is this going back to null. Held here rather than in `WorldDock` because
   * the dock's job is which *section* is open, and a character is not a
   * section — routing it through the dock would put one panel's internal state
   * in a component the other four share.
   */
  const [viewing, setViewing] = useState<WorldActorRecord | null>(null);
  /**
   * Owner, 2026-09-15: "When I place a token I don't necessarily know if it
   * places the actor's image."
   *
   * The roster's imagery, so this panel can answer that question before the
   * click rather than after it. Fetched separately from the roster because it
   * costs the server a query per actor (ADR-057) and only the surfaces that
   * show a face should pay for it.
   */
  const [imagesByActor, setImagesByActor] = useState<
    Record<string, ActorImageRecord[]>
  >({});
  /**
   * The actor currently on the cursor, if any.
   *
   * The engine owns the carry itself — the snapping, the preview and the
   * cancel all live there. This is chrome's copy of "a carry I started is in
   * flight", kept so the panel can say what is about to be placed; the engine
   * is still the one that decides when it ends, which is why both the drop and
   * the abandon are subscribed to below.
   */
  const [carrying, setCarrying] = useState<WorldActorRecord | null>(null);
  /**
   * What just landed, so a plain token does not read as nothing having
   * happened. Cleared on a timer rather than left up: it is a confirmation,
   * not a state, and a notice that stays becomes furniture.
   */
  const [justPlaced, setJustPlaced] = useState<{
    label: string;
    /** Bumped per drop, so placing the same actor twice restarts the clock
     *  instead of inheriting the first drop's timer. */
    seq: number;
  } | null>(null);
  /** A build whose engine cannot take a carry at all. */
  const [placeProblem, setPlaceProblem] = useState<string | null>(null);
  const carryingRef = useRef<WorldActorRecord | null>(null);
  /**
   * The scene's tokens, so a roster row can offer to look at the creature
   * (owner decision 2026-09-15).
   *
   * A creature is somewhere only by standing on the board, so the row needs
   * its token before it can offer to scroll to it — and the token's record is
   * also what says whether this viewer may read its name, which is half the
   * rule for whether they may look at it at all. A failure here costs the
   * target icons and nothing else: the roster, its search and every existing
   * control carry on.
   */
  const [sceneTokens, setSceneTokens] = useState<TokenRecord[]>([]);

  useEffect(() => {
    // No scene, nothing to fetch. Tokens held from a previous scene are left
    // in place rather than cleared here: `tokenByActor` only reads the ones on
    // the scene in play, so they can offer nothing.
    if (!sceneId) return;
    let active = true;
    getTokens(sceneId)
      .then((tokens) => {
        if (active) setSceneTokens(tokens);
      })
      .catch(() => {
        if (active) setSceneTokens([]);
      });
    return () => {
      active = false;
    };
  }, [sceneId]);

  /**
   * One token per actor: the primary when there is one, else the first.
   *
   * A creature standing on the board twice is rare but real — a summoner's
   * copies, a shapechanger's forms — and the target has to go somewhere
   * definite. The primary is the one the rest of the app already treats as
   * "the" token for an actor.
   */
  const tokenByActor = useMemo(() => {
    const byActor = new Map<string, TokenRecord>();
    for (const token of sceneTokens) {
      if (!token.actorId || token.sceneId !== sceneId) continue;
      const held = byActor.get(token.actorId);
      if (!held || (token.isPrimary && !held.isPrimary)) {
        byActor.set(token.actorId, token);
      }
    }
    return byActor;
  }, [sceneTokens, sceneId]);

  useEffect(() => {
    let active = true;
    getWorldActors(worldId)
      .then((result) => {
        if (active) setActors(result);
      })
      .catch((err) => {
        if (active)
          setError(
            err instanceof Error ? err.message : "Failed to load actors",
          );
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  useEffect(() => {
    let active = true;
    getWorldActorImages(worldId)
      .then((byActor) => {
        if (active) setImagesByActor(byActor);
      })
      .catch(() => {
        // Imagery that cannot be read leaves the roster usable. What it costs
        // is the warning below — which is why the warning is only shown for an
        // actor this panel positively knows has nothing, never for one it
        // simply failed to ask about.
        if (active) setImagesByActor({});
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  /**
   * Follow the carry the engine is actually holding.
   *
   * Mounted once rather than per carry: the listener is global and the panel
   * must hear an abandon it did not cause — Escape, a tool change, a scene
   * change — or it would go on claiming to be placing something the engine
   * dropped long ago.
   */
  useEffect(() => {
    const stopConfirmed = onPlacementConfirmed((event) => {
      // A prop is somebody else's carry (`InteractionTool`).
      if (event.kind !== "actor") return;
      const placed = carryingRef.current;
      carryingRef.current = null;
      setCarrying(null);
      if (placed)
        setJustPlaced((current) => ({
          label: placed.label,
          seq: (current?.seq ?? 0) + 1,
        }));
    });
    const stopCancelled = onPlacementCancelled(() => {
      carryingRef.current = null;
      setCarrying(null);
    });
    return () => {
      stopConfirmed();
      stopCancelled();
    };
  }, []);

  // The confirmation clears itself. Keyed on the label so placing a second
  // actor restarts the clock rather than inheriting the first one's.
  useEffect(() => {
    if (justPlaced === null) return;
    const timer = window.setTimeout(() => setJustPlaced(null), 4_000);
    return () => window.clearTimeout(timer);
  }, [justPlaced]);

  useEffect(() => {
    let active = true;
    getMyActorClaim(worldId)
      .then((claim) => {
        if (active) setClaimedActorId(claim?.actorId ?? null);
      })
      .catch(() => {
        if (active) setClaimedActorId(null);
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  const { pcs, npcs } = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = (actor: WorldActorRecord) =>
      needle === "" || actor.label.toLowerCase().includes(needle);
    const visible = (actors ?? []).filter(matches);
    return {
      pcs: visible.filter((actor) => !actor.isNpc),
      npcs: visible.filter((actor) => actor.isNpc),
    };
  }, [actors, query]);

  const searching = query.trim() !== "";

  const [visibilityError, setVisibilityError] = useState<string | null>(null);
  const toggleVisibleToPlayers = async (actor: WorldActorRecord) => {
    setVisibilityError(null);
    try {
      const updated = await setActorVisibleToPlayers(
        actor.id,
        !actor.visibleToPlayers,
      );
      setActors((current) =>
        (current ?? []).map((row) => (row.id === updated.id ? updated : row)),
      );
    } catch (err) {
      setVisibilityError(
        err instanceof Error ? err.message : "Failed to change who sees it",
      );
    }
  };

  /*
    The character replaces the roster rather than opening over it. The dock is
    one column wide, and a sheet floating above a list it cannot fully cover
    reads as a stuck overlay; going back is one control (FR-002/US2 #3). The
    map behind the dock is untouched either way — nothing here is canvas state,
    and no section is unmounted, so the engine keeps running (Principle I).
  */
  if (viewing) {
    return (
      <InPaneCharacterSheet
        worldId={worldId}
        sceneId={sceneId}
        actor={viewing}
        onDismiss={() => setViewing(null)}
      />
    );
  }

  if (error) {
    return <p className="text-sm text-destructive">{error}</p>;
  }

  if (actors === null) {
    return <p className="text-sm text-muted-foreground">Loading actors…</p>;
  }

  /**
   * What the server will find to draw this actor with.
   *
   * The resolution is the server's (`graphql/token_art.rs`): a token with no
   * photo of its own inherits its actor's token image, or failing that its
   * portrait. This reads the same two rows in the same order, so what the
   * panel promises before the click is what arrives after it.
   */
  const artFor = (actor: WorldActorRecord) => {
    const images = imagesByActor[actor.id];
    return tokenImageOf(images) ?? portraitOf(images);
  };

  /**
   * Arm the engine, and say what is on the cursor.
   *
   * Nothing is created here: the engine carries the token and reports where it
   * was dropped, and the server decides whether it exists. The button is the
   * armed signal — it is the control that armed it, and a second indicator
   * elsewhere on screen would be one more thing to reconcile with the engine's
   * own idea of what it is holding.
   */
  const handlePlace = async (actor: WorldActorRecord) => {
    setPlaceProblem(null);
    setJustPlaced(null);
    const began = await beginTokenPlacement(actor.id);
    if (!began) {
      setPlaceProblem("This build cannot place tokens on the map.");
      return;
    }
    carryingRef.current = actor;
    setCarrying(actor);
  };

  const renderActor = (actor: WorldActorRecord) => {
    // Offered only for a creature actually standing on this scene, and only to
    // a viewer allowed to know which one it is. The engine refuses the rest —
    // see `systems::camera_focus` — so this is the chrome half of one rule,
    // not a second rule.
    const token = tokenByActor.get(actor.id);
    const locatable = mayLookAt(token, isGm) ? token : undefined;

    return (
      <li
        key={actor.id}
        className="flex items-center gap-2 rounded-lg border border-border px-2 py-1.5"
      >
        <FantasyIcon name={actor.isNpc ? "skull" : "shield"} size={14} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm">{actor.label}</span>
          <span className="block truncate text-xs text-muted-foreground">
            {actor.actorType}
          </span>
        </span>

        {locatable ? (
          <LookAtButton
            tokenId={locatable.tokenId}
            label={actor.label}
            testIdPrefix="actor-look-at"
          />
        ) : null}

        {/*
        View never navigates.

        This row used to be a `Link`, so looking at a character cost whoever
        clicked it the table: the play view unmounted, the engine tore down,
        and getting back meant a reload. Spec 031 US1 and US2 are both about
        that, and answer it differently by role.

        A Game Master gets a new tab: they are inspecting one of many
        characters while running a table, and want it beside the map, not on
        top of it. A player opening the character they are actually playing
        gets it inside the pane (FR-002), because a new tab is where they stop
        being at the table — the map is no longer in front of them at the
        moment they most need it. Same control, same test id, two behaviours,
        deliberately.

        The condition is the claim, not the role: anybody whose claim is this
        actor is playing it. A Game Master has no claim (spec 017), so they
        fall through to the tab without a role check written here.
      */}
        {actor.id === claimedActorId ? (
          <button
            type="button"
            onClick={() => setViewing(actor)}
            data-testid={`actor-view-${actor.id}`}
            className="rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted"
          >
            View
          </button>
        ) : (
          <a
            href={`/world/${worldId}/actor/${actor.id}/view`}
            target="_blank"
            rel="noreferrer"
            data-testid={`actor-view-${actor.id}`}
            className="rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted"
          >
            View
          </a>
        )}

        {isGm && actor.isNpc ? (
          <button
            type="button"
            aria-pressed={actor.visibleToPlayers}
            aria-label={`Visible to players: ${actor.label}`}
            title={
              actor.visibleToPlayers
                ? "Players see this NPC"
                : "Hidden from players"
            }
            data-testid={`actor-visible-${actor.id}`}
            className="rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted aria-pressed:bg-muted"
            onClick={() => void toggleVisibleToPlayers(actor)}
          >
            {actor.visibleToPlayers ? "Shown" : "Hidden"}
          </button>
        ) : null}

        {/*
        Place hands the token to the engine, which carries it on the cursor
        until a left click drops it. Nothing is created here: the engine
        reports where it was dropped and the server decides whether it exists.
      */}
        <button
          type="button"
          data-testid={`actor-place-${actor.id}`}
          // The button is the armed signal: pressed while its own token is on
          // the cursor, unpressed the moment the engine says the carry ended.
          aria-pressed={carrying?.id === actor.id}
          aria-label={`Place ${actor.label} on the map`}
          className={cn(
            "rounded border border-border px-2 py-1 text-xs transition-colors",
            carrying?.id === actor.id
              ? "border-primary bg-primary text-primary-foreground"
              : "hover:bg-muted",
          )}
          onClick={() => {
            void handlePlace(actor);
          }}
        >
          Place
        </button>
      </li>
    );
  };

  return (
    <div className="grid gap-3" data-testid="actors-panel">
      {visibilityError ? (
        <p role="alert" className="text-sm text-destructive">
          {visibilityError}
        </p>
      ) : null}

      {/*
        What is about to be placed, while it is about to be placed.

        The owner's complaint was that a click on the map is a leap of faith:
        the token lands, and whether it carries the actor's picture is only
        discoverable afterwards. So the answer is given at the moment the
        question is asked — the name, and the very image the server will
        resolve — and when there is no image, that is said outright rather
        than left to be inferred from a plain shape.

        A ghost on the cursor would be better still, and is not possible from
        here: the engine owns the carry's preview and cannot be handed an
        actor's art without an engine change. So it is shown in the rail,
        beside the button that armed it.
      */}
      {carrying ? (
        <section
          // Announced as well as drawn — a Game Master arming a placement is
          // looking at the map, not at this pane.
          role="status"
          className="grid gap-2 rounded-lg border border-primary bg-primary/10 p-2"
          data-testid="placement-preview"
        >
          <div className="flex items-center gap-2">
            {artFor(carrying) ? (
              <img
                src={artFor(carrying)!.thumbnailUrl}
                alt=""
                className="h-10 w-10 rounded-full border border-border object-cover"
                data-testid="placement-preview-art"
              />
            ) : (
              <div
                className="grid h-10 w-10 place-items-center rounded-full border border-dashed border-border text-muted-foreground"
                data-testid="placement-preview-no-art"
                aria-hidden="true"
              >
                <FantasyIcon
                  name={carrying.isNpc ? "skull" : "shield"}
                  size={14}
                />
              </div>
            )}
            <span className="min-w-0 flex-1">
              <span className="block truncate text-sm font-medium">
                Placing {carrying.label}
              </span>
              <span className="block text-xs text-muted-foreground">
                Click the map to drop it, or press Escape.
              </span>
            </span>
          </div>

          {artFor(carrying) ? null : (
            <p className="text-xs text-muted-foreground">
              {carrying.label} has no art, so it will drop as a plain marker.{" "}
              {/* One way to fix it, offered where the gap is noticed. The
                  actor's own edit page is the page that sets both roles
                  (portrait and token); the compendium row sets a portrait in
                  place for whoever is already standing in the list. */}
              <a
                href={`/world/${worldId}/actor/${carrying.id}/edit`}
                target="_blank"
                rel="noreferrer"
                className="underline"
                data-testid="placement-preview-add-art"
              >
                Give {carrying.label} a portrait or token
              </a>
              .
            </p>
          )}

          <button
            type="button"
            onClick={() => void cancelTokenPlacement()}
            data-testid="placement-preview-cancel"
            className="justify-self-start rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted"
          >
            Cancel
          </button>
        </section>
      ) : null}

      {justPlaced ? (
        // So a plain token does not read as nothing having happened.
        <p
          role="status"
          className="rounded-lg border border-border px-2 py-1 text-xs text-muted-foreground"
          data-testid="placement-placed"
        >
          {justPlaced.label} is on the map.
        </p>
      ) : null}

      {placeProblem ? (
        <p role="alert" className="text-sm text-destructive">
          {placeProblem}
        </p>
      ) : null}
      <input
        type="search"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Search actors…"
        aria-label="Search actors"
        data-testid="actor-search-input"
        className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
      />

      <Folder
        label="PCs"
        count={pcs.length}
        // While searching, a folder is forced open so a match is never
        // hidden behind a collapsed header.
        open={pcsOpen || searching}
        onToggle={() => setPcsOpen((open) => !open)}
        emptyLabel={searching ? "No matching PCs." : "No PCs yet."}
      >
        {pcs.map(renderActor)}
      </Folder>

      <Folder
        label="NPCs"
        count={npcs.length}
        open={npcsOpen || searching}
        onToggle={() => setNpcsOpen((open) => !open)}
        emptyLabel={searching ? "No matching NPCs." : "No NPCs yet."}
      >
        {npcs.map(renderActor)}
      </Folder>
    </div>
  );
}
