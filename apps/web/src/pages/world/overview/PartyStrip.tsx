import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  getWorldActorImages,
  getWorldActors,
  type ActorImageRecord,
} from "@/api/actors";
import { getWorldMembers } from "@/api/worldMembers";
import { Card } from "@/components/ui/card/Card";
import { useWorldInvites } from "@/hooks/useWorldInvites";
import { portraitOf } from "@/pages/world/actor/actorImagery";
import { partySeats, type Seat } from "@/pages/world/overview/partySeats";
import { splitLinks } from "@/pages/world/players/worldLinks";

/**
 * Spec 100 US3.2, "who is coming": the party, each player beside their
 * character's portrait, and each unclaimed character as an open seat.
 *
 * Replaces the dashboard's Players card (spec 088 FR-033) and keeps what it
 * said: how many members, and, for those who run the world, how many
 * links are open. The links themselves stay on the players page.
 *
 * No online marker: the web app has no presence, and a dot that is only
 * sometimes right is worse than none.
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
    <span
      className="text-sm text-muted-foreground"
      data-testid="world-players-links"
    >
      {text}
    </span>
  );
}

interface Party {
  worldId: string;
  members: number;
  gms: string[];
  seats: Seat[];
  images: Record<string, ActorImageRecord[]>;
}

function SeatCard({ seat, image }: { seat: Seat; image: string | null }) {
  const initial = (seat.character ?? seat.player ?? "?").charAt(0);
  return (
    <li
      className="flex min-w-0 items-center gap-3 rounded-lg border border-border bg-background/60 p-3"
      data-testid="party-seat"
      data-open={seat.player === null ? "true" : undefined}
    >
      {image ? (
        <img
          src={image}
          alt=""
          loading="lazy"
          className="size-14 shrink-0 rounded-full border border-border object-cover"
        />
      ) : (
        <span
          aria-hidden="true"
          className="grid size-14 shrink-0 place-items-center rounded-full border border-dashed border-border text-lg text-muted-foreground uppercase"
        >
          {initial}
        </span>
      )}
      <span className="grid min-w-0">
        <span className="truncate font-medium">
          {seat.character ?? "No character yet"}
        </span>
        <span className="truncate text-sm text-muted-foreground">
          {seat.player ?? "Open seat"}
        </span>
      </span>
    </li>
  );
}

export function PartyStrip({
  worldId,
  runsWorld,
  canInvite,
}: {
  worldId: string;
  runsWorld: boolean;
  /** False in the demo, whose world has no one to invite (spec 074 FR-013). */
  canInvite: boolean;
}) {
  const [party, setParty] = useState<Party | "failed" | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([
      getWorldMembers(worldId),
      getWorldActors(worldId),
      // A face is a nicety: the party still shows without one.
      getWorldActorImages(worldId).catch(() => ({})),
    ])
      .then(([members, actors, images]) => {
        if (!active) return;
        setParty({
          worldId,
          members: members.length,
          ...partySeats(members, actors),
          images,
        });
      })
      .catch(() => {
        if (active) setParty("failed");
      });
    return () => {
      active = false;
    };
  }, [worldId]);

  const current =
    party === "failed" || party?.worldId === worldId ? party : null;

  return (
    <Card
      surface="parchment"
      className="grid content-start gap-4 p-5"
      role="region"
      aria-labelledby="party-strip-heading"
      data-testid="world-players-card"
    >
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="party-strip-heading" className="text-xl font-semibold">
          The party
        </h2>
        <span className="flex flex-wrap items-baseline gap-x-3">
          <span className="font-medium" data-testid="world-players-members">
            {current === "failed"
              ? "The members could not be read"
              : current === null
                ? "Counting members..."
                : plural(current.members, "member", "members")}
          </span>
          {runsWorld ? <ActiveLinkCount worldId={worldId} /> : null}
        </span>
      </div>

      {current && current !== "failed" ? (
        <>
          {current.gms.length > 0 ? (
            <p className="text-sm text-muted-foreground">
              Run by {current.gms.join(", ")}
            </p>
          ) : null}
          {current.seats.length === 0 ? (
            <p
              className="text-sm text-muted-foreground"
              data-testid="party-empty"
            >
              {canInvite && runsWorld
                ? "No players yet. Send an invite link and they will appear here."
                : "No players yet."}
            </p>
          ) : (
            <ul className="grid auto-grid gap-3">
              {current.seats.map((seat) => (
                <SeatCard
                  key={seat.key}
                  seat={seat}
                  image={
                    seat.actorId
                      ? (portraitOf(current.images[seat.actorId])
                          ?.thumbnailUrl ?? null)
                      : null
                  }
                />
              ))}
            </ul>
          )}
        </>
      ) : null}

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
