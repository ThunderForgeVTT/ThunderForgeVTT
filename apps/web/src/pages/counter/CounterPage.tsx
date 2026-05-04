import { useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
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
import { useAvatar } from "@/hooks/useAvatar";
import type { SeoConfig } from "@/types/seo";
import styles from "./CounterPage.module.scss";

export const counterPageSeo: SeoConfig = {
  title: "Dashboard preview",
  description:
    "Preview reusable dashboard surfaces, setup completion state, and code-split navigation targets inside ThunderForge VTT.",
  keywords: ["ThunderForge dashboard", "React dashboard", "virtual tabletop admin"],
  canonicalPath: "/counter",
  prefetchHrefs: ["/world/demo-world"],
};

export default function CounterPage() {
  const [value, setValue] = useState(0);
  const [searchParams] = useSearchParams();
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
              Use this surface for world discovery, invitations, ownership transfer,
              and world summaries.
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
              Dicebear portraits and token exports are ready for scene placement,
              actor linking, and faction theming.
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
              Permissions can map onto Radix dialogs, dropdowns, and scroll areas as
              the policy surface expands.
            </p>
          </Panel>
        ),
      },
    ],
    [],
  );

  return (
    <>
      <SEO {...counterPageSeo} />
      <Container>
        <main className={styles.shell}>
          <section className={styles.hero}>
            <p className={styles.eyebrow}>Design system showcase</p>
            <h1>The fantasy command deck for ThunderForge.</h1>
            <p>
              This page now acts as the UI guild hall: Radix primitives, themed panels,
              Dicebear identity previews, and integration notes for the next content phases.
            </p>
            <div className={styles.heroActions}>
              <Tooltip content="Preview the scene shell with Bevy and tldraw still mounted beneath the new chrome.">
                <Button asChild icon="worlds">
                  <Link to="/world/demo-world">Open demo world</Link>
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
                    Parchment, umber, forest, violet, gold, and candlelight compose the
                    shared token language for every page.
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
                  { label: "Bootstrap setup", icon: "settings", href: "/setup" },
                  { label: "Demo world", icon: "worlds", href: "/world/demo-world" },
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
                  <p>Keep one lightweight interactive element around as a regression harness.</p>
                </div>
                <span className={styles.counterValue}>{value}</span>
              </div>
              <div className={styles.counterRow}>
                <Button variant="secondary" icon="rune" onClick={() => setValue(0)}>
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
                <Avatar seed="demo-world-builder" name="World builder" size="lg" />
                <div>
                  <p className={styles.panelEyebrow}>Dicebear integration</p>
                  <h2>Avatars and token exports</h2>
                  <p>Player profiles, NPC portraits, and scene tokens can all derive from seeds.</p>
                </div>
              </div>
              <div className={styles.avatarActions}>
                <Button variant="secondary" icon="actors" onClick={() => void exportAvatar("svg")}>
                  Export avatar SVG
                </Button>
                <Button variant="secondary" icon="tokens" onClick={() => void exportToken("png")}>
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

          <RuneDivider label="Future phase integration" />

          <Tabs defaultValue="worlds" items={tabItems} />

          <Grid columns="three">
            {insights.map((insight) => (
              <Card key={insight.title} surface="stone" className={styles.metaList}>
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
                  The current pass establishes theme tokens, layouts, navigation, avatar
                  plumbing, and a skinned world workspace while leaving engine bindings untouched.
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
