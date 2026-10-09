/**
 * The bounded queue (FR-020). It holds at most 200 records and drops the
 * oldest when full, counting each drop. Nothing here waits or retries, so a
 * dead endpoint can never grow it.
 */

import type { TelemetryRecord } from "./records.ts";

export const QUEUE_CAPACITY = 200;

export class BoundedQueue {
  readonly capacity: number;
  private items: TelemetryRecord[] = [];
  dropped = 0;

  constructor(capacity = QUEUE_CAPACITY) {
    this.capacity = capacity;
  }

  push(record: TelemetryRecord): void {
    if (this.items.length >= this.capacity) {
      this.items.shift();
      this.dropped += 1;
    }
    this.items.push(record);
  }

  has(record: TelemetryRecord): boolean {
    return this.items.includes(record);
  }

  get length(): number {
    return this.items.length;
  }

  drain(): TelemetryRecord[] {
    const out = this.items;
    this.items = [];
    return out;
  }
}
