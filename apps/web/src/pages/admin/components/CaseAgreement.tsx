import { useState } from "react";
import {
  type Attestation,
  coveredKindForCase,
  getAttestationsFor,
} from "@/api/attestations";
import { Button } from "@/components/ui/button/Button";
import { Loader } from "@/components/ui/loader/Loader";

interface CaseAgreementProps {
  entityType: string;
  entityId: string;
}

/**
 * Spec 039 SC-003: from a moderation case to the agreements behind it — who
 * agreed, when, and the exact words — without a developer writing a query.
 *
 * Includes the agreements of every collection the content is in, because a
 * shared collection publishes its members, and for lore that is the only way
 * it is ever published.
 *
 * Loaded on request rather than with the case, because each agreement carries
 * a full copy of the terms and most cases are looked at for other reasons.
 */
export function CaseAgreement({ entityType, entityId }: CaseAgreementProps) {
  const kind = coveredKindForCase(entityType);
  const [records, setRecords] = useState<Attestation[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!kind) {
    return null;
  }

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      setRecords(await getAttestationsFor(kind, entityId));
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load the agreement",
      );
    } finally {
      setLoading(false);
    }
  };

  if (records === null) {
    return (
      <div className="grid gap-1">
        <Button
          variant="secondary"
          size="sm"
          className="justify-self-start"
          data-testid="case-agreement-open"
          disabled={loading}
          onClick={() => void load()}
        >
          Show the agreement
        </Button>
        {loading ? <Loader label="Loading the agreement" /> : null}
        {error ? <p className="text-xs text-destructive">{error}</p> : null}
      </div>
    );
  }

  if (records.length === 0) {
    return (
      <p className="text-xs text-muted-foreground" data-testid="case-agreement">
        Nobody has agreed to publish this, on its own or in any collection it is
        currently in.
      </p>
    );
  }

  return (
    <ul className="grid gap-2" data-testid="case-agreement">
      {records.map((record) => (
        <li
          key={record.id}
          className="grid gap-1 rounded border p-3 text-sm"
          data-testid="case-agreement-record"
        >
          <p>
            <span className="font-semibold">
              {record.subjectUsername ?? "A deleted account"}
            </span>{" "}
            agreed on{" "}
            <time dateTime={record.attestedAt}>
              {new Date(record.attestedAt).toLocaleString()}
            </time>
          </p>
          {record.publishableKind === "collection" && kind !== "collection" ? (
            <p className="text-xs" data-testid="case-agreement-via-collection">
              As part of the collection <code>{record.publishableId}</code>
            </p>
          ) : null}
          <p className="text-xs text-muted-foreground">
            Terms <code>{record.termsVersionId}</code>
          </p>
          <details>
            <summary className="cursor-pointer text-xs">
              The words agreed to
            </summary>
            <div className="mt-2 grid gap-2" data-testid="case-agreement-terms">
              {record.terms.sections.map((section, index) => (
                <section key={`${record.id}-${index}`} className="grid gap-1">
                  {section.heading ? (
                    <h4 className="text-xs font-semibold">{section.heading}</h4>
                  ) : null}
                  <p className="text-xs whitespace-pre-line">{section.body}</p>
                </section>
              ))}
            </div>
          </details>
        </li>
      ))}
    </ul>
  );
}
