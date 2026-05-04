import type {
  AuthSessionResponse,
  AuthUser,
  LoginPayload,
  OAuthActionResponse,
  RegisterPayload,
  SetupStatus,
  UserDataDeleteResponse,
} from "@/types/auth";

const API_BASE = "";

export interface SetupProvider {
  provider_key: string;
  display_name: string;
}

interface AuthResponsePayload {
  status?: string;
  message?: string;
  login_two_factor_challenge_id?: string | null;
  requires_email_verification?: boolean;
  session?: {
    authenticated: boolean;
    session_expires_at: string;
    user: AuthUser;
  } | null;
}

interface SetupOAuthStartResponse {
  authorization_url: string;
}

interface OAuthResponsePayload {
  status?: string;
  message?: string;
  challenge_id?: string | null;
  login_two_factor_challenge_id?: string | null;
}

const OAUTH_RETURN_TO_STORAGE_KEY = "thunderforge.oauth.returnTo";

const SEPARATOR = "~UwU~";

function encodeCredentials(username: string, password: string, id = ""): string {
  return btoa([id, username, password].join(SEPARATOR));
}

function readCookie(name: string): string | null {
  if (typeof document === "undefined") {
    return null;
  }

  const prefix = `${name}=`;
  return document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(prefix))
    ?.slice(prefix.length) ?? null;
}

export function withCsrf(headers: HeadersInit = {}): HeadersInit {
  const csrfToken = readCookie("csrf_token");
  if (!csrfToken) {
    return headers;
  }

  return {
    ...headers,
    "x-csrf-token": csrfToken,
  };
}

async function readJson<T>(response: Response): Promise<T | null> {
  const contentType = response.headers.get("content-type") ?? "";
  if (!contentType.includes("application/json")) {
    return null;
  }

  return (await response.json()) as T;
}

function normalizeAuthResponse(payload: AuthResponsePayload | null): AuthSessionResponse {
  return {
    status: payload?.status ?? "unknown",
    message: payload?.message ?? "Request completed",
    loginTwoFactorChallengeId: payload?.login_two_factor_challenge_id ?? null,
    requiresEmailVerification: payload?.requires_email_verification ?? false,
      session: payload?.session
        ? {
            authenticated: payload.session.authenticated,
            sessionExpiresAt: payload.session.session_expires_at,
            user: {
              ...payload.session.user,
              role:
                payload.session.user.role ??
                (payload.session.user.is_admin ? "admin" : "user"),
            },
          }
        : null,
  };
}

function normalizeOAuthResponse(payload: OAuthResponsePayload | null): OAuthActionResponse {
  return {
    status: payload?.status ?? "unknown",
    message: payload?.message ?? "Request completed",
    challengeId: payload?.challenge_id ?? null,
    loginTwoFactorChallengeId: payload?.login_two_factor_challenge_id ?? null,
  };
}

async function expectAuthResponse(response: Response): Promise<AuthSessionResponse> {
  const payload = normalizeAuthResponse(await readJson<AuthResponsePayload>(response));

  if (!response.ok) {
    if (payload.status === "two_factor_required") {
      return payload;
    }

    const error = new Error(payload.message || "Request failed");
    (error as Error & { response?: AuthSessionResponse }).response = payload;
    throw error;
  }

  return payload;
}

async function expectOAuthResponse(response: Response): Promise<OAuthActionResponse> {
  const payload = normalizeOAuthResponse(await readJson<OAuthResponsePayload>(response));

  if (!response.ok) {
    const error = new Error(payload.message || "Request failed");
    (error as Error & { response?: OAuthActionResponse }).response = payload;
    throw error;
  }

  return payload;
}

async function expectDeleteResponse(response: Response): Promise<UserDataDeleteResponse> {
  const payload = await readJson<UserDataDeleteResponse>(response);

  if (!response.ok || !payload) {
    throw new Error(payload?.message || "Delete request failed");
  }

  return payload;
}

export async function getSetupStatus(): Promise<SetupStatus> {
  const response = await fetch(`${API_BASE}/authentication/setup/status`, {
    credentials: "same-origin",
  });

  if (!response.ok) {
    throw new Error("Failed to load setup status");
  }

  return response.json() as Promise<SetupStatus>;
}

export function login(payload: LoginPayload): Promise<AuthSessionResponse> {
  return fetch(`${API_BASE}/authentication/login`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  }).then(expectAuthResponse);
}

export function register(payload: RegisterPayload): Promise<AuthSessionResponse> {
  return fetch(`${API_BASE}/authentication/register`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(payload),
  }).then(expectAuthResponse);
}

export function logout(): Promise<AuthSessionResponse> {
  return fetch(`${API_BASE}/authentication/logout`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
  }).then(expectAuthResponse);
}

