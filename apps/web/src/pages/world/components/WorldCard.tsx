import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { WorldRecord } from "@/types/world";

interface WorldCardProps {
  world: WorldRecord;
  showOwner?: boolean;
}

function formatTimestamp(value: string) {
  return new Date(value).toLocaleDateString();
}

function shortenId(value: string) {
  return `${value.slice(0, 8)}...`;
}

export function WorldCard({ world, showOwner = false }: WorldCardProps) {
  return (
    <Card surface="parchment" className="grid gap-4 p-5">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <span className="inline-grid size-9 shrink-0 place-items-center rounded-full border border-border bg-secondary">
            <FantasyIcon name="worlds" size={16} />
          </span>
          <div>
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              World
            </p>
            <h2 className="text-lg font-semibold">{world.name}</h2>
          </div>
        </div>
        <span className="text-xs text-muted-foreground">
          Updated {formatTimestamp(world.updatedAt)}
        </span>
      </div>

      <p className="text-sm text-muted-foreground">
        {world.description ??
          "This world has been created, but its deeper lore will be written in a later phase."}
      </p>

      <div className="grid grid-cols-2 gap-3 text-sm">
        <div>
          <span className="block text-xs text-muted-foreground">
            Game system
          </span>
          <strong className="font-medium">
            {world.gameSystemId ?? "Unbound placeholder"}
          </strong>
        </div>
        <div>
          <span className="block text-xs text-muted-foreground">
            Interface pack
          </span>
          <strong className="font-medium">
            {world.interfacePackId ?? "Unbound placeholder"}
          </strong>
        </div>
        <div>
          <span className="block text-xs text-muted-foreground">
            Created
          </span>
          <strong className="font-medium">
            {formatTimestamp(world.createdAt)}
          </strong>
        </div>
        {showOwner ? (
          <div>
            <span className="block text-xs text-muted-foreground">
              Owner
            </span>
            <strong className="font-medium">
              {shortenId(world.createdBy)}
            </strong>
          </div>
        ) : null}
      </div>

      <div className="flex flex-wrap gap-2">
        <Button asChild icon="worlds">
          <Link to={`/world/${world.id}`}>Open dashboard</Link>
        </Button>
        <Button asChild variant="ghost" icon="spark">
          <Link to={`/world/${world.id}/play`}>Enter world</Link>
        </Button>
      </div>
    </Card>
  );
}
