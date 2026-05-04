import { Outlet } from "react-router-dom";
import { AppHeader } from "@/components/navigation/AppHeader";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";
import styles from "./MainLayout.module.scss";

interface MainLayoutProps {
  brandHref: string;
  navItems: readonly HeaderNavItem[];
}

export function MainLayout({ brandHref, navItems }: MainLayoutProps) {
  return (
    <div className={styles.layout}>
      <AppHeader brandHref={brandHref} navItems={navItems} />
      <main className={styles.main}>
        <Outlet />
      </main>
      <footer className={styles.footer}>
        <p>ThunderForge VTT keeps setup, collaboration, and world state in sync.</p>
      </footer>
    </div>
  );
}
