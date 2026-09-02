import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";

/**
 * Spec 031 (FR-032): the admin area's sections, in one list.
 *
 * The routes already existed — `AppRoutes.tsx` mounts each of these paths,
 * plus a handful of aliases that redirect into them — but nothing on the
 * screen said so. A playtest found the admin area reachable only by typing
 * URLs, and the one "Sections" card that did exist was rendered by the
 * settings page alone, so moderation was a dead end you could enter and
 * not leave.
 *
 * Declared as data rather than as markup so both the nav and its test can
 * read the same list, and so adding a section is one entry rather than an
 * edit in three files.
 */
export interface AdminSection {
  label: string;
  description: string;
  /** The canonical path. Aliases redirect here; nothing links to them. */
  to: string;
  icon: FantasyIconName;
  testId: string;
}

export const ADMIN_SECTIONS: readonly AdminSection[] = [
  {
    label: "Overview",
    description: "System analytics and live counts",
    to: "/admin",
    icon: "scene",
    testId: "admin-nav-overview",
  },
  {
    label: "Configuration",
    description: "OAuth providers and manifest editing",
    to: "/admin/configuration",
    icon: "wand",
    testId: "admin-nav-configuration",
  },
  {
    label: "Storage",
    description: "Disk posture and persisted footprint",
    to: "/admin/storage",
    icon: "inventory",
    testId: "admin-nav-storage",
  },
  {
    label: "Security",
    description: "2FA enforcement and bootstrap record",
    to: "/admin/security",
    icon: "shield",
    testId: "admin-nav-security",
  },
  {
    label: "Moderation",
    description: "Takedown cases and repeat-infringer flags",
    to: "/admin/moderation",
    icon: "quill",
    testId: "admin-nav-moderation",
  },
];

/**
 * The section a path is currently in, or `null` for a path outside the
 * admin area.
 *
 * Exact matching, deliberately: `/admin` is a prefix of every other admin
 * path, so a `startsWith` test would light up Overview on every screen and
 * the nav would never show where you actually are. The redirect aliases
 * (`/admin/settings`, `/admin/analytics`, …) never render, so they need no
 * entry here.
 */
export function adminSectionForPath(pathname: string): AdminSection | null {
  return ADMIN_SECTIONS.find((section) => section.to === pathname) ?? null;
}
