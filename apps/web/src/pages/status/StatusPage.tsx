import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import type { FantasyIconName } from "@/components/ui/fantasy-icon/FantasyIcon";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { cn } from "@/lib/utils";
import type { SeoConfig } from "@/types/seo";

export const statusPageSeo: SeoConfig = {
  title: "System Status",
  description: "Live status of ThunderForge VTT's core services.",
  canonicalPath: "/status",
  noindex: true,
};

const REFRESH_INTERVAL_MS = 15000;
const STATUS_ENDPOINT = "/api/status";

interface ServiceStatus {
  key: string;
  up: boolean;
  latency_ms: number;
}

interface StatusResponse {
  services: ServiceStatus[];
}

interface FantasyService {
  key: string;
  name: string;
  description: string;
  icon: FantasyIconName;
}

const FANTASY_SERVICES: readonly FantasyService[] = [
  {
    key: "core",
    name: "The Forge Core",
    description: "The central engine that answers every request.",
    icon: "torch",
  },
  {
    key: "database",
    name: "The Archive Vault",
    description: "Where every world, character, and scene is recorded.",
    icon: "tokens",
  },
  {
    key: "storage",
    name: "The Reliquary",
    description: "Safekeeping for maps, tokens, and pasted images.",
    icon: "inventory",
  },
];

type FetchState = "checking" | "ok" | "unreachable";

/** Reachability check, split out of the hook so it writes no state: the
 * effect below can then start the first poll without a setState of its own
 * (react-hooks/set-state-in-effect). */
async function fetchStatus(): Promise<{
  services: ServiceStatus[] | null;
  fetchState: FetchState;
}> {
  try {
    const response = await fetch(STATUS_ENDPOINT, {
      credentials: "omit",
      cache: "no-store",
    });

    if (!response.ok) {
      throw new Error(`Status endpoint returned ${response.status}`);
    }

    const payload = (await response.json()) as StatusResponse;
    return { services: payload.services, fetchState: "ok" };
  } catch {
    return { services: null, fetchState: "unreachable" };
  }
}

function useStatusPoll() {
  const [services, setServices] = useState<ServiceStatus[] | null>(null);
  const [fetchState, setFetchState] = useState<FetchState>("checking");
  const [lastChecked, setLastChecked] = useState<Date | null>(null);

  const applyStatus = useCallback(
    (result: { services: ServiceStatus[] | null; fetchState: FetchState }) => {
      setServices(result.services);
      setFetchState(result.fetchState);
      setLastChecked(new Date());
    },
    [],
  );

  const check = useCallback(async () => {
    applyStatus(await fetchStatus());
  }, [applyStatus]);

  useEffect(() => {
    let active = true;
    void fetchStatus().then((result) => {
      if (active) {
        applyStatus(result);
      }
    });
    const interval = window.setInterval(
      () => void check(),
      REFRESH_INTERVAL_MS,
    );
    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, [check, applyStatus]);

  return { services, fetchState, lastChecked, recheck: check };
}

export default function StatusPage() {
  const { services, fetchState, lastChecked, recheck } = useStatusPoll();

  const rows = FANTASY_SERVICES.map((fantasy) => {
    const live = services?.find((service) => service.key === fantasy.key);
    const up = fetchState === "ok" && Boolean(live?.up);
    return { ...fantasy, up, latencyMs: live?.latency_ms };
  });

  const allUp = fetchState === "ok" && rows.every((row) => row.up);
  const anyUp = fetchState === "ok" && rows.some((row) => row.up);

  const overall =
    fetchState === "checking"
      ? { label: "Checking the wards...", variant: "info" as const }
      : fetchState === "unreachable"
        ? {
            label: "The Forge Core cannot be reached",
            variant: "danger" as const,
          }
        : allUp
          ? { label: "All systems operational", variant: "success" as const }
          : anyUp
            ? {
                label: "Degraded — some services are down",
                variant: "warning" as const,
              }
            : { label: "Major outage", variant: "danger" as const };

  return (
    <>
      <SEO {...statusPageSeo} />
      <main className="min-h-full py-12 pb-16">
        <Container className="grid max-w-2xl gap-6">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="grid gap-2">
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                System status
              </p>
              <h1 className="text-3xl leading-none font-semibold text-balance sm:text-4xl">
                ThunderForge instance status
              </h1>
            </div>
            <Button variant="secondary" icon="worlds" asChild>
              <Link to="/">Home</Link>
            </Button>
          </div>

          <Card className="grid gap-3 p-6">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <StatusBadge variant={overall.variant}>
                {overall.label}
              </StatusBadge>
              <Button
                variant="ghost"
                size="sm"
                icon="spark"
                onClick={() => void recheck()}
              >
                Recheck
              </Button>
            </div>
            {lastChecked ? (
              <p className="text-xs text-muted-foreground">
                Last checked {lastChecked.toLocaleTimeString()} · refreshes
                every {REFRESH_INTERVAL_MS / 1000}s
              </p>
            ) : null}
          </Card>

          <RuneDivider label="Services" />

          <div className="grid gap-3">
            {rows.map((row) => (
              <Card key={row.key} className="flex items-center gap-4 p-4">
                <span
                  className={cn(
                    "inline-grid size-10 shrink-0 place-items-center rounded-full border border-border",
                    row.up ? "bg-emerald-600/10" : "bg-destructive/10",
                  )}
                >
                  <FantasyIcon
                    name={row.icon}
                    size={18}
                    tone={row.up ? "default" : "ember"}
                  />
                </span>

                <div className="grid min-w-0 flex-1 gap-0.5">
                  <strong className="text-sm">{row.name}</strong>
                  <p className="truncate text-sm text-muted-foreground">
                    {row.description}
                  </p>
                </div>

                <div className="grid shrink-0 justify-items-end gap-1">
                  <span
                    className={cn(
                      "inline-flex items-center gap-1.5 text-sm font-medium",
                      row.up ? "text-emerald-600" : "text-destructive",
                    )}
                  >
                    <span
                      className={cn(
                        "size-2 rounded-full",
                        row.up ? "bg-emerald-500" : "bg-destructive",
                        fetchState === "checking" && "animate-pulse",
                      )}
                    />
                    {fetchState === "checking"
                      ? "Checking"
                      : row.up
                        ? "Operational"
                        : "Down"}
                  </span>
                  {row.up && typeof row.latencyMs === "number" ? (
                    <span className="text-xs text-muted-foreground">
                      {row.latencyMs}ms
                    </span>
                  ) : null}
                </div>
              </Card>
            ))}
          </div>
        </Container>
      </main>
    </>
  );
}
