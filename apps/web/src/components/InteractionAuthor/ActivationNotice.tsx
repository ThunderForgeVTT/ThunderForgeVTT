import { refusalNotice, type ActivationResult } from "@/api/interactives";

/**
 * What somebody is told when their click did not do what they expected.
 *
 * Spec 030 FR-014. The reason this exists at all: silence is
 * indistinguishable from the product being broken. A player who clicks a
 * locked door and sees nothing has no way to tell "it is locked" from "this
 * feature does not work", and the second reading is the one people reach for.
 *
 * A performed effect says nothing, because the effect is the feedback.
 */

export interface ActivationNoticeProps {
  result: ActivationResult | null;
}

export function ActivationNotice({ result }: ActivationNoticeProps) {
  if (!result) return null;
  const notice = refusalNotice(result);
  if (!notice) return null;

  // `status` rather than `alert`: a locked door is information, not an error,
  // and an assertive live region would interrupt a screen reader mid-sentence
  // for something the player very likely expected.
  return <p role="status">{notice}</p>;
}
