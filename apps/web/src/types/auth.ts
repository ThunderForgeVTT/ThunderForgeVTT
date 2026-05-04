export interface SetupProvider {
  provider_key: string;
  display_name: string;
}

export interface SetupStatus {
  setup_required: boolean;
  setup_completed: boolean;
  configured_oauth_providers: SetupProvider[];
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
