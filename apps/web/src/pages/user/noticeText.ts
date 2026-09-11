import type { AccountNotice } from "@/api/standing";

/** A date as the reader writes it, in their own locale. */
export function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/** The adopter's own copies, by name — never the source, never the sharer. */
function copiesOf(payload: Record<string, unknown>): string[] {
  return Array.isArray(payload.copies)
    ? (payload.copies as unknown[]).map(String)
    : [];
}

/**
 * The words of a notice, rendered from its kind and payload. Not stored on the
 * server on purpose, so this is the one place they live.
 */
export function noticeText(notice: AccountNotice): string {
  const payload = notice.payload ?? {};
  const count = Number(payload.strikeCount ?? 0);
  const dateOf = (key: string, otherwise: string) =>
    typeof payload[key] === "string"
      ? formatDate(payload[key] as string)
      : otherwise;
  switch (notice.kind) {
    case "strike_recorded": {
      const suspendAt = Number(payload.suspendPublishingAt ?? 0);
      const threshold = Number(payload.threshold ?? 0);
      return (
        `A takedown against something you shared was upheld, and it counts as ` +
        `strike ${count}. Sharing pauses at ${suspendAt} and the account is ` +
        `disabled at ${threshold}. This strike stops counting on ` +
        `${dateOf("agesOutAt", "the end of the lookback")}, or sooner if a ` +
        `counter-notice succeeds.`
      );
    }
    case "publishing_suspended":
      return (
        `Sharing is paused: ${count} strikes are counting. Your worlds are ` +
        `untouched — you can still play, edit and read everything you have ` +
        `made. Sharing comes back when a strike stops counting or a ` +
        `counter-notice succeeds.`
      );
    case "account_disabled":
      // FR-036: that deletion is irreversible, in the first sentence.
      return (
        `This account will be permanently and irreversibly deleted on ` +
        `${dateOf("deletionDueAt", "the date shown above")}` +
        `${payload.requiresHuman ? ", once an administrator confirms it" : ""}. ` +
        `It was disabled after ${count} strikes. Until then you can download ` +
        `everything you have, appeal, or file a counter-notice against any ` +
        `strike — none of these uses up another.`
      );
    case "appeal_resolved":
      return payload.upheld
        ? "Your appeal was upheld. The account is restored, the deletion is " +
            "cancelled, and the strike it overturned no longer counts."
        : `Your appeal was not upheld. The account will be deleted on ` +
            `${dateOf("deletionDueAt", "the date already set")}` +
            `${payload.requiresHuman ? ", once an administrator confirms it" : ""}.`;
    case "account_restored":
      return (
        "Your account is restored. Fewer strikes are counting than the " +
        "number that disables an account, so the deletion is cancelled — " +
        "nobody had to ask."
      );
    case "actor_rescued": {
      const names = Array.isArray(payload.characters)
        ? (payload.characters as string[]).join(", ")
        : "your characters";
      return (
        `The world "${String(payload.sourceWorldName ?? "")}" was deleted ` +
        `with its creator's account. Before it went, ${names} ` +
        `${Array.isArray(payload.characters) && payload.characters.length === 1 ? "was" : "were"} ` +
        `moved to a world of yours, in a collection named after it.`
      );
    }
    case "adopted_copy_disabled": {
      // FR-023b: told, and not accused — said plainly, before anything else
      // they might read as blame. Names their copy and nothing about the notice.
      const copies = copiesOf(payload);
      const one = copies.length <= 1;
      const what = copies.length > 0 ? copies.join(", ") : "A copy you took";
      return (
        `${what} ${one ? "was" : "were"} disabled, because what ` +
        `${one ? "it was" : "they were"} copied from was taken down after a ` +
        `copyright notice. You are not accused of anything, and this is not ` +
        `a strike against you. Nothing was deleted: ` +
        `${one ? "it stays" : "they stay"} in your world, everything around ` +
        `${one ? "it" : "them"} is untouched, and ${one ? "it comes" : "they come"} ` +
        `back on ${one ? "its" : "their"} own if the notice is withdrawn or a ` +
        `counter-notice succeeds.`
      );
    }
    case "share_taken_down": {
      const copies = Number(payload.copiesDisabled ?? 0);
      const one = copies === 1;
      return (
        `The takedown also reached ${copies} ${one ? "copy" : "copies"} that ` +
        `other people had taken of what you shared, and ` +
        `${one ? "it was" : "they were"} disabled with it. Nobody who took a ` +
        `copy is accused of anything. If a counter-notice succeeds, ` +
        `${one ? "it comes" : "they come"} back with yours.`
      );
    }
    case "adopted_copy_restored": {
      const copies = copiesOf(payload);
      const one = copies.length <= 1;
      const what = copies.length > 0 ? copies.join(", ") : "A copy you took";
      return (
        `${what} ${one ? "is" : "are"} back. What ${one ? "it was" : "they were"} ` +
        `copied from was restored, so ${one ? "your copy was" : "your copies were"} ` +
        `too — nobody had to ask.`
      );
    }
    default:
      return "A notice about your account.";
  }
}