export function refresh(): Promise<AuthSessionResponse> {
  return fetch(`${API_BASE}/authentication/session/refresh`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
  }).then(expectAuthResponse);
}

export function getCurrentSession(): Promise<AuthSessionResponse> {
  return fetch(`${API_BASE}/authentication/session`, {
    credentials: "same-origin",
  }).then(expectAuthResponse);
}

export function startOAuthLogin(providerKey: string, returnTo = "/counter"): void {
  if (typeof window === "undefined") {
    return;
  }

  window.sessionStorage.setItem(OAUTH_RETURN_TO_STORAGE_KEY, returnTo);
  const redirectUri = `${window.location.origin}/oauth/callback/${providerKey}`;
  const absoluteReturnTo = new URL(returnTo, window.location.origin).toString();
  const startUrl = new URL(
    `${API_BASE}/authentication/oauth/${providerKey}/start`,
    window.location.origin,
  );
  startUrl.searchParams.set("redirect_uri", redirectUri);
  startUrl.searchParams.set("return_to", absoluteReturnTo);
  window.location.assign(startUrl.toString());
}

export function consumeOAuthReturnTo(fallback = "/welcome"): string {
  if (typeof window === "undefined") {
    return fallback;
  }

  const stored = window.sessionStorage.getItem(OAUTH_RETURN_TO_STORAGE_KEY);
  window.sessionStorage.removeItem(OAUTH_RETURN_TO_STORAGE_KEY);
  return stored && stored.startsWith("/") ? stored : fallback;
}

export function exchangeOAuthCode(
  providerKey: string,
  code: string,
  state: string,
): Promise<OAuthActionResponse> {
  return fetch(`${API_BASE}/authentication/oauth/${providerKey}/token`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ code, state }),
  }).then(expectOAuthResponse);
}

export function confirmOAuthLink(
  challengeId: string,
  password: string,
): Promise<OAuthActionResponse> {
  return fetch(`${API_BASE}/authentication/oauth/link/confirm`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      challenge_id: challengeId,
      password,
    }),
  }).then(expectOAuthResponse);
}

export function verifyTwoFactor(
  challengeId: string,
  code: string,
): Promise<OAuthActionResponse> {
  return fetch(`${API_BASE}/authentication/2fa/verify`, {
    method: "POST",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
    body: JSON.stringify({
      challenge_id: challengeId,
      code,
    }),
  }).then(expectOAuthResponse);
}

export async function exportUserData(format: "json" | "zip" = "json"): Promise<Blob> {
  const response = await fetch(`${API_BASE}/user/data/export?format=${format}`, {
    credentials: "same-origin",
  });

  if (!response.ok) {
    const payload = await readJson<{ message?: string }>(response);
    throw new Error(payload?.message || "Export request failed");
  }

  return response.blob();
}

export function deleteUserData(): Promise<UserDataDeleteResponse> {
  return fetch(`${API_BASE}/user/data`, {
    method: "DELETE",
    credentials: "same-origin",
    headers: withCsrf({
      "Content-Type": "application/json",
    }),
  }).then(expectDeleteResponse);
}

export function basicLogin(username: string, password: string): Promise<string> {
  return fetch(`${API_BASE}/authentication/basic`, {
    method: "POST",
    credentials: "same-origin",
    body: encodeCredentials(username, password),
  }).then(async (response) => normalizeAuthResponse(await readJson(response)).message);
}

export function basicSignUp(username: string, password: string): Promise<string> {
  return basicLogin(username, password);
}

export async function setupBasic(
  adminCode: string,
  username: string,
  email: string,
  password: string,
): Promise<string> {
  const response = await fetch(`${API_BASE}/authentication/setup/basic`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      admin_code: adminCode,
      username,
      email,
      password,
    }),
  });

  const payload = normalizeAuthResponse(await readJson<AuthResponsePayload>(response));
  if (!response.ok) {
    throw new Error(payload.message);
  }

  return payload.message;
}

export async function startSetupOAuth(
  providerKey: string,
  adminCode: string,
  username?: string,
): Promise<void> {
  const redirectUri = `${window.location.origin}${API_BASE}/authentication/setup/oauth/${providerKey}/callback`;
  const returnTo = `${window.location.origin}/setup/callback?oauth=success`;

  const response = await fetch(
    `${API_BASE}/authentication/setup/oauth/${providerKey}/start`,
    {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        admin_code: adminCode,
        redirect_uri: redirectUri,
        username: username?.trim() ? username.trim() : undefined,
        return_to: returnTo,
      }),
    },
  );

  if (!response.ok) {
    const payload = normalizeAuthResponse(await readJson<AuthResponsePayload>(response));
    throw new Error(payload.message);
  }

  const payload = await readJson<SetupOAuthStartResponse>(response);
  if (!payload) {
    throw new Error("OAuth start response was not valid JSON");
  }

  window.location.assign(payload.authorization_url);
}
