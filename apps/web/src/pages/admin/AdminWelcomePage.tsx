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
import styles from "./AdminWelcomePage.module.scss";

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
          setStatus(error instanceof Error ? error.message : "Failed to load admin summary.");
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
        <main className={styles.shell}>
          <section className={styles.hero}>
            <p className={styles.eyebrow}>Administrator landing</p>
            <h1>Welcome, Administrator of ThunderForge.</h1>
            <p>
              {user?.username ?? "Steward"}, the guild hall is ready. Review the
              realm at a glance, then move into settings, OAuth provider setup,
              analytics, or system policy.
            </p>
            <div className={styles.actions}>
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
              <section className={styles.metricsGrid}>
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

              <section className={styles.quickGrid}>
                <Card surface="parchment" className={styles.quickCard}>
                  <h2>Quick links</h2>
                  <div className={styles.linkList}>
                    <Link to="/admin/settings">System Settings</Link>
                    <Link to="/admin/oauth">OAuth Providers</Link>
                    <Link to="/admin/analytics">Analytics Dashboard</Link>
                    <Link to="/admin/system">Manifest Configuration</Link>
                  </div>
                </Card>
                <Card surface="leather" className={styles.quickCard}>
                  <h2>Realm notes</h2>
                  <p>
                    Admin metrics here are sourced from persisted tables and live
                    data-directory inspection, so they reflect the current state
                    without relying on mock counters.
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
