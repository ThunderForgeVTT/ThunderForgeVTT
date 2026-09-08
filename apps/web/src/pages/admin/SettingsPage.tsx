import { useEffect, useMemo, useState } from "react";
import {
  getAdminSettingsData,
  recalculateDiskUsage,
  updateManifestKey,
  updateOAuthProvider,
  updateTwoFactorPolicy,
} from "@/api/admin";
import { SEO } from "@/components/seo/SEO";
import { Card } from "@/components/ui/card/Card";
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
import {
  getInstanceAccessSettings,
  type InstanceAccessSettings,
} from "@/api/instanceAccess";
import { AccessPanel } from "./components/AccessPanel";
import { AdminSectionShell } from "./components/AdminSectionShell";
import { DiskUsageChart } from "./components/DiskUsageChart";
import { GitHubAppsPanel } from "./components/GitHubAppsPanel";
import { InstanceSettingsPanel } from "./components/InstanceSettingsPanel";
import { LegalEnquiriesPanel } from "./components/LegalEnquiriesPanel";
import { MailPanel } from "./components/MailPanel";
import { ManifestEditor } from "./components/ManifestEditor";
import { MetricsCard } from "./components/MetricsCard";
import { OAuthProviderForm } from "./components/OAuthProviderForm";
import { ReadinessPanel } from "./components/ReadinessPanel";
import { SecurityPanel } from "./components/SecurityPanel";

type AdminSettingsSection =
  | "overview"
  | "configuration"
  | "storage"
  | "security"
  | "access"
  | "instance"
  | "readiness"
  | "mail"
  | "legal";

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
    case "access":
      return "Access";
    case "instance":
      return "Instance";
    case "readiness":
      return "Readiness";
    case "mail":
      return "Mail";
    case "legal":
      return "Legal";
    default:
      return "Overview";
  }
}

