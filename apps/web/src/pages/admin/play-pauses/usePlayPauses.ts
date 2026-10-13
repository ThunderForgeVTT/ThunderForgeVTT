import {
  type FormEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  alreadyLiftedBy,
  decidePlayPauseRequest,
  liftWorldPlayPause,
  listPlayPauseRequests,
  listPlayPauses,
  type PauseCandidateWorld,
  type PauseDecision,
  type PauseRequest,
  pauseWorldPlay,
  type PlayPause,
  searchPauseCandidates,
} from "@/api/playPause";
import {
  formatMoment,
  messageOf,
  RECORD_MAX_PAGE,
  RECORD_PAGE,
  REFRESH_MS,
} from "./playPauseFormat";

export interface PendingDecision {
  request: PauseRequest;
  decision: PauseDecision;
}

/** The state and actions behind the Play pauses page (spec 051). */
export function usePlayPauses() {
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

  return {
    search,
    setSearch,
    candidates,
    searching,
    active,
    error,
    outcome,
    outcomeRef,
    openerRef,
    target,
    setTarget,
    grounds,
    setGrounds,
    pausing,
    dialogError,
    requests,
    deciding,
    setDeciding,
    note,
    setNote,
    submittingDecision,
    decisionError,
    lifting,
    setLifting,
    liftGrounds,
    setLiftGrounds,
    submittingLift,
    liftError,
    recordPauses,
    pausesCursor,
    recordRequests,
    requestsCursor,
    loadingMore,
    recordPausesHeadingRef,
    recordRequestsHeadingRef,
    submitSearch,
    openConfirm,
    openDecision,
    confirmDecision,
    openLift,
    confirmLift,
    showMore,
    confirmPause,
  };
}

export type PlayPausesState = ReturnType<typeof usePlayPauses>;
