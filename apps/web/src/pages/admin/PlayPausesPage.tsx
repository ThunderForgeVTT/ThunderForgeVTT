import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { SeoConfig } from "@/types/seo";
import { AdminSectionShell } from "./components/AdminSectionShell";
import { TriggerList, WorldName } from "./play-pauses/PauseParts";
import { PlayPauseDialogs } from "./play-pauses/PlayPauseDialogs";
import { PlayPauseRecord } from "./play-pauses/PlayPauseRecord";
import { formatMoment } from "./play-pauses/playPauseFormat";
import { usePlayPauses } from "./play-pauses/usePlayPauses";

export const playPausesPageSeo: SeoConfig = {
  title: "Play pauses",
  description: "Pause a world's live play, and see the pauses in force.",
  canonicalPath: "/admin/play-pauses",
  noindex: true,
};

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
 * US3 adds *Requests*: a takedown on a world being played asks for a pause,
 * and an operator approves or declines it with a note. When two operators
 * decide one request at once, the one who lost is told who decided and when
 * (FR-035), which is an outcome, not an error.
 *
 * US4 adds *Lift pause* on each pause in force, with grounds as required as
 * a pause's. Two operators lifting at once is the same kind of outcome: the
 * second is told who lifted it and when (FR-040).
 *
 * US5 adds *Record*: every pause, in force or lifted, and every decided
 * request, newest first and a page at a time, each naming the world (marked
 * when it has since been deleted, FR-053), who, when, the grounds or note,
 * what prompted it, and the lift (FR-051, FR-052).
 */
export default function PlayPausesPage() {
  const state = usePlayPauses();
  const {
    search,
    setSearch,
    candidates,
    searching,
    active,
    error,
    outcome,
    outcomeRef,
    requests,
    submitSearch,
    openConfirm,
    openDecision,
    openLift,
  } = state;

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
              ref={outcomeRef}
              role="status"
              tabIndex={-1}
              className="text-sm"
              data-testid="play-pause-outcome"
            >
              {outcome}
            </p>
          ) : null}

          <Card className="grid gap-3 p-6">
            <h2 className="text-lg font-semibold">Requests</h2>
            <p className="max-w-[70ch] text-sm text-muted-foreground">
              A takedown on a world that is being played asks for its play to be
              paused. Nothing is paused until an operator approves.
            </p>
            {requests === null ? (
              <Loader label="Loading the pause requests" />
            ) : requests.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No request is waiting for a decision.
              </p>
            ) : (
              <ul className="grid gap-3" data-testid="play-pause-requests">
                {requests.map((request) => (
                  <li
                    key={request.id}
                    className="grid gap-2 border-b pb-3 last:border-0"
                    data-testid="play-pause-request"
                    data-request-id={request.id}
                    data-world-id={request.worldId}
                  >
                    <div className="flex flex-wrap items-center justify-between gap-3">
                      <div className="grid gap-0.5">
                        <WorldName
                          name={request.worldName}
                          exists={request.worldExists}
                        />
                        <span className="text-xs text-muted-foreground">
                          Raised{" "}
                          <time dateTime={request.raisedAt}>
                            {formatMoment(request.raisedAt)}
                          </time>{" "}
                          · <code>{request.worldId}</code>
                        </span>
                      </div>
                      <div className="flex items-center gap-2">
                        {request.playedNow ? (
                          <StatusBadge
                            variant="info"
                            data-testid="play-pause-request-played-now"
                          >
                            Being played
                          </StatusBadge>
                        ) : (
                          <span className="text-xs text-muted-foreground">
                            Not being played
                          </span>
                        )}
                        {/* Not `danger`: that variant's red on its own tint
                            measures 3.99:1, under AA for this text (axe, T043).
                            The dialog behind each button is the confirmation. */}
                        <Button
                          variant="secondary"
                          size="sm"
                          aria-label={`Approve pausing ${request.worldName}`}
                          onClick={() => openDecision(request, "APPROVE")}
                        >
                          Approve
                        </Button>
                        <Button
                          variant="secondary"
                          size="sm"
                          aria-label={`Decline pausing ${request.worldName}`}
                          onClick={() => openDecision(request, "DECLINE")}
                        >
                          Decline
                        </Button>
                      </div>
                    </div>
                    <TriggerList
                      triggers={request.triggers}
                      label={`What prompted the request for ${request.worldName}`}
                      testId="play-pause-request"
                    />
                  </li>
                ))}
              </ul>
            )}
          </Card>

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
                      <div className="grid gap-0.5">
                        <WorldName
                          name={pause.worldName}
                          exists={pause.worldExists}
                        />
                        <span className="text-xs text-muted-foreground">
                          Paused{" "}
                          <time dateTime={pause.pausedAt}>
                            {formatMoment(pause.pausedAt)}
                          </time>{" "}
                          by {pause.pausedBy.name}
                        </span>
                      </div>
                      {/* Secondary, as Approve and Decline are: the dialog
                          behind it is the confirmation. */}
                      <Button
                        variant="secondary"
                        size="sm"
                        aria-label={`Lift the pause on ${pause.worldName}`}
                        onClick={() => openLift(pause)}
                        data-testid="play-pause-lift-open"
                      >
                        Lift pause
                      </Button>
                    </div>
                    <p className="text-sm whitespace-pre-line">
                      {pause.grounds}
                    </p>
                  </li>
                ))}
              </ul>
            )}
          </Card>

          <PlayPauseRecord state={state} />
        </div>
      </AdminSectionShell>

      <PlayPauseDialogs state={state} />
    </>
  );
}
