import { useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { deleteUserData, exportUserData } from "@/api/auth";
import { SEO } from "@/components/seo/SEO";
import { Avatar } from "@/components/ui/avatar/Avatar";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { Dialog } from "@/components/ui/dialog/Dialog";
import { Dropdown } from "@/components/ui/dropdown/Dropdown";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Grid } from "@/components/ui/grid/Grid";
import { Panel } from "@/components/ui/panel/Panel";
import { Popover } from "@/components/ui/popover/Popover";
import { RuneDivider } from "@/components/ui/rune-divider/RuneDivider";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { TokenAvatar } from "@/components/ui/token-avatar/TokenAvatar";
import { Tooltip } from "@/components/ui/tooltip/Tooltip";
import { useAuth } from "@/hooks/useAuth";
import { useAvatar } from "@/hooks/useAvatar";
import type { SeoConfig } from "@/types/seo";
import styles from "./CounterPage.module.scss";

export const counterPageSeo: SeoConfig = {
  title: "Dashboard preview",
  description:
    "Preview reusable dashboard surfaces, setup completion state, and code-split navigation targets inside ThunderForge VTT.",
  keywords: [
    "ThunderForge dashboard",
    "React dashboard",
    "virtual tabletop admin",
  ],
  canonicalPath: "/counter",
  prefetchHrefs: ["/world/demo-world/play", "/worlds"],
};

