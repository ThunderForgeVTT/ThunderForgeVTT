import { useEffect, useState } from "react";
import {
  createInstanceInvitation,
  getInstanceAccessEvents,
  getInstanceInvitations,
  revokeInstanceInvitation,
  setInstanceAccessPolicy,
  type InstanceAccessEvent,
  type InstanceAccessSettings,
  type InstanceInvitation,
} from "@/api/instanceAccess";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 035 / ADR-072: who may create an account on this instance, and the
 * invitations that let named people through a shut door.
 *
 * The three states are not a toggle, and the wording matters: an operator who
 * believes "closed" only hides the registration form is the person this whole
 * feature exists to protect. Each option says what it admits.
 */

const POLICY_COPY: Record<
  InstanceAccessSettings["policy"],
  { label: string; detail: string }
> = {
  OPEN: {
    label: "Open",
    detail: "Anyone may create an account, by password or by any provider.",
  },
  INVITE_ONLY: {
    label: "Invite only",
    detail:
      "Only someone holding a valid invitation may create an account. Every other route is refused, providers included.",
  },
  CLOSED: {
    label: "Closed",
    detail:
      "Nobody may create an account by any means — a valid invitation included. Existing users are unaffected.",
  },
};

interface AccessPanelProps {
  settings: InstanceAccessSettings;
  onPolicyChange: (settings: InstanceAccessSettings) => void;
}

