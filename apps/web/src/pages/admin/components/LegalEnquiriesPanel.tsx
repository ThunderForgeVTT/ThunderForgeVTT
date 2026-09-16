import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  fetchLegalEnquiries,
  setLegalEnquiryStatus,
  type LegalEnquiry,
  type LegalEnquiryKind,
  type LegalEnquiryStatus,
} from "@/api/legalEnquiries";

/**
 * Terms disputes and privacy requests, as a queue the operator works.
 *
 * # Why takedowns are linked rather than listed
 *
 * They are moderation cases, not enquiries: a takedown carries statutory
 * elements and its acceptance disables content, so it has its own table, its
 * own validation and its own review screen with counter-notice handling. This
 * panel points at that screen instead of rendering a second, weaker view of
 * the same cases — the alternative is two places to work one queue, which is
 * how a case gets actioned twice or missed entirely.
 *
 * # Oldest first
 *
 * An intake queue is worked front to back. Every other admin list here is
 * newest-first because it is an audit trail; this one is a backlog, and the
 * person waiting longest is the one to answer next.
 *
 * # Why the tabs carry counts, and why one fetch feeds all three
 *
 * The owner's report was "the tabs do absolutely nothing". They always did —
 * the panel refetched by `kind` — but on an instance with no enquiries all
 * three tabs drew the same three words, so clicking one changed nothing
 * visible and the control read as dead. That is a real defect even though the
 * code was correct: a filter with no feedback is indistinguishable from a
 * filter that is not wired up.
 *
 * So every enquiry is read once and the tabs filter what is already here.
 * That buys the two things that make a tab look alive: a count on each one
 * before it is clicked, and an empty state that names *what would be here*
 * rather than a shared "nothing outstanding". It costs nothing — the server
 * has no paging on this query and the queue is small by construction; a
 * backlog large enough to matter is a backlog nobody is working.
 */

interface Tab {
  label: string;
  kind?: LegalEnquiryKind;
  /** What would be here, and what put it here. Shown when nothing is. */
  empty: string;
}

const TABS: Tab[] = [
  {
    label: "All",
    empty:
      "Nothing has been sent to this instance. Terms disputes and privacy requests both land here when somebody submits one.",
  },
  {
    label: "Terms disputes",
    kind: "TERMS",
    empty:
      "No terms disputes. These arrive from the “dispute these terms” form on the terms of service page.",
  },
  {
    label: "Privacy requests",
    kind: "PRIVACY",
    empty:
      "No privacy requests. These arrive from the privacy policy page, where a person asks to see, correct or erase what this instance holds about them.",
  },
];

const STATUS_VARIANT: Record<string, "info" | "warning" | "success"> = {
  open: "warning",
  acknowledged: "info",
  closed: "success",
};

