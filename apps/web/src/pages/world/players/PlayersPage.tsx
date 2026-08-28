import { useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link } from "react-router-dom";
import {
  getWorldMembers,
  removeMember,
  updateMemberRole,
} from "@/api/worldMembers";
import type { WorldMemberRecord } from "@/api/worldMembers";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button/Button";
import { useAuth } from "@/hooks/useAuth";

export interface PlayersPageProps {
  worldId: string;
  isGm: boolean;
}

const ROLE_HIERARCHY: Record<string, number> = { Owner: 3, GM: 2, Player: 1 };

/**
 * Spec 023: the Players section. Every world member sees the roster paired
 * with the character each member has claimed (US1). GM/Owner members
 * additionally get role-change and removal controls per row (US2) — this
 * supersedes (not duplicates) the equivalent controls formerly on the
 * world dashboard's Campaign Settings panel.
 */
export function PlayersPage({ worldId, isGm }: PlayersPageProps) {
  const { user } = useAuth();
  const [members, setMembers] = useState<WorldMemberRecord[] | null>(null);
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

      {actionError ? (
        <p className="text-sm text-destructive">{actionError}</p>
      ) : null}

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-sm" data-testid="players-table">
          <thead>
            <tr className="border-b border-border bg-muted/50 text-left text-xs tracking-wide text-muted-foreground uppercase">
              <th className="p-2 font-semibold">Role</th>
              <th className="p-2 font-semibold">Character</th>
              {isGm ? <th className="p-2 font-semibold">Manage</th> : null}
            </tr>
          </thead>
          <tbody>
            {members.map((member) => (
              <tr
                key={member.id}
                className="border-b border-border last:border-0 hover:bg-muted/40"
                data-testid={`player-row-${member.id}`}
              >
                <td className="p-2">
                  <Badge variant="secondary">{member.role}</Badge>
                </td>
                <td
                  className="p-2"
                  data-testid={`player-character-${member.id}`}
                >
                  {member.claimedActor ? (
                    <Link
                      to={`/world/${worldId}/actor/${member.claimedActor.id}/view`}
                      className="font-medium hover:underline"
                    >
                      {member.claimedActor.label}
                    </Link>
                  ) : (
                    <span className="text-muted-foreground italic">
                      No character claimed
                    </span>
                  )}
                </td>
                {isGm ? (
                  <td className="p-2">
                    {canManage(member) ? (
                      <div className="flex items-center gap-2">
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
                  </td>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
