import { useEffect, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { cn } from "@/lib/utils";
import {
  ADMIN_SECTIONS,
  type AdminSection,
} from "@/pages/admin/components/adminSections";

const COLLAPSE_STORAGE_KEY = "tf:admin-sidebar-collapsed";

function readStoredCollapsed(): boolean {
  try {
    return window.localStorage.getItem(COLLAPSE_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

/**
 * Spec 031 (FR-032): persistent navigation between the admin sections.
 *
 * Deliberately the same component as `WorldSidebarNav` in everything but
 * its list — same rail, same collapse affordance, same active styling,
 * same `localStorage` remembering. An administrator moving between a world
 * and the admin area should not have to learn a second way to get around,
 * and a second navigation idiom would be a second thing to keep consistent
 * every time either one changes.
 *
 * Its own storage key, though: collapsing the world nav to make room for a
 * map says nothing about how much room the admin pages need.
 */
export function AdminSidebarNav() {
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

  const renderLink = (section: AdminSection) => {
    const active = location.pathname === section.to;
    return (
      <Link
        key={section.testId}
        to={section.to}
        data-testid={section.testId}
        title={collapsed ? section.label : undefined}
        aria-current={active ? "page" : undefined}
        className={cn(
          "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-colors",
          collapsed && "justify-center px-2",
          active
            ? "bg-primary/10 text-primary"
            : "text-muted-foreground hover:bg-secondary hover:text-foreground",
        )}
      >
        <FantasyIcon name={section.icon} size={18} />
        {collapsed ? null : <span>{section.label}</span>}
      </Link>
    );
  };

  return (
    <nav
      className={cn(
        "grid h-fit shrink-0 gap-4 rounded-xl border border-border bg-card p-3 transition-[width]",
        collapsed ? "w-[3.75rem]" : "w-56",
      )}
      data-testid="admin-sidebar-nav"
      aria-label="Admin navigation"
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
          data-testid="admin-sidebar-toggle"
          onClick={() => setCollapsed((current) => !current)}
          aria-label={collapsed ? "Expand navigation" : "Collapse navigation"}
        >
          {collapsed ? "" : "Collapse"}
        </Button>
      </div>

      <div className="grid gap-1">{ADMIN_SECTIONS.map(renderLink)}</div>
    </nav>
  );
}
