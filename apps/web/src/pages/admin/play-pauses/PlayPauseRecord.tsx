import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { TriggerList, WorldName } from "./PauseParts";
import { formatMoment } from "./playPauseFormat";
import type { PlayPausesState } from "./usePlayPauses";

/** Spec 051 US5: the *Record* card, every pause and every decided request. */
export function PlayPauseRecord({ state }: { state: PlayPausesState }) {
  const {
    recordPausesHeadingRef,
    recordPauses,
    openLift,
    pausesCursor,
    loadingMore,
    showMore,
    recordRequestsHeadingRef,
    recordRequests,
    requestsCursor,
  } = state;
  return (
    <Card className="grid gap-5 p-6" data-testid="play-pause-record">
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">Record</h2>
        <p className="max-w-[70ch] text-sm text-muted-foreground">
          Every pause, in force or lifted, and every decided request, newest
          first. The record stays when a world is deleted.
        </p>
      </div>

      <section
        className="grid gap-3"
        aria-labelledby="play-pause-record-pauses"
      >
        <h3
          id="play-pause-record-pauses"
          ref={recordPausesHeadingRef}
          tabIndex={-1}
          className="font-semibold outline-none"
        >
          Pauses
        </h3>
        {recordPauses === null ? (
          <Loader label="Loading the pause record" />
        ) : recordPauses.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No world's play has ever been paused.
          </p>
        ) : (
          <ul className="grid gap-3">
            {recordPauses.map((pause) => (
              <li
                key={pause.id}
                className="grid gap-2 border-b pb-3 last:border-0"
                data-testid="play-pause-record-pause"
                data-pause-id={pause.id}
                data-world-id={pause.worldId}
                data-lifted={pause.liftedAt !== null}
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
                      by {pause.pausedBy.name} · <code>{pause.worldId}</code>
                    </span>
                  </div>
                  {pause.liftedAt === null ? (
                    <div className="flex items-center gap-2">
                      <StatusBadge variant="info">In force</StatusBadge>
                      <Button
                        variant="secondary"
                        size="sm"
                        aria-label={`Lift the pause on ${pause.worldName}`}
                        onClick={() => openLift(pause)}
                      >
                        Lift pause
                      </Button>
                    </div>
                  ) : null}
                </div>
                <p
                  className="text-sm whitespace-pre-line"
                  data-testid="play-pause-record-grounds"
                >
                  {pause.grounds}
                </p>
                <TriggerList
                  triggers={pause.triggers}
                  label={`What prompted the pause on ${pause.worldName}`}
                  testId="play-pause-record"
                />
                {pause.liftedAt !== null ? (
                  <div
                    className="grid gap-0.5 text-sm"
                    data-testid="play-pause-record-lift"
                  >
                    <span className="text-xs text-muted-foreground">
                      Lifted{" "}
                      <time dateTime={pause.liftedAt}>
                        {formatMoment(pause.liftedAt)}
                      </time>{" "}
                      by {pause.liftedBy?.name ?? "an operator"}
                    </span>
                    {pause.liftGrounds ? (
                      <span className="whitespace-pre-line">
                        {pause.liftGrounds}
                      </span>
                    ) : null}
                  </div>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        {pausesCursor ? (
          <div>
            <Button
              variant="secondary"
              size="sm"
              disabled={loadingMore === "pauses"}
              onClick={() => void showMore("pauses")}
              data-testid="play-pause-record-more-pauses"
            >
              Show more pauses
            </Button>
          </div>
        ) : null}
      </section>

      <section
        className="grid gap-3"
        aria-labelledby="play-pause-record-requests"
      >
        <h3
          id="play-pause-record-requests"
          ref={recordRequestsHeadingRef}
          tabIndex={-1}
          className="font-semibold outline-none"
        >
          {/* Not "Decided requests": a heading named like the
                    *Requests* section above makes that name ambiguous to
                    anyone finding it by name, a screen reader's heading list
                    included. */}
          Decisions
        </h3>
        {recordRequests === null ? (
          <Loader label="Loading the decided requests" />
        ) : recordRequests.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No request has been decided.
          </p>
        ) : (
          <ul className="grid gap-3">
            {recordRequests.map((request) => (
              <li
                key={request.id}
                className="grid gap-2 border-b pb-3 last:border-0"
                data-testid="play-pause-record-request"
                data-request-id={request.id}
                data-world-id={request.worldId}
                data-state={request.state}
              >
                <div className="grid gap-0.5">
                  <WorldName
                    name={request.worldName}
                    exists={request.worldExists}
                  />
                  <span className="text-xs text-muted-foreground">
                    Raised{" "}
                    <time dateTime={request.raisedAt}>
                      {formatMoment(request.raisedAt)}
                    </time>
                    {" · "}
                    {request.state === "APPROVED" ? "Approved" : "Declined"}
                    {request.decidedAt ? (
                      <>
                        {" "}
                        <time dateTime={request.decidedAt}>
                          {formatMoment(request.decidedAt)}
                        </time>
                      </>
                    ) : null}{" "}
                    by {request.decidedBy?.name ?? "an operator"} ·{" "}
                    <code>{request.worldId}</code>
                  </span>
                </div>
                {request.decisionNote ? (
                  <p className="text-sm whitespace-pre-line">
                    {request.decisionNote}
                  </p>
                ) : null}
                <TriggerList
                  triggers={request.triggers}
                  label={`What prompted the request for ${request.worldName}`}
                  testId="play-pause-record"
                />
              </li>
            ))}
          </ul>
        )}
        {requestsCursor ? (
          <div>
            <Button
              variant="secondary"
              size="sm"
              disabled={loadingMore === "requests"}
              onClick={() => void showMore("requests")}
              data-testid="play-pause-record-more-requests"
            >
              Show more requests
            </Button>
          </div>
        ) : null}
      </section>
    </Card>
  );
}
