import { NavLink } from "react-router-dom";
import { prefetchPage } from "@/routes/pageLoaders";
import type { PrefetchablePage } from "@/routes/pageLoaders";
import { cn } from "@/utils/cn";
import styles from "./AppHeader.module.scss";

export interface HeaderNavItem {
  to: string;
  label: string;
  prefetch?: PrefetchablePage;
}

interface AppHeaderProps {
  brandHref: string;
  navItems: readonly HeaderNavItem[];
}

export function AppHeader({ brandHref, navItems }: AppHeaderProps) {
  return (
    <header className={styles.header}>
      <div className={styles.inner}>
        <NavLink to={brandHref} className={styles.brand}>
          <img
            src="/brand-mark.svg"
            alt="ThunderForge"
            width="40"
            height="40"
            loading="eager"
          />
          <span>
            <strong>ThunderForge</strong>
            <small>Collaborative tabletop control room</small>
          </span>
        </NavLink>

        <nav className={styles.nav} aria-label="Primary">
          {navItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) => cn(styles.link, isActive && styles.active)}
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
              {item.label}
            </NavLink>
          ))}
        </nav>
      </div>
    </header>
  );
}
