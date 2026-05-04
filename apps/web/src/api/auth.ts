import type {
  AuthSessionResponse,
  AuthUser,
  LoginPayload,
  RegisterPayload,
  SetupStatus,
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

function withCsrf(headers: HeadersInit = {}): HeadersInit {
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
          user: payload.session.user,
        }
      : null,
  };
}

async function expectAuthResponse(response: Response): Promise<AuthSessionResponse> {
  const payload = normalizeAuthResponse(await readJson<AuthResponsePayload>(response));

  if (!response.ok) {
    throw new Error(payload.message || "Request failed");
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
