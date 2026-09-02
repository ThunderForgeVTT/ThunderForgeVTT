import { useEffect, useState } from "react";
import {
  getAuthoringToolGrants,
  setAuthoringToolGrant,
} from "@/api/authoringTools";
import { getWorldMembers, type WorldMemberRecord } from "@/api/worldMembers";
import { Card } from "@/components/ui/card/Card";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import {
  GM_TOOL_IDS,
  type GmToolId,
} from "@/components/world/GmToolRail/GmToolRail";
import { useResetOnChange } from "@/hooks/useResetOnChange";

export interface AuthoringToolGrantsCardProps {
  worldId: string;
}

/**
 * The tools offered as toggles, in rail order.
 *
 * `GM_TOOL_IDS` itself, not a list of the same names typed out again. A
 * hand-copied second copy of these ids has already drifted once and took the
 * suite with it silently, and a card that offered a tool the rail does not
 * have would hand out a permission nothing could ever use.
 */
const TOOLS: readonly GmToolId[] = GM_TOOL_IDS;

/**
 * Sentence case for a tool id, for the checkbox beside it.
 *
 * Derived rather than imported: the rail's labels are written where its tools
 * are *constructed*, next to their icons and flyout contents, and that
 * construction is not exportable as a list of names. Copying the labels here
 * would be exactly the second hand-written copy the id list above refuses,
 * and the ids are already the vocabulary every layer speaks.
 */
function toolLabel(tool: GmToolId): string {
  return tool.charAt(0).toUpperCase() + tool.slice(1);
}

/**
 * Spec 031 (T032b, FR-046): where a Game Master hands a specific player
 * specific authoring tools.
 *
 * Only players are listed. A Game Master holds every tool implicitly and
 * un-removably (FR-045), so rendering their row would be six switches that
 * cannot be turned off — an offer the server would refuse to honour.
 *
 * This card is chrome. The refusal lives in `set_authoring_tool_grant_impl`,
 * which is DM-gated, and the tools a person may actually *use* are resolved
 * server-side and enforced by the engine (Constitution Principle III). Nothing
 * here decides anything; it only shows what the table says and asks it to
 * change.
 */
export function AuthoringToolGrantsCard({
  worldId,
}: AuthoringToolGrantsCardProps) {
  const [players, setPlayers] = useState<WorldMemberRecord[] | null>(null);
  const [granted, setGranted] = useState<Record<string, GmToolId[]>>({});
  const [pending, setPending] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  // Reset during render rather than at the top of the effect below: this is
  // state derived from the arguments, and doing it in the effect commits one
  // render pairing the new world's key with the previous world's grants.
  useResetOnChange(worldId, () => {
    setPlayers(null);
    setGranted({});
    setStatus(null);
  });

  useEffect(() => {
    let active = true;

    void (async () => {
      try {
        const [members, grants] = await Promise.all([
          getWorldMembers(worldId),
          getAuthoringToolGrants(worldId),
        ]);
        if (!active) {
          return;
        }
        setPlayers(members.filter((member) => member.role === "Player"));
        setGranted(
          Object.fromEntries(
            grants.map((entry) => [entry.worldMemberId, entry.tools]),
          ),
        );
      } catch (err) {
        if (active) {
          setPlayers([]);
          setStatus(
            err instanceof Error ? err.message : "Failed to load tool grants",
          );
        }
      }
    })();

    return () => {
      active = false;
    };
  }, [worldId]);

  const handleToggle = async (
    member: WorldMemberRecord,
    tool: GmToolId,
    next: boolean,
  ) => {
    setPending(`${member.id}:${tool}`);
    setStatus(null);
    try {
      // The server's answer replaces the row's state wholesale rather than
      // the click's being applied optimistically. A grant is a permission;
      // showing it as held before the write landed would be the one place a
      // client should never guess.
      const tools = await setAuthoringToolGrant({
        worldId,
        worldMemberId: member.id,
        tool,
        granted: next,
      });
      setGranted((current) => ({ ...current, [member.id]: tools }));
    } catch (err) {
      setStatus(
        err instanceof Error ? err.message : "Failed to update tool grants",
      );
    } finally {
      setPending(null);
    }
  };

  return (
    <Card className="grid gap-4 p-6" data-testid="authoring-tool-grants-card">
      <div>
        <h2 className="text-lg font-semibold">Player authoring tools</h2>
        <p className="text-sm text-muted-foreground">
          By default only you author the map. Give a player a tool here and it
          appears in their rail; take it back and it stops working for them
          immediately.
        </p>
      </div>

      {players === null ? (
        <Loader label="Loading players" />
      ) : players.length === 0 ? (
        <p
          className="text-sm text-muted-foreground italic"
          data-testid="authoring-tool-grants-empty"
        >
          No players have joined this world yet.
        </p>
      ) : (
        <div className="grid gap-4">
          {players.map((member) => (
            <div
              key={member.id}
              className="grid gap-2"
              data-testid={`authoring-tool-grants-player-${member.id}`}
            >
              <p className="text-sm font-medium">{member.username}</p>
              <div className="flex flex-wrap gap-x-4 gap-y-2">
                {TOOLS.map((tool) => {
                  const isGranted = (granted[member.id] ?? []).includes(tool);
                  return (
                    <label
                      key={tool}
                      className="flex items-center gap-2 text-sm"
                    >
                      <input
                        type="checkbox"
                        checked={isGranted}
                        disabled={pending === `${member.id}:${tool}`}
                        onChange={(event) =>
                          void handleToggle(member, tool, event.target.checked)
                        }
                        data-testid={`authoring-tool-toggle-${member.id}-${tool}`}
                      />
                      {toolLabel(tool)}
                    </label>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      )}

      {status ? (
        <StatusBadge
          variant="danger"
          data-testid="authoring-tool-grants-status"
        >
          {status}
        </StatusBadge>
      ) : null}
    </Card>
  );
}
