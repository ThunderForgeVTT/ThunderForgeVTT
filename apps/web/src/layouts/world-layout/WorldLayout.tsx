import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { TokenAvatar } from "@/components/ui/token-avatar/TokenAvatar";
import type { WorldToken } from "@/engine/world/types";
import styles from "./WorldLayout.module.scss";

interface WorldLayoutProps {
  worldId: string;
  canvas: ReactNode;
  whiteboard: ReactNode;
  tokens: WorldToken[];
}

export function WorldLayout({ worldId, canvas, whiteboard, tokens }: WorldLayoutProps) {
  return (
    <main className={styles.layout} data-world-id={worldId}>
      <header className={styles.header}>
        <div>
          <p className={styles.eyebrow}>World workspace</p>
          <h1>{worldId || "Untitled realm"}</h1>
          <p className={styles.copy}>
            Bevy remains the authority for the 3D stage while the fantasy shell layers
            in navigation, party rosters, and whiteboard controls.
          </p>
        </div>
        <div className={styles.headerActions}>
          <StatusBadge variant="success">World sync active</StatusBadge>
          <Button asChild icon="arrow-left" variant="ghost">
            <Link to="/counter">Return to dashboard</Link>
          </Button>
        </div>
      </header>

      <section className={styles.grid}>
        <aside className={styles.sidebar}>
          <Panel variant="leather" className={styles.sidebarPanel}>
            <p className={styles.panelEyebrow}>Party roster</p>
            <ScrollArea className={styles.tokenScroll}>
              <div className={styles.tokenList}>
                {tokens.map((token) => (
                  <div key={token.id} className={styles.tokenRow}>
                    <TokenAvatar seed={token.id} label={token.label ?? token.id} />
                    <div>
                      <strong>{token.label ?? token.id}</strong>
                      <small>
                        X {Math.round(token.x)} / Y {Math.round(token.y)}
                      </small>
                    </div>
                  </div>
                ))}
              </div>
            </ScrollArea>
          </Panel>

          <Panel variant="stone">
            <Tabs
              defaultValue="worlds"
              items={[
                {
                  value: "worlds",
                  label: "Worlds",
                  icon: "worlds",
                  content: (
                    <p className={styles.tabCopy}>
                      Future world metadata, discovery, and handoff flows can mount in
                      this sidebar without moving the engine canvas.
                    </p>
                  ),
                },
                {
                  value: "actors",
                  label: "Actors",
                  icon: "actors",
                  content: (
                    <p className={styles.tabCopy}>
                      Actor sheets, permissions, and compendium panels can sit beside
                      the scene while staying decoupled from runtime sync.
                    </p>
                  ),
                },
                {
                  value: "permissions",
                  label: "Permissions",
                  icon: "shield",
                  content: (
                    <p className={styles.tabCopy}>
                      Player roles and world governance can layer onto this shell
                      without rewriting the whiteboard or Bevy entrypoints.
                    </p>
                  ),
                },
              ]}
            />
          </Panel>
        </aside>

        <div className={styles.canvas}>{canvas}</div>
        <aside className={styles.whiteboard}>{whiteboard}</aside>
      </section>
    </main>
  );
}
