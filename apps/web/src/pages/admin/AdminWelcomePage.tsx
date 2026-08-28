import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getAdminWelcomeSummary } from "@/api/admin";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { useAuth } from "@/hooks/useAuth";
import type { AdminWelcomeSummary } from "@/types/admin";
import type { SeoConfig } from "@/types/seo";
import { MetricsCard } from "./components/MetricsCard";

export const adminWelcomePageSeo: SeoConfig = {
  title: "Admin welcome",
  description:
    "Land in the ThunderForge administrator welcome page for quick metrics and fast access to settings, analytics, OAuth configuration, and system controls.",
  canonicalPath: "/admin/welcome",
  noindex: true,
};

function formatMegabytes(bytes: number) {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export default function AdminWelcomePage() {
  const { user } = useAuth();
  const [summary, setSummary] = useState<AdminWelcomeSummary | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    let active = true;

    void getAdminWelcomeSummary()
      .then((response) => {
        if (active) {
          setSummary(response);
        }
      })
      .catch((error) => {
        if (active) {
          setStatus(
            error instanceof Error
              ? error.message
              : "Failed to load admin summary.",
          );
        }
      });

    return () => {
      active = false;
    };
  }, []);

  if (!summary && !status) {
    return <Loader fullScreen label="Loading admin welcome" />;
  }

  return (
    <>
      <SEO {...adminWelcomePageSeo} />
      <Container>
        <main className="grid gap-6 pb-12">
          <section className="grid gap-3 rounded-xl border border-border bg-card p-6">
            <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Administrator landing
            </p>
            <h1 className="text-2xl font-semibold">
              Welcome, Administrator of ThunderForge.
            </h1>
            <p className="max-w-[60ch] text-muted-foreground">
              {user?.username ?? "Admin"}, here&apos;s the current state of the
              system. Review it at a glance, then move into settings, OAuth
              provider setup, analytics, or system policy.
            </p>
            <div className="flex flex-wrap gap-3">
              <Button asChild icon="settings">
                <Link to="/admin/settings">System Settings</Link>
              </Button>
              <Button asChild variant="secondary" icon="scene">
                <Link to="/admin/analytics">Analytics Dashboard</Link>
              </Button>
              <Button asChild variant="ghost" icon="wand">
                <Link to="/admin/oauth">OAuth Providers</Link>
              </Button>
            </div>
          </section>

          {status ? <StatusBadge variant="danger">{status}</StatusBadge> : null}

          {summary ? (
            <>
              <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-5">
                <MetricsCard
                  title="Users"
                  value={summary.totalUsers.toLocaleString()}
                  subtitle="Active account records"
                  icon="actors"
                />
                <MetricsCard
                  title="Worlds"
                  value={summary.totalWorlds.toLocaleString()}
                  subtitle="Persisted realms"
                  icon="worlds"
                  emphasis="violet"
                />
                <MetricsCard
                  title="Tokens"
                  value={summary.totalTokens.toLocaleString()}
                  subtitle="Scene entities"
                  icon="tokens"
                  emphasis="forest"
                />
                <MetricsCard
                  title="Events"
                  value={summary.totalEvents.toLocaleString()}
                  subtitle="World event records"
                  icon="spark"
                />
                <MetricsCard
                  title="Disk usage"
                  value={formatMegabytes(summary.diskUsage)}
                  subtitle="Approximate data root footprint"
                  icon="inventory"
                  emphasis="violet"
                />
              </section>

              <section className="grid grid-cols-1 gap-4 lg:grid-cols-2">
                <Card surface="parchment" className="grid gap-3 p-6">
                  <h2 className="text-lg font-semibold">Quick links</h2>
                  <div className="grid gap-2">
                    <Link
                      to="/admin/settings"
                      className="font-semibold text-primary hover:underline"
                    >
                      System Settings
                    </Link>
                    <Link
                      to="/admin/oauth"
                      className="font-semibold text-primary hover:underline"
                    >
                      OAuth Providers
                    </Link>
                    <Link
                      to="/admin/analytics"
                      className="font-semibold text-primary hover:underline"
                    >
                      Analytics Dashboard
                    </Link>
                    <Link
                      to="/admin/system"
                      className="font-semibold text-primary hover:underline"
                    >
                      Manifest Configuration
                    </Link>
                  </div>
                </Card>
                <Card surface="leather" className="grid gap-3 p-6">
                  <h2 className="text-lg font-semibold">Notes</h2>
                  <p className="text-muted-foreground">
                    Admin metrics here are sourced from persisted tables and
                    live data-directory inspection, so they reflect the current
                    state without relying on mock counters.
                  </p>
                </Card>
              </section>
            </>
          ) : null}
        </main>
      </Container>
    </>
  );
}
