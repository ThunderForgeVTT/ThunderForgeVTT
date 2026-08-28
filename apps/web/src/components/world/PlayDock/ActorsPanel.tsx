import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { getWorldActors } from "@/api/actors";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { WorldActorRecord } from "@/types/actor";

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

  if (error) {
    return <p className="text-sm text-destructive">{error}</p>;
  }

  if (actors === null) {
    return <p className="text-sm text-muted-foreground">Loading actors…</p>;
  }

  const renderActor = (actor: WorldActorRecord) => (
    <li key={actor.id}>
      <Link
        to={`/world/${worldId}/actor/${actor.id}/view`}
        className="flex items-center gap-2 rounded-lg border border-border px-2 py-1.5 transition-colors hover:bg-muted"
      >
        <FantasyIcon name={actor.isNpc ? "skull" : "shield"} size={14} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm">{actor.label}</span>
          <span className="block truncate text-xs text-muted-foreground">
            {actor.actorType}
          </span>
        </span>
      </Link>
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
