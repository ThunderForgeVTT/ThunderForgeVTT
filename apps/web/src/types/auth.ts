export interface SetupProvider {
  provider_key: string;
  display_name: string;
}

export type InstanceAccessPolicy = "open" | "invite_only" | "closed";

export interface SetupStatus {
  setup_required: boolean;
  setup_completed: boolean;
  configured_oauth_providers: SetupProvider[];
  /**
   * Spec 035 (FR-003): the instance's admission policy, so the signed-out
   * surface offers only routes that will actually work.
   *
   * Hiding a route is NOT enforcing it — the server refuses independently
   * (ADR-072). This exists so the page can be honest, not so it can be the
   * gate.
   */
  access_policy: InstanceAccessPolicy;
  accepting_access_requests: boolean;
  /**
   * Spec 041 FR-026: who to write to when you are locked out.
   *
   * Optional because an instance may not have set one, and because a server
   * older than this field simply does not send it. Both cases are the same
   * to the interface: say plainly that there is nobody to write to, rather
   * than send somebody looking for a contact that does not exist.
   */
  support_email?: string | null;
}

export interface AuthUser {
  id: string;
  username: string;
  email: string;
  role: "admin" | "user";
  is_admin: boolean;
  created_at: string;
  updated_at: string;
}

export interface AuthSession {
  authenticated: boolean;
  sessionExpiresAt: string;
  user: AuthUser;
  /**
   * Spec 039 US7: the account is disabled and may reach only its standing
   * page — the download and the appeal. Everywhere else would refuse it.
   */
  accountDisabled: boolean;
}

export interface AuthSessionResponse {
  status: string;
  message: string;
  session: AuthSession | null;
  loginTwoFactorChallengeId: string | null;
  requiresEmailVerification: boolean;
}

export interface LoginPayload {
  identifier: string;
  password: string;
  two_factor_code?: string;
}

export interface RegisterPayload {
  username: string;
  email: string;
  password: string;
  /** Spec 035 (FR-016): present when the visitor arrived from an invitation. */
  invitation_code?: string;
}

export interface OAuthActionResponse {
  status: string;
  message: string;
  challengeId: string | null;
  loginTwoFactorChallengeId: string | null;
}

export interface UserDataDeleteSummary {
  worlds_deleted: number;
  world_memberships_removed: number;
  world_tokens_deleted: number;
  world_events_deleted: number;
  policies_deleted: number;
  oauth_links_deleted: number;
  sessions_deleted: number;
  login_challenges_deleted: number;
  oauth_link_challenges_deleted: number;
  users_deleted: number;
}

export interface UserDataDeleteResponse {
  status: string;
  message: string;
  summary: UserDataDeleteSummary;
}
