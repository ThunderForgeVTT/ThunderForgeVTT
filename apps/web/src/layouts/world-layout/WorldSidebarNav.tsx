import { useEffect, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";

const COLLAPSE_STORAGE_KEY = "tf:world-sidebar-collapsed";

interface WorldNavCategory {
  label: string;
  icon: FantasyIconName;
  to: string;
  testId: string;
  /** Also matches this path prefix as "active" (e.g. a tab link matching
   * the shared /compendium route regardless of which tab is selected). */
  matchPrefix?: string;
}

function readStoredCollapsed(): boolean {
  try {
    return window.localStorage.getItem(COLLAPSE_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export interface WorldSidebarNavProps {
  worldId: string;
  /** Owner/GM gates the admin-only System settings & license entry. */
  isGm: boolean;
}

/**
 * Persistent world section nav — every member sees the same set of
 * category links (Session Setup, Play, and the Compendium's NPCs/Lore/
 * Items/Abilities tabs deep-linked directly); GMs/Owners additionally see
 * System settings & license. Collapses to an icon rail, remembered across
 * pages via localStorage.
 */
export function WorldSidebarNav({ worldId, isGm }: WorldSidebarNavProps) {
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(readStoredCollapsed);

  useEffect(() => {
    try {
      window.localStorage.setItem(COLLAPSE_STORAGE_KEY, collapsed ? "1" : "0");
    } catch {
      // Best-effort only — a private browsing context or blocked storage
      // just means the collapsed state won't persist across pages.
    }
  }, [collapsed]);

  const categories: WorldNavCategory[] = [
    {
      label: "Overview",
      icon: "compass",
      to: `/world/${worldId}/staging`,
      testId: "world-nav-overview",
    },
    {
      label: "Scenes",
      icon: "map",
      to: `/world/${worldId}/scenes`,
      testId: "world-nav-scenes",
    },
    {
      label: "Players",
      icon: "actors",
      to: `/world/${worldId}/players`,
      testId: "world-nav-players",
    },
    {
      label: "NPCs",
      icon: "skull",
      to: `/world/${worldId}/compendium?tab=npcs`,
      matchPrefix: `/world/${worldId}/compendium`,
      testId: "world-nav-npcs",
    },
    {
      label: "Lore",
      icon: "quill",
      to: `/world/${worldId}/compendium?tab=lore`,
      matchPrefix: `/world/${worldId}/compendium`,
      testId: "world-nav-lore",
    },
    {
      label: "Items",
      icon: "inventory",
      to: `/world/${worldId}/compendium?tab=items`,
      matchPrefix: `/world/${worldId}/compendium`,
      testId: "world-nav-items",
    },
    {
      label: "Abilities",
      icon: "spells",
      to: `/world/${worldId}/compendium?tab=abilities`,
      matchPrefix: `/world/${worldId}/compendium`,
      testId: "world-nav-abilities",
    },
  ];

  const adminCategories: WorldNavCategory[] = [
    {
      label: "System settings & license",
      icon: "settings",
      to: `/world/${worldId}/settings/system`,
      testId: "world-nav-system-settings",
    },
  ];

  const currentTab = new URLSearchParams(location.search).get("tab");
  const isActive = (category: WorldNavCategory) => {
    if (category.matchPrefix) {
      const [, query] = category.to.split("?");
      const wantedTab = new URLSearchParams(query).get("tab");
      return (
        location.pathname === category.matchPrefix && currentTab === wantedTab
      );
    }
    return location.pathname === category.to;
  };

  const renderLink = (category: WorldNavCategory) => {
    const active = isActive(category);
    return (
      <Link
        key={category.testId}
        to={category.to}
        data-testid={category.testId}
        title={collapsed ? category.label : undefined}
        aria-current={active ? "page" : undefined}
        className={cn(
          "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
          collapsed && "justify-center px-2",
          active
            ? "bg-primary/10 text-primary"
            : "text-muted-foreground hover:bg-secondary hover:text-foreground",
        )}
      >
        <FantasyIcon name={category.icon} size={18} />
        {collapsed ? null : <span>{category.label}</span>}
      </Link>
    );
  };

  return (
    <nav
      className={cn(
        "grid h-fit shrink-0 gap-4 rounded-xl border border-border bg-card p-3 transition-[width]",
        collapsed ? "w-[3.75rem]" : "w-56",
      )}
      data-testid="world-sidebar-nav"
      aria-label="World navigation"
    >
      <div
        className={cn(
          "flex items-center",
          collapsed ? "justify-center" : "justify-end",
        )}
      >
        <Button
          type="button"
          variant="ghost"
          size="sm"
          icon="arrow-left"
          iconPosition={collapsed ? "start" : "end"}
          className={collapsed ? "[&_svg]:rotate-180" : undefined}
          data-testid="world-sidebar-toggle"
          onClick={() => setCollapsed((current) => !current)}
          aria-label={collapsed ? "Expand navigation" : "Collapse navigation"}
        >
          {collapsed ? "" : "Collapse"}
        </Button>
      </div>

      <div className="grid gap-1">{categories.map(renderLink)}</div>

      {isGm ? (
        <div className="grid gap-1 border-t border-border pt-3">
          {!collapsed ? (
            <p className="px-3 pb-1 text-[0.65rem] font-semibold tracking-widest text-muted-foreground uppercase">
              Admin
            </p>
          ) : null}
          {adminCategories.map(renderLink)}
        </div>
      ) : null}
    </nav>
  );
}
