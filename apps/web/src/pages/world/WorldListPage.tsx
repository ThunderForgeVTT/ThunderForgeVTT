import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listGameSystems, type GameSystemSummary } from "@/api/gameSystems";
import { getAllWorlds, getMyWorldsWithRole } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Checkbox } from "@/components/ui/checkbox";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { SeoConfig } from "@/types/seo";
import {
  readStoredWorldsView,
  resolveWorldsView,
  storeWorldsView,
  type WorldsView,
} from "@/pages/world/worldsView";
import { WorldCard } from "./components/WorldCard";
import { WorldsTable, type WorldListEntry } from "./components/WorldsTable";
import { WorldsViewToggle } from "./components/WorldsViewToggle";

export const worldListPageSeo: SeoConfig = {
  title: "World archive",
  description:
    "Browse your ThunderForge worlds, create new ones, and inspect the full archive from a clean dashboard.",
  canonicalPath: "/worlds",
  noindex: true,
};

/**
 * The world archive, drawn as tiles or as a table.
 *
 * # Why the list query changed
 *
 * This page asked `myWorlds`, which is **owned-only**, and so could not have
 * a "your role" column: every row would have said Owner. `myWorldsWithRole`
 * is the query `/welcome` already uses — every world the caller owns *or* is
 * an accepted member of, each paired with the role they hold there. A player
 * invited to a table now finds that table in their archive, which is what an
 * archive of "your worlds" was always claiming to be.
 *
 * # The threshold and the choice
 *
 * See `worldsView.ts`: seven worlds is where tiles stop being readable, and a
 * person's own choice beats the count at any number. The control is visible
 * at every count — including at two worlds, where the table is a strange
 * thing to want and is nonetheless a click away.
 */
export default function WorldListPage() {
  const { isAdmin } = useAuth();
  const [includeAll, setIncludeAll] = useState(false);
  const scopeKey = isAdmin && includeAll ? "all" : "mine";
  const [archiveState, setArchiveState] = useState<{
    requestedScope: string;
    entries: WorldListEntry[];
    status: string | null;
    isLoading: boolean;
  }>({
    requestedScope: scopeKey,
    entries: [],
    status: null,
    isLoading: true,
  });
  /** `null` until this person has expressed one — see `worldsView.ts`. */
  const [chosenView, setChosenView] = useState<WorldsView | null>(
    readStoredWorldsView,
  );
  /** Titles for the system column. A failed read leaves ids, never blanks. */
  const [systems, setSystems] = useState<GameSystemSummary[]>([]);

  useEffect(() => {
    let active = true;
    listGameSystems()
      .then((installed) => {
        if (active) setSystems(installed.systems);
      })
      .catch(() => {
        // `titleFor` falls back to the id, which is still an answer.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    const request =
      isAdmin && includeAll
        ? getAllWorlds().then((worlds) =>
            worlds.map((world) => ({ world, role: null })),
          )
        : getMyWorldsWithRole().then((entries) =>
            entries.map((entry) => ({ world: entry.world, role: entry.role })),
          );
    void request
      .then((entries) => {
        if (active) {
          setArchiveState({
            requestedScope: scopeKey,
            entries,
            status: null,
            isLoading: false,
          });
        }
      })
      .catch((error) => {
        if (active) {
          setArchiveState({
            requestedScope: scopeKey,
            entries: [],
            status:
              error instanceof Error
                ? error.message
                : "Failed to load world archive.",
            isLoading: false,
          });
        }
      });

    return () => {
      active = false;
    };
  }, [includeAll, isAdmin, scopeKey]);

  const isLoading =
    archiveState.requestedScope !== scopeKey || archiveState.isLoading;
  const entries =
    archiveState.requestedScope === scopeKey ? archiveState.entries : [];
  const status =
    archiveState.requestedScope === scopeKey ? archiveState.status : null;

  const view = resolveWorldsView(chosenView, entries.length);

  const handleChangeView = (next: WorldsView) => {
    setChosenView(next);
    storeWorldsView(next);
  };

  return (
    <>
      <SEO {...worldListPageSeo} />
      <Container>
        <main className="grid gap-6 pb-16">
          <section className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                World archive
              </p>
              <h1 className="text-3xl font-semibold">
                Every world in your library.
              </h1>
              <p className="mt-1 max-w-2xl text-muted-foreground">
                Create a new world, revisit an existing one, or inspect the
                wider archive when administrator privileges permit it.
              </p>
            </div>
            <div className="grid gap-3">
              <Link to="/worlds/create">
                <Button icon="quill">Create world</Button>
              </Link>
              {isAdmin ? (
                <label className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Checkbox
                    checked={includeAll}
                    onCheckedChange={(checked) =>
                      setIncludeAll(checked === true)
                    }
                  />
                  <span>Show every world in the archive</span>
                </label>
              ) : null}
            </div>
          </section>

          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {isLoading ? (
            <Loader label="Opening world archive" />
          ) : entries.length > 0 ? (
            <>
              <section className="flex flex-wrap items-center justify-between gap-3">
                <p className="text-sm text-muted-foreground">
                  {entries.length} world{entries.length === 1 ? "" : "s"}
                </p>
                <WorldsViewToggle view={view} onChange={handleChangeView} />
              </section>

              {view === "table" ? (
                <WorldsTable
                  entries={entries}
                  systems={systems}
                  showOwner={Boolean(includeAll && isAdmin)}
                />
              ) : (
                <section className="grid grid-cols-[repeat(auto-fit,minmax(280px,1fr))] gap-6">
                  {entries.map((entry) => (
                    <WorldCard
                      key={entry.world.id}
                      world={entry.world}
                      showOwner={Boolean(includeAll && isAdmin)}
                    />
                  ))}
                </section>
              )}
            </>
          ) : (
            <Card surface="leather" className="grid gap-3 p-8 text-center">
              <h2 className="text-xl font-semibold">
                No worlds have been created yet.
              </h2>
              <p className="text-muted-foreground">
                Begin with a fresh world and ThunderForge will carry its scenes,
                actors, events, and interface hooks through later phases.
              </p>
              <Link to="/worlds/create" className="mx-auto">
                <Button variant="secondary" icon="worlds">
                  Create the first world
                </Button>
              </Link>
            </Card>
          )}
        </main>
      </Container>
    </>
  );
}
