import {
  Children,
  createElement,
  useEffect,
  useId,
  useState,
  type ReactNode,
} from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  getGameSystemManifest,
  listGameSystems,
  titleFor,
  type GameSystemSummary,
} from "@/api/gameSystems";
import {
  getWorldContentInventory,
  type ContentInventory,
} from "@/api/worldContent";
import {
  getWorld,
  updateWorldDefaultSceneGridType,
  updateWorldGameSystem,
} from "@/api/world";
import { SEO } from "@/components/seo/SEO";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Field } from "@/components/ui/field/Field";
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
import type { SystemManifest } from "@/types/systemManifest";
import { resolvePanel } from "@/panels/systemPanels";
import { useWorldRole } from "@/hooks/useWorldRole";
import { WorldSectionShell } from "@/layouts/world-layout/WorldSectionShell";
import { AuthoringToolGrantsCard } from "@/pages/world/settings/AuthoringToolGrantsCard";
import { CompendiumOverviewSettingsCard } from "@/pages/world/settings/CompendiumOverviewSettingsCard";
import { LoreRepositoryCard } from "@/pages/world/settings/LoreRepositoryCard";
import { PlayPauseHistoryCard } from "@/pages/world/settings/PlayPauseHistoryCard";
import { WorldAppearanceSettingsCard } from "@/pages/world/settings/WorldAppearanceSettingsCard";
import type { WorldRecord } from "@/types/world";
import { GRID_TYPE_OPTIONS, gridTypeLabel } from "@/utils/gridType";

/**
 * A named group of settings, with room around it.
 *
 * # Why this page needed one
 *
 * The owner's complaint was that the page "slams everything to the left" —
 * true, and `Container`'s `mx-0` was literally that — but the deeper problem
 * was that eight cards in a single column are eight things of equal weight
 * with nothing saying which belong together. A person looking for the grid
 * default had to read the page. Grouping is what the admin screens settled
 * on the same week (`005860d`, "one group at a time"), and this is the same
 * vocabulary: a heading, a sentence saying what the group is for, and the
 * cards beneath it.
 *
 * # Renders nothing when it holds nothing
 *
 * Most of what this page shows is gated by role — a player sees three of
 * these cards and a GM sees seven. A section whose every child was gated
 * away would otherwise leave a heading over empty space, which reads as
 * something failing to load. `Children.toArray` drops `null` and `false`,
 * so "no cards" is exactly "nothing to head".
 */
