/**
 * The coarse device facts a browser resource carries (FR-019): a family and
 * a major version, an OS family, and buckets. Never the user agent itself.
 * Pure, so `node --test` runs it with plain objects.
 */

import type { AttrValue } from "./allowList.ts";

export interface DeviceFacts {
  userAgent?: string;
  /** `navigator.userAgentData`, where the browser has it. */
  brands?: { brand: string; version: string }[];
  platform?: string;
  mobile?: boolean;
  width?: number;
  deviceMemory?: number;
  hardwareConcurrency?: number;
}

const FAMILIES: [RegExp, string][] = [
  [/Edg\/(\d+)/, "edge"],
  [/OPR\/(\d+)/, "opera"],
  [/SamsungBrowser\/(\d+)/, "samsung"],
  [/Firefox\/(\d+)/, "firefox"],
  [/Chrome\/(\d+)/, "chrome"],
  [/Version\/(\d+)[^ ]* .*Safari\//, "safari"],
];

export function browserOf(f: DeviceFacts): { family: string; major: string } {
  const brands = f.brands ?? [];
  for (const [name, family] of [
    ["Microsoft Edge", "edge"],
    ["Opera", "opera"],
    ["Google Chrome", "chrome"],
    ["Chromium", "chrome"],
  ] as const) {
    const b = brands.find((x) => x.brand === name);
    if (b && /^\d+/.test(b.version)) {
      return { family, major: /^\d+/.exec(b.version)![0] };
    }
  }
  const ua = f.userAgent ?? "";
  for (const [re, family] of FAMILIES) {
    const m = re.exec(ua);
    if (m) return { family, major: m[1] };
  }
  return { family: "other", major: "unknown" };
}

export function osOf(f: DeviceFacts): string {
  const p = `${f.platform ?? ""} ${f.userAgent ?? ""}`;
  if (/CrOS|Chrome OS/i.test(p)) return "chromeos";
  if (/Android/i.test(p)) return "android";
  if (/iPhone|iPad|iPod|iOS/i.test(p)) return "ios";
  if (/Windows|Win32|Win64/i.test(p)) return "windows";
  if (/Mac/i.test(p)) return "macos";
  if (/Linux|X11/i.test(p)) return "linux";
  return "other";
}

export function viewportBucket(width: number | undefined): string {
  if (width === undefined || !Number.isFinite(width)) return "unknown";
  if (width < 480) return "<480";
  if (width < 768) return "<768";
  if (width < 1024) return "<1024";
  if (width < 1440) return "<1440";
  if (width < 1920) return "<1920";
  return ">=1920";
}

export function memoryBucket(gb: number | undefined): string {
  if (gb === undefined || !Number.isFinite(gb)) return "unknown";
  if (gb <= 1) return "<=1";
  if (gb < 4) return "2";
  if (gb < 8) return "4";
  return ">=8";
}

export function coresBucket(n: number | undefined): string {
  if (n === undefined || !Number.isFinite(n) || n <= 0) return "unknown";
  if (n <= 2) return "<=2";
  if (n <= 4) return "4";
  if (n < 12) return "8";
  return ">=12";
}

/** The device part of a browser resource. */
export function deviceAttributes(f: DeviceFacts): Record<string, AttrValue> {
  const b = browserOf(f);
  const os = osOf(f);
  return {
    "browser.family": b.family,
    "browser.major": b.major,
    "os.family": os,
    "device.mobile":
      f.mobile ??
      (os === "android" || os === "ios" || /Mobi/.test(f.userAgent ?? "")),
    "viewport.bucket": viewportBucket(f.width),
    "device.memory.bucket": memoryBucket(f.deviceMemory),
    "device.cores.bucket": coresBucket(f.hardwareConcurrency),
  };
}
