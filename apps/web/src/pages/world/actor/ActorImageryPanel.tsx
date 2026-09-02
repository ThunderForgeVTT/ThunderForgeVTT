import { useEffect, useState } from "react";
import {
  ACTOR_IMAGE_PORTRAIT,
  ACTOR_IMAGE_TOKEN,
  getWorldActorImages,
  removeActorImage,
  uploadActorImage,
  type ActorImageRecord,
} from "@/api/actors";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { imageForRole } from "@/pages/world/actor/actorImagery";

/**
 * Spec 031 (T070, FR-036): giving an actor a portrait and a token image.
 *
 * # Why two slots and not one uploader
 *
 * A playtest found actors with no art at all, and the fix is not "an image" —
 * it is two images that are used in different places. A portrait is a face,
 * shown large in a sheet or a panel; a token is what stands on the map, seen
 * at map scale and cropped to a circle. Offering one box and using the result
 * for both would put a waist-up portrait on the battlemap, which is precisely
 * the thing this is meant to stop. So each role is uploaded, previewed and
 * cleared on its own, and each preview is shaped like the place its image
 * ends up.
 *
 * # Why the actor must already exist
 *
 * Imagery is rows against an actor id (ADR-057), so there is nothing to
 * attach a file to until the actor has been saved once. The editing page
 * therefore saves first and shows this panel afterwards, rather than holding
 * bytes in memory across a creation that might fail.
 *
 * # Why the upload is not optimistic
 *
 * The server transcodes to WebP and refuses an oversized or undecodable file
 * before writing anything. Showing the chosen file immediately would mean
 * showing an image that may never exist, and the refusal message — the size
 * limit, in practice — is the useful part of the answer.
 */
export interface ActorImageryPanelProps {
  worldId: string;
  actorId: string;
  /**
   * Whether this caller may change the imagery. The server decides regardless
   * (Constitution Principle III); this only governs whether the controls are
   * offered at all.
   */
  canEdit: boolean;
}

const ROLE_SLOTS = [
  {
    role: ACTOR_IMAGE_PORTRAIT,
    label: "Portrait",
    hint: "The character's face, shown on sheets and panels.",
    // Portrait-shaped, so what a Game Master sees here is what a sheet shows.
    previewClass: "h-32 w-24 rounded-md object-cover",
  },
  {
    role: ACTOR_IMAGE_TOKEN,
    label: "Token",
    hint: "What stands on the map, seen at map scale.",
    // Round and small, because that is how the map crops it.
    previewClass: "h-16 w-16 rounded-full object-cover",
  },
] as const;

export function ActorImageryPanel({
  worldId,
  actorId,
  canEdit,
}: ActorImageryPanelProps) {
  const [images, setImages] = useState<ActorImageRecord[]>([]);
  const [busyRole, setBusyRole] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    // The panel loads its own imagery and then keeps it from the mutation
    // results, so the page around it never has to know when to refetch.
    getWorldActorImages(worldId)
      .then((byActor) => {
        if (active) {
          setImages(byActor[actorId] ?? []);
        }
      })
      .catch(() => {
        if (active) {
          setImages([]);
        }
      });
    return () => {
      active = false;
    };
  }, [worldId, actorId]);

  const handleUpload = async (role: string, file: File) => {
    setBusyRole(role);
    setStatus(null);
    try {
      const saved = await uploadActorImage(actorId, role, file);
      // The upload replaces this role and nothing else, so the reply is the
      // whole of the change — refetching the world's imagery to learn one row
      // would ask the server for every other actor's as well.
      setImages((current) => [
        ...current.filter((image) => image.role !== role),
        saved,
      ]);
      setStatus(`${role === ACTOR_IMAGE_TOKEN ? "Token" : "Portrait"} saved.`);
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setBusyRole(null);
    }
  };

  const handleRemove = async (role: string) => {
    setBusyRole(role);
    setStatus(null);
    try {
      await removeActorImage(actorId, role);
      setImages((current) => current.filter((image) => image.role !== role));
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Removal failed");
    } finally {
      setBusyRole(null);
    }
  };

  return (
    <Card className="grid gap-4 p-5" data-testid="actor-imagery-panel">
      <div className="grid gap-1">
        <h2 className="text-sm font-semibold tracking-widest text-muted-foreground uppercase">
          Imagery
        </h2>
        <p className="text-sm text-muted-foreground">
          A portrait and a token are different pictures for different places.
        </p>
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        {ROLE_SLOTS.map((slot) => {
          const image = imageForRole(images, slot.role);
          const isBusy = busyRole === slot.role;
          return (
            <div
              key={slot.role}
              className="grid gap-2"
              data-testid={`actor-imagery-slot-${slot.role}`}
            >
              <p className="text-xs font-semibold tracking-wider uppercase">
                {slot.label}
              </p>
              {image ? (
                <img
                  src={image.thumbnailUrl}
                  alt={`${slot.label} for this actor`}
                  className={`border border-border bg-muted ${slot.previewClass}`}
                  data-testid={`actor-imagery-preview-${slot.role}`}
                />
              ) : (
                <div
                  className={`grid place-items-center border border-dashed border-border text-[0.65rem] text-muted-foreground ${slot.previewClass}`}
                  data-testid={`actor-imagery-empty-${slot.role}`}
                >
                  None
                </div>
              )}
              <p className="text-xs text-muted-foreground">{slot.hint}</p>
              {canEdit ? (
                <div className="grid gap-2">
                  <input
                    type="file"
                    accept="image/*"
                    disabled={isBusy}
                    className="text-xs"
                    aria-label={`Upload ${slot.label.toLowerCase()}`}
                    data-testid={`actor-imagery-input-${slot.role}`}
                    onChange={(event) => {
                      const file = event.target.files?.[0];
                      // The input is cleared so choosing the same file twice
                      // after a failure still fires a change event.
                      event.target.value = "";
                      if (file) {
                        void handleUpload(slot.role, file);
                      }
                    }}
                  />
                  {image ? (
                    <Button
                      type="button"
                      variant="ghost"
                      size="sm"
                      disabled={isBusy}
                      onClick={() => void handleRemove(slot.role)}
                      data-testid={`actor-imagery-remove-${slot.role}`}
                    >
                      Remove
                    </Button>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>

      {status ? (
        <p
          className="text-sm text-muted-foreground"
          data-testid="actor-imagery-status"
        >
          {status}
        </p>
      ) : null}
    </Card>
  );
}
