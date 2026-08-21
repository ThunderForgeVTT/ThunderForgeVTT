import type { HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

export interface ContainerProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  narrow?: boolean;
}

export function Container({
  children,
  className,
  narrow = false,
  ...props
}: ContainerProps) {
  return (
    <div
      className={cn(
        "mx-auto w-[calc(100%-2rem)] max-w-[1160px]",
        narrow && "max-w-[780px]",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}
