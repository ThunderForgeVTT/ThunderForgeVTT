import { useState, type FormEvent } from "react";
import { legalStatement } from "@/legal/legalDocuments";
import { submitCounterNotice } from "@/api/moderation";
import { Button } from "@/components/ui/button/Button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import type { ModerationCaseRecord } from "@/types/moderation";

export interface CounterNoticeFormProps {
  caseId: string;
  onSubmitted: (record: ModerationCaseRecord) => void;
}

/**
 * Spec 015 (FR-005, US2): the owner-facing counter-notice flow, shown from
 * `ModeratedContentBanner` only to the disabled content's owner. Requires
 * an authenticated session — `submitCounterNotice` checks world ownership
 * server-side regardless (Principle III).
 */
export function CounterNoticeForm({
  caseId,
  onSubmitted,
}: CounterNoticeFormProps) {
  const [removedMaterialDescription, setRemovedMaterialDescription] =
    useState("");
  const [contactInformation, setContactInformation] = useState("");
  const [goodFaithMistakeStatement, setGoodFaithMistakeStatement] =
    useState(false);
  const [consentToJurisdiction, setConsentToJurisdiction] = useState(false);
  const [signature, setSignature] = useState("");

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const outcome = await submitCounterNotice({
        caseId,
        removedMaterialDescription,
        goodFaithMistakeStatement,
        consentToJurisdiction,
        contactInformation,
        signature,
      });
      onSubmitted(outcome);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to submit counter-notice",
      );
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form
      onSubmit={(e) => void handleSubmit(e)}
      className="grid gap-3"
      data-testid="counter-notice-form"
    >
      <Field
        label="Identification of the removed material"
        htmlFor="counter-notice-material"
      >
        <Textarea
          id="counter-notice-material"
          value={removedMaterialDescription}
          onChange={(e) => setRemovedMaterialDescription(e.target.value)}
          rows={2}
          required
        />
      </Field>

      <Field label="Your contact information" htmlFor="counter-notice-contact">
        <Input
          id="counter-notice-contact"
          value={contactInformation}
          onChange={(e) => setContactInformation(e.target.value)}
          placeholder="Email or mailing address"
          required
        />
      </Field>

      <div className="flex items-start gap-2">
        <Checkbox
          id="counter-notice-good-faith"
          checked={goodFaithMistakeStatement}
          onCheckedChange={(v) => setGoodFaithMistakeStatement(v === true)}
        />
        <Label
          htmlFor="counter-notice-good-faith"
          className="text-sm font-normal"
        >
          {legalStatement("notice-attestations", "counter-notice-good-faith")}
        </Label>
      </div>

      <div className="flex items-start gap-2">
        <Checkbox
          id="counter-notice-jurisdiction"
          checked={consentToJurisdiction}
          onCheckedChange={(v) => setConsentToJurisdiction(v === true)}
        />
        <Label
          htmlFor="counter-notice-jurisdiction"
          className="text-sm font-normal"
        >
          {legalStatement("notice-attestations", "counter-notice-jurisdiction")}
        </Label>
      </div>

      <Field
        label="Signature (type your full legal name)"
        htmlFor="counter-notice-signature"
      >
        <Input
          id="counter-notice-signature"
          value={signature}
          onChange={(e) => setSignature(e.target.value)}
          required
        />
      </Field>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      <Button
        type="submit"
        variant="secondary"
        disabled={submitting}
        data-testid="counter-notice-submit"
      >
        {submitting ? "Submitting..." : "File counter-notice"}
      </Button>
    </form>
  );
}
