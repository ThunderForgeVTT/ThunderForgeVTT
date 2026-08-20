import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

type GridColumns = "two" | "three";

export interface GridProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  columns?: GridColumns;
}

const COLUMN_CLASSES: Record<GridColumns, string> = {
  two: "grid-cols-[repeat(auto-fit,minmax(280px,1fr))]",
  three: "grid-cols-[repeat(auto-fit,minmax(220px,1fr))]",
};

export function Grid({
  children,
  className,
  columns = "two",
  ...props
}: GridProps) {
  return (
    <div
      className={cn("grid gap-6", COLUMN_CLASSES[columns], className)}
      {...props}
    >
      {children}
    </div>
  );
}
