const FALLBACK_SITE_URL = "https://example.com";
const FALLBACK_OG_IMAGE = "/social-card.svg";

function normalizeBaseUrl(url: string): string {
  return url.endsWith("/") ? url : `${url}/`;
}

export function getSiteUrl(): string {
  const configuredUrl = import.meta.env.VITE_SITE_URL;

  if (configuredUrl) {
    return normalizeBaseUrl(configuredUrl);
  }

  if (typeof window !== "undefined") {
    return normalizeBaseUrl(window.location.origin);
  }

  return normalizeBaseUrl(FALLBACK_SITE_URL);
}

export function toAbsoluteUrl(urlOrPath: string): string {
  if (/^https?:\/\//.test(urlOrPath)) {
    return urlOrPath;
  }

  return new URL(urlOrPath, getSiteUrl()).toString();
}

export function getDefaultOgImage(): string {
  return import.meta.env.VITE_DEFAULT_OG_IMAGE ?? FALLBACK_OG_IMAGE;
}
