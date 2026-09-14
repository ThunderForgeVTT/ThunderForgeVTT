import {
  type FormEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import { Link } from "react-router-dom";
import {
  alreadyLiftedBy,
  decidePlayPauseRequest,
  liftWorldPlayPause,
  listPlayPauseRequests,
  listPlayPauses,
  type PauseCandidateWorld,
  type PauseDecision,
  type PauseRequest,
  type PauseTrigger,
  pauseWorldPlay,
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
 * How often the page re-reads the requests and the pauses in force. A
 * takedown raises a request with nobody watching, and another operator may
 * decide one or pause a world meanwhile; the page shows that on its own.
 */
const REFRESH_MS = 5_000;

const TRIGGER_KIND_LABEL: Record<PauseTrigger["kind"], string> = {
  TAKEDOWN: "Takedown",
  OPERATOR: "Operator",
  ABUSE_REPORT: "Abuse report",
};

/** `world_actor` and friends, as a person would say them. */
function entityLabel(entityType: string | null): string | null {
  switch (entityType) {
    case null:
      return null;
    case "scene":
      return "a scene";
    case "world_actor":
      return "an actor";
    case "world_item":
      return "an item";
    case "world_lore_entry":
      return "a lore entry";
    default:
      return `a ${entityType.replace(/_/g, " ")}`;
  }
}

/** How many entries of each part of the record one "Show more" reads. */
const RECORD_PAGE = 20;
/** The most the server hands back in one page (`MAX_PAUSES_PER_PAGE`). */
const RECORD_MAX_PAGE = 200;

/**
 * What prompted a pause or a request: kind, what it landed on, when, and the
 * moderation case a takedown belongs to. Shared by *Requests* and *Record*;
 * `testId` keeps each section's hooks its own.
 */
function TriggerList({
  triggers,
  label,
  testId,
}: {
  triggers: PauseTrigger[];
  label: string;
  testId: string;
}) {
  if (triggers.length === 0) return null;
  return (
    <ul className="grid gap-1 text-sm" aria-label={label}>
      {triggers.map((trigger, index) => (
        <li
          key={`${trigger.moderationActionId ?? "trigger"}-${index}`}
          className="flex flex-wrap items-center gap-x-2 gap-y-0.5"
          data-testid={`${testId}-trigger`}
          data-kind={trigger.kind}
          data-entity-id={trigger.entityId ?? undefined}
        >
          {/* Spaces written out: flex gaps separate the parts on screen, not
              in the text a screen reader or a copy reads. */}
          <span className="font-medium">
            {TRIGGER_KIND_LABEL[trigger.kind]}
          </span>{" "}
          {entityLabel(trigger.entityType) ? (
            <>
              <span className="text-muted-foreground">
                on {entityLabel(trigger.entityType)}
              </span>{" "}
            </>
          ) : null}
          <span className="text-xs text-muted-foreground">
            recorded{" "}
            <time dateTime={trigger.recordedAt}>
              {formatMoment(trigger.recordedAt)}
            </time>
          </span>{" "}
          {trigger.caseId ? (
            <>
              <Link
                to={`/admin/moderation?case=${trigger.caseId}`}
                className="text-xs underline underline-offset-2"
                data-testid={`${testId}-case-link`}
              >
                Moderation case <code>{trigger.caseId.slice(0, 8)}</code>
              </Link>{" "}
            </>
          ) : null}
          {trigger.note ? (
            <span className="basis-full whitespace-pre-line text-muted-foreground">
              {trigger.note}
            </span>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

/** A world's name as the record holds it, marked when the world is gone. */
function WorldName({ name, exists }: { name: string; exists: boolean }) {
  return (
    <span className="font-medium" data-world-exists={exists}>
      {name}
      {exists ? "" : " (deleted)"}
    </span>
  );
}

interface PendingDecision {
  request: PauseRequest;
  decision: PauseDecision;
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

  // Where focus goes when the dialog closes. Radix returns it to a
  // `DialogTrigger`, and this dialog has none — it is opened from a button in
  // a list — so without these, closing it dropped focus onto the page body and
  // a keyboard user started again from the top (found by the T067 journey).
  const openerRef = useRef<HTMLElement | null>(null);
  const outcomeRef = useRef<HTMLParagraphElement>(null);

  // Spec 051 US3: requests a takedown raised, waiting for an operator.
  const [requests, setRequests] = useState<PauseRequest[] | null>(null);
  const [deciding, setDeciding] = useState<PendingDecision | null>(null);
  const [note, setNote] = useState("");
  const [submittingDecision, setSubmittingDecision] = useState(false);
  const [decisionError, setDecisionError] = useState<string | null>(null);

  // Spec 051 US4: lifting a pause in force.
  const [lifting, setLifting] = useState<PlayPause | null>(null);
  const [liftGrounds, setLiftGrounds] = useState("");
  const [submittingLift, setSubmittingLift] = useState(false);
  const [liftError, setLiftError] = useState<string | null>(null);

  // Spec 051 US5: the record. Two lists, each paged on its own cursor, because
  // the server pages pauses and requests separately and merging two cursors
  // into one order would show an entry out of place whenever one list had
  // been read further than the other.
  const [recordPauses, setRecordPauses] = useState<PlayPause[] | null>(null);
  const [pausesCursor, setPausesCursor] = useState<string | null>(null);
  const [recordRequests, setRecordRequests] = useState<PauseRequest[] | null>(
    null,
  );
  const [requestsCursor, setRequestsCursor] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState<"pauses" | "requests" | null>(
    null,
  );
  // How far each list has been read, so a refresh re-reads that much rather
  // than collapsing what an operator paged to. Requests count raw rows,
  // pending ones included, because the cursor does.
  const pausesDepth = useRef(RECORD_PAGE);
  const requestsDepth = useRef(RECORD_PAGE);
  const recordPausesHeadingRef = useRef<HTMLHeadingElement>(null);
  const recordRequestsHeadingRef = useRef<HTMLHeadingElement>(null);

  const loadRecord = useCallback(async () => {
    try {
      const [pauses, requested] = await Promise.all([
        listPlayPauses({
          first: Math.min(pausesDepth.current, RECORD_MAX_PAGE),
        }),
        // Every state, and the pending ones dropped here: they are the
        // *Requests* section's until an operator decides them.
        listPlayPauseRequests({
          state: null,
          first: Math.min(requestsDepth.current, RECORD_MAX_PAGE),
        }),
      ]);
      setRecordPauses(pauses.nodes);
      setPausesCursor(pauses.nextCursor);
      setRecordRequests(
        requested.nodes.filter((request) => request.state !== "PENDING"),
      );
      setRequestsCursor(requested.nextCursor);
    } catch (cause) {
      setError(messageOf(cause, "Failed to load the record"));
    }
  }, []);

  const loadActive = useCallback(async () => {
    try {
      setActive((await listPlayPauses({ active: true })).nodes);
    } catch (cause) {
      setError(messageOf(cause, "Failed to load the pauses in force"));
    }
  }, []);

  const loadRequests = useCallback(async () => {
    try {
      setRequests(
        (await listPlayPauseRequests({ state: "PENDING", first: 50 })).nodes,
      );
    } catch (cause) {
      setError(messageOf(cause, "Failed to load the pause requests"));
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    const refresh = () => {
      // Stable (no dependencies), so the timer is set once.
      if (mounted) void loadRecord();
      listPlayPauses({ active: true })
        .then((page) => {
          if (mounted) setActive(page.nodes);
        })
        .catch((cause: unknown) => {
          if (mounted) {
            setError(messageOf(cause, "Failed to load the pauses in force"));
          }
        });
      listPlayPauseRequests({ state: "PENDING", first: 50 })
        .then((page) => {
          if (mounted) setRequests(page.nodes);
        })
        .catch((cause: unknown) => {
          if (mounted) {
            setError(messageOf(cause, "Failed to load the pause requests"));
          }
        });
    };
    refresh();
    const timer = window.setInterval(() => {
      if (document.visibilityState === "visible") refresh();
    }, REFRESH_MS);
    return () => {
      mounted = false;
      window.clearInterval(timer);
    };
  }, [loadRecord]);

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
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setTarget(world);
    setGrounds("");
    setDialogError(null);
    setOutcome(null);
  };

  const openDecision = (request: PauseRequest, decision: PauseDecision) => {
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setDeciding({ request, decision });
    setNote("");
    setDecisionError(null);
    setOutcome(null);
  };

  const confirmDecision = async (event: FormEvent) => {
    event.preventDefault();
    if (!deciding) return;
    const written = note.trim();
    if (!written) {
      setDecisionError("Write a note for this decision.");
      return;
    }
    const { request, decision } = deciding;
    setSubmittingDecision(true);
    setDecisionError(null);
    try {
      const result = await decidePlayPauseRequest(
        request.id,
        decision,
        written,
      );
      const decided = result.request;
      if (!result.decidedHere) {
        // FR-035: not an error. Somebody got there first; say who and when.
        const who = decided.decidedBy?.name ?? "another operator";
        const when = decided.decidedAt
          ? formatMoment(decided.decidedAt)
          : "an earlier moment";
        const how = decided.state === "APPROVED" ? "approved" : "declined";
        setOutcome(
          `Already decided by ${who} at ${when}. The request to pause ${decided.worldName} was ${how}.`,
        );
      } else if (decision === "APPROVE") {
        setOutcome(`Play in ${decided.worldName} is paused.`);
      } else {
        setOutcome(
          `The request to pause ${decided.worldName} was declined. Nothing in the world changed.`,
        );
      }
      setDeciding(null);
      await Promise.all([loadRequests(), loadActive(), loadRecord()]);
    } catch (cause) {
      setDecisionError(messageOf(cause, "The request was not decided"));
    } finally {
      setSubmittingDecision(false);
    }
  };

  const openLift = (pause: PlayPause) => {
    openerRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setLifting(pause);
    setLiftGrounds("");
    setLiftError(null);
    setOutcome(null);
  };

  const confirmLift = async (event: FormEvent) => {
    event.preventDefault();
    if (!lifting) return;
    const written = liftGrounds.trim();
    if (!written) {
      setLiftError("Give the grounds for lifting the pause.");
      return;
    }
    setSubmittingLift(true);
    setLiftError(null);
    try {
      const lifted = await liftWorldPlayPause(lifting.id, written);
      setOutcome(`Play in ${lifted.worldName} is no longer paused.`);
      setLifting(null);
    } catch (cause) {
      const earlier = alreadyLiftedBy(cause);
      if (earlier) {
        // FR-040: not an error. Somebody lifted it first; say who and when.
        const who = earlier.liftedBy?.name ?? "another operator";
        const when = earlier.liftedAt
          ? formatMoment(earlier.liftedAt)
          : "an earlier moment";
        setOutcome(
          `Already lifted by ${who} at ${when}. Play in ${lifting.worldName} is no longer paused.`,
        );
        setLifting(null);
      } else {
        setLiftError(messageOf(cause, "The pause was not lifted"));
        return;
      }
    } finally {
      setSubmittingLift(false);
    }
    await Promise.all([loadActive(), loadRecord()]);
  };

  const showMore = async (which: "pauses" | "requests") => {
    setLoadingMore(which);
    try {
      if (which === "pauses" && pausesCursor) {
        const page = await listPlayPauses({
          first: RECORD_PAGE,
          after: pausesCursor,
        });
        pausesDepth.current += RECORD_PAGE;
        setRecordPauses((shown) => [...(shown ?? []), ...page.nodes]);
        setPausesCursor(page.nextCursor);
        if (!page.nextCursor) recordPausesHeadingRef.current?.focus();
      } else if (which === "requests" && requestsCursor) {
        const page = await listPlayPauseRequests({
          state: null,
          first: RECORD_PAGE,
          after: requestsCursor,
        });
        requestsDepth.current += RECORD_PAGE;
        setRecordRequests((shown) => [
          ...(shown ?? []),
          ...page.nodes.filter((request) => request.state !== "PENDING"),
        ]);
        setRequestsCursor(page.nextCursor);
        // The button goes with the last page; keep focus in the section.
        if (!page.nextCursor) recordRequestsHeadingRef.current?.focus();
      }
    } catch (cause) {
      setError(messageOf(cause, "Failed to load more of the record"));
    } finally {
      setLoadingMore(null);
    }
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
      await Promise.all([loadActive(), loadRecord()]);
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

          <Card className="grid gap-5 p-6" data-testid="play-pause-record">
            <div className="grid gap-1">
              <h2 className="text-lg font-semibold">Record</h2>
              <p className="max-w-[70ch] text-sm text-muted-foreground">
                Every pause, in force or lifted, and every decided request,
                newest first. The record stays when a world is deleted.
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
                            by {pause.pausedBy.name} ·{" "}
                            <code>{pause.worldId}</code>
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
                          {request.state === "APPROVED"
                            ? "Approved"
                            : "Declined"}
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
        </div>
      </AdminSectionShell>

      <Dialog
        open={target !== null}
        onOpenChange={(open) => {
          if (!open && !pausing) setTarget(null);
        }}
      >
        <DialogContent
          data-testid="play-pause-confirm"
          onCloseAutoFocus={(event) => {
            // After a pause, to the sentence saying what happened: the list
            // the dialog was opened from is re-fetched and re-rendered, so the
            // button that opened it may no longer exist. After a cancel,
            // nothing changed, so back to that button.
            event.preventDefault();
            const back = outcomeRef.current ?? openerRef.current;
            if (back?.isConnected) back.focus();
          }}
        >
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
      <Dialog
        open={deciding !== null}
        onOpenChange={(open) => {
          if (!open && !submittingDecision) setDeciding(null);
        }}
      >
        <DialogContent
          data-testid="play-pause-decide"
          onCloseAutoFocus={(event) => {
            // As the pause dialog: to what happened after a decision (the
            // request has left the list), back to the opener after a cancel.
            event.preventDefault();
            const back = outcomeRef.current ?? openerRef.current;
            if (back?.isConnected) back.focus();
          }}
        >
          <form
            className="grid gap-4"
            onSubmit={(e) => void confirmDecision(e)}
          >
            <DialogHeader>
              <DialogTitle>
                {deciding?.decision === "APPROVE"
                  ? `Approve, and pause play in ${deciding.request.worldName}?`
                  : `Decline the request to pause ${deciding?.request.worldName ?? ""}?`}
              </DialogTitle>
              <DialogDescription>
                {deciding?.decision === "APPROVE"
                  ? "Everyone playing in this world leaves the playfield within seconds and is told that an operator has paused play. Nobody can play in it again until an operator lifts the pause."
                  : "Play goes on. Nobody in the world is told anything, and the decision is recorded."}
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="play-pause-decision-note">Note</Label>
              <Textarea
                id="play-pause-decision-note"
                required
                aria-required="true"
                aria-describedby="play-pause-decision-note-hint"
                value={note}
                maxLength={5000}
                onChange={(event) => setNote(event.target.value)}
              />
              <p
                id="play-pause-decision-note-hint"
                className="text-xs text-muted-foreground"
              >
                {deciding?.decision === "APPROVE"
                  ? "Required. Recorded with your name as the pause's grounds, and never shown to anyone in the world."
                  : "Required. Recorded with your name, and never shown to anyone in the world."}
              </p>
              {decisionError ? (
                <p role="alert" className="text-sm text-destructive">
                  {decisionError}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={submittingDecision}
                onClick={() => setDeciding(null)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="primary"
                disabled={submittingDecision || note.trim() === ""}
                data-testid="play-pause-decide-submit"
              >
                {deciding?.decision === "APPROVE"
                  ? "Approve and pause play"
                  : "Decline request"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
      <Dialog
        open={lifting !== null}
        onOpenChange={(open) => {
          if (!open && !submittingLift) setLifting(null);
        }}
      >
        <DialogContent
          data-testid="play-pause-lift"
          onCloseAutoFocus={(event) => {
            // As the other dialogs: to what happened after a lift (the pause
            // has left *Active pauses*), back to the opener after a cancel.
            event.preventDefault();
            const back = outcomeRef.current ?? openerRef.current;
            if (back?.isConnected) back.focus();
          }}
        >
          <form className="grid gap-4" onSubmit={(e) => void confirmLift(e)}>
            <DialogHeader>
              <DialogTitle>Lift the pause on {lifting?.worldName}?</DialogTitle>
              <DialogDescription>
                Its Game Master and players can play in this world again.
                Nothing else about the world changes, and anything taken down
                separately stays withheld.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-2">
              <Label htmlFor="play-pause-lift-grounds">Grounds</Label>
              <Textarea
                id="play-pause-lift-grounds"
                required
                aria-required="true"
                aria-describedby="play-pause-lift-grounds-hint"
                value={liftGrounds}
                maxLength={5000}
                onChange={(event) => setLiftGrounds(event.target.value)}
              />
              <p
                id="play-pause-lift-grounds-hint"
                className="text-xs text-muted-foreground"
              >
                Required. Recorded with your name, and never shown to anyone in
                the world.
              </p>
              {liftError ? (
                <p role="alert" className="text-sm text-destructive">
                  {liftError}
                </p>
              ) : null}
            </div>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={submittingLift}
                onClick={() => setLifting(null)}
              >
                Cancel
              </Button>
              <Button
                type="submit"
                variant="primary"
                disabled={submittingLift || liftGrounds.trim() === ""}
                data-testid="play-pause-lift-submit"
              >
                Lift pause
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}