export function AccessPanel({ settings, onPolicyChange }: AccessPanelProps) {
  const [policy, setPolicy] = useState(settings.policy);
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [invitations, setInvitations] = useState<InstanceInvitation[]>([]);
  const [events, setEvents] = useState<InstanceAccessEvent[]>([]);
  const [maxUses, setMaxUses] = useState(1);
  const [expiresInHours, setExpiresInHours] = useState(168);
  const [note, setNote] = useState("");

  const refresh = () => {
    void getInstanceInvitations()
      .then(setInvitations)
      .catch(() => undefined);
    void getInstanceAccessEvents(25)
      .then(setEvents)
      .catch(() => undefined);
  };

  useEffect(refresh, []);

  const handleSavePolicy = async () => {
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await setInstanceAccessPolicy(policy);
      onPolicyChange(updated);
      setStatus(`Instance access is now ${POLICY_COPY[policy].label}.`);
      refresh();
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : "Failed to update the policy.",
      );
    } finally {
      setIsSaving(false);
    }
  };

  const handleIssue = async () => {
    setStatus(null);
    try {
      await createInstanceInvitation({
        maxUses,
        expiresInHours: expiresInHours > 0 ? expiresInHours : undefined,
        note: note.trim() || undefined,
      });
      setNote("");
      refresh();
    } catch (error) {
      setStatus(
        error instanceof Error
          ? error.message
          : "Failed to issue the invitation.",
      );
    }
  };

  const handleRevoke = async (id: string) => {
    await revokeInstanceInvitation(id).catch(() => false);
    refresh();
  };

  const inviteUrl = (code: string) =>
    `${window.location.origin}/invite/${code}`;

  return (
    <div className="grid gap-6" data-testid="access-panel">
      <Card className="grid gap-4 p-6">
        <div>
          <h2 className="text-xl font-semibold">Who may create an account</h2>
          <p className="text-sm text-muted-foreground">
            This governs every route that can create an account, including
            first-time sign-in with a configured provider. Existing users and
            their sessions are never affected.
          </p>
        </div>

        <div className="grid gap-2">
          {(Object.keys(POLICY_COPY) as InstanceAccessSettings["policy"][]).map(
            (option) => (
              <label
                key={option}
                className="flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3"
                data-testid={`access-policy-${option.toLowerCase()}`}
              >
                <input
                  type="radio"
                  name="instance-access-policy"
                  className="mt-1"
                  checked={policy === option}
                  onChange={() => setPolicy(option)}
                />
                <span>
                  <span className="font-medium">
                    {POLICY_COPY[option].label}
                  </span>
                  <span className="block text-sm text-muted-foreground">
                    {POLICY_COPY[option].detail}
                  </span>
                </span>
              </label>
            ),
          )}
        </div>

        <div className="flex items-center gap-3">
          <Button
            onClick={() => void handleSavePolicy()}
            disabled={isSaving || policy === settings.policy}
            data-testid="access-policy-save"
          >
            {isSaving ? "Saving..." : "Save policy"}
          </Button>
          {status ? <StatusBadge>{status}</StatusBadge> : null}
        </div>
      </Card>

      <Card className="grid gap-4 p-6">
        <div>
          <h2 className="text-xl font-semibold">Invitations</h2>
          <p className="text-sm text-muted-foreground">
            An invitation admits one person to this instance. It grants no world
            membership and no role — the account it creates is an ordinary one.
          </p>
        </div>

        <div className="flex flex-wrap items-end gap-3">
          <label className="grid gap-1 text-sm">
            Uses
            <input
              type="number"
              min={1}
              value={maxUses}
              onChange={(e) => setMaxUses(Number(e.target.value))}
              className="h-9 w-20 rounded-lg border border-input bg-transparent px-2.5"
              data-testid="invitation-max-uses"
            />
          </label>
          <label className="grid gap-1 text-sm">
            Expires in (hours, 0 = never)
            <input
              type="number"
              min={0}
              value={expiresInHours}
              onChange={(e) => setExpiresInHours(Number(e.target.value))}
              className="h-9 w-32 rounded-lg border border-input bg-transparent px-2.5"
              data-testid="invitation-expiry"
            />
          </label>
          <label className="grid flex-1 gap-1 text-sm">
            Note (only you see this)
            <input
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Priya, from the forum"
              className="h-9 rounded-lg border border-input bg-transparent px-2.5"
              data-testid="invitation-note"
            />
          </label>
          <Button
            onClick={() => void handleIssue()}
            data-testid="invitation-issue"
          >
            Issue invitation
          </Button>
        </div>

        {invitations.length === 0 ? (
          <p className="text-sm text-muted-foreground">No invitations yet.</p>
        ) : (
          <ul className="grid gap-2" data-testid="invitation-list">
            {invitations.map((invitation) => (
              <li
                key={invitation.id}
                className="grid gap-1 rounded-lg border border-border p-3 text-sm"
              >
                <div className="flex items-center justify-between gap-3">
                  <code className="text-xs break-all">
                    {inviteUrl(invitation.inviteCode)}
                  </code>
                  <StatusBadge
                    variant={
                      invitation.state === "ACTIVE" ? "success" : "warning"
                    }
                  >
                    {invitation.state}
                  </StatusBadge>
                </div>
                <div className="text-muted-foreground">
                  {invitation.remainingUses} of {invitation.maxUses} uses left
                  {invitation.expiresAt
                    ? ` · expires ${new Date(invitation.expiresAt).toLocaleString()}`
                    : " · no expiry"}
                  {invitation.note ? ` · ${invitation.note}` : ""}
                </div>
                {invitation.redemptions.length > 0 ? (
                  <ul className="text-muted-foreground">
                    {invitation.redemptions.map((r) => (
                      <li key={r.userId}>
                        Redeemed by {r.username} on{" "}
                        {new Date(r.redeemedAt).toLocaleString()} ({r.route})
                      </li>
                    ))}
                  </ul>
                ) : null}
                {invitation.state === "ACTIVE" ? (
                  <div>
                    <Button
                      variant="ghost"
                      onClick={() => void handleRevoke(invitation.id)}
                    >
                      Revoke
                    </Button>
                  </div>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card className="grid gap-3 p-6">
        <h2 className="text-xl font-semibold">Recent activity</h2>
        <p className="text-sm text-muted-foreground">
          Policy changes, refused admissions and redemptions. Refusals record
          the route and the policy at the time, never the address that was
          submitted.
        </p>
        {events.length === 0 ? (
          <p className="text-sm text-muted-foreground">Nothing yet.</p>
        ) : (
          <ul className="grid gap-1 text-sm" data-testid="access-event-list">
            {events.map((event) => (
              <li key={event.id} className="text-muted-foreground">
                <span className="text-foreground">{event.eventType}</span>{" "}
                {new Date(event.occurredAt).toLocaleString()}
                {event.attemptedRoute ? ` · ${event.attemptedRoute}` : ""}
                {event.previousPolicy && event.newPolicy
                  ? ` · ${event.previousPolicy} → ${event.newPolicy}`
                  : ""}
                {event.policyAtAttempt
                  ? ` · policy ${event.policyAtAttempt}`
                  : ""}
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
