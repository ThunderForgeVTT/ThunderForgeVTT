import type { Dispatch, SetStateAction } from "react";
import type { NavigateFunction } from "react-router-dom";
import type { DockSection } from "@/components/world/PlayDock/WorldDock";
import { ChatPanel } from "@/components/world/PlayDock/ChatPanel";
import { ActorsPanel } from "@/components/world/PlayDock/ActorsPanel";
import { CombatPanel } from "@/components/world/PlayDock/CombatPanel";
import { SystemDockPanel } from "@/components/world/PlayDock/SystemDockPanel";
import { useSystemDockTitle } from "@/panels/systemPanels";
import { SettingsPanel } from "@/components/world/PlayDock/SettingsPanel";
import { HelpPanel } from "@/components/world/PlayDock/HelpPanel";
import { ApprovalQueue } from "@/components/ApprovalQueue";
import type { WorldStore } from "@/engine/world/store";
import type { WorldRecord } from "@/types/world";
import type { SceneRecord } from "@/types/scene";
import type { AuthUser } from "@/types/auth";

interface UseDockSectionsParams {
  id: string;
  sceneId: string | null;
  user: AuthUser | null;
  world: WorldRecord | null;
  worldStore: WorldStore;
  isSceneOwner: boolean;
  scenes: SceneRecord[];
  setScenes: Dispatch<SetStateAction<SceneRecord[]>>;
  setSelectedSceneId: Dispatch<SetStateAction<string | null>>;
  approvalRevision: number;
  setApprovalRevision: Dispatch<SetStateAction<number>>;
  handleMapImportComplete: () => void;
  navigate: NavigateFunction;
}

/**
 * The play dock's sections, in the order the dock shows them.
 *
 * Moved out of `WorldPage` unchanged; a hook only because the system tab's
 * title is one. Built afresh each render, as it always was.
 */
export function useDockSections({
  id,
  sceneId,
  user,
  world,
  worldStore,
  isSceneOwner,
  scenes,
  setScenes,
  setSelectedSceneId,
  approvalRevision,
  setApprovalRevision,
  handleMapImportComplete,
  navigate,
}: UseDockSectionsParams) {
  const systemDockLabel = useSystemDockTitle(world?.gameSystemId ?? null);
  const dockSections: DockSection[] = [
    {
      id: "chat",
      label: "Chat",
      icon: "quill",
      content: (
        <ChatPanel
          worldId={id}
          sceneId={sceneId}
          currentUserId={user?.id ?? null}
          isGm={isSceneOwner}
        />
      ),
    },
    {
      id: "actors",
      label: "Actors",
      icon: "actors",
      content: (
        <ActorsPanel
          worldId={id}
          worldStore={worldStore}
          sceneId={sceneId}
          isGm={isSceneOwner}
        />
      ),
    },
    {
      id: "combat",
      label: "Combat",
      icon: "shield",
      content: (
        <CombatPanel
          worldId={id}
          worldStore={worldStore}
          sceneId={sceneId}
          isGm={isSceneOwner}
        />
      ),
    },
    // Spec 067: the tab is the pack's, titled by the pack, and absent for a
    // system that puts nothing in the dock.
    ...(systemDockLabel
      ? [
          {
            id: "system" as const,
            label: systemDockLabel,
            icon: "moon" as const,
            content: (
              <SystemDockPanel
                worldId={id}
                isGm={isSceneOwner}
                currentUserId={user?.id}
                gameSystemId={world?.gameSystemId ?? null}
              />
            ),
          },
        ]
      : []),
    // Spec 030. Game-Master-only: a player is not shown the queue. Their own
    // outcome reaches them directly, and the rest of it is a list of what other
    // people asked for — which at some tables is information the GM is
    // deliberately not sharing yet.
    ...(isSceneOwner && sceneId
      ? [
          {
            id: "requests" as const,
            label: "Player requests",
            icon: "quill" as const,
            content: (
              <ApprovalQueue
                sceneId={sceneId}
                revision={approvalRevision}
                onDecided={() => setApprovalRevision((n) => n + 1)}
              />
            ),
          },
        ]
      : []),
    {
      id: "settings",
      label: "Settings",
      icon: "settings",
      content: (
        <SettingsPanel
          worldId={id}
          sceneId={sceneId}
          scenes={scenes}
          isGm={isSceneOwner}
          onSceneChange={setSelectedSceneId}
          onSceneCreated={(scene) =>
            setScenes((current) => [...current, scene])
          }
          onMapImportComplete={handleMapImportComplete}
          onBackToStaging={() => navigate(`/world/${id}/staging`)}
        />
      ),
    },
    // Playtest 2026-09-10 P4: About and Feedback, for every role. They used
    // to float over the map, under the tool rail and the dock.
    {
      id: "help",
      label: "About & feedback",
      icon: "rune",
      content: <HelpPanel />,
    },
  ];

  return dockSections;
}
