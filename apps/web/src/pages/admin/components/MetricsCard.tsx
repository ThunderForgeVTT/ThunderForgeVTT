import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./MetricsCard.module.scss";

interface MetricsCardProps {
  title: string;
  value: string;
  subtitle: string;
  icon: FantasyIconName;
  emphasis?: "gold" | "violet" | "forest";
}

export function MetricsCard({
  title,
  value,
  subtitle,
  icon,
  emphasis = "gold",
}: MetricsCardProps) {
  return (
    <article className={styles.card} data-emphasis={emphasis}>
      <div className={styles.header}>
        <span className={styles.iconWrap}>
          <FantasyIcon name={icon} size={18} />
        </span>
        <p>{title}</p>
      </div>
      <strong className={styles.value}>{value}</strong>
      <span className={styles.subtitle}>{subtitle}</span>
    </article>
  );
}
