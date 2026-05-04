import * as RadixTabs from "@radix-ui/react-tabs";
import type { ReactNode } from "react";
import { cn } from "@/utils/cn";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./Tabs.module.scss";

export interface TabsItem {
  value: string;
  label: string;
  icon?: FantasyIconName;
  content: ReactNode;
}

export interface TabsProps {
  items: TabsItem[];
  defaultValue: string;
  className?: string;
}

export function Tabs({ items, defaultValue, className }: TabsProps) {
  return (
    <RadixTabs.Root
      className={cn(styles.tabs, className)}
      defaultValue={defaultValue}
    >
      <RadixTabs.List className={styles.list} aria-label="Sections">
        {items.map((item) => (
          <RadixTabs.Trigger
            key={item.value}
            className={styles.trigger}
            value={item.value}
          >
            {item.icon ? <FantasyIcon name={item.icon} size={16} /> : null}
            <span>{item.label}</span>
          </RadixTabs.Trigger>
        ))}
      </RadixTabs.List>

      {items.map((item) => (
        <RadixTabs.Content
          key={item.value}
          className={styles.content}
          value={item.value}
        >
          {item.content}
        </RadixTabs.Content>
      ))}
    </RadixTabs.Root>
  );
}
