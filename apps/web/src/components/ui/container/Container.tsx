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
        "mx-auto w-full max-w-(--page-max) px-4 sm:px-6 lg:px-8",
        narrow && "max-w-[780px]",
        className,
      )}
      {...props}
    >
      {children}
    </div>
  );
}
