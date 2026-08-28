import { SEO } from "@/components/seo/SEO";
import { Container } from "@/components/ui/container/Container";
import { StoragePanel } from "@/components/diagnostics/StoragePanel";
import { PeerPanel } from "@/components/diagnostics/PeerPanel";
import type { SeoConfig } from "@/types/seo";

export const storageSettingsPageSeo: SeoConfig = {
  title: "Offline storage",
  description:
    "See how much space ThunderForge is using on this device and clear any of it.",
  canonicalPath: "/settings/storage",
  noindex: true,
};

/**
 * The user-facing home for spec 028's US5 (FR-025/FR-026).
 *
 * Deliberately *not* `/admin/storage`, which is admin-only and concerns the
 * server's object store. This is per-user, per-device, and about the browser's
 * own disk — a player with no admin rights is exactly who needs it, since the
 * cache is on their machine and the space is theirs.
 */
export function StorageSettingsPage() {
  return (
    <>
      <SEO {...storageSettingsPageSeo} />
      <Container>
        <div className="grid gap-6 py-8">
          <header className="grid gap-1">
            <h1 className="text-2xl font-semibold">
              Storage and sharing on this device
            </h1>
            <p className="text-sm text-muted-foreground">
              Everything here is a choice about this browser, on this machine.
              Clearing stored worlds never touches your account, your worlds, or
              anyone else&apos;s copy.
            </p>
          </header>
          <StoragePanel />
          {/*
            Spec 028 FR-049. Peer transfer is the other thing this machine does
            on its own behalf while a world is open, and it is a per-user,
            per-device choice — the same shape as the cache above, so it sits
            on the same page rather than getting a settings screen of its own
            that nobody would find.
          */}
          <PeerPanel />
        </div>
      </Container>
    </>
  );
}

export default StorageSettingsPage;
