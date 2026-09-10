import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  readSharingTerms,
  type VersionedLegalDocument,
} from "@/api/sharingTerms";

/**
 * Spec 039 US1: the one place anything in this app asks somebody to attest
 * before publishing.
 *
 * # One dialog, four paths
 *
 * Collections showed these terms; the actor, item and ability paths showed
 * nothing at all — and ADR-071 made all three readable without an account, so
 * they publish exactly as much as a collection does. Somebody sharing a
 * character sheet full of transcribed rules text was being asked to agree to
 * nothing.
 *
 * Four copies of this would be four places for the wording, the version
 * handling and the refusal to drift.
 *
 * # Short on purpose
 *
 * SC-007 asks that sharing five things in a row stay a task somebody will
 * finish. So: the terms as they are, one button, and no scroll trap. The words
 * are two short sections because `legal/sharing-terms.md` is deliberately two
 * short sections — a wall of text at a button is a wall of text nobody reads.
 *
 * # What it hands back
 *
 * The `versionId` the server sent, unmodified. Not the text, not a timestamp,
 * not an identity — every one of those is the server's to write (FR-014). A
 * client that could supply them is a client that could write its own evidence.
 */
interface AttestationDialogProps {
  /** What is about to be published, in the words of the surface asking. */
  publishing: string;
  /** Called with the version identity to echo back. */
  onAgree: (termsVersionId: string) => void;
  onCancel: () => void;
  /** True while the publish this dialog authorised is in flight. */
  busy?: boolean;
}

export function AttestationDialog({
  publishing,
  onAgree,
  onCancel,
  busy = false,
}: AttestationDialogProps) {
  const [terms, setTerms] = useState<VersionedLegalDocument | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void readSharingTerms()
      .then((document) => {
        if (active) {
          setTerms(document);
        }
      })
      .catch((cause: unknown) => {
        if (active) {
          setError(
            cause instanceof Error
              ? cause.message
              : "The sharing agreement could not be loaded.",
          );
        }
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <div
      className="grid gap-3 rounded-lg border border-input p-4"
      data-testid="share-terms"
    >
      {error ? (
        <>
          <StatusBadge variant="danger" data-testid="share-terms-error">
            {error}
          </StatusBadge>
          {/*
           * No agreement, no publish. The button is not offered rather than
           * offered-and-disabled, because there is nothing here for somebody
           * to agree *to* — and a client that published anyway would be
           * refused by the server, which is the right answer arriving in the
           * wrong place.
           */}
          <p className="text-sm text-muted-foreground">
            Sharing needs the current agreement, and this instance could not
            provide it. Reload the page and try again.
          </p>
        </>
      ) : !terms ? (
        <p className="text-sm text-muted-foreground">
          Loading the sharing agreement&hellip;
        </p>
      ) : (
        <>
          {terms.sections.map((section) => (
            <div key={section.heading ?? "opening"} className="grid gap-1">
              {section.heading ? (
                <h4 className="font-semibold">{section.heading}</h4>
              ) : null}
              <p className="text-sm text-muted-foreground whitespace-pre-line">
                {section.body}
              </p>
            </div>
          ))}
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="primary"
              icon="spark"
              disabled={busy}
              data-testid="share-terms-agree"
              onClick={() => onAgree(terms.versionId)}
            >
              {busy
                ? "Sharing…"
                : `I have the right to share this ${publishing}`}
            </Button>
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              data-testid="share-terms-cancel"
              onClick={onCancel}
            >
              Cancel
            </Button>
          </div>
          {/*
           * FR-005: declining publishes nothing and discards nothing. Worth
           * saying, because a person assembling a collection needs to know
           * that backing out here does not undo their work.
           */}
          <p className="text-xs text-muted-foreground">
            Cancelling changes nothing. Nothing is shared until you agree.
          </p>
        </>
      )}
    </div>
  );
}
