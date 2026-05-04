import type { ReactNode } from "react";
import { cn } from "@/utils/cn";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./StatusBadge.module.scss";

type StatusBadgeVariant = "danger" | "info" | "success" | "warning";

export interface StatusBadgeProps {
  children: ReactNode;
  variant?: StatusBadgeVariant;
  className?: string;
}

export function StatusBadge({
  children,
  variant = "info",
  className,
}: StatusBadgeProps) {
  const iconName =
    variant === "success"
      ? "shield"
      : variant === "warning"
      ? "torch"
      : variant === "danger"
      ? "skull"
      : "spark";

  return (
    <p className={cn(styles.badge, styles[variant], className)}>
      <FantasyIcon name={iconName} size={16} />
      <span>{children}</span>
    </p>
  );
}
