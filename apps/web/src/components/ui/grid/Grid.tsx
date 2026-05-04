import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/utils/cn";
import styles from "./Grid.module.scss";

type GridColumns = "two" | "three";

export interface GridProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  columns?: GridColumns;
}

export function Grid({
  children,
  className,
  columns = "two",
  ...props
}: GridProps) {
  return (
    <div className={cn(styles.grid, styles[columns], className)} {...props}>
      {children}
    </div>
  );
}