export default function CounterPage() {
  const { logout, user } = useAuth();
  const [value, setValue] = useState(0);
  const [searchParams] = useSearchParams();
  const [accountStatus, setAccountStatus] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState<"json" | "zip" | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const bootstrapComplete = searchParams.get("bootstrap") === "complete";
  const { exportAvatar, exportToken } = useAvatar("demo-world-builder");
  const insights = useMemo(
    () => [
      {
        title: "Worlds",
        body: "World discovery, invitations, and indexing can live in this shell without touching engine boot.",
      },
      {
        title: "Scenes",
        body: "Scene chrome is isolated around the tldraw editor, so future whiteboard tools stay swappable.",
      },
      {
        title: "Permissions",
        body: "Role controls can slot into themed sidebars and dialogs while the sync contract remains unchanged.",
      },
    ],
    [],
  );
  const tabItems = useMemo(
    () => [
      {
        value: "worlds",
        label: "Worlds",
        icon: "worlds" as const,
        content: (
          <Panel variant="parchment" className={styles.tabPanel}>
            <h3>Guild atlas</h3>
            <p>
              Use this surface for world discovery, invitations, ownership
              transfer, and world summaries.
            </p>
          </Panel>
        ),
      },
      {
        value: "tokens",
        label: "Tokens",
        icon: "tokens" as const,
        content: (
          <Panel variant="parchment" className={styles.tabPanel}>
            <h3>Token forge</h3>
            <p>
              Dicebear portraits and token exports are ready for scene
              placement, actor linking, and faction theming.
            </p>
          </Panel>
        ),
      },
      {
        value: "permissions",
        label: "Permissions",
        icon: "shield" as const,
        content: (
          <Panel variant="parchment" className={styles.tabPanel}>
            <h3>Ward sigils</h3>
            <p>
              Permissions can map onto Radix dialogs, dropdowns, and scroll
              areas as the policy surface expands.
            </p>
          </Panel>
        ),
      },
    ],
    [],
  );

  const downloadExport = async (format: "json" | "zip") => {
    setIsExporting(format);
    setAccountStatus(null);

    try {
      const blob = await exportUserData(format);
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download =
        format === "zip"
          ? "thunderforge-user-export.zip"
          : "thunderforge-user-export.json";
      anchor.click();
      URL.revokeObjectURL(url);
      setAccountStatus(
        format === "zip"
          ? "ZIP export is ready for download."
          : "JSON export is ready for download.",
      );
    } catch (error) {
      setAccountStatus(
        error instanceof Error ? error.message : "Export request failed.",
      );
    } finally {
      setIsExporting(null);
    }
  };

  const permanentlyDeleteAccount = async () => {
    setIsDeleting(true);
    setAccountStatus(null);

    try {
      const response = await deleteUserData();
      setAccountStatus(response.message);
      await logout();
      window.location.assign("/login");
    } catch (error) {
      setAccountStatus(
        error instanceof Error ? error.message : "Delete request failed.",
      );
      setIsDeleting(false);
    }
  };

  return (
    <>
      <SEO {...counterPageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <p className={styles.eyebrow}>Design system showcase</p>
            <h1>The fantasy command deck for ThunderForge.</h1>
            <p>
              This page now acts as the UI guild hall: Radix primitives, themed
              panels, Dicebear identity previews, and integration notes for the
              next content phases.
            </p>
            <div className={styles.heroActions}>
              <Tooltip content="Preview the scene shell with Bevy and tldraw still mounted beneath the new chrome.">
                <Button asChild icon="worlds">
                  <Link to="/world/demo-world/play">Open demo world</Link>
                </Button>
              </Tooltip>
              <Popover
                trigger={
                  <Button variant="secondary" icon="spark">
                    Theme notes
                  </Button>
                }
              >
                <div className={styles.popoverCopy}>
                  <strong>Fantasy palette</strong>
                  <p>
                    Parchment, umber, forest, violet, gold, and candlelight
                    compose the shared token language for every page.
                  </p>
                </div>
              </Popover>
              <Dropdown
                trigger={
                  <Button variant="ghost" icon="rune">
                    Quick actions
                  </Button>
                }
                items={[
                  { label: "Login chamber", icon: "shield", href: "/login" },
                  {
                    label: "Bootstrap setup",
                    icon: "settings",
                    href: "/setup",
                  },
                  {
                    label: "Demo world",
                    icon: "worlds",
                    href: "/world/demo-world/play",
                  },
                ]}
              />
            </div>
          </section>

          {bootstrapComplete ? (
            <StatusBadge variant="success">
              The initial administrator was created successfully.
            </StatusBadge>
          ) : null}

          <Grid columns="two">
            <Card surface="leather" className={styles.counterPanel}>
              <div className={styles.counterRow}>
                <div>
                  <p className={styles.panelEyebrow}>Motion and controls</p>
                  <h2>Command rune counter</h2>
                  <p>
                    Keep one lightweight interactive element around as a
                    regression harness.
                  </p>
                </div>
                <span className={styles.counterValue}>{value}</span>
              </div>
              <div className={styles.counterRow}>
                <Button
                  variant="secondary"
                  icon="rune"
                  onClick={() => setValue(0)}
                >
                  Reset
                </Button>
                <Button
                  variant="primary"
                  icon="spark"
                  onClick={() => setValue((current) => current + 1)}
                >
                  Increment
                </Button>
              </div>
            </Card>

            <Card surface="parchment" className={styles.stack}>
              <div className={styles.avatarStrip}>
                <Avatar
                  seed="demo-world-builder"
                  name="World builder"
                  size="lg"
                />
                <div>
                  <p className={styles.panelEyebrow}>Dicebear integration</p>
                  <h2>Avatars and token exports</h2>
                  <p>
                    Player profiles, NPC portraits, and scene tokens can all
                    derive from seeds.
                  </p>
                </div>
              </div>
              <div className={styles.avatarActions}>
                <Button
                  variant="secondary"
                  icon="actors"
                  onClick={() => void exportAvatar("svg")}
                >
                  Export avatar SVG
                </Button>
                <Button
                  variant="secondary"
                  icon="tokens"
                  onClick={() => void exportToken("png")}
                >
                  Export token PNG
                </Button>
              </div>
              <div className={styles.tokenPreview}>
                <TokenAvatar seed="player" label="Player" />
                <TokenAvatar seed="npc" label="NPC" />
                <TokenAvatar seed="dragon-keeper" label="Warden" />
              </div>
            </Card>
          </Grid>

          <Card surface="stone" className={styles.notesCard}>
            <div className={styles.notesHeader}>
              <div>
                <p className={styles.panelEyebrow}>Account controls</p>
                <h2>Data ownership and privacy</h2>
              </div>
            </div>
            <p>
              Manage the authenticated account for{" "}
              <strong>{user?.username ?? "this session"}</strong> with
              self-service export and permanent deletion actions.
            </p>
            <div className={styles.counterRow}>
              <Button
                variant="secondary"
                icon="actors"
                disabled={isExporting !== null}
                onClick={() => void downloadExport("json")}
              >
                {isExporting === "json"
                  ? "Preparing JSON..."
                  : "Download JSON export"}
              </Button>
              <Button
                variant="secondary"
                icon="inventory"
                disabled={isExporting !== null}
                onClick={() => void downloadExport("zip")}
              >
                {isExporting === "zip"
                  ? "Preparing ZIP..."
                  : "Download ZIP export"}
              </Button>
              <Dialog
                trigger={
                  <Button variant="danger" icon="skull" disabled={isDeleting}>
                    {isDeleting ? "Deleting account..." : "Delete account"}
                  </Button>
                }
                title="Permanently delete this account?"
                description="This deletes the local profile, linked OAuth identities, sessions, and all currently persisted user-created data. Shared worlds keep their remaining owners."
                footer={
                  <Button
                    variant="danger"
                    icon="skull"
                    disabled={isDeleting}
                    onClick={() => void permanentlyDeleteAccount()}
                  >
                    {isDeleting ? "Deleting..." : "Delete permanently"}
                  </Button>
                }
              >
                <p>
                  This action is irreversible. ThunderForge will remove local
                  credentials, OAuth links, sessions, and owned persisted
                  content.
                </p>
              </Dialog>
            </div>
            {accountStatus ? <StatusBadge>{accountStatus}</StatusBadge> : null}
          </Card>

          <RuneDivider label="Future phase integration" />

          <Tabs defaultValue="worlds" items={tabItems} />

          <Grid columns="three">
            {insights.map((insight) => (
              <Card
                key={insight.title}
                surface="stone"
                className={styles.metaList}
              >
                <h2>{insight.title}</h2>
                <p>{insight.body}</p>
              </Card>
            ))}
          </Grid>

          <Card surface="leather" className={styles.notesCard}>
            <div className={styles.notesHeader}>
              <div>
                <p className={styles.panelEyebrow}>Scene editor notes</p>
                <h2>Fantasy UI primitives in one place</h2>
              </div>
              <Dialog
                trigger={
                  <Button variant="secondary" icon="spells">
                    Open release notes
                  </Button>
                }
                title="Fantasy overhaul package"
                description="This modal demonstrates the Radix dialog wrapper with the new parchment styling."
                footer={
                  <Button variant="primary" icon="spark">
                    Bound to future changelog flows
                  </Button>
                }
              >
                <p>
                  The current pass establishes theme tokens, layouts,
                  navigation, avatar plumbing, and a skinned world workspace
                  while leaving engine bindings untouched.
                </p>
              </Dialog>
            </div>
            <ScrollArea className={styles.notesScroll}>
              <div className={styles.notesList}>
                {[
                  "Worlds: add index cards, ownership badges, and party summaries.",
                  "Scenes: attach scene metadata to the world shell and expand toolbar actions.",
                  "Tokens: connect avatar seeds to actor records and persistence.",
                  "Actors: use tabs, dialogs, and scroll areas for sheets and inventories.",
                  "Permissions: add dropdown and dialog flows for role assignment.",
                ].map((note) => (
                  <div key={note} className={styles.noteItem}>
                    <FantasyIcon name="spark" size={16} tone="gold" />
                    <span>{note}</span>
                  </div>
                ))}
              </div>
            </ScrollArea>
          </Card>
        </main>
      </Container>
    </>
  );
}
