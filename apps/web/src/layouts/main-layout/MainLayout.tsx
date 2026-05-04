import { Outlet } from "react-router-dom";
import { AppHeader } from "@/components/navigation/AppHeader";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";
import { Container } from "@/components/ui/container/Container";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
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
        <Container className={styles.chrome}>
          <div className={styles.hero}>
            <p className={styles.eyebrow}>Fantasy command deck</p>
            <h1>Persistent worlds with arcane controls and collaborative scenecraft.</h1>
            <p>
              The web shell now reads like a guild hall: parchment surfaces, gilded
              controls, and a consistent entrypoint into setup, auth, and active worlds.
            </p>
          </div>
          <RuneDivider label="Forge channels" />
        </Container>
        <Outlet />
      </main>
      <footer className={styles.footer}>
        <Container className={styles.footerInner}>
          <p>ThunderForge keeps Bevy, tldraw, and world sync aligned beneath a single fantasy UI shell.</p>
        </Container>
      </footer>
    </div>
  );
}
