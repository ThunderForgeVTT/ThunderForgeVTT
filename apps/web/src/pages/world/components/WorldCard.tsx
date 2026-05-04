import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { WorldRecord } from "@/types/world";
import styles from "./WorldCard.module.scss";

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
    <Card surface="parchment" className={styles.card}>
      <div className={styles.header}>
        <div className={styles.titleGroup}>
          <span className={styles.icon}>
            <FantasyIcon name="worlds" size={18} tone="gold" />
          </span>
          <div>
            <p className={styles.kicker}>Guild atlas</p>
            <h2>{world.name}</h2>
          </div>
        </div>
        <span className={styles.updated}>
          Updated {formatTimestamp(world.updatedAt)}
        </span>
      </div>

      <p className={styles.description}>
        {world.description ??
          "This realm has been chartered, but its deeper lore will be written in a later phase."}
      </p>

      <div className={styles.metaGrid}>
        <div>
          <span>Game system</span>
          <strong>{world.gameSystemId ?? "Unbound placeholder"}</strong>
        </div>
        <div>
          <span>Interface pack</span>
          <strong>{world.interfacePackId ?? "Unbound placeholder"}</strong>
        </div>
        <div>
          <span>Created</span>
          <strong>{formatTimestamp(world.createdAt)}</strong>
        </div>
        {showOwner ? (
          <div>
            <span>Owner</span>
            <strong>{shortenId(world.createdBy)}</strong>
          </div>
        ) : null}
      </div>

      <div className={styles.actions}>
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
