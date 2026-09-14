import { type FormEvent, useCallback, useEffect, useState } from "react";
import {
  listPlayPauses,
  pauseWorldPlay,
  type PauseCandidateWorld,
  type PlayPause,
  searchPauseCandidates,
} from "@/api/playPause";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import type { SeoConfig } from "@/types/seo";
import { AdminSectionShell } from "./components/AdminSectionShell";

export const playPausesPageSeo: SeoConfig = {
  title: "Play pauses",
  description: "Pause a world's live play, and see the pauses in force.",
  canonicalPath: "/admin/play-pauses",
  noindex: true,
};

function formatMoment(iso: string): string {
  return new Date(iso).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

function messageOf(cause: unknown, otherwise: string): string {
  return cause instanceof Error ? cause.message : otherwise;
}

/**
 * Spec 051 US1: an operator stops a live table.
 *
 * One lever, for the case that cannot wait for a request: find the world,
 * give the grounds, confirm. Every browser playing in it leaves the playfield
 * within seconds, and the world cannot be played again until an operator lifts
 * the pause (ADR-100).
 *
 * The grounds are required and recorded, and the dialog says plainly that the
 * table never sees them, because an operator writing them should know who
 * will read them.
 *
 * Later stories add sections here rather than pages: requests a takedown
 * raises (US3), lifting (US4), and the full record (US5).
 */
export default function PlayPausesPage() {
  const [search, setSearch] = useState("");
  const [candidates, setCandidates] = useState<PauseCandidateWorld[] | null>(
    null,
  );
  const [searching, setSearching] = useState(false);
  const [active, setActive] = useState<PlayPause[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<string | null>(null);

  const [target, setTarget] = useState<PauseCandidateWorld | null>(null);
  const [grounds, setGrounds] = useState("");
  const [pausing, setPausing] = useState(false);
  const [dialogError, setDialogError] = useState<string | null>(null);

  const loadActive = useCallback(async () => {
    try {
      setActive((await listPlayPauses({ active: true })).nodes);
    } catch (cause) {
      setError(messageOf(cause, "Failed to load the pauses in force"));
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    listPlayPauses({ active: true })
      .then((page) => {
        if (mounted) setActive(page.nodes);
      })
      .catch((cause: unknown) => {
        if (mounted) {
          setError(messageOf(cause, "Failed to load the pauses in force"));
        }
      });
    return () => {
      mounted = false;
    };
  }, []);

  const runSearch = useCallback(async (term: string) => {
    setSearching(true);
    setError(null);
    try {
      setCandidates(await searchPauseCandidates(term));
    } catch (cause) {
      setError(messageOf(cause, "Failed to search worlds"));
    } finally {
      setSearching(false);
    }
  }, []);

  const submitSearch = (event: FormEvent) => {
    event.preventDefault();
    const term = search.trim();
    if (!term) return;
    void runSearch(term);
  };

  const openConfirm = (world: PauseCandidateWorld) => {
    setTarget(world);
    setGrounds("");
    setDialogError(null);
    setOutcome(null);
  };

  const confirmPause = async (event: FormEvent) => {
    event.preventDefault();
    if (!target) return;
    const written = grounds.trim();
    if (!written) {
      setDialogError("Give the grounds for pausing play.");
      return;
    }
    setPausing(true);
    setDialogError(null);
    try {
      const result = await pauseWorldPlay(target.id, written);
      setOutcome(
        result.alreadyPaused
          ? `Play in ${result.pause.worldName} was already paused. Your grounds were added to that pause.`
          : `Play in ${result.pause.worldName} is paused.`,
      );
      setTarget(null);
      await loadActive();
      const term = search.trim();
      if (term) await runSearch(term);
    } catch (cause) {
      setDialogError(messageOf(cause, "Play was not paused"));
    } finally {
      setPausing(false);
    }
  };

  return (
    <>
      <SEO {...playPausesPageSeo} />
      <AdminSectionShell>
        <div className="grid gap-6">
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Operations
            </p>
            <h1 className="text-2xl font-semibold">Play pauses</h1>
            <p className="max-w-[70ch] text-sm text-muted-foreground">
              Pausing a world ends every live session in it at once, on every
              scene, and keeps it from being played until an operator lifts the
              pause. Nothing in the world is changed or removed.
            </p>
          </div>

          {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}
          {outcome ? (
            <p
              role="status"
              className="text-sm"
              data-testid="play-pause-outcome"
            >
              {outcome}
            </p>
          ) : null}

          <Card className="grid gap-3 p-6">
            <h2 className="text-lg font-semibold">Pause a world's play</h2>
            <form
              className="flex flex-wrap items-center gap-2"
              onSubmit={submitSearch}
            >
              <label htmlFor="play-pause-search" className="sr-only">
                World name or id
              </label>
              <Input
                id="play-pause-search"
                className="max-w-md"
                placeholder="World name or id"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                data-testid="play-pause-search"
              />
              <Button type="submit" size="sm" disabled={searching}>
                Find world
              </Button>
            </form>

            {searching ? (
              <Loader label="Searching worlds" />
            ) : candidates === null ? null : candidates.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No world matches that.
              </p>
            ) : (
              <ul className="grid gap-2" data-testid="play-pause-candidates">
                {candidates.map((world) => (
                  <li
                    key={world.id}
                    className="flex flex-wrap items-center justify-between gap-3 border-b pb-2 last:border-0"
                    data-testid="play-pause-candidate"
                    data-world-id={world.id}
                  >
                    <div className="grid gap-0.5">
                      <span className="font-medium">{world.name}</span>
                      <span className="text-xs text-muted-foreground">
                        Owned by {world.ownerName} · <code>{world.id}</code>
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      {world.playedNow ? (
                        <StatusBadge variant="info">Being played</StatusBadge>
                      ) : null}
                      {world.paused ? (
                        <StatusBadge variant="warning">Paused</StatusBadge>
                      ) : null}
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => openConfirm(world)}
                      >
                        Pause play
                      </Button>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </Card>

          <Card className="grid gap-3 p-6">
            <h2 className="text-lg font-semibold">Active pauses</h2>
            {active === null ? (
              <Loader label="Loading the pauses in force" />
            ) : active.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No world's play is paused.
              </p>
            ) : (
              <ul className="grid gap-3" data-testid="play-pauses-active">
                {active.map((pause) => (
                  <li
                    key={pause.id}
                    className="grid gap-1 border-b pb-3 last:border-0"
                    data-testid="play-pause-active"
                    data-world-id={pause.worldId}
                  >
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <span className="font-medium">
                        {pause.worldName}
                        {pause.worldExists ? "" : " (deleted)"}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        Paused{" "}
                        <time dateTime={pause.pausedAt}>
                          {formatMoment(pause.pausedAt)}
                        </time>{" "}
                        by {pause.pausedBy.name}
                      </span>
                    </div>
                    <p className="text-sm whitespace-pre-line">
                      {pause.grounds}
                    </p>
                  </li>
                ))}
              </ul>
            )}
          </Card>
        </div>
      </AdminSectionShell>

      <Dialog
        open={target !== null}
        onOpenChange={(open) => {
          if (!open && !pausing) setTarget(null);
        }}
      >
        <DialogContent data-testid="play-pause-confirm">
          <form className="grid gap-4" onSubmit={(e) => void confirmPause(e)}>
            <DialogHeader>
              <DialogTitle>Pause play in {target?.name}?</DialogTitle>
              <DialogDescription>
                Everyone playing in this world leaves the playfield within
                seconds and is told that an operator has paused play. Nobody can
                play in it again until an operator lifts the pause.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="play-pause-grounds">Grounds</Label>
              <Textarea
                id="play-pause-grounds"
                required
                aria-required="true"
                aria-describedby="play-pause-grounds-hint"
                value={grounds}
                maxLength={5000}
                onChange={(event) => setGrounds(event.target.value)}
              />
              <p
                id="play-pause-grounds-hint"
                className="text-xs text-muted-foreground"
              >
                Required. Recorded with your name, and never shown to anyone in
                the world.
              </p>
              {dialogError ? (
                <p role="alert" className="text-sm text-destructive">
                  {dialogError}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={pausing}
                onClick={() => setTarget(null)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="danger"
                disabled={pausing || grounds.trim() === ""}
                data-testid="play-pause-confirm-submit"
              >
                Pause play
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
