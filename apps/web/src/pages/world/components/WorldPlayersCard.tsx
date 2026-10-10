import { Link } from "react-router-dom";
import { Card } from "@/components/ui/card/Card";
import { useWorldInvites } from "@/hooks/useWorldInvites";
import { useWorldMembers } from "@/hooks/useWorldMembers";
import { splitLinks } from "@/pages/world/players/worldLinks";

/**
 * Spec 088 US4 (FR-033, contracts/layouts.md): the world page's Players card.
 * Who is in the world, and, for those who run it, how many links are open.
 * The links themselves are managed on the players page (FR-001); this card
 * only counts them and points there.
 */

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}

/** Mounted only for those who run the world: a player may not list links. */
function ActiveLinkCount({ worldId }: { worldId: string }) {
  const { invites, loading, error } = useWorldInvites(worldId);
  const text = error
    ? "Links could not be read"
    : loading
      ? "Counting links..."
      : plural(
          splitLinks(invites).active.length,
          "active link",
          "active links",
        );
  return (
    <p
      className="text-sm text-muted-foreground"
      data-testid="world-players-links"
    >
      {text}
    </p>
  );
}

export function WorldPlayersCard({
  worldId,
  runsWorld,
}: {
  worldId: string;
  runsWorld: boolean;
}) {
  const { members, loading, error } = useWorldMembers(worldId);
  return (
    <Card
      surface="parchment"
      className="grid gap-3 p-6"
      data-testid="world-players-card"
    >
      <h2 className="text-xl font-semibold">Players</h2>
      <p className="font-medium" data-testid="world-players-members">
        {error
          ? "The members could not be read"
          : loading
            ? "Counting members..."
            : plural(members.length, "member", "members")}
      </p>
      {runsWorld ? <ActiveLinkCount worldId={worldId} /> : null}
      <Link
        to={`/world/${worldId}/players`}
        className="text-sm text-primary underline-offset-4 hover:underline"
        data-testid="world-players-page-link"
      >
        {runsWorld ? "Invite and manage players" : "See who is playing"}
      </Link>
    </Card>
  );
}
