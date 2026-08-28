import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button/Button";
import { Panel } from "@/components/ui/panel/Panel";
import {
  fetchCanvasImageAssetsForScene,
  uploadCanvasImage,
  type CanvasImageAsset,
} from "@/api/assets";
import {
  didApply,
  type IntentResult,
  type ManipulateIntent,
  type TokenControlFacet,
} from "@/engine/world/facets";

export interface TokenToolProps {
  /**
   * The token-control facet. Supplies both the permissions this panel
   * renders from and the calls its buttons make, so a control cannot be
   * enabled while the intent behind it would be refused.
   */
  control: TokenControlFacet;
  selectedTokenId: string | null;
  /** Needed to upload new token art; art is stored per world/scene. */
  worldId: string;
  sceneId: string;
}

/**
 * The URL form the engine can actually load.
 *
 * Same-origin and extension-bearing, both deliberately: the engine hands
 * this straight to Bevy's `AssetServer`, which picks an image loader by
 * file extension and never requests an extensionless path, and which has
 * no way to satisfy a cross-origin fetch that CORS declines. This is why
 * art is chosen from uploaded assets rather than typed in as an arbitrary
 * URL — a free-text field would happily accept addresses that leave the
 * token invisible on the canvas with nothing to explain why.
 */
function assetUrl(asset: CanvasImageAsset): string {
  return `/api/canvas-assets/${asset.id}.webp`;
}

const MIN_TOKEN_SCALE = 1;
const MAX_TOKEN_SCALE = 5;
const ROTATE_STEP_DEGREES = 30;

/**
 * TokenTool: property panel for the currently selected token's size,
 * facing and art, per specs/004-token-canvas-authoring T020.
 *
 * Resize/rotate were shipped in an earlier session as keyboard-only
 * shortcuts (`]`/`[` resize, `,`/`.` rotate — see
 * `src/engine/src/systems/selection.rs`'s `handle_token_resize_rotate_keyboard`)
 * with no UI surface at all, which meant a GM had no way to discover or
 * verify the capability existed. This panel closes that gap: it displays
 * the selected token's current scale/rotation and offers equivalent
 * buttons, without introducing a second implementation of the
 * resize/rotate logic itself.
 *
 * Every button goes through the token-control facet rather than
 * dispatching `upsert_token` itself. That is the point of the migration:
 * the facet resolves what this viewer may do to this token *once*
 * (`resolveTokenPermissions`), and this panel both disables its controls
 * and issues its intents from that same answer — so an enabled button and
 * a refused intent cannot disagree. Previously the panel dispatched
 * directly and relied on the caller mounting it only for a GM, which put
 * the permission rule in two places and neither of them next to the
 * server's.
 *
 * Per Constitution Principle I this component never renders or simulates
 * the token itself; the facet proposes, the adjudicator decides, and the
 * engine (via `bindWorldStore`'s `apply_world_command` forwarding) is what
 * actually updates the on-canvas Transform.
 *
 * The panel no longer *depends* on being mounted GM-only — every control
 * is gated by the facet, so a player seeing it would get their own art
 * control and disabled size/facing, matching what the server enforces.
 * `WorldPage` does still mount it GM-only today, because it lives in the
 * GM tool rail; that is now the caller's placement decision rather than
 * this panel's security boundary.
 */
/** A refusal or rejection, phrased for someone looking at the panel. */
function explain(result: IntentResult<unknown>): string | null {
  if (didApply(result)) return null;
  if (result.status === "rejected") return result.reason;
  switch (result.reason) {
    case "not-yours":
      return "You do not control this token.";
    case "gm-only":
      return "Only the GM can change that.";
    case "observer":
      return "You are viewing this scene, not playing in it.";
    case "not-connected":
      return "Could not reach the server; nothing was changed.";
    case "unknown-subject":
      return "That token is no longer on this scene.";
  }
}