export function LegalEnquiriesPanel() {
  const [kind, setKind] = useState<LegalEnquiryKind | undefined>(undefined);
  const [showClosed, setShowClosed] = useState(false);
  const [enquiries, setEnquiries] = useState<LegalEnquiry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(() => {
    fetchLegalEnquiries(undefined)
      .then((rows) => {
        setEnquiries(rows);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "The enquiries could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  const move = async (id: string, status: LegalEnquiryStatus) => {
    setBusyId(id);
    try {
      const updated = await setLegalEnquiryStatus(id, status);
      setEnquiries((current) =>
        current
          ? current.map((row) => (row.id === updated.id ? updated : row))
          : current,
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "That was refused.");
    } finally {
      setBusyId(null);
    }
  };

  // The server sends `kind` lower-cased on a row and upper-cased in the
  // filter argument. Compared case-insensitively rather than by trusting
  // either, because getting that wrong shows an empty tab that looks exactly
  // like an empty queue — which is the defect this whole pass is about.
  const counts = useMemo(() => {
    const open = (enquiries ?? []).filter((row) => row.status !== "closed");
    return new Map(
      TABS.map((tab) => [
        tab.kind ?? "all",
        tab.kind
          ? open.filter(
              (row) => row.kind.toUpperCase() === tab.kind?.toUpperCase(),
            ).length
          : open.length,
      ]),
    );
  }, [enquiries]);

  const activeTab = TABS.find((tab) => tab.kind === kind) ?? TABS[0];

  const visible = (enquiries ?? []).filter(
    (row) =>
      (showClosed || row.status !== "closed") &&
      (!kind || row.kind.toUpperCase() === kind),
  );

  return (
    <div className="grid gap-4" data-testid="legal-enquiries-panel">
      {/* Takedowns belong to the moderation queue, which handles the statutory
          side and the counter-notice window. Prominent rather than a footnote:
          an operator who came here holding a copyright notice is holding the
          one thing this screen does not do, and finding that out at the bottom
          is finding it out too late. */}
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-primary/20 bg-primary/5 p-4">
        <p className="max-w-[60ch] text-sm">
          <strong>Holding a copyright takedown notice?</strong> Those are worked
          in Moderation, where the counter-notice window and repeat-infringer
          flags live. They are never listed here.
        </p>
        <Button asChild variant="secondary" size="sm" icon="quill">
          <Link to="/admin/moderation">Go to Moderation</Link>
        </Button>
      </div>

      <div
        className="flex flex-wrap items-center gap-2"
        role="tablist"
        aria-label="Enquiry kinds"
      >
        {TABS.map((tab) => {
          const selected = kind === tab.kind;
          const count = counts.get(tab.kind ?? "all") ?? 0;
          return (
            <Button
              key={tab.label}
              type="button"
              role="tab"
              aria-selected={selected}
              variant={selected ? "secondary" : "ghost"}
              size="sm"
              onClick={() => setKind(tab.kind)}
              data-testid={`legal-enquiries-tab-${tab.kind ?? "all"}`}
            >
              {tab.label}
              {/* The count is what makes an empty tab read as empty rather
                  than as inert. Spoken as words so it is not a bare number
                  beside a label a screen reader has already moved past. */}
              <span
                className="ml-1.5 rounded-full bg-secondary px-1.5 py-0.5 text-xs tabular-nums"
                aria-label={`${count} open`}
              >
                {count}
              </span>
            </Button>
          );
        })}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-pressed={showClosed}
          onClick={() => setShowClosed((current) => !current)}
          data-testid="legal-enquiries-toggle-closed"
        >
          {showClosed ? "Hide closed" : "Show closed"}
        </Button>
      </div>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      {enquiries === null ? (
        <p className="text-muted-foreground">Reading the queue...</p>
      ) : visible.length === 0 ? (
        <div
          className="grid gap-1 rounded-lg border border-dashed border-border p-6 text-center"
          data-testid="legal-enquiries-empty"
        >
          <p className="font-semibold">Nothing outstanding</p>
          <p className="mx-auto max-w-[60ch] text-sm text-muted-foreground">
            {activeTab.empty}
          </p>
          {!showClosed ? (
            <p className="text-sm text-muted-foreground">
              Anything already closed is hidden — “Show closed” brings it back.
            </p>
          ) : null}
        </div>
      ) : (
        <ul className="grid gap-3">
          {visible.map((row) => (
            <li
              key={row.id}
              className="grid gap-2 rounded-lg border border-border bg-secondary/40 p-4"
              data-testid={`legal-enquiry-${row.id}`}
              data-kind={row.kind}
              data-status={row.status}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <strong>{row.subject}</strong>
                <StatusBadge variant={STATUS_VARIANT[row.status] ?? "info"}>
                  {row.status}
                </StatusBadge>
              </div>
              <p className="text-sm text-muted-foreground">
                {row.kind === "terms" ? "Terms dispute" : "Privacy request"} ·{" "}
                {row.submitterName} · {row.submitterContact} ·{" "}
                {new Date(row.createdAt).toLocaleString()}
              </p>
              <p className="text-sm whitespace-pre-wrap">{row.body}</p>
              {row.resolutionNote ? (
                <p className="text-sm text-muted-foreground">
                  {row.resolutionNote}
                </p>
              ) : null}
              <div className="flex flex-wrap gap-2">
                {row.status !== "acknowledged" ? (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={busyId === row.id}
                    onClick={() => void move(row.id, "ACKNOWLEDGED")}
                    data-testid={`legal-enquiry-acknowledge-${row.id}`}
                  >
                    Acknowledge
                  </Button>
                ) : null}
                {row.status !== "closed" ? (
                  <Button
                    type="button"
                    variant="secondary"
                    size="sm"
                    disabled={busyId === row.id}
                    onClick={() => void move(row.id, "CLOSED")}
                    data-testid={`legal-enquiry-close-${row.id}`}
                  >
                    Close
                  </Button>
                ) : (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={busyId === row.id}
                    onClick={() => void move(row.id, "OPEN")}
                    data-testid={`legal-enquiry-reopen-${row.id}`}
                  >
                    Reopen
                  </Button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
