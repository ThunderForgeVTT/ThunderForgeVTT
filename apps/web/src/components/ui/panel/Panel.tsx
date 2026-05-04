import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";
import styles from "./Panel.module.scss";

type PanelVariant = "parchment" | "stone" | "leather" | "veil";

export interface PanelProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  variant?: PanelVariant;
}

export function Panel({
  children,
  variant = "parchment",
  className,
  ...props
}: PanelProps) {
  return (
    <section className={cn(styles.panel, styles[variant], className)} {...props}>
      {children}
    </section>
  );
}
