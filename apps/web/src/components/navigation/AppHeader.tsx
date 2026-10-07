import { NavLink, useNavigate } from "react-router-dom";
import { prefetchPage } from "@/routes/pageLoaders";
import type { PrefetchablePage } from "@/routes/pageLoaders";
import { cn } from "@/lib/utils";
import { IN_DEMO } from "@/lib/demoBuild";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { Button } from "@/components/ui/button/Button";
import { Dropdown } from "@/components/ui/dropdown/Dropdown";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import { ThemeToggle } from "@/components/navigation/ThemeToggle";
import { DEMO_LINK } from "@/components/navigation/siteLinks";
import { FEATURE_DEMO } from "@/api/featureFlags";
import { useAuth } from "@/hooks/useAuth";
import { useFeatureFlag } from "@/hooks/useFeatureFlag";

export interface HeaderNavItem {
  to: string;
  label: string;
  prefetch?: PrefetchablePage;
  icon?: FantasyIconName;
}

interface AppHeaderProps {
  brandHref: string;
  navItems: readonly HeaderNavItem[];
}

export function AppHeader({ brandHref, navItems }: AppHeaderProps) {
  const navigate = useNavigate();
  const { isAdmin, isAuthenticated, logout, user } = useAuth();
  // The demo is a second client served beside this one, so entering it is a
  // page load, and it is offered only where the instance offers it.
  const demoOffered = useFeatureFlag(FEATURE_DEMO);
  const demoItems = demoOffered
    ? [
        {
          label: DEMO_LINK.label,
          icon: "spark" as const,
          onSelect: () => window.location.assign(DEMO_LINK.to),
        },
      ]
    : [];
  const menuItems = isAuthenticated
    ? [
        {
          label: isAdmin ? "Open admin command center" : "Open welcome hall",
          icon: isAdmin ? ("crown" as const) : ("scene" as const),
          onSelect: () => navigate(isAdmin ? "/admin" : "/welcome"),
        },
        {
          label: "World archive",
          icon: "worlds" as const,
          onSelect: () => navigate("/worlds"),
        },
        {
          label: "Your account",
          icon: "settings" as const,
          onSelect: () => navigate("/settings/account"),
        },
        ...demoItems,
        // Spec 074 FR-013: the demo visitor never signed in, so there is no
        // session to end.
        ...(IN_DEMO
          ? []
          : [
              {
                label: "Sign out",
                icon: "arrow-left" as const,
                onSelect: () => {
                  void logout().then(() => navigate("/login"));
                },
              },
            ]),
      ]
    : demoItems;

  return (
    <header className="sticky top-0 z-30 border-b border-border bg-background/95 backdrop-blur-sm">
      <div className="mx-auto flex min-h-20 w-[calc(100%-2rem)] max-w-[1160px] flex-wrap items-center justify-between gap-4 py-3 md:flex-nowrap md:py-0">
        <NavLink
          to={brandHref}
          className="inline-flex min-w-0 items-center gap-3"
        >
          <span className="inline-grid size-10 shrink-0 place-items-center rounded-full border border-border bg-secondary">
            <FantasyIcon name="crown" size={20} />
          </span>
          <span className="grid">
            <strong className="text-sm tracking-wide">ThunderForge</strong>
            <small className="text-xs text-muted-foreground">
              Command center
            </small>
          </span>
        </NavLink>

        <nav
          className="order-3 flex w-full flex-wrap justify-start gap-1 md:order-none md:w-auto md:flex-1 md:justify-center"
          aria-label="Primary"
        >
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(
                  "inline-flex items-center gap-2 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
                  isActive && "bg-muted text-foreground",
                )
              }
              onMouseEnter={() => {
                if (item.prefetch) {
                  prefetchPage(item.prefetch);
                }
              }}
              onFocus={() => {
                if (item.prefetch) {
                  prefetchPage(item.prefetch);
                }
              }}
            >
              {item.icon ? <FantasyIcon name={item.icon} size={16} /> : null}
              {item.label}
            </NavLink>
          ))}
        </nav>

        <div className="flex items-center gap-3">
          <ThemeToggle />
          {/*
           * Signed out, this menu offers the demo and nothing else, and where
           * the instance offers no demo there is no menu.
           *
           * It used to offer the welcome hall, the world archive and system
           * settings to everybody, and all three are behind authentication —
           * so a visitor who had never signed in was given three doors that
           * bounce them to `/login` and one that works. Signing in is not
           * missing from the header as a result: the primary nav carries
           * Login and Register for exactly this visitor (`AppRoutes.tsx`),
           * which is why there is deliberately no sign-in entry duplicated
           * here.
           */}
          {menuItems.length > 0 ? (
            <Dropdown
              trigger={
                <Button variant="ghost" size="sm" icon="rune">
                  Menu
                </Button>
              }
              items={menuItems}
            />
          ) : null}
          {/* Only for somebody who actually is somebody. This block used to
              render "Archmage" and the role "Member" to a signed-out visitor,
              which is a made-up name and a membership they do not have sitting
              where their own identity would go. */}
          {isAuthenticated ? (
            <div className="flex items-center gap-2 border-l border-border pl-3">
              <Avatar
                seed={user?.id ?? "archmage-of-thunderforge"}
                name={user?.username ?? "Archmage"}
                size="sm"
              />
              <div className="hidden sm:grid">
                <strong className="text-sm">
                  {user?.username ?? "Archmage"}
                </strong>
                <small className="text-xs tracking-wide text-muted-foreground uppercase">
                  {user?.role === "admin" ? "Administrator" : "Member"}
                </small>
              </div>
            </div>
          ) : null}
        </div>
      </div>
    </header>
  );
}
