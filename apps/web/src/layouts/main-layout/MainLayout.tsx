import { Outlet } from "react-router-dom";
import { AppHeader } from "@/components/navigation/AppHeader";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";
import { Container } from "@/components/ui/container/Container";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";

interface MainLayoutProps {
  brandHref: string;
  navItems: readonly HeaderNavItem[];
}

export function MainLayout({ brandHref, navItems }: MainLayoutProps) {
  return (
    <div className="grid min-h-screen grid-rows-[auto_1fr_auto] bg-background">
      <AppHeader brandHref={brandHref} navItems={navItems} />
      <main className="pt-6 pb-12">
        <Container className="mb-6 grid gap-4">
          <div className="grid max-w-4xl gap-3">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Command deck
            </p>
            <h1 className="text-4xl leading-tight font-semibold text-balance sm:text-5xl">
              Persistent worlds with a clean, consistent control surface.
            </h1>
            <p className="max-w-3xl text-muted-foreground">
              One entrypoint into setup, authentication, and your active
              worlds — built on shadcn/ui so every screen shares the same
              components and behavior.
            </p>
          </div>
          <RuneDivider label="Forge channels" />
        </Container>
        <Outlet />
      </main>
      <footer className="pb-8">
        <Container>
          <p className="border-t border-border pt-4 text-sm text-muted-foreground">
            ThunderForge keeps Bevy and world sync aligned beneath a single UI
            shell.
          </p>
        </Container>
      </footer>
    </div>
  );
}
