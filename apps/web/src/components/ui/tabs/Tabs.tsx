import type { ReactNode } from "react";
import {
  Tabs as ShadcnTabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

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
  /** Controlled active tab — pass together with `onValueChange` to drive
   * the selection externally (e.g. from a URL search param). Omit both
   * for the plain uncontrolled `defaultValue` behavior. */
  value?: string;
  onValueChange?: (value: string) => void;
}

export function Tabs({
  items,
  defaultValue,
  className,
  value,
  onValueChange,
}: TabsProps) {
  return (
    <ShadcnTabs
      defaultValue={defaultValue}
      value={value}
      onValueChange={onValueChange}
      // `min-w-0` so a grid or flex parent may shrink the tabs below the
      // strip's full width; without it the strip below cannot scroll,
      // because its parent never gets narrower than it.
      className={cn("min-w-0", className)}
    >
      {/* `max-w-full` and its own horizontal scroll: the list is `w-fit`,
          so with five labelled tabs it grew past a 375px viewport and took
          the whole page sideways with it (44px on the compendium). The
          strip scrolls instead of the page, and arrow keys still move
          focus along it, which scrolls the focused tab into view. */}
      <TabsList
        aria-label="Sections"
        className="max-w-full justify-start overflow-x-auto"
      >
        {items.map((item) => (
          <TabsTrigger key={item.value} value={item.value}>
            {item.icon ? <FantasyIcon name={item.icon} size={16} /> : null}
            <span>{item.label}</span>
          </TabsTrigger>
        ))}
      </TabsList>

      {items.map((item) => (
        <TabsContent key={item.value} value={item.value}>
          {item.content}
        </TabsContent>
      ))}
    </ShadcnTabs>
  );
}
