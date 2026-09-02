import { useEffect, useMemo, useState } from "react";
import { beginTokenPlacement } from "@/engine/bevy";
import { getMyActorClaim } from "@/api/actorClaims";
import { getWorldActors } from "@/api/actors";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { WorldActorRecord } from "@/types/actor";
import { InPaneCharacterSheet } from "./InPaneCharacterSheet";

export interface ActorsPanelProps {
  worldId: string;
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
export function ActorsPanel({ worldId }: ActorsPanelProps) {
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

  const renderActor = (actor: WorldActorRecord) => (
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

      {/*
        Place hands the token to the engine, which carries it on the cursor
        until a left click drops it. Nothing is created here: the engine
        reports where it was dropped and the server decides whether it exists.
      */}
      <button
        type="button"
        data-testid={`actor-place-${actor.id}`}
        className="rounded border border-border px-2 py-1 text-xs transition-colors hover:bg-muted"
        onClick={() => {
          void beginTokenPlacement(actor.id);
        }}
      >
        Place
      </button>
    </li>
  );

  return (
    <div className="grid gap-3" data-testid="actors-panel">
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
