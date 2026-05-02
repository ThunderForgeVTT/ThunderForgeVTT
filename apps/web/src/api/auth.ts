const SEPARATOR = "~UwU~";
const API_BASE = "/api/v1";

export interface SetupProvider {
  provider_key: string;
  display_name: string;
}

export interface SetupStatus {
  setup_required: boolean;
  setup_completed: boolean;
  configured_oauth_providers: SetupProvider[];
}

interface AuthResponse {
  status?: string;
  message?: string;
}

interface SetupOAuthStartResponse {
  authorization_url: string;
}

function encodeCredentials(username: string, password: string, id = ""): string {
  return btoa([id, username, password].join(SEPARATOR));
}

async function readAuthMessage(response: Response): Promise<string> {
  const contentType = response.headers.get("content-type") ?? "";

  if (contentType.includes("application/json")) {
    const payload = (await response.json()) as AuthResponse;
    return payload.message ?? payload.status ?? "Request completed";
  }

  return response.text();
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

export async function basicLogin(username: string, password: string): Promise<string> {
  const response = await fetch(`${API_BASE}/authentication/basic`, {
    method: "POST",
    credentials: "same-origin",
    body: encodeCredentials(username, password),
  });

  return readAuthMessage(response);
}

export async function basicSignUp(username: string, password: string): Promise<string> {
  // Server-side signup endpoint is not implemented yet; this preserves current behavior.
  return basicLogin(username, password);
}

export async function setupBasic(
  adminCode: string,
  username: string,
  email: string,
  password: string
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

  return readAuthMessage(response);
}

export async function startSetupOAuth(
  providerKey: string,
  adminCode: string,
  username?: string
): Promise<void> {
  const redirectUri = `${window.location.origin}${API_BASE}/authentication/setup/oauth/${providerKey}/callback`;
  const returnTo = `${window.location.origin}/setup/callback?oauth=success`;

  const response = await fetch(`${API_BASE}/authentication/setup/oauth/${providerKey}/start`, {
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
  });

  const contentType = response.headers.get("content-type") ?? "";
  if (!response.ok) {
    throw new Error(await readAuthMessage(response));
  }

  if (!contentType.includes("application/json")) {
    throw new Error("OAuth start response was not valid JSON");
  }

  const payload = (await response.json()) as SetupOAuthStartResponse;
  window.location.assign(payload.authorization_url);
}