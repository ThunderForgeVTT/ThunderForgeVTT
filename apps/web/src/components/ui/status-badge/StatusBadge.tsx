import type { ReactNode } from "react";
import { cn } from "@/utils/cn";
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
  return <p className={cn(styles.badge, styles[variant], className)}>{children}</p>;
}
