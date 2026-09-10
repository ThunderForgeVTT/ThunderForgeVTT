import { useEffect, useState } from "react";
import {
  acknowledgeOperatorStatement,
  type OperatorAcknowledgementState,
  readOperatorAcknowledgement,
} from "@/api/operatorAcknowledgement";
import {
  readOperatorStatement,
  type VersionedLegalDocument,
} from "@/api/sharingTerms";
import { LegalProse } from "@/components/legal/LegalProse";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";

/**
 * Spec 039 T085 (FR-044): when an upgrade has changed the operator statement,
 * the administrator is shown the words and asked to acknowledge them — not
 * left holding an acknowledgement of words the instance no longer ships.
 *
 * Renders nothing while the acknowledgement is current, which is almost
 * always.
 */
export function OperatorAcknowledgementBanner() {
  const [state, setState] = useState<OperatorAcknowledgementState | null>(null);
  const [statement, setStatement] = useState<VersionedLegalDocument | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let active = true;
    void Promise.all([readOperatorAcknowledgement(), readOperatorStatement()])
      .then(([acknowledgement, document]) => {
        if (active) {
          setState(acknowledgement);
          setStatement(document);
        }
      })
      .catch(() => {
        // Not worth an error box on the landing page: the banner exists to
        // ask a question, and if it cannot ask it, the rest of the page is
        // still what the administrator came for.
      });
    return () => {
      active = false;
    };
  }, []);

  if (!state || state.isCurrent || !statement) {
    return null;
  }

  const acknowledge = async () => {
    setBusy(true);
    setError(null);
    try {
      setState(await acknowledgeOperatorStatement(statement.versionId));
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : "The acknowledgement was not recorded",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card
      className="grid gap-4 p-6"
      data-testid="operator-acknowledgement-banner"
    >
      <div className="grid gap-1">
        <h2 className="text-lg font-semibold">
          The operator responsibilities have changed
        </h2>
        <p className="text-sm text-muted-foreground">
          {state.acknowledgement
            ? "This instance was set up under an earlier version. Read what you are responsible for now, and acknowledge it."
            : "This instance has no record of its operator acknowledging these. Read them, and acknowledge them."}
        </p>
      </div>
      <div className="grid max-h-80 gap-3 overflow-y-auto rounded border p-4">
        {statement.sections.map((section, index) => (
          <section
            key={`${section.heading ?? ""}-${index}`}
            className="grid gap-1"
          >
            {section.heading ? (
              <h3 className="text-sm font-semibold">{section.heading}</h3>
            ) : null}
            <LegalProse body={section.body} />
          </section>
        ))}
      </div>
      <Button
        className="justify-self-start"
        disabled={busy}
        onClick={() => void acknowledge()}
        data-testid="operator-acknowledge"
      >
        I have read this and I take it on
      </Button>
      {error ? <p className="text-xs text-destructive">{error}</p> : null}
    </Card>
  );
}
