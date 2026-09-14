import { useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { onPlayPaused } from "@/api/playPauseSignal";

/** Where a paused world's table is sent. */
export function playPausedPath(worldId: string): string {
  return `/world/${worldId}/paused`;
}

/**
 * Spec 051 US1: go to the notice when an operator pauses a world's play.
 *
 * Mounted once, by the router, so every road a pause arrives by — a world
 * event, a stream's last error, a refused request from any page — ends in the
 * same place without each page wiring it (contracts/live-play-lock.md,
 * "Client signals"). The playfield also stops what would outlive its unmount;
 * that half is its own (`WorldPage.tsx`).
 *
 * `replace`, because the playfield is not somewhere Back can return to while
 * the pause holds: it would only refuse and send the person here again.
 */
export function usePlayPauseRedirect(): void {
  const navigate = useNavigate();
  // Read through a ref so the listener is registered once, not again on
  // every navigation.
  const { pathname } = useLocation();
  const pathnameRef = useRef(pathname);
  useEffect(() => {
    pathnameRef.current = pathname;
  }, [pathname]);

  useEffect(() => {
    return onPlayPaused((worldId) => {
      const target = playPausedPath(worldId);
      if (pathnameRef.current === target) return;
      navigate(target, { replace: true });
    });
  }, [navigate]);
}
