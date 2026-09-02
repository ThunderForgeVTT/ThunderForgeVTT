import type { ReactNode } from "react";
import { AdminSidebarNav } from "@/pages/admin/components/AdminSidebarNav";

export interface AdminSectionShellProps {
  children: ReactNode;
}

/**
 * Spec 031 (FR-032): shared chrome for every admin page.
 *
 * A thin wrapper rather than a nested route layout, for the same reason
 * `WorldSectionShell` is one: each admin route page owns its own fetching
 * and its own full-screen loader, and turning them into layout children
 * would mean untangling that first. Wrapping is enough for the nav to be
 * present on every section, which is the whole complaint.
 *
 * Width matches `WorldSectionShell` rather than the app's `Container`
 * (max-w-[1160px]) — these pages sit behind a fixed-width rail and are
 * dense (metric grids, provider forms, a disk chart).
 */
export function AdminSectionShell({ children }: AdminSectionShellProps) {
  return (
    <div className="mx-auto grid w-full max-w-[1800px] gap-4 p-4 sm:p-6 lg:p-8">
      <div className="flex items-start gap-6">
        <AdminSidebarNav />
        <div className="min-w-0 flex-1">{children}</div>
      </div>
    </div>
  );
}
