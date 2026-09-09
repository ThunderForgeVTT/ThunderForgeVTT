import { withCsrf } from "@/api/auth";

const API_BASE = "/api";

/**
 * Spec 036 FR-008: change the password, and end every other session with it.
 *
 * # Why there was nothing here before
 *
 * There was no password-change route at all. `password_hash` was written at
 * registration, at the admin bootstrap and by OAuth auto-provisioning, and
 * never again — so somebody who believed their password had been seen had one
 * remedy, which was to abandon the account.
 *
 * # Why the other sessions go
 *
 * The usual reason to change a password is that somebody else has it, and
 * somebody else who has it is signed in. A change that left those sessions
 * alive would be cosmetic. `sessionsEnded` is how many went, so the screen
 * can say what happened instead of claiming it in the abstract.
 */
export interface PasswordChangeResult {
  message: string;
  sessionsEnded: number;
}

export async function changePassword(input: {
  currentPassword: string;
  newPassword: string;
}): Promise<PasswordChangeResult> {
  const response = await fetch(`${API_BASE}/authentication/password`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({ "Content-Type": "application/json" }),
    body: JSON.stringify({
      current_password: input.currentPassword,
      new_password: input.newPassword,
    }),
  });

  const body = (await response.json().catch(() => null)) as {
    status?: string;
    message?: string;
    sessions_ended?: number;
  } | null;

  if (!response.ok || body?.status !== "success") {
    throw new Error(body?.message ?? "The password could not be changed.");
  }

  return {
    message: body.message ?? "Your password has been changed.",
    sessionsEnded: body.sessions_ended ?? 0,
  };
}
