import * as Separator from "@radix-ui/react-separator";
import { cn } from "@/utils/cn";
import styles from "./RuneDivider.module.scss";

export interface RuneDividerProps {
  label?: string;
  className?: string;
}

export function RuneDivider({ label = "Arcane sigils", className }: RuneDividerProps) {
  return (
    <div className={cn(styles.divider, className)}>
      <Separator.Root className={styles.line} decorative orientation="horizontal" />
      <span className={styles.label}>{label}</span>
      <Separator.Root className={styles.line} decorative orientation="horizontal" />
    </div>
  );
}
