import { Outlet } from "react-router-dom";
import { AppFooter } from "@/components/navigation/AppFooter";
import { AppHeader } from "@/components/navigation/AppHeader";
import type { HeaderNavItem } from "@/components/navigation/AppHeader";

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
      <AppFooter />
    </div>
  );
}
