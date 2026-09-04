import { useEffect, useState } from "react";
import {
  acknowledgeLoreSyncNotice,
  beginLoreRepositoryConnection,
  getInstanceRepositoryIntegration,
  getLoreRepositoryConnection,
  type ConnectionGrantHandoff,
  type LoreRepositoryConnection,
  type RepositoryIntegrationStatus,
} from "@/api/loreSync";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useResetOnChange } from "@/hooks/useResetOnChange";

export interface LoreRepositoryCardProps {
  worldId: string;
}

/** Absolute, not relative: "3 days ago" reads as freshness, and the whole
 * point of FR-040a's timestamp is that an old observation should look old. */
function formatMoment(value?: string | null): string | null {
  return value ? new Date(value).toLocaleString() : null;
}

/**
 * FR-029 requires a needing-attention state to name its remedy. The server
 * writes that sentence, but a null `state_reason` must not degrade into a
 * badge with nothing beside it — the failure mode FR-029 exists to prevent is
 * precisely a Game Master who is told something is wrong and not what to do.
 */
const UNSTATED_REMEDY =
  "Synchronisation stopped and the reason was not recorded. Reconnect the " +
  "repository from this page; if it stops again, ask your instance operator " +
  "to check the repository integration.";

export function LoreRepositoryCard({ worldId }: LoreRepositoryCardProps) {
  const [integration, setIntegration] =
    useState<RepositoryIntegrationStatus | null>(null);
  const [connection, setConnection] = useState<LoreRepositoryConnection | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);
  const [handoff, setHandoff] = useState<ConnectionGrantHandoff | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset during render rather than at the top of the effect: this is state
  // derived from the arguments, and doing it in the effect commits one render
  // pairing the new world with the previous world's connection — here that
  // would briefly show one world's repository under another world's name.
  useResetOnChange(worldId, () => {
    setIntegration(null);
    setConnection(null);
    setIsLoading(true);
    setHandoff(null);
    setError(null);
  });

  useEffect(() => {
    let active = true;

    void (async () => {
      try {
        // The integration status is asked for first and unconditionally: on an
        // instance whose operator registered no application, nothing
        // connectable may be rendered at all (FR-036b), so the answer has to
        // be in hand before any affordance is chosen — not after one fails.
        const status = await getInstanceRepositoryIntegration();
        if (!active) {
          return;
        }
        setIntegration(status);

        if (status.configured) {
          const existing = await getLoreRepositoryConnection(worldId);
          if (active) {
            setConnection(existing);
          }
        }
      } catch (err) {
        if (active) {
          setError(
            err instanceof Error
              ? err.message
              : "Failed to load the repository connection",
          );
        }
      } finally {
        if (active) {
          setIsLoading(false);
        }
      }
    })();

    return () => {
      active = false;
    };
  }, [worldId]);

  const handleConnect = async () => {
    setPending(true);
    setError(null);
    try {
      setHandoff(await beginLoreRepositoryConnection(worldId));
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to start the connection",
      );
    } finally {
      setPending(false);
    }
  };

  const handleAcknowledge = async () => {
    setPending(true);
    setError(null);
    try {
      // The server's connection replaces ours wholesale. The acknowledgement
      // is the gate synchronisation waits on, so showing it as taken before
      // the write landed would claim a run had been unblocked when it had not.
      setConnection(await acknowledgeLoreSyncNotice(worldId));
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to record acknowledgement",
      );
    } finally {
      setPending(false);
    }
  };

  return (
    <Card className="grid gap-4 p-6" data-testid="lore-repository-card">
      <div>
        <h2 className="text-lg font-semibold">Lore repository</h2>
        <p className="text-sm text-muted-foreground">
          Mirror this world&apos;s lore, as Markdown files, into a Git
          repository you own. ThunderForge writes to the repository; it never
          reads your lore back from it.
        </p>
      </div>

      {isLoading ? (
        <Loader label="Loading repository connection" />
      ) : integration?.configured !== true ? (
        <UnconfiguredNotice guidance={integration?.operatorGuidance} />
      ) : connection === null ? (
        <ConnectAffordance
          handoff={handoff}
          pending={pending}
          onConnect={() => void handleConnect()}
        />
      ) : connection.noticeAcknowledgedAt ? (
        <ConnectionState connection={connection} />
      ) : (
        <PreSyncNotice
          connection={connection}
          pending={pending}
          onAcknowledge={() => void handleAcknowledge()}
        />
      )}

      {error ? (
        <StatusBadge variant="danger" data-testid="lore-repository-error">
          {error}
        </StatusBadge>
      ) : null}
    </Card>
  );
}

