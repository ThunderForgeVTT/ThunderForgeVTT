const SEPARATOR = "~UwU~";

function encodeCredentials(username: string, password: string, id = ""): string {
  return btoa([id, username, password].join(SEPARATOR));
}

export async function basicLogin(username: string, password: string): Promise<string> {
  const response = await fetch("/api/v1/authentication/basic", {
    method: "POST",
    body: encodeCredentials(username, password),
  });

  return response.text();
}

export async function basicSignUp(username: string, password: string): Promise<string> {
  // Server-side signup endpoint is not implemented yet; this preserves current behavior.
  return basicLogin(username, password);
}