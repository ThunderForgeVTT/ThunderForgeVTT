import * as RadixScrollArea from "@radix-ui/react-scroll-area";
import type { ReactNode } from "react";
import { cn } from "@/utils/cn";
import styles from "./ScrollArea.module.scss";

export interface ScrollAreaProps {
  children: ReactNode;
  className?: string;
  viewportClassName?: string;
}

export function ScrollArea({
  children,
  className,
  viewportClassName,
}: ScrollAreaProps) {
  return (
    <RadixScrollArea.Root className={cn(styles.root, className)}>
      <RadixScrollArea.Viewport
        className={cn(styles.viewport, viewportClassName)}
      >
        {children}
      </RadixScrollArea.Viewport>
      <RadixScrollArea.Scrollbar
        className={styles.scrollbar}
        orientation="vertical"
      >
        <RadixScrollArea.Thumb className={styles.thumb} />
      </RadixScrollArea.Scrollbar>
    </RadixScrollArea.Root>
  );
}