/**
 * FR-036b. This instance's operator has registered no repository application,
 * so there is nothing to connect to — and the correct response is to say so
 * and stop, not to offer a button that will fail at the host.
 *
 * This is the state every self-hosted instance starts in, and the one nobody
 * developing the feature ever sees, which is exactly why it is a component of
 * its own rather than a conditional wrapped around a disabled button: there is
 * no path through this branch that renders a connect affordance.
 */
function UnconfiguredNotice({ guidance }: { guidance?: string | null }) {
  return (
    <div className="grid gap-2" data-testid="lore-sync-unconfigured">
      <StatusBadge variant="info">
        Repository synchronisation is not available on this instance
      </StatusBadge>
      <p className="text-sm text-muted-foreground">
        Nothing is wrong with your world. This ThunderForge instance has no
        repository integration configured, so there is no repository to connect
        to. Ask whoever runs this instance:
      </p>
      <p className="text-sm">
        {guidance ??
          "Register a repository application for this instance and make the " +
            "git binary available to the server, then this card will offer a " +
            "connection."}
      </p>
    </div>
  );
}

/** FR-036/FR-036e: the permissions are shown before the user leaves to grant
 * anything, each with the reason this feature asks for it — including the
 * issue-opening one, which exists so FR-040b's public disassociation is
 * something the product can actually carry out. */
