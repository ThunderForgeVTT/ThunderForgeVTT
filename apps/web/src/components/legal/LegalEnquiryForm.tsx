import { useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Textarea } from "@/components/ui/textarea";
import {
  submitLegalEnquiry,
  type LegalEnquiryKind,
} from "@/api/legalEnquiries";

/**
 * The intake form on the terms and privacy pages.
 *
 * # Why a form instead of an address
 *
 * An address published on a page is scraped within days, and the operator of
 * a small instance then has a spam problem attached to their personal mailbox.
 * The form reaches the same person without publishing where they live.
 *
 * The copyright page is the exception and keeps its published contact: 17
 * U.S.C. §512(c)(2) conditions the safe harbour on the designated agent's
 * name, address and email being publicly available, and losing that protection
 * to save an operator some spam is not a trade worth making. So that page has
 * a form *and* the designation, in that order.
 *
 * # It does not require an account, and must not start to
 *
 * Somebody disputing the terms of service is frequently disputing the terms
 * they were asked to accept. A complaints channel reachable only after
 * accepting them is not a complaints channel. The submission goes to
 * `/api/graphql/public`; being signed in is recorded when true and is never
 * required.
 *
 * # What it promises
 *
 * A reply to the address given, and nothing else. There is deliberately no
 * reference number: the person has no account and no way to read the enquiry
 * back, so an identifier would imply a status page that does not exist.
 */

export interface LegalEnquiryFormProps {
  kind: LegalEnquiryKind;
  /** What this form is for, in the operator's words. */
  heading: string;
  description: string;
}

export function LegalEnquiryForm({
  kind,
  heading,
  description,
}: LegalEnquiryFormProps) {
  const [name, setName] = useState("");
  const [contact, setContact] = useState("");
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [receipt, setReceipt] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const testIdPrefix = `legal-enquiry-${kind.toLowerCase()}`;

  const onSubmit = async () => {
    setBusy(true);
    setFailure(null);
    setReceipt(null);
    try {
      const result = await submitLegalEnquiry({
        kind,
        submitterName: name,
        submitterContact: contact,
        subject,
        body,
      });
      setReceipt(result.message);
      // Cleared only on success. A refusal keeps every field, because
      // retyping four fields after a validation message is the fastest way to
      // make somebody give up on complaining.
      setName("");
      setContact("");
      setSubject("");
      setBody("");
    } catch (error) {
      setFailure(
        error instanceof Error
          ? error.message
          : "This could not be sent. Try again shortly.",
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid gap-4" data-testid={testIdPrefix}>
      <div className="grid gap-1">
        <h3 className="text-lg font-semibold">{heading}</h3>
        <p className="text-muted-foreground">{description}</p>
      </div>

      <Field label="Your name" htmlFor={`${testIdPrefix}-name`}>
        <Input
          id={`${testIdPrefix}-name`}
          data-testid={`${testIdPrefix}-name`}
          value={name}
          autoComplete="name"
          onChange={(event) => setName(event.target.value)}
        />
      </Field>

      <Field
        label="Email address"
        htmlFor={`${testIdPrefix}-contact`}
        hint="Where the reply goes. It is stored only so this can be answered."
      >
        <Input
          id={`${testIdPrefix}-contact`}
          data-testid={`${testIdPrefix}-contact`}
          type="email"
          autoComplete="email"
          value={contact}
          onChange={(event) => setContact(event.target.value)}
        />
      </Field>

      <Field label="Subject" htmlFor={`${testIdPrefix}-subject`}>
        <Input
          id={`${testIdPrefix}-subject`}
          data-testid={`${testIdPrefix}-subject`}
          value={subject}
          onChange={(event) => setSubject(event.target.value)}
        />
      </Field>

      <Field label="What are you asking for?" htmlFor={`${testIdPrefix}-body`}>
        <Textarea
          id={`${testIdPrefix}-body`}
          data-testid={`${testIdPrefix}-body`}
          rows={8}
          value={body}
          onChange={(event) => setBody(event.target.value)}
        />
      </Field>

      <div>
        <Button
          type="button"
          variant="primary"
          icon="quill"
          disabled={busy}
          onClick={() => void onSubmit()}
          data-testid={`${testIdPrefix}-submit`}
        >
          {busy ? "Sending..." : "Send"}
        </Button>
      </div>

      {failure ? (
        <div data-testid={`${testIdPrefix}-error`}>
          <StatusBadge variant="danger">{failure}</StatusBadge>
        </div>
      ) : null}
      {receipt ? (
        <div data-testid={`${testIdPrefix}-receipt`}>
          <StatusBadge variant="success">{receipt}</StatusBadge>
        </div>
      ) : null}
    </div>
  );
}
