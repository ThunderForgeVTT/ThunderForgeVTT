import type { CSSProperties, HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";
import styles from "./Card.module.scss";

export interface CardProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  surface?: "default" | "strong" | "parchment" | "stone" | "leather";
  style?: CSSProperties;
}

export function Card({
  children,
  className,
  surface = "default",
  ...props
}: CardProps) {
  return (
    <section className={cn(styles.card, styles[surface], className)} {...props}>
      {children}
    </section>
  );
}