function ConnectAffordance({
  handoff,
  pending,
  onConnect,
}: {
  handoff: ConnectionGrantHandoff | null;
  pending: boolean;
  onConnect: () => void;
}) {
  if (!handoff) {
    return (
      <div className="grid gap-2" data-testid="lore-sync-connect">
        <p className="text-sm text-muted-foreground">
          No repository is connected to this world.
        </p>
        <div>
          <Button onClick={onConnect} disabled={pending}>
            {pending ? "Preparing..." : "Connect a repository"}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="grid gap-3" data-testid="lore-sync-connect">
      <p className="text-sm">
        You are about to grant ThunderForge the following access to the one
        repository you choose, and to no other:
      </p>
      <ul className="grid gap-2 text-sm">
        {handoff.permissions.map((permission) => (
          <li key={permission.id}>
            <span className="font-medium">{permission.summary}</span>
            <span className="text-muted-foreground">
              {" "}
              — {permission.reason}
            </span>
          </li>
        ))}
      </ul>
      <div>
        <Button asChild data-testid="lore-sync-grant-link">
          <a href={handoff.url} rel="noreferrer">
            Continue to your repository host
          </a>
        </Button>
      </div>
    </div>
  );
}

/**
 * FR-037, FR-037a, FR-037b and FR-038, in one block that cannot be skipped:
 * until the acknowledgement mutation succeeds the background task will not
 * pick this connection up, so nothing has been exported at the moment this is
 * read.
 *
 * The wording separates two consequences that are not the same size. "Everyone
 * you invited to the repository" and "everyone on the internet" are different
 * sentences, and a notice that covers only the first is silently wrong for
 * exactly the users most exposed by this feature — so the public case is its
 * own prominent paragraph rather than a clause.
 */
function PreSyncNotice({
  connection,
  pending,
  onAcknowledge,
}: {
  connection: LoreRepositoryConnection;
  pending: boolean;
  onAcknowledge: () => void;
}) {
  return (
    <div className="grid gap-3" data-testid="lore-sync-notice">
      <StatusBadge variant="warning">
        Read this before the first synchronisation
      </StatusBadge>

      <p className="text-sm">
        Per-entry lore permissions do not survive the mirror. Every entry this
        world exports is written as a plain file in{" "}
        <span className="font-medium">{connection.repositoryRef}</span>, and
        anyone who can read that repository can read all of it — including
        entries you restricted to some of your world&apos;s members. Those
        restrictions are not carried across and cannot be reconstructed there.
      </p>
      <p className="text-sm">
        Who can read that repository is yours to manage, not
        ThunderForge&apos;s. ThunderForge writes to the repository you
        connected; it does not control, review, or change who has access to it.
        Once a file is in a repository outside this platform, ThunderForge
        cannot take it back.
      </p>

      <RepositoryVisibility connection={connection} />

      <div>
        <Button
          onClick={onAcknowledge}
          disabled={pending}
          data-testid="lore-sync-acknowledge"
        >
          {pending
            ? "Recording..."
            : "I understand — begin synchronising this world's lore"}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Nothing has been exported yet. Synchronisation begins only after you
        acknowledge this.
      </p>
    </div>
  );
}

/**
 * FR-037a and FR-040a. Three cases, and the third is the one that matters:
 * a repository whose visibility has not been observed is **not** a private
 * one. Free plans, shared accounts and organisations that forbid private
 * repositories are all ordinary, so an unobserved repository is described as
 * possibly public rather than quietly assumed safe.
 *
 * Shown both in the notice and beside a live connection, because what is
 * displayed is an observation from the last run and visibility can be changed
 * at the host without telling us — which is why the timestamp travels with it.
 */
function RepositoryVisibility({
  connection,
}: {
  connection: LoreRepositoryConnection;
}) {
  const checkedAt = formatMoment(connection.visibilityCheckedAt);

  if (connection.repositoryIsPublic === true) {
    return (
      <div
        className="grid gap-2 rounded-md border border-amber-500 p-3"
        data-testid="lore-sync-public-warning"
      >
        <p className="text-sm font-semibold">
          This repository is public. Everything exported is visible to everyone
          on the internet — not only the people you invited to it. Anyone can
          read it, copy it, and keep their copy after you delete yours.
        </p>
        <p className="text-sm">
          Because the repository is public, if content mirrored from this world
          is ever disabled by a takedown, ThunderForge will lodge a public issue
          on this repository stating that it has disabled the content and
          stopped exporting it. That issue is visible to anyone who can read the
          repository.
        </p>
        <p className="text-xs text-muted-foreground">
          {checkedAt
            ? `Seen as public when ThunderForge last looked, on ${checkedAt}. This is what was observed then, not a guarantee — visibility can be changed at your repository host without telling ThunderForge.`
            : "This is what was observed, not a guarantee — visibility can be changed at your repository host without telling ThunderForge."}
        </p>
      </div>
    );
  }

  if (connection.repositoryIsPublic === false) {
    return (
      <div className="grid gap-2" data-testid="lore-sync-visibility">
        <p className="text-sm">
          This repository was private when ThunderForge last looked. Everything
          exported is visible to everyone you have given access to the
          repository, which may be more people — or different people — than you
          invited to this world.
        </p>
        <p className="text-xs text-muted-foreground">
          {checkedAt
            ? `Seen as private on ${checkedAt}. This is what was observed then, not a guarantee — visibility can be changed at your repository host without telling ThunderForge.`
            : "This is what was observed, not a guarantee — visibility can be changed at your repository host without telling ThunderForge."}
        </p>
      </div>
    );
  }

  return (
    <div
      className="grid gap-2 rounded-md border border-amber-500 p-3"
      data-testid="lore-sync-public-warning"
    >
      <p className="text-sm font-semibold">
        ThunderForge has not yet observed whether this repository is public.
        Treat it as though it may be: if it is public, everything exported is
        visible to everyone on the internet, not only the people you invited to
        the repository.
      </p>
      <p className="text-sm">
        If the repository is public and content mirrored from this world is ever
        disabled by a takedown, ThunderForge will lodge a public issue on this
        repository stating that it has disabled the content and stopped
        exporting it. Check the repository&apos;s visibility at your host before
        you continue.
      </p>
    </div>
  );
}

/** FR-029's states, plus FR-041c's. Each one either says the connection is
 * fine or says what to do about it — never only that something is wrong. */
function ConnectionState({
  connection,
}: {
  connection: LoreRepositoryConnection;
}) {
  const lastSynced = formatMoment(connection.lastSyncedAt);

  return (
    <div className="grid gap-3" data-testid="lore-sync-state">
      <div className="grid gap-1">
        <p className="text-sm font-medium">{connection.repositoryRef}</p>
        <p className="text-xs text-muted-foreground">
          Branch {connection.branch} · directory {connection.directory}
        </p>
      </div>

      {connection.state === "WORKING" ? (
        <>
          <StatusBadge variant="success">Working</StatusBadge>
          <p className="text-sm text-muted-foreground">
            {lastSynced
              ? `Last synchronised ${lastSynced}.`
              : "Connected. The first synchronisation has not run yet."}
          </p>
        </>
      ) : connection.state === "DEACTIVATED" ? (
        // FR-041c: this must not read like something to retry. A Game Master
        // told to "check the connection" for a state only an administrator can
        // lift will keep trying to fix it, so no retry control is rendered
        // here and the sentence says plainly who can restore it.
        <>
          <StatusBadge variant="danger">
            Deactivated by ThunderForge
          </StatusBadge>
          <p className="text-sm">
            {connection.stateReason ??
              "This connection was deactivated by a ThunderForge administrator."}
          </p>
          <p className="text-sm text-muted-foreground">
            This is not something you can restore, and reconnecting the
            repository will not lift it. Contact ThunderForge support if you
            believe it was deactivated in error. Your world&apos;s lore inside
            ThunderForge is unaffected, and nothing already in the repository
            has been changed or removed.
          </p>
        </>
      ) : connection.state === "NEEDS_ATTENTION" ? (
        <>
          <StatusBadge variant="warning">Needs attention</StatusBadge>
          <p className="text-sm">{connection.stateReason ?? UNSTATED_REMEDY}</p>
          <p className="text-sm text-muted-foreground">
            {lastSynced
              ? `Last successful synchronisation ${lastSynced}. Your world's lore inside ThunderForge is unaffected.`
              : "No synchronisation has ever succeeded. Your world's lore inside ThunderForge is unaffected."}
          </p>
          {/* No "try again" control yet, deliberately.
           *
           * `retryLoreSync` is in the contract and is Story 2's work
           * (T040/T043); the server does not implement it. A button calling a
           * mutation that does not exist is worse than no button — it fails
           * with a schema error a Game Master cannot act on, and it fails
           * only in the state where they are already being told something is
           * wrong.
           *
           * Nothing is lost by waiting: retries back off on their own
           * (FR-030), so the honest thing to say here is what the remedy is,
           * which the line above does. */}
        </>
      ) : (
        <>
          <StatusBadge variant="info">Never configured</StatusBadge>
          <p className="text-sm text-muted-foreground">
            {connection.stateReason ??
              "This connection has not synchronised yet. It will run on the next scheduled pass."}
          </p>
        </>
      )}

      <RepositoryVisibility connection={connection} />

      {connection.fidelityNotes.length > 0 ? (
        // SC-008: losses are enumerated here rather than left to be discovered
        // in the repository, which is the whole reason they are stored as rows.
        <div className="grid gap-1" data-testid="lore-sync-fidelity-notes">
          <p className="text-sm font-medium">
            What the mirror could not represent
          </p>
          <ul className="grid gap-1 text-sm text-muted-foreground">
            {connection.fidelityNotes.map((note) => (
              <li key={note.id}>{note.detail}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
