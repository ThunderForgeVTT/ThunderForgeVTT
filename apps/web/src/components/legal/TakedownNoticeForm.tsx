import { useState, type FormEvent } from "react";
import { legalStatement } from "@/legal/legalDocuments";
import { submitTakedownNotice } from "@/api/moderation";
import { Button } from "@/components/ui/button/Button";
import { Checkbox } from "@/components/ui/checkbox";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import type {
  ModerationCaseRecord,
  ModerationEntityType,
} from "@/types/moderation";

/**
 * Spec 015 (FR-002, FR-003): the public takedown intake channel. Collects
 * every statutory element (17 U.S.C. § 512(c)(3)) and submits directly —
 * no login required. A statutorily-incomplete submission still logs a
 * case (never silently dropped) and tells the submitter exactly what's
 * missing.
 */
export function TakedownNoticeForm() {
  const [entityType, setEntityType] =
    useState<ModerationEntityType>("WORLD_ACTOR");
  const [entityId, setEntityId] = useState("");
  const [claimantName, setClaimantName] = useState("");
  const [claimantContact, setClaimantContact] = useState("");
  const [copyrightedWorkDescription, setCopyrightedWorkDescription] =
    useState("");
  const [infringingMaterialLocation, setInfringingMaterialLocation] =
    useState("");
  const [goodFaithStatement, setGoodFaithStatement] = useState(false);
  const [accuracyStatement, setAccuracyStatement] = useState(false);
  const [signature, setSignature] = useState("");

  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<ModerationCaseRecord | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    setResult(null);
    try {
      const outcome = await submitTakedownNotice({
        entityType,
        entityId,
        claimantName,
        claimantContact,
        copyrightedWorkDescription,
        infringingMaterialLocation,
        goodFaithStatement,
        accuracyStatement,
        signature,
      });
      setResult(outcome);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to submit notice");
    } finally {
      setSubmitting(false);
    }
  };

  if (result) {
    if (result.currentStatus === "NOTICE_REJECTED_INCOMPLETE") {
      return (
        <div className="grid gap-3" data-testid="takedown-notice-rejected">
          <StatusBadge variant="danger">Notice incomplete</StatusBadge>
          <p className="text-sm text-muted-foreground">
            Your notice is missing required elements and was not actioned:
          </p>
          <ul className="list-disc pl-5 text-sm">
            {(result.events.at(-1)?.missingElements ?? []).map((field) => (
              <li key={field}>{field}</li>
            ))}
          </ul>
        </div>
      );
    }
    return (
      <div className="grid gap-2" data-testid="takedown-notice-accepted">
        <StatusBadge variant="success">Notice received</StatusBadge>
        <p className="text-sm text-muted-foreground">
          Case reference: <code>{result.caseId}</code>. The identified content
          has been disabled and its owner has been notified.
        </p>
      </div>
    );
  }

  return (
    <form
      onSubmit={(e) => void handleSubmit(e)}
      className="grid gap-4"
      data-testid="takedown-notice-form"
    >
      <Field label="Content type" htmlFor="dmca-entity-type">
        <Select
          value={entityType}
          onValueChange={(v) => setEntityType(v as ModerationEntityType)}
        >
          <SelectTrigger id="dmca-entity-type" aria-label="Content type">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="WORLD_ACTOR">Actor / NPC / character</SelectItem>
            <SelectItem value="WORLD_ITEM">Item</SelectItem>
            <SelectItem value="WORLD_LORE_ENTRY">Lore entry</SelectItem>
          </SelectContent>
        </Select>
      </Field>

      <Field label="Content ID (from the page URL)" htmlFor="dmca-entity-id">
        <Input
          id="dmca-entity-id"
          value={entityId}
          onChange={(e) => setEntityId(e.target.value)}
          placeholder="e.g. the actor/item/lore-entry ID"
          required
        />
      </Field>

      <Field label="Your name" htmlFor="dmca-claimant-name">
        <Input
          id="dmca-claimant-name"
          value={claimantName}
          onChange={(e) => setClaimantName(e.target.value)}
          required
        />
      </Field>

      <Field label="Your contact information" htmlFor="dmca-claimant-contact">
        <Input
          id="dmca-claimant-contact"
          value={claimantContact}
          onChange={(e) => setClaimantContact(e.target.value)}
          placeholder="Email or mailing address"
          required
        />
      </Field>

      <Field
        label="Identification of the copyrighted work"
        htmlFor="dmca-work-description"
      >
        <Textarea
          id="dmca-work-description"
          value={copyrightedWorkDescription}
          onChange={(e) => setCopyrightedWorkDescription(e.target.value)}
          rows={3}
          required
        />
      </Field>

      <Field
        label="Location of the allegedly infringing material"
        htmlFor="dmca-infringing-location"
      >
        <Textarea
          id="dmca-infringing-location"
          value={infringingMaterialLocation}
          onChange={(e) => setInfringingMaterialLocation(e.target.value)}
          rows={2}
          placeholder="URL or specific content ID identifying the exact material"
          required
        />
      </Field>

      <div className="flex items-start gap-2">
        <Checkbox
          id="dmca-good-faith"
          checked={goodFaithStatement}
          onCheckedChange={(v) => setGoodFaithStatement(v === true)}
        />
        <Label htmlFor="dmca-good-faith" className="text-sm font-normal">
          {legalStatement("notice-attestations", "takedown-good-faith")}
        </Label>
      </div>

      <div className="flex items-start gap-2">
        <Checkbox
          id="dmca-accuracy"
          checked={accuracyStatement}
          onCheckedChange={(v) => setAccuracyStatement(v === true)}
        />
        <Label htmlFor="dmca-accuracy" className="text-sm font-normal">
          {legalStatement("notice-attestations", "takedown-accuracy")}
        </Label>
      </div>

      <Field
        label="Signature (type your full legal name)"
        htmlFor="dmca-signature"
      >
        <Input
          id="dmca-signature"
          value={signature}
          onChange={(e) => setSignature(e.target.value)}
          required
        />
      </Field>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      <Button
        type="submit"
        disabled={submitting}
        data-testid="takedown-notice-submit"
      >
        {submitting ? "Submitting..." : "Submit takedown notice"}
      </Button>
    </form>
  );
}