export function TokenTool({
  control,
  selectedTokenId,
  worldId,
  sceneId,
}: TokenToolProps) {
  // Read through the facet on every render rather than from a `tokens`
  // prop: permissions and token data then come from one place, and the
  // parent re-renders on every store change anyway (it holds the world
  // state), so this stays current without a second subscription.
  const selected = selectedTokenId
    ? control.permissions(selectedTokenId)
    : null;
  const selectedToken = selected?.token ?? null;

  const fileInputRef = useRef<HTMLInputElement>(null);
  const [artStatus, setArtStatus] = useState<"idle" | "uploading" | "error">(
    "idle",
  );
  const [artError, setArtError] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const [assets, setAssets] = useState<CanvasImageAsset[]>([]);
  /** Why the last intent did not apply, if it did not. */
  const [refusal, setRefusal] = useState<string | null>(null);

  const applyTokenChange = useCallback(
    async (changes: Omit<ManipulateIntent, "tokenId">) => {
      if (!selectedTokenId) {
        return;
      }
      const result = await control.manipulate({
        tokenId: selectedTokenId,
        ...changes,
      });
      // Shown rather than swallowed: a button that does nothing and says
      // nothing is the failure mode this migration exists to remove.
      setRefusal(explain(result));
    },
    [control, selectedTokenId],
  );

  const setArt = useCallback(
    // `null` removes the art, and has to stay distinguishable from
    // "unchanged" all the way to the server — see `UpdateTokenInput`.
    (photoUrl: string | null) => {
      void applyTokenChange({ photoUrl });
    },
    [applyTokenChange],
  );

  const upload = useCallback(
    (file: File) => {
      setArtStatus("uploading");
      setArtError(null);
      uploadCanvasImage(worldId, sceneId, "PASTED", file)
        .then((asset) => {
          setArtStatus("idle");
          setAssets((current) => [asset, ...current]);
          setArt(assetUrl(asset));
        })
        .catch((error: unknown) => {
          setArtStatus("error");
          setArtError(
            error instanceof Error ? error.message : "Failed to upload art",
          );
        });
    },
    [worldId, sceneId, setArt],
  );

  useEffect(() => {
    if (!picking) {
      return;
    }
    let cancelled = false;
    fetchCanvasImageAssetsForScene(sceneId)
      .then((found) => {
        if (!cancelled) setAssets(found);
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        setArtStatus("error");
        setArtError(
          error instanceof Error ? error.message : "Failed to load art",
        );
      });
    return () => {
      cancelled = true;
    };
  }, [picking, sceneId]);

  if (!selected || !selectedToken) {
    return null;
  }

  const currentScale = selectedToken.scale ?? 1;
  const currentRotationDegrees = Math.round(
    (((selectedToken.rotation ?? 0) * 180) / Math.PI) % 360,
  );
  const currentArt = selectedToken.photoUrl ?? null;

  const resize = (delta: number) => {
    const nextScale = Math.min(
      MAX_TOKEN_SCALE,
      Math.max(MIN_TOKEN_SCALE, currentScale + delta),
    );
    void applyTokenChange({ scale: nextScale });
  };

  const rotate = (deltaDegrees: number) => {
    const nextRotation =
      (selectedToken.rotation ?? 0) + (deltaDegrees * Math.PI) / 180;
    void applyTokenChange({ rotation: nextRotation });
  };

  return (
    <Panel variant="stone" className="grid gap-3" data-testid="token-tool">
      <p className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        Selected token
      </p>

      <div className="grid gap-1.5">
        <span className="text-sm" data-testid="token-tool-scale">
          Size: {currentScale}x
        </span>
        <div className="flex gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-shrink"
            onClick={() => resize(-1)}
            disabled={!selected.canResize || currentScale <= MIN_TOKEN_SCALE}
          >
            Shrink
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-grow"
            onClick={() => resize(1)}
            disabled={!selected.canResize || currentScale >= MAX_TOKEN_SCALE}
          >
            Grow
          </Button>
        </div>
        <span className="text-xs text-muted-foreground">
          Keyboard: {"]"} grow, {"["} shrink (whole grid cells only)
        </span>
      </div>

      <div className="grid gap-1.5">
        <span className="text-sm" data-testid="token-tool-rotation">
          Facing: {currentRotationDegrees}°
        </span>
        <div className="flex gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-rotate-left"
            disabled={!selected.canRotate}
            onClick={() => rotate(ROTATE_STEP_DEGREES)}
          >
            Rotate left
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-rotate-right"
            disabled={!selected.canRotate}
            onClick={() => rotate(-ROTATE_STEP_DEGREES)}
          >
            Rotate right
          </Button>
        </div>
        <span className="text-xs text-muted-foreground">
          Keyboard: , rotate left, . rotate right
        </span>
      </div>

      <div className="grid gap-1.5" data-testid="token-tool-art">
        <span className="text-sm">Art</span>

        <div className="flex items-center gap-2">
          <div className="bg-muted grid size-12 shrink-0 place-items-center overflow-hidden rounded border">
            {currentArt ? (
              <img
                src={currentArt}
                alt="Selected token art"
                className="size-full object-contain"
                data-testid="token-tool-art-preview"
              />
            ) : (
              <span className="text-muted-foreground text-[0.6rem]">None</span>
            )}
          </div>
          <span className="text-xs text-muted-foreground">
            {currentArt
              ? "Fitted inside the token's grid footprint."
              : "No art — draws as a plain colour token."}
          </span>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-art-upload"
            disabled={!selected.canSetArt || artStatus === "uploading"}
            onClick={() => fileInputRef.current?.click()}
          >
            {artStatus === "uploading" ? "Uploading…" : "Upload art"}
          </Button>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            data-testid="token-tool-art-pick"
            disabled={!selected.canSetArt}
            onClick={() => setPicking((open) => !open)}
          >
            {picking ? "Hide scene art" : "Scene art"}
          </Button>
          {currentArt && selected.canSetArt ? (
            <Button
              type="button"
              variant="secondary"
              size="sm"
              data-testid="token-tool-art-clear"
              onClick={() => setArt(null)}
            >
              Remove art
            </Button>
          ) : null}
        </div>

        {/* Hidden, driven by the button above: a bare file input cannot be
            styled to match the rest of this panel. */}
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          className="hidden"
          data-testid="token-tool-art-file"
          onChange={(event) => {
            const file = event.target.files?.[0];
            // Cleared so picking the same file twice in a row still fires
            // a change event.
            event.target.value = "";
            if (file) {
              upload(file);
            }
          }}
        />

        {picking ? (
          assets.length > 0 ? (
            <div
              className="grid grid-cols-5 gap-1.5"
              data-testid="token-tool-art-choices"
            >
              {assets.map((asset) => {
                const url = assetUrl(asset);
                return (
                  <button
                    key={asset.id}
                    type="button"
                    title={`${asset.widthPx}x${asset.heightPx}`}
                    aria-pressed={currentArt === url}
                    onClick={() => setArt(url)}
                    className={`bg-muted overflow-hidden rounded border transition ${
                      currentArt === url
                        ? "border-primary"
                        : "hover:border-primary/50"
                    }`}
                  >
                    <img
                      src={url}
                      alt=""
                      className="aspect-square size-full object-contain"
                    />
                  </button>
                );
              })}
            </div>
          ) : (
            <span className="text-xs text-muted-foreground">
              No uploaded art on this scene yet.
            </span>
          )
        ) : null}

        {artStatus === "error" && artError ? (
          <span className="text-destructive text-xs" role="alert">
            {artError}
          </span>
        ) : null}
      </div>

      {refusal ? (
        <span
          className="text-destructive text-xs"
          role="alert"
          data-testid="token-tool-refusal"
        >
          {refusal}
        </span>
      ) : null}
    </Panel>
  );
}
