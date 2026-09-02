import { useEffect, useMemo, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import { isAlreadyClaimed, setPlayerCharacterBinding } from "@/api/actorClaims";
import { getWorldActors } from "@/api/actors";
import {
  getWorldMembers,
  removeMember,
  updateMemberRole,
} from "@/api/worldMembers";
import type { WorldMemberRecord } from "@/api/worldMembers";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import { filterPlayers } from "@/pages/world/players/playerFilter";
import type { WorldActorRecord } from "@/types/actor";
import { useAuth } from "@/hooks/useAuth";

export interface PlayersPageProps {
  worldId: string;
  isGm: boolean;
}

const ROLE_HIERARCHY: Record<string, number> = { Owner: 3, GM: 2, Player: 1 };

/** The picker's "nobody" option. `""` rather than a sentinel id so the
 * select's own empty value carries it and no id space is invented. */
const NO_CHARACTER = "";

/**
 * Spec 023: the Players section. Every world member sees the roster paired
 * with the character each member has claimed (US1). GM/Owner members
 * additionally get role-change and removal controls per row (US2) — this
 * supersedes (not duplicates) the equivalent controls formerly on the
 * world dashboard's Campaign Settings panel.
 *
 * # Cards, not a table
 *
 * Spec 031 FR-033. A playtest found this screen unusable for the thing it
 * exists for: a bare table with no player names and no way to see or set
 * who was playing what. A card per player puts the two facts that matter —
 * who they are and who they are playing — next to each other at a glance,
 * and gives the binding control somewhere to live that a table row's
 * "Manage" column could not.
 *
 * # Why the search is client-side
 *
 * A world's roster is a table's worth of people, already fetched in full
 * for the list itself. A server-side search would add a round trip per
 * keystroke to filter data the browser is already holding.
 */
export function PlayersPage({ worldId, isGm }: PlayersPageProps) {
  const { user } = useAuth();
  const [members, setMembers] = useState<WorldMemberRecord[] | null>(null);
  const [characters, setCharacters] = useState<WorldActorRecord[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<Error | null>(null);
  const [refreshTick, setRefreshTick] = useState(0);
  const [busyMemberId, setBusyMemberId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const currentUserRole =
    members?.find((member) => member.userId === user?.id)?.role ?? null;

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(`${worldId}|${refreshTick}`, () => {
    setError(null);
  });

  useEffect(() => {
    let active = true;

    getWorldMembers(worldId)
      .then((result) => {
        if (active) {
          setMembers(result);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, refreshTick]);

  // Only a GM can bind, so only a GM pays for the actor list. A failure
  // here leaves the picker empty rather than breaking the roster — the
  // roster is what every member came for.
  useEffect(() => {
    if (!isGm) {
      return;
    }
    let active = true;

    getWorldActors(worldId)
      .then((result) => {
        if (active) {
          setCharacters(result.filter((actor) => !actor.isNpc));
        }
      })
      .catch(() => {
        if (active) {
          setCharacters([]);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId, isGm, refreshTick]);

  const visibleMembers = useMemo(
    () => (members ? filterPlayers(members, query) : []),
    [members, query],
  );

  const canManage = (targetMember: WorldMemberRecord): boolean => {
    if (!isGm || !currentUserRole) return false;
    if (targetMember.userId === user?.id) return false;
    const currentLevel = ROLE_HIERARCHY[currentUserRole] ?? 0;
    const targetLevel = ROLE_HIERARCHY[targetMember.role] ?? 0;
    return currentLevel > targetLevel;
  };

  const handleChangeRole = async (
    member: WorldMemberRecord,
    newRole: string,
  ) => {
    setActionError(null);
    setBusyMemberId(member.id);
    try {
      await updateMemberRole(worldId, member.userId, newRole);
      setRefreshTick((current) => current + 1);
    } catch (err) {
      setActionError(
        err instanceof Error ? err.message : "Failed to change member role",
      );
    } finally {
      setBusyMemberId(null);
    }
  };

  /**
   * Spec 031 FR-034. The server decides, and a lost race is reported as
   * what it is: the actor page and the player's own selection screen write
   * this same relation, so the character this picker showed as free may
   * have been taken between the read and the click. Refetching afterwards
   * is what makes the picker honest again — a client-side "is it free"
   * check would only have been the same stale read, one step earlier.
   */
  const handleChangeCharacter = async (
    member: WorldMemberRecord,
    actorId: string,
  ) => {
    setActionError(null);
    setBusyMemberId(member.id);
    try {
      await setPlayerCharacterBinding(
        worldId,
        member.id,
        actorId === NO_CHARACTER ? null : actorId,
      );
    } catch (err) {
      setActionError(
        isAlreadyClaimed(err)
          ? "Someone else is already playing that character — the roster below is up to date again."
          : err instanceof Error
            ? err.message
            : "Failed to set the player's character",
      );
    } finally {
      setBusyMemberId(null);
      setRefreshTick((current) => current + 1);
    }
  };

  const handleRemove = async (member: WorldMemberRecord) => {
    if (
      !window.confirm(
        "Are you sure you want to remove this member from the world?",
      )
    ) {
      return;
    }
    setActionError(null);
    setBusyMemberId(member.id);
    try {
      await removeMember(worldId, member.userId);
      setRefreshTick((current) => current + 1);
    } catch (err) {
      setActionError(
        err instanceof Error ? err.message : "Failed to remove member",
      );
    } finally {
      setBusyMemberId(null);
    }
  };

  if (error) {
    return (
      <p className="text-sm text-destructive">
        Failed to load players: {error.message}
      </p>
    );
  }

  if (members === null) {
    return <p className="text-sm text-muted-foreground">Loading players…</p>;
  }

  return (
    <div className="grid gap-4">
      <header className="grid gap-1">
        <h1 className="text-xl font-semibold">Players</h1>
        <p className="text-sm text-muted-foreground">
          {isGm
            ? "See who's playing what character, and manage roles and membership."
            : "See who's playing what character in this world."}
        </p>
      </header>

      <Input
        placeholder="Search players by name, role or character…"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        data-testid="players-search-input"
        aria-label="Search players"
      />

      {actionError ? (
        <p className="text-sm text-destructive" data-testid="players-error">
          {actionError}
        </p>
      ) : null}

      <div
        className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3"
        data-testid="players-list"
      >
        {visibleMembers.map((member) => (
          <Card
            key={member.id}
            className="grid content-start gap-3 p-4"
            data-testid={`player-card-${member.id}`}
          >
            <div className="flex items-start justify-between gap-2">
              <span
                className="font-medium break-words"
                data-testid={`player-name-${member.id}`}
              >
                {member.username}
              </span>
              <Badge variant="secondary">{member.role}</Badge>
            </div>

            <div
              className="grid gap-1"
              data-testid={`player-character-${member.id}`}
            >
              <span className="text-xs tracking-wide text-muted-foreground uppercase">
                Playing
              </span>
              {member.claimedActor ? (
                <Link
                  to={`/world/${worldId}/actor/${member.claimedActor.id}/view`}
                  className="font-medium hover:underline"
                >
                  {member.claimedActor.label}
                </Link>
              ) : (
                <span className="text-muted-foreground italic">
                  No character
                </span>
              )}
            </div>

            {isGm ? (
              <label className="grid gap-1 text-sm">
                <span className="text-xs tracking-wide text-muted-foreground uppercase">
                  Set character
                </span>
                <select
                  value={member.claimedActor?.id ?? NO_CHARACTER}
                  onChange={(event) =>
                    void handleChangeCharacter(member, event.target.value)
                  }
                  disabled={busyMemberId === member.id}
                  data-testid={`player-character-select-${member.id}`}
                  className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
                >
                  <option value={NO_CHARACTER}>No character</option>
                  {characters.map((actor) => (
                    <option
                      key={actor.id}
                      value={actor.id}
                      // Taken characters stay listed rather than
                      // disappearing: a GM looking for Aria needs to see
                      // that she is spoken for, not that she is missing.
                      disabled={
                        actor.claimedBy !== null &&
                        actor.claimedBy?.id !== member.id
                      }
                    >
                      {actor.claimedBy && actor.claimedBy.id !== member.id
                        ? `${actor.label} — played by ${actor.claimedBy.username}`
                        : actor.label}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}

            {canManage(member) ? (
              <div className="flex flex-wrap items-center gap-2">
                <select
                  value={member.role}
                  onChange={(event) =>
                    void handleChangeRole(member, event.target.value)
                  }
                  disabled={busyMemberId === member.id}
                  data-testid={`player-role-select-${member.id}`}
                  className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
                >
                  <option value="Owner">Owner</option>
                  <option value="GM">GM</option>
                  <option value="Player">Player</option>
                </select>
                <Button
                  variant="danger"
                  size="sm"
                  icon="trash"
                  onClick={() => void handleRemove(member)}
                  disabled={busyMemberId === member.id}
                  data-testid={`player-remove-${member.id}`}
                >
                  Remove
                </Button>
              </div>
            ) : null}
          </Card>
        ))}
      </div>

      {visibleMembers.length === 0 ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid="players-empty"
        >
          {members.length === 0
            ? "Nobody has joined this world yet."
            : "No players match that search."}
        </p>
      ) : null}
    </div>
  );
}
