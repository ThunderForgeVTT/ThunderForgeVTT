import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { BUNDLED_SYSTEM_IDS, getGameSystemManifest } from "@/api/gameSystems";
import { getWorld, updateWorldGameSystem, updateWorldGenieResourceCarryover } from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Loader } from "@/components/ui/loader/Loader";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { SystemLegalNotice } from "@/components/game-systems/legal/SystemLegalNotice";
import type { SystemManifest } from "@/contexts/GameSystemContext";
import { useWorldRole } from "@/hooks/useWorldRole";
import type { WorldRecord } from "@/types/world";

/**
 * Spec 016 (T010, FR-004/FR-005): the world's persistent System Settings
 * view. Per tasks.md's scope-correction note, this single surface serves
 * both required call sites — it's where a GM assigns/changes the world's
 * system (FR-004's "point of selection") AND where anyone can find the
 * active system's legal notice at any later time (FR-005's "persistent,
 * easily discoverable location") — because no other system-selection UI
 * exists anywhere in the app today (spec 008 deliberately removed it from
 * world creation).
 */
export default function WorldSystemSettingsPage() {
  const { id: worldId = "" } = useParams();
  const navigate = useNavigate();
  const [world, setWorld] = useState<WorldRecord | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [activeManifest, setActiveManifest] = useState<SystemManifest | null>(null);
  const [pendingSystemId, setPendingSystemId] = useState<string | null>(null);
  const [pendingManifest, setPendingManifest] = useState<SystemManifest | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const { isGm } = useWorldRole(worldId, world);

  useEffect(() => {
    let active = true;
    setIsLoading(true);

    getWorld(worldId)
      .then((worldResult) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        if (worldResult?.gameSystemId) {
          return getGameSystemManifest(worldResult.gameSystemId).then((manifest) => {
            if (active) {
              setActiveManifest(manifest);
            }
          });
        }
        return undefined;
      })
      .finally(() => {
        if (active) {
          setIsLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, [worldId]);

  const handlePickSystem = async (systemId: string) => {
    setStatus(null);
    setPendingSystemId(systemId);
    try {
      const manifest = await getGameSystemManifest(systemId);
      setPendingManifest(manifest);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to load system manifest");
      setPendingSystemId(null);
    }
  };

  const handleConfirm = async () => {
    if (!pendingSystemId || !pendingManifest) {
      return;
    }
    setIsSaving(true);
    setStatus(null);
    try {
      const updated = await updateWorldGameSystem(worldId, pendingSystemId);
      setWorld(updated);
      setActiveManifest(pendingManifest);
      setPendingSystemId(null);
      setPendingManifest(null);
      setStatus("System assigned.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to assign system");
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancelPick = () => {
    setPendingSystemId(null);
    setPendingManifest(null);
  };

  const [isSavingCarryover, setIsSavingCarryover] = useState(false);
  const handleToggleCarryover = async (enabled: boolean) => {
    setIsSavingCarryover(true);
    try {
      const updated = await updateWorldGenieResourceCarryover(worldId, enabled);
      setWorld(updated);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to update resource carryover setting");
    } finally {
      setIsSavingCarryover(false);
    }
  };

  if (isLoading) {
    return <Loader fullScreen label="Loading system settings" />;
  }

  if (!world) {
    return (
      <Container>
        <main className="grid min-h-[60vh] place-items-center py-16">
          <Card className="grid gap-3 p-6 text-center">
            <h1 className="text-xl font-semibold">World not found</h1>
            <p className="text-muted-foreground">
              This world doesn't exist or you don't have access to it.
            </p>
          </Card>
        </main>
      </Container>
    );
  }

  return (
    <>
      <SEO title={`${world.name} — System settings`} description="World system settings" noindex />
      <Container className="grid max-w-2xl gap-6 py-10">
        <Button
          variant="ghost"
          size="sm"
          icon="arrow-left"
          className="justify-self-start"
          onClick={() => navigate(`/world/${worldId}/staging`)}
        >
          Back to Session Setup
        </Button>

        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            System settings
          </p>
          <h1 className="text-2xl font-semibold">{world.name}</h1>
        </div>

        <Card className="grid gap-4 p-6" data-testid="active-system-card">
          <h2 className="text-lg font-semibold">Active system</h2>
          {activeManifest ? (
            <div className="grid gap-3">
              <p className="text-sm">
                Currently using <strong>{activeManifest.title}</strong>.
              </p>
              <SystemLegalNotice legal={activeManifest.legal} variant="settings" />
            </div>
          ) : (
            <p className="text-sm text-muted-foreground italic">No system assigned yet.</p>
          )}
        </Card>

        {isGm ? (
          <Card className="grid gap-4 p-6" data-testid="system-picker-card">
            <h2 className="text-lg font-semibold">
              {activeManifest ? "Change system" : "Assign a system"}
            </h2>
            <Select value={pendingSystemId ?? undefined} onValueChange={(v) => void handlePickSystem(v)}>
              <SelectTrigger aria-label="System" data-testid="system-picker">
                <SelectValue placeholder="Select a system" />
              </SelectTrigger>
              <SelectContent>
                {BUNDLED_SYSTEM_IDS.map((systemId) => (
                  <SelectItem key={systemId} value={systemId}>
                    {systemId}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            {pendingManifest ? (
              <div className="grid gap-3" data-testid="pending-system-confirmation">
                <p className="text-sm text-muted-foreground">
                  Review the legal notice below, then confirm to assign{" "}
                  <strong>{pendingManifest.title}</strong> to this world.
                </p>
                <SystemLegalNotice legal={pendingManifest.legal} variant="selection" />
                <div className="flex gap-3">
                  <Button onClick={() => void handleConfirm()} disabled={isSaving}>
                    {isSaving ? "Assigning..." : "Confirm"}
                  </Button>
                  <Button variant="ghost" onClick={handleCancelPick} disabled={isSaving}>
                    Cancel
                  </Button>
                </div>
              </div>
            ) : null}

            {status ? (
              <StatusBadge variant={status === "System assigned." ? "success" : "danger"}>
                {status}
              </StatusBadge>
            ) : null}
          </Card>
        ) : null}

        {isGm && world.gameSystemId === "genie" ? (
          <Card className="grid gap-3 p-6" data-testid="genie-resource-carryover-card">
            <h2 className="text-lg font-semibold">Session Resource carryover</h2>
            <p className="text-sm text-muted-foreground">
              When enabled, players' Insight/Favor/Essence holdings carry over into the next Genie
              session instead of resetting to 0 — "the rope doesn't disappear."
            </p>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={world.genieResourceCarryoverEnabled}
                disabled={isSavingCarryover}
                onChange={(event) => void handleToggleCarryover(event.target.checked)}
                data-testid="genie-resource-carryover-toggle"
              />
              Carry over Session Resource holdings between sessions
            </label>
          </Card>
        ) : null}

        {!activeManifest && !isGm ? (
          <p className="text-sm text-muted-foreground">
            This world's GM hasn't assigned a system yet.{" "}
            <Link to={`/world/${worldId}/staging`} className="underline underline-offset-2">
              Back to Session Setup
            </Link>
          </p>
        ) : null}
      </Container>
    </>
  );
}
