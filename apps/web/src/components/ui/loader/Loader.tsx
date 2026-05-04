import { cn } from "@/utils/cn";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./Loader.module.scss";

export interface LoaderProps {
  label?: string;
  fullScreen?: boolean;
  className?: string;
}

export function Loader({
  label = "Loading...",
  fullScreen = false,
  className,
}: LoaderProps) {
  return (
    <div
      className={cn(styles.loader, fullScreen && styles.fullScreen, className)}
    >
      <span className={styles.spinner} aria-hidden="true">
        <FantasyIcon name="spark" size={20} />
      </span>
      <span>{label}</span>
    </div>
  );
}
