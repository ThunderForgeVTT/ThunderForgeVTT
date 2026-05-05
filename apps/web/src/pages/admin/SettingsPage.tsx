import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  getAdminSettingsData,
  recalculateDiskUsage,
  updateManifestKey,
  updateOAuthProvider,
  updateTwoFactorPolicy,
} from "@/api/admin";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Loader } from "@/components/ui/loader/Loader";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import type {
  AdminBootstrapSettings,
  AdminSettingsData,
  AuthSecuritySettings,
  OAuthProviderConfig,
  SystemManifest,
  UpdateOAuthProviderInput,
} from "@/types/admin";
import type { SeoConfig } from "@/types/seo";
import { DiskUsageChart } from "./components/DiskUsageChart";
import { ManifestEditor } from "./components/ManifestEditor";
import { MetricsCard } from "./components/MetricsCard";
import { OAuthProviderForm } from "./components/OAuthProviderForm";
import { SecurityPanel } from "./components/SecurityPanel";
import styles from "./SettingsPage.module.scss";

type AdminSettingsSection = "overview" | "configuration" | "storage" | "security";

interface SettingsPageProps {
  initialSection?: AdminSettingsSection;
}

export const settingsPageSeo: SeoConfig = {
  title: "Admin command center",
  description:
    "Review ThunderForge system analytics, storage posture, OAuth provider configuration, and manifest metadata from the admin control center.",
  canonicalPath: "/admin",
  noindex: true,
};

function sectionLabel(section: AdminSettingsSection) {
  switch (section) {
    case "configuration":
      return "Configuration";
    case "storage":
      return "Storage";
    case "security":
      return "Security";
    default:
      return "Overview";
  }
}

