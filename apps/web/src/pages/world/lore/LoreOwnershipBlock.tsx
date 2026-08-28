import { useEffect, useState } from "react";
import {
  getLoreEntryPermissions,
  removeLorePermission,
  setLorePermission,
} from "@/api/lore";
import { getWorldMembers, type WorldMemberRecord } from "@/api/worldMembers";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type { ActorPermissionLevel } from "@/types/actor";
import type { LorePermissionRecord } from "@/types/lore";
import type { WorldRecord } from "@/types/world";

export interface LoreOwnershipBlockProps {
  loreEntryId: string;
  worldId: string;
  world: WorldRecord | null;
}

const LEVEL_OPTIONS: Array<{
  value: "" | ActorPermissionLevel;
  label: string;
}> = [
  { value: "", label: "Default (Viewer)" },
  { value: "VIEWER", label: "Viewer" },
  { value: "EDITOR", label: "Editor" },
  { value: "OWNER", label: "Owner" },
];

/**
 * Spec 012 (T029c, FR-003): the DM-only "ownership block" editor for a lore
 * entry — mirrors `ActorOwnershipBlock.tsx` exactly. Lists every world
 * member plus the DM, shows each one's explicit Viewer/Editor/Owner grant
 * (or "default"), and lets the DM change it. The caller
 * (`LoreEntryDetailPage.tsx`) is responsible for only rendering this for a
 * DM viewer — non-DM callers who reach it anyway are still rejected
 * server-side by every mutation here.
 */
export function LoreOwnershipBlock({
  loreEntryId,
  worldId,
  world,
}: LoreOwnershipBlockProps) {
  const [members, setMembers] = useState<WorldMemberRecord[] | null>(null);
  const [permissions, setPermissions] = useState<LorePermissionRecord[] | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [pendingUserId, setPendingUserId] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    getWorldMembers(worldId)
      .then((rows) => {
        if (active) {
          setMembers(rows);
        }
      })
      .catch((err) => {
        if (active) {
          setError(
            err instanceof Error ? err.message : "Failed to load world members",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  useEffect(() => {
    let active = true;
    getLoreEntryPermissions(loreEntryId)
      .then((rows) => {
        if (active) {
          setPermissions(rows);
        }
      })
      .catch((err) => {
        if (active) {
          setError(
            err instanceof Error
              ? err.message
              : "Failed to load ownership block",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [loreEntryId]);

  const ownerHasMembership = (members ?? []).some(
    (member) => member.userId === world?.createdBy,
  );
  const subjects = [
    ...(world && !ownerHasMembership
      ? [
          {
            userId: world.createdBy,
            role: "Owner",
            displayName: null as string | null,
          },
        ]
      : []),
    ...(members ?? []).map((member) => ({
      userId: member.userId,
      role: member.role,
      displayName: null as string | null,
    })),
  ];

  const handleChange = async (userId: string, value: string) => {
    setPendingUserId(userId);
    setError(null);
    try {
      if (value === "") {
        await removeLorePermission(loreEntryId, userId);
        setPermissions((current) =>
          (current ?? []).filter((row) => row.userId !== userId),
        );
      } else {
        const updated = await setLorePermission(
          loreEntryId,
          userId,
          value as ActorPermissionLevel,
        );
        setPermissions((current) => {
          const rest = (current ?? []).filter((row) => row.userId !== userId);
          return [...rest, updated];
        });
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to update permission",
      );
    } finally {
      setPendingUserId(null);
    }
  };

  if (members === null || permissions === null) {
    return <Loader label="Loading ownership block" />;
  }

  return (
    <Card className="grid gap-3 p-6" data-testid="lore-ownership-block">
      <h3 className="text-lg font-semibold">Ownership block</h3>
      <p className="text-sm text-muted-foreground">
        Only the DM can change who has access to this lore entry.
      </p>

      {error ? <StatusBadge variant="danger">{error}</StatusBadge> : null}

      <div className="grid gap-2">
        {subjects.map((subject) => {
          const explicit = permissions.find(
            (row) => row.userId === subject.userId,
          );
          const isDmSubject =
            subject.userId === world?.createdBy && subject.role === "Owner";
          return (
            <div
              key={subject.userId}
              data-testid={`lore-ownership-row-${subject.userId}`}
              className="flex items-center justify-between gap-3 rounded-lg border border-border p-3"
            >
              <div>
                <strong className="text-sm">
                  {subject.displayName ?? subject.userId}
                  {isDmSubject ? " (DM)" : ""}
                </strong>
                <p className="text-xs text-muted-foreground uppercase tracking-wide">
                  {subject.role}
                </p>
              </div>
              <select
                data-testid={`lore-ownership-select-${subject.userId}`}
                value={explicit?.level ?? ""}
                disabled={pendingUserId === subject.userId}
                onChange={(event) =>
                  void handleChange(subject.userId, event.target.value)
                }
                className="h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
              >
                {LEVEL_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
          );
        })}
      </div>
    </Card>
  );
}
