/**
 * Times in the chat feed. Chat messages arrive as the server's zone-less UTC
 * time (`2026-10-07T12:00:00.123456`), rolls as RFC 3339 with a zone; a
 * zone-less time read as local would sort and show hours off, so it is read
 * as UTC here, once, for both.
 */
const HAS_ZONE = /(Z|[+-]\d{2}:?\d{2})$/;

export function feedInstant(time: string): number {
  return Date.parse(HAS_ZONE.test(time) ? time : `${time}Z`);
}

export function feedTime(time: string): string {
  return new Date(feedInstant(time)).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}