export default function SettingsPage({
  initialSection = "overview",
}: SettingsPageProps) {
  const [data, setData] = useState<AdminSettingsData | null>(null);
  const [pageStatus, setPageStatus] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshingDisk, setIsRefreshingDisk] = useState(false);
  const section = initialSection;

  useEffect(() => {
    let active = true;

    void getAdminSettingsData()
      .then((response) => {
        if (active) {
          setData(response);
        }
      })
      .catch((error) => {
        if (active) {
          setPageStatus(error instanceof Error ? error.message : "Failed to load admin settings.");
        }
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  const metricItems = useMemo(
    () =>
      data
        ? [
            {
              title: "Users",
              value: data.adminStats.totalUsers.toLocaleString(),
              subtitle: "Total stewards recorded in persisted auth state.",
              icon: "actors" as const,
              emphasis: "gold" as const,
            },
            {
              title: "Worlds",
              value: data.adminStats.totalWorlds.toLocaleString(),
              subtitle: "Durable realms currently tracked in PostgreSQL.",
              icon: "worlds" as const,
              emphasis: "violet" as const,
            },
            {
              title: "Tokens",
              value: data.adminStats.totalWorldTokens.toLocaleString(),
              subtitle: "Scene entities persisted for active worlds.",
              icon: "tokens" as const,
              emphasis: "forest" as const,
            },
            {
              title: "Events",
              value: data.adminStats.totalWorldEvents.toLocaleString(),
              subtitle: "Durable world event records flowing through sync.",
              icon: "spark" as const,
              emphasis: "gold" as const,
            },
            {
              title: "Policies",
              value: data.adminStats.totalPolicies.toLocaleString(),
              subtitle: "Permission wards currently stored in policy tables.",
              icon: "shield" as const,
              emphasis: "violet" as const,
            },
            {
              title: "Storage",
              value: `${(data.adminStats.diskUsageBytes / (1024 * 1024)).toFixed(1)} MB`,
              subtitle: "Approximate footprint under the configured data root.",
              icon: "inventory" as const,
              emphasis: "forest" as const,
            },
          ]
        : [],
    [data],
  );

  const sectionLinks: Array<{
    section: AdminSettingsSection;
    path: string;
    description: string;
  }> = [
    {
      section: "overview",
      path: "/admin",
      description: "Realm analytics and live system counts",
    },
    {
      section: "configuration",
      path: "/admin/configuration",
      description: "OAuth envoys and manifest editing",
    },
    {
      section: "storage",
      path: "/admin/storage",
      description: "Disk posture and persisted footprint",
    },
    {
      section: "security",
      path: "/admin/security",
      description: "2FA enforcement and bootstrap record",
    },
  ];

  const updateProvider = async (
    providerId: string,
    config: UpdateOAuthProviderInput,
  ): Promise<OAuthProviderConfig> => {
    const updated = await updateOAuthProvider(providerId, config);
    setData((current) =>
      current
        ? {
            ...current,
            oauthProviders: current.oauthProviders.map((provider) =>
              provider.id === providerId ? updated : provider,
            ),
          }
        : current,
    );
    return updated;
  };

  const updateManifest = async (key: string, value: string): Promise<SystemManifest> => {
    const updated = await updateManifestKey(key, value);
    setData((current) =>
      current
        ? {
            ...current,
            systemManifest: updated,
          }
        : current,
    );
    return updated;
  };

  const refreshDiskUsage = async () => {
    setIsRefreshingDisk(true);
    setPageStatus(null);

    try {
      const updated = await recalculateDiskUsage();
      setData((current) =>
        current
          ? {
              ...current,
              adminStats: updated,
            }
          : current,
      );
      setPageStatus("Disk usage recalculated from the current data root.");
    } catch (error) {
      setPageStatus(error instanceof Error ? error.message : "Failed to recalculate disk usage.");
    } finally {
      setIsRefreshingDisk(false);
    }
  };

  const updateSecurity = async (
    requiredForAllUsers: boolean,
  ): Promise<AuthSecuritySettings> => {
    const updated = await updateTwoFactorPolicy(requiredForAllUsers);
    setData((current) =>
      current
        ? {
            ...current,
            authSecuritySettings: updated,
          }
        : current,
    );
    return updated;
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading admin settings" />;
  }

  if (!data) {
    return (
      <Container>
        <main className={styles.errorShell}>
          <StatusBadge variant="danger">
            {pageStatus ?? "Admin settings are unavailable."}
          </StatusBadge>
        </main>
      </Container>
    );
  }

  return (
    <>
      <SEO {...settingsPageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <div className={styles.heroCopy}>
              <p className={styles.eyebrow}>Admin control center</p>
              <h1>Oversee analytics, configuration, and realm stewardship.</h1>
              <p>
                This page binds together persisted metrics, OAuth provider state,
                manifest metadata, disk usage, and security policy inside a
                single fantasy command surface.
              </p>
            </div>
            <div className={styles.heroBadge}>
              <span>Active chapter</span>
              <strong>{sectionLabel(section)}</strong>
              <small>Live persisted state</small>
            </div>
          </section>

          <div className={styles.layout}>
            <aside className={styles.sidebar}>
              <Card surface="leather" className={styles.sidebarCard}>
                <div className={styles.sidebarHeader}>
                  <p className={styles.sidebarEyebrow}>Admin navigation</p>
                  <h2>Command chapters</h2>
                </div>
                <nav className={styles.sidebarNav}>
                  {sectionLinks.map((item) => (
                    <Link
                        key={item.section}
                        to={item.path}
                        className={styles.sidebarLink}
                        data-active={section === item.section}
                      >
                      <span>{sectionLabel(item.section)}</span>
                      <small>{item.description}</small>
                    </Link>
                  ))}
                </nav>
              </Card>
            </aside>

            <div className={styles.content}>
              <section className={styles.section} id="overview">
                <div className={styles.sectionHeader}>
                  <div>
                    <p className={styles.sectionKicker}>
                      <FantasyIcon name="scene" size={16} />
                      Overview
                    </p>
                      <h2>Realm analytics</h2>
                  </div>
                  {section !== "overview" ? (
                    <Button
                      asChild
                      variant="secondary"
                      size="sm"
                      icon="crown"
                    >
                      <Link to="/admin">Return to admin overview</Link>
                    </Button>
                  ) : null}
                </div>
                <div className={styles.metricsGrid}>
                  {metricItems.map((item) => (
                    <MetricsCard key={item.title} {...item} />
                  ))}
                </div>
              </section>

              <section className={styles.section} id="configuration">
                <div className={styles.sectionHeader}>
                  <div>
                    <p className={styles.sectionKicker}>
                      <FantasyIcon name="wand" size={16} />
                      Configuration
                    </p>
                    <h2>OAuth providers and manifest</h2>
                  </div>
                </div>
                <div className={styles.stack}>
                  <Card surface="parchment" className={styles.panel}>
                    <div className={styles.panelHeader}>
                      <h3>OAuth envoys</h3>
                      <p>Edit stored provider credentials and availability.</p>
                    </div>
                    <div className={styles.stack}>
                      {data.oauthProviders.length ? (
                        data.oauthProviders.map((provider) => (
                          <OAuthProviderForm
                            key={provider.id}
                            provider={provider}
                            onSave={updateProvider}
                          />
                        ))
                      ) : (
                        <StatusBadge variant="warning">
                          No persisted OAuth providers are currently configured.
                        </StatusBadge>
                      )}
                    </div>
                  </Card>

                  <Card surface="parchment" className={styles.panel}>
                    <div className={styles.panelHeader}>
                      <h3>Manifest viewer</h3>
                      <p>Adjust editable MVP keys stored in the system manifest.</p>
                    </div>
                    <ManifestEditor
                      manifest={data.systemManifest}
                      onSaveKey={updateManifest}
                    />
                  </Card>
                </div>
              </section>

              <section className={styles.section} id="storage">
                <div className={styles.sectionHeader}>
                  <div>
                    <p className={styles.sectionKicker}>
                      <FantasyIcon name="inventory" size={16} />
                      Storage
                    </p>
                    <h2>Disk usage visualization</h2>
                  </div>
                </div>
                <Card surface="stone" className={styles.panel}>
                  <DiskUsageChart
                    usage={data.adminStats.diskUsage}
                    onRefresh={refreshDiskUsage}
                    isRefreshing={isRefreshingDisk}
                  />
                </Card>
              </section>

              <section className={styles.section} id="security">
                <div className={styles.sectionHeader}>
                  <div>
                    <p className={styles.sectionKicker}>
                      <FantasyIcon name="shield" size={16} />
                      Security
                    </p>
                    <h2>2FA and bootstrap governance</h2>
                  </div>
                </div>
                <Card surface="stone" className={styles.panel}>
                  <SecurityPanel
                    settings={data.authSecuritySettings}
                    bootstrapSettings={data.adminBootstrapSettings as AdminBootstrapSettings | null}
                    onUpdate={updateSecurity}
                  />
                </Card>
              </section>

              {pageStatus ? <StatusBadge variant="info">{pageStatus}</StatusBadge> : null}
            </div>
          </div>
        </main>
      </Container>
    </>
  );
}
