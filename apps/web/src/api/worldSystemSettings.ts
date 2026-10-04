/**
 * A world's answers to the settings its game system declares (spec 067).
 *
 * The declaration and the value arrive together, so the form on the world's
 * settings page is built from this one read and no shared file knows which
 * system it is showing. A pack reads a setting through
 * `useWorldSystemSettings`, exported from `@thunderforge/host`.
 */
import { postGraphQL } from "@/api/graphqlClient";
import {
  subscribeToWorldEvents,
  type WorldEventLike,
} from "@/engine/world/sync";

export type WorldSystemSettingKind = "boolean" | "integer" | "choice" | "text";

/** What a setting may hold: on or off, a whole number, or text. */
export type WorldSystemSettingValue = boolean | number | string;

export interface WorldSystemSetting {
  key: string;
  label: string;
  description: string | null;
  kind: WorldSystemSettingKind;
  options: { value: string; label: string }[];
  min: number | null;
  max: number | null;
  maxLength: number | null;
  defaultValue: WorldSystemSettingValue;
  value: WorldSystemSettingValue;
  /** True when nothing usable is stored and `value` is the default. */
  isDefault: boolean;
}

const SETTING_FIELDS = `
  key
  label
  description
  kind
  options {
    value
    label
  }
  min
  max
  maxLength
  defaultValue
  value
  isDefault
`;

const WORLD_SYSTEM_SETTINGS_QUERY = `
  query WorldSystemSettings($worldId: UUID!) {
    worldSystemSettings(worldId: $worldId) {
      ${SETTING_FIELDS}
    }
  }
`;

const SET_WORLD_SYSTEM_SETTING_MUTATION = `
  mutation SetWorldSystemSetting($worldId: UUID!, $key: String!, $value: JSON!) {
    setWorldSystemSetting(worldId: $worldId, key: $key, value: $value) {
      ${SETTING_FIELDS}
    }
  }
`;

const KINDS: readonly WorldSystemSettingKind[] = [
  "boolean",
  "integer",
  "choice",
  "text",
];

interface SettingPayload extends Omit<WorldSystemSetting, "kind"> {
  kind: string;
}

/**
 * A type this build cannot render — a newer server — is left out rather than
 * drawn as the wrong control. The world still plays by it; this client just
 * cannot offer to change it.
 */
export function knownSettings(
  payload: readonly SettingPayload[],
): WorldSystemSetting[] {
  return payload.flatMap((setting) => {
    const kind = KINDS.find((known) => known === setting.kind);
    return kind ? [{ ...setting, kind }] : [];
  });
}

/** Any member of the world. Throws on refusal. */
export async function fetchWorldSystemSettings(
  worldId: string,
): Promise<WorldSystemSetting[]> {
  const { worldSystemSettings } = await postGraphQL<{
    worldSystemSettings: SettingPayload[];
  }>(WORLD_SYSTEM_SETTINGS_QUERY, { worldId });
  return knownSettings(worldSystemSettings);
}

/** Game Master only; the server refuses anyone else and any value the
 * declaration does not allow. Answers with the setting as stored. */
export async function setWorldSystemSetting(
  worldId: string,
  key: string,
  value: WorldSystemSettingValue,
): Promise<WorldSystemSetting | null> {
  const { setWorldSystemSetting: stored } = await postGraphQL<{
    setWorldSystemSetting: SettingPayload;
  }>(SET_WORLD_SYSTEM_SETTING_MUTATION, { worldId, key, value });
  return knownSettings([stored])[0] ?? null;
}

/** The server's code for "a system setting of this world changed". */
export const WORLD_SYSTEM_SETTING_CHANGED = 35;

function codeOf(event: WorldEventLike): number | undefined {
  return event.event_code ?? event.eventCode;
}

/**
 * Call `onChanged` whenever the Game Master changes a setting. Returns the
 * function that stops watching.
 *
 * The event says only that something changed; the listener reads again. The
 * stream never routes the page on a pause: settings are read on pages that
 * stay open while play is paused.
 */
export function watchWorldSystemSettings(
  worldId: string,
  onChanged: () => void,
): () => void {
  const iterator = subscribeToWorldEvents(worldId, { announcePause: false })[
    Symbol.asyncIterator
  ]();
  let stopped = false;

  void (async () => {
    try {
      while (!stopped) {
        const { value: event, done } = await iterator.next();
        if (done || stopped || !event) {
          break;
        }
        if (codeOf(event) === WORLD_SYSTEM_SETTING_CHANGED) {
          onChanged();
        }
      }
    } catch (error) {
      console.error("World system settings sync error:", error);
    }
  })();

  return () => {
    stopped = true;
    void iterator.return?.();
  };
}
