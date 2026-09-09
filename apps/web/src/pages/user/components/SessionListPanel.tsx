import { useCallback, useEffect, useState } from "react";
import {
  endAllSessions,
  endSession,
  getMySessions,
  type UserSession,
} from "@/api/sessions";
import { Button } from "@/components/ui/button/Button";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";

/**
 * Spec 036 US4: where this account is signed in, and how to end it.
 *
 * # Why this screen is the other half of a fix
 *
 * Signing in used to end every other session. That was the defect — one person
 * with a character sheet on a second screen is the ordinary case, not an
 * attack — and removing it is what spec 036 is for. But an account that can
 * hold ten live sessions and cannot see them has traded one problem for a
 * worse one: "am I still signed in on that machine" stopped having an answer.
 *
 * # No addresses, deliberately
 *
 * A coarse client description and two times. This is the most tempting place
 * in the product to show an IP — it looks like security — and spec 035's rule
 * is that the instance records the act and never the person. "Chrome on Linux,
 * two hours ago" answers the question somebody actually has; adding an address
 * turns the same screen into a location history nobody asked this instance to
 * keep.
 *
 * # Ending the current session is allowed
 *
 * It signs this browser out, which is what somebody who clicked it meant. It
 * is labelled as the current one so the click is informed rather than
 * surprising.
 */

function when(iso: string): string {
  const at = new Date(iso);
  const minutes = Math.round((Date.now() - at.getTime()) / 60_000);
  if (minutes < 2) return "just now";
  if (minutes < 60) return `${minutes} minutes ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? "" : "s"} ago`;
  return at.toLocaleDateString();
}

export function SessionListPanel() {
  const [sessions, setSessions] = useState<UserSession[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    getMySessions()
      .then((next) => {
        setSessions(next);
        setError(null);
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error
            ? cause.message
            : "Your sessions could not be read.",
        );
      });
  }, []);

  useEffect(load, [load]);

  const end = async (session: UserSession) => {
    setBusy(session.id);
    try {
      await endSession(session.id);
      if (session.isCurrent) {
        // Ending the current session signs this browser out. Reloading is what
        // makes that visible immediately rather than on the next request.
        window.location.assign("/login");
        return;
      }
      load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "That was refused.");
    } finally {
      setBusy(null);
    }
  };

  /**
   * FR-007 ends *every* session for the account, this one included. The
   * button said "sign out the other N" for a while, which was a promise the
   * server does not make: the click signed this browser out too and the
   * reload that followed answered 401, so the screen reported an error for
   * having worked. Saying what it does, and going where it sends you, is the
   * fix — not a second server behaviour nothing asked for.
   */
  const endEverywhere = async () => {
    setBusy("all");
    try {
      await endAllSessions();
      window.location.assign("/login");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "That was refused.");
      setBusy(null);
    }
  };

  if (error) {
    return <StatusBadge variant="danger">{error}</StatusBadge>;
  }
  if (!sessions) {
    return <p className="text-muted-foreground">Reading your sessions...</p>;
  }

  const others = sessions.filter((s) => !s.isCurrent).length;

  return (
    <section className="grid gap-3" data-testid="session-list">
      <div className="grid gap-1">
        <h3 className="font-semibold">Where you are signed in</h3>
        <p className="text-sm text-muted-foreground">
          Signing in on another device does not sign you out here. This is every
          session this account currently holds.
        </p>
      </div>

      <ul className="grid gap-2">
        {sessions.map((session) => (
          <li
            key={session.id}
            className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-border bg-secondary/40 p-3 text-sm"
            data-testid={`session-${session.id}`}
            data-current={session.isCurrent ? "true" : "false"}
          >
            <div className="grid gap-0.5">
              <span className="font-medium">
                {session.clientDescription ?? "An unrecognised client"}
                {session.isCurrent ? (
                  <StatusBadge variant="info">This one</StatusBadge>
                ) : null}
              </span>
              <span className="text-muted-foreground">
                Last used {when(session.lastSeenAt)} · started{" "}
                {new Date(session.createdAt).toLocaleDateString()}
              </span>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              disabled={busy === session.id}
              onClick={() => void end(session)}
              data-testid={`session-end-${session.id}`}
            >
              {session.isCurrent ? "Sign out here" : "Sign out"}
            </Button>
          </li>
        ))}
      </ul>

      {others > 0 ? (
        <div>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={busy === "all"}
            onClick={() => void endEverywhere()}
            data-testid="session-end-all"
          >
            Sign out everywhere, including here ({others + 1} sessions)
          </Button>
        </div>
      ) : null}
    </section>
  );
}
