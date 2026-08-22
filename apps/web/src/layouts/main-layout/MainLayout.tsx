import { Link, Outlet } from "react-router-dom";
import { AppHeader } from "@/components/navigation/AppHeader";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";
import { Container } from "@/components/ui/container/Container";

interface MainLayoutProps {
  brandHref: string;
  navItems: readonly HeaderNavItem[];
}

export function MainLayout({ brandHref, navItems }: MainLayoutProps) {
  return (
    <div className="grid min-h-screen grid-rows-[auto_1fr_auto] bg-background">
      <AppHeader brandHref={brandHref} navItems={navItems} />
      <main className="pt-6 pb-12">
        <Outlet />
      </main>
      <footer className="pb-8">
        <Container>
          <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border pt-4 text-sm text-muted-foreground">
            <p>
              ThunderForge keeps Bevy and world sync aligned beneath a single
              UI shell.
            </p>
            <Link to="/status" className="hover:text-foreground hover:underline">
              System status
            </Link>
          </div>
        </Container>
      </footer>
    </div>
  );
}
