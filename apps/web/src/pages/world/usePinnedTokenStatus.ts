import { useEffect, useMemo, useState } from "react";
import {
  readTokenStatus,
  type TokenStatusResource,
} from "@/engine/bevy/tokenStatus";
import type { PanelPosition } from "@/components/StatusPanel/StatusPanel";
import type { useEngineBridge } from "./useEngineBridge";

interface UsePinnedTokenStatusParams {
  pinned: {
    tokenId: string;
    at: PanelPosition;
  } | null;
  worldState: ReturnType<typeof useEngineBridge>["worldState"];
}

/**
 * What the pinned status panel shows: the pinned token's name and resources.
 *
 * Moved out of `WorldPage` unchanged. The pin itself stays the page's, since
 * the board's double-click sets it and the panel moves it.
 */
export function usePinnedTokenStatus({
  pinned,
  worldState,
}: UsePinnedTokenStatusParams) {
  // Spec 029: the selected token's resources, read back from the engine rather
  // than held here. React observes; it does not become a second store
  // (Constitution I, ADR-053).
  // Keyed by the token it was read for, so a slow read that resolves after
  // the selection has moved cannot paint one token's resources under
  // another's name — and so deselection needs no synchronous state write.
  const [panelStatus, setPanelStatus] = useState<{
    tokenId: string;
    resources: TokenStatusResource[] | null;
  } | null>(null);

  /** The pinned token's name, for the panel heading. */
  const pinnedTokenLabel = useMemo(() => {
    const id = pinned?.tokenId;
    if (!id) return undefined;
    return worldState.tokens[id]?.label ?? undefined;
  }, [pinned?.tokenId, worldState.tokens]);

  // Follow the pin, not the selection (spec 029 FR-012a): read what the
  // engine would draw for the pinned token, whatever is selected now.
  useEffect(() => {
    const tokenId = pinned?.tokenId;
    if (!tokenId) {
      return;
    }

    let cancelled = false;
    void readTokenStatus(tokenId).then((resources) => {
      if (!cancelled) setPanelStatus({ tokenId, resources });
    });
    return () => {
      cancelled = true;
    };
  }, [pinned?.tokenId, worldState.tokens]);

  // Derived rather than stored: the panel shows a token's resources only while
  // that token is the pinned one, so unpinning — or pinning another — can
  // never show the previous token's numbers.
  const panelResources =
    pinned && panelStatus?.tokenId === pinned.tokenId
      ? panelStatus.resources
      : null;

  return { pinnedTokenLabel, panelResources };
}
