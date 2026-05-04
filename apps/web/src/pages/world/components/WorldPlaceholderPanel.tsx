import { Card } from "@/components/ui/card/Card";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./WorldPlaceholderPanel.module.scss";

interface WorldPlaceholderPanelProps {
  title: string;
  icon: FantasyIconName;
  copy: string;
  items: readonly string[];
  surface?: "default" | "strong" | "parchment" | "stone" | "leather";
}

export function WorldPlaceholderPanel({
  title,
  icon,
  copy,
  items,
  surface = "stone",
}: WorldPlaceholderPanelProps) {
  return (
    <Card surface={surface} className={styles.panel}>
      <div className={styles.header}>
        <span className={styles.icon}>
          <FantasyIcon name={icon} size={18} tone="gold" />
        </span>
        <div>
          <p className={styles.kicker}>Placeholder domain</p>
          <h3>{title}</h3>
        </div>
      </div>
      <p className={styles.copy}>{copy}</p>
      {items.length > 0 ? (
        <ul className={styles.list}>
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      ) : (
        <p className={styles.empty}>
          Awaiting a later phase. No persisted records yet.
        </p>
      )}
    </Card>
  );
}
