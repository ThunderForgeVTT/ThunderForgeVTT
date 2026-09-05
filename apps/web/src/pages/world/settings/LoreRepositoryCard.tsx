import { useEffect, useState } from "react";
import {
  acceptLoreIncomingChange,
  acknowledgeLoreSyncNotice,
  beginLoreRepositoryConnection,
  declineLoreIncomingChange,
  getInstanceRepositoryIntegration,
  getLoreRepositoryConnection,
  getLorePendingChanges,
  type ConnectionGrantHandoff,
  type LorePendingChange,
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
  const [pendingChanges, setPendingChanges] = useState<LorePendingChange[]>([]);

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
          if (!active) {
            return;
          }
          setConnection(existing);

          // Asked for only where the world has actually enabled incoming
          // acceptance (FR-022). A world that never enabled it has no answer
          // to give here, and querying anyway would put a "nothing pending"
          // shape in front of a feature that world has not opted into.
          if (existing?.incomingEnabled === true) {
            const changes = await getLorePendingChanges(worldId);
            if (active) {
              setPendingChanges(changes);
            }
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

  /** FR-023: the decision is the mutation, so the list is re-read from the
   * server afterwards rather than the row being dropped locally. A local
   * removal would show a change as settled on the strength of a click, which
   * is precisely the claim this feature may never make on its own. */
  const handleResolve = async (changeId: string, accept: boolean) => {
    setPending(true);
    setError(null);
    try {
      if (accept) {
        await acceptLoreIncomingChange(worldId, changeId);
      } else {
        await declineLoreIncomingChange(worldId, changeId);
      }
      setPendingChanges(await getLorePendingChanges(worldId));
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Failed to record your decision. Your world's lore is unchanged.",
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

      {connection?.incomingEnabled === true && pendingChanges.length > 0 ? (
        <PendingIncomingChanges
          changes={pendingChanges}
          pending={pending}
          onResolve={(changeId, accept) => void handleResolve(changeId, accept)}
        />
      ) : null}

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

/** What each kind is called where a person reads it. "Proposed" throughout,
 * because until someone accepts, none of these has happened (FR-023). */
const KIND_LABEL: Record<LorePendingChange["kind"], string> = {
  UPDATE: "Proposed update",
  NEW_ENTRY: "Proposed new entry",
  DELETION: "Proposed deletion",
};

/**
 * User Story 3's review surface: everything the repository proposes, waiting on
 * a person. Rendered only where the world enabled incoming acceptance and
 * something is actually pending — an empty "no incoming changes" panel would
 * advertise a write path on worlds that have one for no reason.
 *
 * There is no "accept all". A conflict (FR-024) and a deletion (FR-026) each
 * ask a question a bulk control would answer on the user's behalf, so the only
 * granularity offered is the one the requirements are written at: per change.
 */
function PendingIncomingChanges({
  changes,
  pending,
  onResolve,
}: {
  changes: LorePendingChange[];
  pending: boolean;
  onResolve: (changeId: string, accept: boolean) => void;
}) {
  return (
    <div className="grid gap-3" data-testid="lore-incoming-changes">
      <div>
        <h3 className="text-sm font-semibold">
          Changes in the repository, waiting for you
        </h3>
        <p className="text-sm text-muted-foreground">
          {changes.length === 1
            ? "One change was found in the repository that this world does not have."
            : `${changes.length} changes were found in the repository that this world does not have.`}{" "}
          None of them has altered your world&apos;s lore. Each is applied only
          if you accept it, and an accepted change is recorded as an ordinary
          revision in the entry&apos;s history, attributed to you and marked as
          coming from the repository.
        </p>
      </div>

      {changes.map((change) => (
        <PendingIncomingChange
          key={change.id}
          change={change}
          pending={pending}
          onResolve={onResolve}
        />
      ))}
    </div>
  );
}

function PendingIncomingChange({
  change,
  pending,
  onResolve,
}: {
  change: LorePendingChange;
  pending: boolean;
  onResolve: (changeId: string, accept: boolean) => void;
}) {
  // A deletion is the one decision that cannot be undone from inside the app,
  // so it is taken in two steps (FR-026). The step exists to make the act
  // deliberate; the sentence beside it exists so declining does not look like
  // the risky option, which is the mistake this shape is guarding against.
  const [confirmingDeletion, setConfirmingDeletion] = useState(false);
  const detectedAt = formatMoment(change.detectedAt);
  const title = change.proposedTitle ?? change.currentTitle ?? null;

  return (
    <div
      className="grid gap-2 rounded-md border p-3"
      data-testid="lore-incoming-change"
    >
      <div className="grid gap-1">
        <p className="text-sm font-medium" data-testid="lore-incoming-kind">
          {KIND_LABEL[change.kind]}
          {title ? ` — ${title}` : null}
        </p>
        <p className="text-xs text-muted-foreground">
          From <span className="font-medium">{change.repositoryPath}</span> in
          the repository
          {detectedAt ? `, noticed ${detectedAt}` : null}.
        </p>
      </div>

      {change.kind === "NEW_ENTRY" ? (
        // FR-027. Said outright, because the plausible-looking wrong answer —
        // "this is probably that entry over there, matched by its filename" —
        // is exactly what the requirement forbids, and a user who assumes it
        // happened would read an accept as an overwrite.
        <p className="text-sm">
          This file carries no ThunderForge entry identifier, so it matches
          nothing in this world. It is offered as a new entry to create.
          ThunderForge does not match a file to an existing entry by its path or
          its title, so accepting this creates a new entry and changes nothing
          you already have.
        </p>
      ) : null}

      {change.kind === "DELETION" ? (
        <p className="text-sm">
          This entry&apos;s file was deleted in the repository. The entry is
          still here and untouched. Accepting deletes it from this world;
          declining is safe — the entry stays exactly as it is, and the file is
          written back to the repository on the next synchronisation.
        </p>
      ) : null}

      {change.kind === "UPDATE" && !change.alsoChangedInApp ? (
        <p className="text-sm">
          The file changed in the repository, and this entry has not changed in
          this world since the last synchronisation. Accepting replaces the
          entry&apos;s text with the version below.
        </p>
      ) : null}

      {change.alsoChangedInApp ? (
        <ConflictingVersions change={change} />
      ) : (
        <ProposedText
          label={
            change.kind === "DELETION"
              ? "The text that would be deleted"
              : "The text from the repository"
          }
          body={
            change.kind === "DELETION"
              ? change.currentBody
              : change.incomingBody
          }
        />
      )}

      <div className="flex flex-wrap gap-2">
        {change.kind === "DELETION" && confirmingDeletion ? (
          <>
            <Button
              variant="danger"
              disabled={pending}
              data-testid="lore-incoming-accept-confirm"
              onClick={() => {
                setConfirmingDeletion(false);
                onResolve(change.id, true);
              }}
            >
              {pending ? "Working..." : "Yes, delete this entry"}
            </Button>
            <Button
              variant="ghost"
              disabled={pending}
              onClick={() => setConfirmingDeletion(false)}
            >
              Keep the entry
            </Button>
          </>
        ) : (
          <>
            <Button
              variant={change.kind === "DELETION" ? "danger" : "primary"}
              disabled={pending}
              data-testid="lore-incoming-accept"
              onClick={() => {
                if (change.kind === "DELETION") {
                  setConfirmingDeletion(true);
                  return;
                }
                onResolve(change.id, true);
              }}
            >
              {change.kind === "DELETION"
                ? "Delete this entry from the world"
                : change.kind === "NEW_ENTRY"
                  ? "Create this entry"
                  : change.alsoChangedInApp
                    ? "Keep the repository's version"
                    : "Accept this change"}
            </Button>
            <Button
              variant="secondary"
              disabled={pending}
              data-testid="lore-incoming-decline"
              onClick={() => onResolve(change.id, false)}
            >
              {change.kind === "DELETION"
                ? "Decline — keep the entry"
                : change.alsoChangedInApp
                  ? "Keep this world's version"
                  : "Decline"}
            </Button>
          </>
        )}
      </div>

      {change.kind === "DELETION" && confirmingDeletion ? (
        <p className="text-sm">
          This removes the entry from this world. Its revision history goes with
          it, and ThunderForge cannot bring it back for you.
        </p>
      ) : null}
    </div>
  );
}

/**
 * FR-024, and the reason this component exists at all rather than a diff.
 *
 * The two texts are rendered whole, side by side, and never combined — no
 * unified diff, no merged draft, no "resolved" text with both sides folded in.
 * A view that interleaved them would look like a result, and a user would
 * reasonably accept it believing that result is what gets saved. Nothing merges
 * prose here, so nothing may be drawn that suggests something did: the only two
 * things that can be saved are the two texts shown, and each is shown entire so
 * the choice is between things the user has actually read.
 */
function ConflictingVersions({ change }: { change: LorePendingChange }) {
  return (
    <div
      className="grid gap-2 rounded-md border border-amber-500 p-3"
      data-testid="lore-incoming-conflict"
    >
      <StatusBadge variant="warning">
        Changed in both places — nothing will be merged
      </StatusBadge>
      <p className="text-sm">
        This entry changed in the repository and in this world since the last
        synchronisation. ThunderForge will not combine the two texts, and
        neither text below is a merge or a preview of one. Choose one whole
        version to keep: accepting saves the repository&apos;s version exactly
        as shown, and declining leaves this world&apos;s version exactly as
        shown. Whichever you do not choose is not folded into the other.
      </p>
      <div className="grid gap-2 md:grid-cols-2">
        <ProposedText
          label="This world's version — kept if you decline"
          body={change.currentBody}
        />
        <ProposedText
          label="The repository's version — saved if you accept"
          body={change.incomingBody}
        />
      </div>
    </div>
  );
}

/** One whole text, shown as written. Preformatted rather than rendered: what
 * is being chosen between is the Markdown source in the file, and rendering it
 * would hide the very characters an author went to a text editor to change. */
function ProposedText({
  label,
  body,
}: {
  label: string;
  body?: string | null;
}) {
  return (
    <div className="grid gap-1">
      <p className="text-xs font-medium text-muted-foreground">{label}</p>
      <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-md bg-muted p-2 text-xs">
        {body ?? "(empty)"}
      </pre>
    </div>
  );
}
