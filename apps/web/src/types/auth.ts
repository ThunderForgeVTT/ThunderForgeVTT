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
