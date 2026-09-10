import { useEffect, useState } from "react";
import { Card } from "@/components/ui/card/Card";
import { readTwoFactorHistory } from "@/api/twoFactor";
import type { TwoFactorHistoryEntry } from "@/types/twoFactor";

/**
 * Spec 041 FR-015: what has happened to this account's second factor.
 *
 * # Why this panel is not a nicety
 *
 * The requirement is that the account holder is *told*. Mail is the obvious
 * way and is wired up — but a great many ThunderForge instances have no mail
 * configured, and that is the ordinary state for somebody running this for
 * their own table, not a broken one. On such an instance the notice sits in
 * the outbox, blocked, and this list is the only way anybody finds out that an
 * administrator reset their factor or that their recovery codes were replaced.
 *
 * So it is written for the question somebody actually arrives with — "did
 * something happen to my account that I did not do?" — which is why the
 * by-someone-else case is the one that gets the emphasis.
 */
export const HISTORY_WORDING: Record<string, { own: string; other: string }> = {
  enrolled: {
    own: "You set up two-factor authentication.",
    other: "Two-factor authentication was set up on this account.",
  },
  removed: {
    own: "You turned two-factor authentication off.",
    other: "Two-factor authentication was turned off on this account.",
  },
  recovery_code_used: {
    own: "You signed in with a recovery code.",
    other: "A recovery code was used to sign in to this account.",
  },
  recovery_codes_issued: {
    own: "You issued a new set of recovery codes. Every earlier code stopped working.",
    other:
      "A new set of recovery codes was issued. Every earlier code stopped working.",
  },
  reset_by_operator: {
    own: "Your second factor was reset.",
    other:
      "An administrator reset the second factor on this account. It signs in on a password alone until you enrol again.",
  },
  requirement_set: {
    own: "A second factor became required on this account.",
    other: "An administrator required a second factor on this account.",
  },
  requirement_cleared: {
    own: "A second factor is no longer required on this account.",
    other:
      "An administrator stopped requiring a second factor on this account.",
  },
};

export function describeHistoryEntry(entry: TwoFactorHistoryEntry): string {
  const wording = HISTORY_WORDING[entry.eventType];
  if (!wording) {
    // An event type this build does not know about is still worth showing —
    // it happened, and hiding it would make the list quietly incomplete in
    // exactly the direction that matters.
    return `Something changed on this account's second factor (${entry.eventType}).`;
  }
  return entry.bySomeoneElse ? wording.other : wording.own;
}

export function TwoFactorHistoryPanel() {
  const [entries, setEntries] = useState<TwoFactorHistoryEntry[] | null>(null);

  useEffect(() => {
    let active = true;
    void readTwoFactorHistory().then((history) => {
      if (active) {
        setEntries(history);
      }
    });
    return () => {
      active = false;
    };
  }, []);

  return (
    <Card surface="stone" className="grid gap-3 p-6">
      <div className="grid gap-1">
        <h3 className="font-semibold">Second-factor history</h3>
        <p className="text-sm text-muted-foreground">
          Every change to this account&rsquo;s second factor. If something here
          was not you, change your password and speak to whoever runs this
          instance.
        </p>
      </div>

      {entries === null ? (
        <p className="text-sm text-muted-foreground">Loading&hellip;</p>
      ) : entries.length === 0 ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid="two-factor-history-empty"
        >
          Nothing has changed on this account&rsquo;s second factor.
        </p>
      ) : (
        <ul className="grid gap-2" data-testid="two-factor-history">
          {entries.map((entry) => (
            <li
              key={`${entry.occurredAt}-${entry.eventType}`}
              className="grid gap-0.5 border-l-2 border-border pl-3 text-sm"
              data-event-type={entry.eventType}
              data-by-someone-else={entry.bySomeoneElse}
            >
              <span>{describeHistoryEntry(entry)}</span>
              <span className="text-xs text-muted-foreground">
                {new Date(`${entry.occurredAt}Z`).toLocaleString()}
              </span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