function SettingsSection({
  title,
  description,
  columns = 1,
  children,
}: {
  title: string;
  description?: string;
  /** Two columns from `lg` for groups of small, independent cards. */
  columns?: 1 | 2;
  children: ReactNode;
}) {
  const headingId = useId();
  if (Children.toArray(children).length === 0) {
    return null;
  }
  return (
    <section className="grid gap-4" aria-labelledby={headingId}>
      <div className="grid gap-1">
        <h2 id={headingId} className="text-lg font-semibold">
          {title}
        </h2>
        {description ? (
          <p className="max-w-prose text-sm text-muted-foreground">
            {description}
          </p>
        ) : null}
      </div>
      <div
        className={
          columns === 2
            ? "grid items-start gap-4 lg:grid-cols-2"
            : "grid items-start gap-4"
        }
      >
        {children}
      </div>
    </section>
  );
}

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
  const [activeManifest, setActiveManifest] = useState<SystemManifest | null>(
    null,
  );
  /** Offered by the deployment, read from `packs/systems/` (spec 032 T089). */
  const [systems, setSystems] = useState<GameSystemSummary[]>([]);
  const [pendingSystemId, setPendingSystemId] = useState<string | null>(null);
  /** What the pending change would affect. `null` until counted (FR-025). */
  const [inventory, setInventory] = useState<ContentInventory | null>(null);
  /**
   * The first of the two confirmations FR-027 requires.
   *
   * Deliberately separate from the legal-notice confirmation below, which
   * exists for spec 016 and means "I have read the licence". One control
   * meaning both that and "I accept this data consequence" would weaken both.
   */
  const [dataRiskAccepted, setDataRiskAccepted] = useState(false);
  const [pendingManifest, setPendingManifest] = useState<SystemManifest | null>(
    null,
  );
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const { isGm, role } = useWorldRole(worldId, world);

  // Reset during render rather than at the top of the effect below: this
  // is state derived from the arguments, and doing it in the effect commits
  // one render pairing the new key with the previous key's data.
  useResetOnChange(worldId, () => {
    setIsLoading(true);
  });

  useEffect(() => {
    let live = true;
    listGameSystems()
      .then((installed) => {
        if (live) setSystems(installed.systems);
      })
      .catch(() => {
        if (live) setSystems([]);
      });
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    let active = true;

    getWorld(worldId)
      .then((worldResult) => {
        if (!active) {
          return;
        }
        setWorld(worldResult);
        if (worldResult?.gameSystemId) {
          return getGameSystemManifest(worldResult.gameSystemId).then(
            (manifest) => {
              if (active) {
                setActiveManifest(manifest);
              }
            },
          );
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
    setInventory(null);
    setDataRiskAccepted(false);
    try {
      const manifest = await getGameSystemManifest(systemId);
      setPendingManifest(manifest);
      // What this change would affect, counted now and acknowledged by digest
      // (FR-025, ADR-065). An empty world takes the one-step path and never
      // sees the red panel at all (FR-029).
      setInventory(await getWorldContentInventory(worldId, systemId));
    } catch (err) {
      setStatus(
        err instanceof Error ? err.message : "Failed to load system manifest",
      );
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
      const updated = await updateWorldGameSystem(
        worldId,
        pendingSystemId,
        inventory && !inventory.isEmpty ? inventory.digest : undefined,
      );
      setWorld(updated);
      setActiveManifest(pendingManifest);
      const hidden = inventory && !inventory.isEmpty ? inventory : null;
      setPendingSystemId(null);
      setPendingManifest(null);
      setDataRiskAccepted(false);
      // FR-033: say what became hidden and how to get it back.
      setStatus(
        hidden
          ? `System assigned. Content authored for another system is hidden, not deleted — switching back restores it.`
          : "System assigned.",
      );
      setInventory(null);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to assign system");
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancelPick = () => {
    setPendingSystemId(null);
    setPendingManifest(null);
    setInventory(null);
    setDataRiskAccepted(false);
  };

  const [isSavingGridType, setIsSavingGridType] = useState(false);
  const handleChangeDefaultGridType = async (gridType: string) => {
    setIsSavingGridType(true);
    try {
      const updated = await updateWorldDefaultSceneGridType(worldId, gridType);
      setWorld(updated);
    } catch (err) {
      setStatus(
        err instanceof Error
          ? err.message
          : "Failed to update default scene grid type",
      );
    } finally {
      setIsSavingGridType(false);
    }
  };

  /**
   * What the picker shows when it is closed.
   *
   * The pending choice while one is being confirmed, and otherwise the system
   * the world actually runs — which is the whole of the owner's complaint:
   * "if you are Genie, it should show Genie on the select dropdown". `null`
   * (no system assigned yet) is the one case where the placeholder is the
   * truth.
   */
  const selectedSystemId = pendingSystemId ?? world?.gameSystemId ?? null;

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
      <SEO
        title={`${world.name} — System settings`}
        description="World system settings"
        noindex
      />
      <WorldSectionShell worldId={worldId} isGm={isGm}>
        {/* `mx-0` used to be here, and it was the owner's complaint made
            literal: every card pinned to the left edge with the rest of a
            wide monitor empty beside it. `Container` centres and insets by
            default; 64rem is wide enough for two cards side by side and
            narrow enough that a sentence in one of them is still readable. */}
        <Container
          className="grid w-full max-w-5xl gap-10 py-8 sm:py-10"
          data-testid="world-system-settings-page"
        >
          <div className="grid gap-4">
            <Button
              variant="ghost"
              size="sm"
              icon="arrow-left"
              className="justify-self-start"
              onClick={() => navigate(`/world/${worldId}/staging`)}
            >
              Back to Overview
            </Button>

            <div className="grid gap-1">
              <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
                System settings
              </p>
              <h1 className="text-2xl font-semibold sm:text-3xl">
                {world.name}
              </h1>
              <p className="max-w-prose text-muted-foreground">
                What this world runs on, how it looks at the table, and who may
                add to it.
              </p>
            </div>
          </div>

          {/* Page-level rather than inside one card: both the system change
              and the grid default report through it, and they now live in
              different sections. A message about a control you have scrolled
              past is a message nobody reads. */}
          {status ? (
            <StatusBadge
              variant={status === "System assigned." ? "success" : "danger"}
            >
              {status}
            </StatusBadge>
          ) : null}

          <SettingsSection
            title="Game system"
            description="The rules this world runs on, and the licence that comes with them. Changing it hides content authored for another system; it never deletes it."
          >
            <Card className="grid gap-4 p-6" data-testid="active-system-card">
              <h3 className="text-lg font-semibold">Active system</h3>
              {activeManifest ? (
                <div className="grid gap-3">
                  <p className="text-sm">
                    Currently using <strong>{activeManifest.title}</strong>.
                  </p>
                  <SystemLegalNotice
                    legal={activeManifest.legal}
                    variant="settings"
                  />
                </div>
              ) : (
                <p className="text-sm text-muted-foreground italic">
                  No system assigned yet.
                </p>
              )}
            </Card>

            {isGm ? (
              <Card className="grid gap-4 p-6" data-testid="system-picker-card">
                <h3 className="text-lg font-semibold">Change the system</h3>

                <Field label="Change System" htmlFor="system-picker">
                  <Select
                    /* The world's own system is the value when nothing is
                     * pending, so a Genie world reads "Genie" on arrival
                     * rather than the placeholder. It used to be
                     * `pendingSystemId ?? undefined` — undefined until you
                     * touched it — which left the trigger of an assigned
                     * world blank, telling somebody who had already chosen a
                     * system to select one. */
                    value={selectedSystemId ?? undefined}
                    /* No item here has an empty value, so "" is never a choice
                     * a person made — Radix emits one while the options are
                     * still arriving. Passing it through would ask the server
                     * for the manifest of a system with no id. */
                    onValueChange={(v) => {
                      if (v) void handlePickSystem(v);
                    }}
                    disabled={systems.length === 0}
                  >
                    <SelectTrigger
                      id="system-picker"
                      aria-label="Change System"
                      data-testid="system-picker"
                    >
                      {/* Rendered here rather than resolved by Radix from a
                          `SelectItem`, which is only mounted while the dropdown
                          is open. See `CreateWorldPage` for the failure. */}
                      <SelectValue placeholder="Select a system">
                        {selectedSystemId
                          ? titleFor(systems, selectedSystemId)
                          : null}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {systems.map((system) => (
                        <SelectItem key={system.id} value={system.id}>
                          {system.title}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>

                {/*
                 * FR-025 to FR-027: a world holding authored content gets a red
                 * panel with real counts, and two distinct confirmations. This
                 * is the first; the legal-notice confirmation below is the
                 * second, and it does not appear until this one is accepted.
                 *
                 * The wording is severe and *true*. Nothing is deleted here —
                 * content authored for another system becomes hidden and comes
                 * back if that system does — so the panel never says "delete",
                 * "lose" or "destroy" (FR-026). A false warning teaches a Game
                 * Master to distrust every warning this product shows them.
                 */}
                {pendingManifest &&
                inventory &&
                !inventory.isEmpty &&
                !dataRiskAccepted ? (
                  <div
                    className="grid gap-3 rounded-lg border-2 border-destructive bg-destructive/10 p-4"
                    role="alert"
                    data-testid="system-change-warning"
                  >
                    <p className="text-sm font-semibold text-destructive">
                      This world already holds content authored for another
                      system.
                    </p>
                    <ul
                      className="grid gap-1 text-sm"
                      data-testid="system-change-counts"
                    >
                      {inventory.counts.map((entry) => (
                        <li key={`${entry.kind}-${entry.systemId ?? "none"}`}>
                          <strong>{entry.count}</strong>{" "}
                          {entry.count === 1
                            ? entry.kind.replace(/ies$/, "y").replace(/s$/, "")
                            : entry.kind}
                          {entry.systemId
                            ? ` authored for ${titleFor(systems, entry.systemId)}`
                            : ""}
                        </li>
                      ))}
                      {inventory.becomingUnrecognised > 0 ? (
                        <li data-testid="system-change-unrecognised">
                          <strong>{inventory.becomingUnrecognised}</strong>{" "}
                          {inventory.becomingUnrecognised === 1
                            ? "ability"
                            : "abilities"}{" "}
                          of a type {titleFor(systems, pendingSystemId ?? "")}{" "}
                          does not recognise — still listed and editable,
                          grouped on their own, and returning to their own
                          section if you switch back.
                        </li>
                      ) : null}
                    </ul>
                    <p className="text-sm">
                      Switching to{" "}
                      <strong>
                        {titleFor(systems, pendingSystemId ?? "")}
                      </strong>{" "}
                      hides this content rather than destroying it. Nothing is
                      deleted, nothing is renamed, and switching back restores
                      all of it.
                    </p>
                    <div className="flex flex-wrap gap-3">
                      <Button
                        variant="danger"
                        onClick={() => setDataRiskAccepted(true)}
                        data-testid="system-change-accept-risk"
                      >
                        I understand — continue
                      </Button>
                      <Button
                        variant="ghost"
                        onClick={handleCancelPick}
                        data-testid="system-change-cancel"
                      >
                        Cancel
                      </Button>
                    </div>
                  </div>
                ) : null}

                {pendingManifest &&
                (dataRiskAccepted || (inventory?.isEmpty ?? true)) ? (
                  <div
                    className="grid gap-3"
                    data-testid="pending-system-confirmation"
                  >
                    <p className="text-sm text-muted-foreground">
                      Review the legal notice below, then confirm to assign{" "}
                      <strong>{pendingManifest.title}</strong> to this world.
                    </p>
                    <SystemLegalNotice
                      legal={pendingManifest.legal}
                      variant="selection"
                    />
                    <div className="flex flex-wrap gap-3">
                      <Button
                        onClick={() => void handleConfirm()}
                        disabled={isSaving}
                      >
                        {isSaving
                          ? "Assigning..."
                          : `Confirm — switch to ${pendingManifest.title}`}
                      </Button>
                      <Button
                        variant="ghost"
                        onClick={handleCancelPick}
                        disabled={isSaving}
                      >
                        Cancel
                      </Button>
                    </div>
                  </div>
                ) : null}
              </Card>
            ) : null}
          </SettingsSection>

          <SettingsSection
            title="At the table"
            description="How this world looks and behaves for everyone in it."
            columns={2}
          >
            {/* Spec 032 (FR-008, FR-010). Rendered for everyone rather than
                gated like the cards below it: a player seeing which look the
                table is using, read-only, is honest, whereas hiding it would
                make the setting look like it does not exist. The control
                itself is disabled, and `is_dm_of_world` refuses the write
                regardless. */}
            <WorldAppearanceSettingsCard
              worldId={worldId}
              interfacePackId={world?.interfacePackId ?? null}
              gameSystemId={world?.gameSystemId ?? null}
              isGm={isGm}
              onChanged={(interfacePackId) =>
                setWorld((current) =>
                  current ? { ...current, interfacePackId } : current,
                )
              }
            />

            {/* Its own card rather than a field at the bottom of the system
                picker, where it was: the grid a new scene starts with is a
                fact about scenes, not about which rules the world runs, and
                filing it under the system is part of why it was hard to
                find. */}
            {isGm ? (
              <Card
                className="grid gap-4 p-6"
                data-testid="default-scene-grid-card"
              >
                <h3 className="text-lg font-semibold">Scenes</h3>
                <Field
                  label="Default Scene Grid Type"
                  htmlFor="default-scene-grid-type"
                  hint="Applied to every newly created scene unless the GM picks a different grid type at creation time."
                >
                  <Select
                    value={world.defaultSceneGridType}
                    onValueChange={(v) => void handleChangeDefaultGridType(v)}
                    disabled={isSavingGridType}
                  >
                    <SelectTrigger
                      id="default-scene-grid-type"
                      aria-label="Default Scene Grid Type"
                      data-testid="default-scene-grid-type-picker"
                    >
                      {/* Spelled out for the same reason as the system picker:
                          the closed trigger must read the stored value. */}
                      <SelectValue placeholder="Select a grid type">
                        {gridTypeLabel(world.defaultSceneGridType)}
                      </SelectValue>
                    </SelectTrigger>
                    <SelectContent>
                      {GRID_TYPE_OPTIONS.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </Field>
              </Card>
            ) : null}

            {/* Spec 051 US5 (FR-050). For every member: that play was paused
                and when, never why. Renders nothing for a world never
                paused. */}
            <PlayPauseHistoryCard worldId={worldId} />
          </SettingsSection>

          <SettingsSection
            title="Content and contributors"
            description="What this world's book holds, where it is kept, and who may add to it."
          >
            {isGm ? <CompendiumOverviewSettingsCard worldId={worldId} /> : null}

            {/* Room is left here, deliberately empty, for the switch the owner
                also asked for: which compendium content a world has turned on.
                It is not built because the data does not exist — a world has
                no record of enabled content to read or write — and inventing
                one here would commit the compendium spec being written now to
                a shape chosen by a settings page. */}

            {/* Spec 031 (FR-046). GM-only chrome over a GM-only mutation: a
                player who reached this markup would still be refused by
                `is_dm_of_world` on the write. */}
            {isGm ? <AuthoringToolGrantsCard worldId={worldId} /> : null}

            {/* Spec 034 (T035). Owner-only rather than `isGm`, matching the
                authority every mutation behind it requires (FR-002): a GM shown
                a connect button the server would refuse is the same broken
                promise FR-036b forbids for an unconfigured instance. */}
            {role === "Owner" ? <LoreRepositoryCard worldId={worldId} /> : null}
          </SettingsSection>

          {/* Whatever the world's system contributes to its own settings.
              This was an inline card behind a comparison of
              `world.gameSystemId` against one system's id, with its own
              mutation in the shared world API — the third of the four
              violations `check-system-registry.mjs` listed against
              `032/T108`. `isGm` is passed rather than gating the mount: a
              panel may well have something to show a player, and only the
              panel knows which half of itself is the GM's. */}
          {(() => {
            // `createElement` with a lowercase local — see `ActorDetailPage`
            // for why the registry lookup is written out rather than
            // rendered as `<Panel />`.
            const panel = resolvePanel(world.gameSystemId, "world-settings");
            return panel
              ? createElement(panel, {
                  worldId,
                  world,
                  isGm,
                  onWorldChanged: () => {
                    void getWorld(worldId)
                      .then(setWorld)
                      .catch(() => {});
                  },
                })
              : null;
          })()}

          {!activeManifest && !isGm ? (
            <p className="text-sm text-muted-foreground">
              This world's GM hasn't assigned a system yet.{" "}
              <Link
                to={`/world/${worldId}/staging`}
                className="underline underline-offset-2"
              >
                Back to Overview
              </Link>
            </p>
          ) : null}
        </Container>
      </WorldSectionShell>
    </>
  );
}