export default function SettingsPage({
  initialSection = "overview",
}: SettingsPageProps) {
  const [data, setData] = useState<AdminSettingsData | null>(null);
  const [pageStatus, setPageStatus] = useState<string | null>(null);
  // Spec 035: loaded only for the access section, so every other admin page
  // does not pay for a query it never renders.
  const [accessSettings, setAccessSettings] =
    useState<InstanceAccessSettings | null>(null);

  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshingDisk, setIsRefreshingDisk] = useState(false);
  const section = initialSection;

  useEffect(() => {
    if (section !== "access" || accessSettings) {
      return;
    }
    let active = true;
    void getInstanceAccessSettings()
      .then((value) => {
        if (active) {
          setAccessSettings(value);
        }
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [section, accessSettings]);

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
          setPageStatus(
            error instanceof Error
              ? error.message
              : "Failed to load admin settings.",
          );
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
              subtitle: "Total accounts recorded in persisted auth state.",
              icon: "actors" as const,
              emphasis: "gold" as const,
            },
            {
              title: "Worlds",
              value: data.adminStats.totalWorlds.toLocaleString(),
              subtitle: "Durable worlds currently tracked in PostgreSQL.",
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
              subtitle: "Permission rules currently stored in policy tables.",
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

  const updateManifest = async (
    key: string,
    value: string,
  ): Promise<SystemManifest> => {
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
      setPageStatus(
        error instanceof Error
          ? error.message
          : "Failed to recalculate disk usage.",
      );
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
    // The shell stays even when the payload didn't arrive: a failed load
    // is exactly when an administrator most needs a way off this page.
    return (
      <AdminSectionShell>
        <main className="py-8">
          <StatusBadge variant="danger">
            {pageStatus ?? "Admin settings are unavailable."}
          </StatusBadge>
        </main>
      </AdminSectionShell>
    );
  }

  return (
    <>
      <SEO {...settingsPageSeo} />
      <AdminSectionShell>
        <main className="grid gap-6 pb-12">
          <section className="grid grid-cols-1 gap-4 rounded-xl border border-border bg-card p-6 lg:grid-cols-[minmax(0,1.7fr)_minmax(13rem,0.7fr)]">
            <div className="grid gap-2.5">
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                Admin control center
              </p>
              <h1 className="text-2xl font-semibold">
                Oversee analytics, configuration, and system administration.
              </h1>
              <p className="max-w-[62ch] text-muted-foreground">
                This page binds together persisted metrics, OAuth provider
                state, manifest metadata, disk usage, and security policy in a
                single command surface.
              </p>
            </div>
            <div className="grid content-start gap-1 rounded-lg border border-border bg-secondary p-4">
              <span className="text-xs tracking-widest text-muted-foreground uppercase">
                Active section
              </span>
              <strong className="text-xl font-semibold text-primary">
                {sectionLabel(section)}
              </strong>
              <small className="text-xs tracking-widest text-muted-foreground uppercase">
                Live persisted state
              </small>
            </div>
          </section>

          {/* Spec 031 FR-032: one section at a time. Stacking all four and
              letting the links scroll was what the playtest was looking at
              when it reported no navigation — every link led to the same
              screen, so nothing appeared to happen. */}
          <div className="grid gap-5">
            {section === "overview" ? (
              <section className="grid gap-3" id="overview">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="scene" size={16} />
                      Overview
                    </p>
                    <h2 className="text-xl font-semibold">System analytics</h2>
                  </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  {metricItems.map((item) => (
                    <MetricsCard key={item.title} {...item} />
                  ))}
                </div>
              </section>
            ) : null}

            {section === "configuration" ? (
              <section className="grid gap-3" id="configuration">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="wand" size={16} />
                      Configuration
                    </p>
                    <h2 className="text-xl font-semibold">
                      OAuth providers and manifest
                    </h2>
                  </div>
                </div>
                <div className="grid gap-4">
                  <Card surface="parchment" className="grid gap-4 p-6">
                    <div className="grid gap-1">
                      <h3 className="text-lg font-semibold">OAuth providers</h3>
                      <p className="text-muted-foreground">
                        Edit stored provider credentials and availability.
                      </p>
                    </div>
                    <div className="grid gap-4">
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

                  {/* Spec 040 US5 / ADR-090: one GitHub application for
                      everything, or one per subsystem, and which acts for
                      what. Reads its own data rather than joining
                      `getAdminSettingsData` — it is the only surface that
                      needs the resolution, and folding it into the page's
                      single fetch would make every other admin screen carry
                      it. */}
                  <Card surface="parchment" className="grid gap-4 p-6">
                    <div className="grid gap-1">
                      <h3 className="text-lg font-semibold">
                        GitHub applications
                      </h3>
                      <p className="text-muted-foreground">
                        Which application each subsystem acts as, and where each
                        value came from.
                      </p>
                    </div>
                    <GitHubAppsPanel />
                  </Card>

                  <Card surface="parchment" className="grid gap-4 p-6">
                    <div className="grid gap-1">
                      <h3 className="text-lg font-semibold">Manifest viewer</h3>
                      <p className="text-muted-foreground">
                        Adjust editable MVP keys stored in the system manifest.
                      </p>
                    </div>
                    <ManifestEditor
                      manifest={data.systemManifest}
                      onSaveKey={updateManifest}
                    />
                  </Card>
                </div>
              </section>
            ) : null}

            {/* Spec 040: the three panels below read their own data rather
                than joining `getAdminSettingsData`, for the reason
                `GitHubAppsPanel` gives — folding a resolution only these
                screens need into the page's single fetch would make every
                other admin screen pay for it. */}
            {section === "instance" ? (
              <section className="grid gap-3" id="instance">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="settings" size={16} />
                      Instance
                    </p>
                    <h2 className="text-xl font-semibold">
                      Settings, their source, and their history
                    </h2>
                  </div>
                </div>
                <Card surface="parchment" className="grid gap-4 p-6">
                  <InstanceSettingsPanel />
                </Card>
              </section>
            ) : null}

            {section === "readiness" ? (
              <section className="grid gap-3" id="readiness">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="compass" size={16} />
                      Readiness
                    </p>
                    <h2 className="text-xl font-semibold">
                      What this instance can and cannot do
                    </h2>
                  </div>
                </div>
                <Card surface="stone" className="grid gap-4 p-6">
                  <ReadinessPanel />
                </Card>
              </section>
            ) : null}

            {section === "mail" ? (
              <section className="grid gap-3" id="mail">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="rune" size={16} />
                      Mail
                    </p>
                    <h2 className="text-xl font-semibold">
                      Delivery, proof of delivery, and the outbox
                    </h2>
                  </div>
                </div>
                <Card surface="stone" className="grid gap-4 p-6">
                  <MailPanel />
                </Card>
              </section>
            ) : null}

            {section === "legal" ? (
              <section className="grid gap-3" id="legal">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="quill" size={16} />
                      Legal
                    </p>
                    <h2 className="text-xl font-semibold">
                      Terms disputes and privacy requests
                    </h2>
                  </div>
                </div>
                <Card surface="parchment" className="grid gap-4 p-6">
                  <LegalEnquiriesPanel />
                </Card>
              </section>
            ) : null}

            {section === "storage" ? (
              <section className="grid gap-3" id="storage">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="inventory" size={16} />
                      Storage
                    </p>
                    <h2 className="text-xl font-semibold">
                      Disk usage visualization
                    </h2>
                  </div>
                </div>
                <Card surface="stone" className="grid gap-4 p-6">
                  <DiskUsageChart
                    usage={data.adminStats.diskUsage}
                    onRefresh={refreshDiskUsage}
                    isRefreshing={isRefreshingDisk}
                  />
                </Card>
              </section>
            ) : null}

            {section === "security" ? (
              <section className="grid gap-3" id="security">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="shield" size={16} />
                      Security
                    </p>
                    <h2 className="text-xl font-semibold">
                      2FA and bootstrap governance
                    </h2>
                  </div>
                </div>
                <Card surface="stone" className="grid gap-4 p-6">
                  <SecurityPanel
                    settings={data.authSecuritySettings}
                    bootstrapSettings={
                      data.adminBootstrapSettings as AdminBootstrapSettings | null
                    }
                    onUpdate={updateSecurity}
                  />
                </Card>
              </section>
            ) : null}

            {section === "access" ? (
              <section className="grid gap-3" id="access">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="inline-flex items-center gap-2 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                      <FantasyIcon name="shield" size={16} />
                      Access
                    </p>
                    <h2 className="text-xl font-semibold">
                      Instance admission and invitations
                    </h2>
                  </div>
                </div>
                {accessSettings ? (
                  <AccessPanel
                    settings={accessSettings}
                    onPolicyChange={setAccessSettings}
                  />
                ) : (
                  <Card surface="stone" className="p-6">
                    <p className="text-sm text-muted-foreground">
                      Loading access settings...
                    </p>
                  </Card>
                )}
              </section>
            ) : null}

            {pageStatus ? (
              <StatusBadge variant="info">{pageStatus}</StatusBadge>
            ) : null}
          </div>
        </main>
      </AdminSectionShell>
    </>
  );
}
