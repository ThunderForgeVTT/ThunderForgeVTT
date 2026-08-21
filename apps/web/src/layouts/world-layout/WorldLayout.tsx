import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import { ScrollArea } from "@/components/ui/scroll-area/ScrollArea";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Tabs } from "@/components/ui/tabs/Tabs";
import { TokenAvatar } from "@/components/ui/token-avatar/TokenAvatar";
import type { WorldToken } from "@/engine/world/types";

interface WorldLayoutProps {
  worldId: string;
  canvas: ReactNode;
  tokens: WorldToken[];
}

export function WorldLayout({
  worldId,
  canvas,
  tokens,
}: WorldLayoutProps) {
  return (
    <main
      className="grid min-h-screen grid-rows-[auto_1fr] gap-4 bg-background p-4"
      data-world-id={worldId}
    >
      <header className="flex items-start justify-between gap-4 rounded-xl border border-border bg-card p-5">
        <div>
          <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
            World workspace
          </p>
          <h1 className="text-2xl font-semibold">
            {worldId || "Untitled realm"}
          </h1>
          <p className="mt-2 max-w-3xl text-muted-foreground">
            Bevy remains the authority for the game canvas — walls, lighting,
            shapes, and map import are all native canvas tools — while the
            shell layers in navigation and party rosters around it.
          </p>
        </div>
        <div className="grid justify-items-end gap-3">
          <StatusBadge variant="success">World sync active</StatusBadge>
          <Button asChild icon="arrow-left" variant="ghost">
            <Link to="/counter">Return to dashboard</Link>
          </Button>
        </div>
      </header>

      <section className="grid min-h-0 grid-cols-1 gap-4 lg:grid-cols-[minmax(260px,0.9fr)_minmax(0,2.2fr)]">
        <aside className="grid min-h-0 gap-4">
          <Panel variant="leather" className="overflow-hidden rounded-xl border border-border">
            <p className="mb-3 text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Party roster
            </p>
            <ScrollArea className="h-72">
              <div className="grid gap-4">
                {tokens.map((token) => (
                  <div key={token.id} className="flex items-center gap-3">
                    <TokenAvatar
                      seed={token.id}
                      label={token.label ?? token.id}
                    />
                    <div>
                      <strong className="block">
                        {token.label ?? token.id}
                      </strong>
                      <small className="mt-1 block text-[0.72rem] tracking-widest text-muted-foreground uppercase">
                        X {Math.round(token.x)} / Y {Math.round(token.y)}
                      </small>
                    </div>
                  </div>
                ))}
              </div>
            </ScrollArea>
          </Panel>

          <Panel variant="stone" className="rounded-xl border border-border">
            <Tabs
              defaultValue="worlds"
              items={[
                {
                  value: "worlds",
                  label: "Worlds",
                  icon: "worlds",
                  content: (
                    <p className="text-muted-foreground">
                      Future world metadata, discovery, and handoff flows can
                      mount in this sidebar without moving the engine canvas.
                    </p>
                  ),
                },
                {
                  value: "actors",
                  label: "Actors",
                  icon: "actors",
                  content: (
                    <p className="text-muted-foreground">
                      Actor sheets, permissions, and compendium panels can sit
                      beside the scene while staying decoupled from runtime
                      sync.
                    </p>
                  ),
                },
                {
                  value: "permissions",
                  label: "Permissions",
                  icon: "shield",
                  content: (
                    <p className="text-muted-foreground">
                      Player roles and world governance can layer onto this
                      shell without rewriting the Bevy entrypoints.
                    </p>
                  ),
                },
              ]}
            />
          </Panel>
        </aside>

        <div className="min-h-[32rem] overflow-hidden rounded-xl border border-border bg-background/60 p-3 lg:min-h-0">
          {canvas}
        </div>
      </section>
    </main>
  );
}
