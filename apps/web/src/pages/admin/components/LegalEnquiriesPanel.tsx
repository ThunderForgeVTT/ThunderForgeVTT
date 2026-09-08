import { useCallback, useEffect, useState } from "react";
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
 */

const TABS: { label: string; kind?: LegalEnquiryKind }[] = [
  { label: "All" },
  { label: "Terms disputes", kind: "TERMS" },
  { label: "Privacy requests", kind: "PRIVACY" },
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
    fetchLegalEnquiries(kind)
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
  }, [kind]);

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

  const visible = (enquiries ?? []).filter(
    (row) => showClosed || row.status !== "closed",
  );

  return (
    <div className="grid gap-4" data-testid="legal-enquiries-panel">
      <div className="flex flex-wrap items-center gap-2">
        {TABS.map((tab) => (
          <Button
            key={tab.label}
            type="button"
            variant={kind === tab.kind ? "secondary" : "ghost"}
            size="sm"
            onClick={() => setKind(tab.kind)}
            data-testid={`legal-enquiries-tab-${tab.kind ?? "all"}`}
          >
            {tab.label}
          </Button>
        ))}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => setShowClosed((current) => !current)}
          data-testid="legal-enquiries-toggle-closed"
        >
          {showClosed ? "Hide closed" : "Show closed"}
        </Button>
      </div>

      {/* Takedowns belong to the moderation queue, which handles the statutory
          side and the counter-notice window. Named here so an operator looking
          for "the legal inbox" finds all three. */}
      <p className="text-sm text-muted-foreground">
        Copyright takedown notices are worked in{" "}
        <Link
          className="underline hover:text-foreground"
          to="/admin/moderation"
        >
          Moderation
        </Link>
        , where the counter-notice window and repeat-infringer flags live.
      </p>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      {enquiries === null ? (
        <p className="text-muted-foreground">Reading the queue...</p>
      ) : visible.length === 0 ? (
        <p
          className="text-muted-foreground"
          data-testid="legal-enquiries-empty"
        >
          Nothing outstanding.
        </p>
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
