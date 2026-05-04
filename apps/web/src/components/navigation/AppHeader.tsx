import { NavLink, useNavigate } from "react-router-dom";
import { prefetchPage } from "@/routes/pageLoaders";
import type { PrefetchablePage } from "@/routes/pageLoaders";
import { cn } from "@/utils/cn";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { Button } from "@/components/ui/button/Button";
import { Dropdown } from "@/components/ui/dropdown/Dropdown";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import styles from "./AppHeader.module.scss";

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

  return (
    <header className={styles.header}>
      <div className={styles.inner}>
        <NavLink to={brandHref} className={styles.brand}>
          <span className={styles.brandMark}>
            <FantasyIcon name="crown" size={22} tone="gold" />
          </span>
          <span>
            <strong>ThunderForge</strong>
            <small>Leather-bound command tome</small>
          </span>
        </NavLink>

        <nav className={styles.nav} aria-label="Primary">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(styles.link, isActive && styles.active)
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

        <div className={styles.actions}>
          <Dropdown
            trigger={
              <Button variant="ghost" size="sm" icon="rune">
                Codex
              </Button>
            }
            items={[
              {
                label: "Open dashboard preview",
                icon: "scene",
                onSelect: () => navigate("/counter"),
              },
              {
                label: "Enter demo world",
                icon: "worlds",
                onSelect: () => navigate("/world/demo-world"),
              },
              {
                label: "Review setup gate",
                icon: "settings",
                onSelect: () => navigate("/setup"),
              },
            ]}
          />
          <div className={styles.profile}>
            <Avatar seed="archmage-of-thunderforge" name="Archmage" size="sm" />
            <div>
              <strong>Archmage</strong>
              <small>Realm steward</small>
            </div>
          </div>
        </div>
      </div>
    </header>
  );
}
