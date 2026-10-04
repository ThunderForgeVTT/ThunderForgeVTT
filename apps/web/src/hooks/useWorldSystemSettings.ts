import { useCallback, useEffect, useState } from "react";
import {
  fetchWorldSystemSettings,
  setWorldSystemSetting,
  watchWorldSystemSettings,
  type WorldSystemSetting,
  type WorldSystemSettingValue,
} from "@/api/worldSystemSettings";
import { useResetOnChange } from "@/hooks/useResetOnChange";

export interface WorldSystemSettingsHandle {
  /** Every setting the world's system declares, in form order. */
  settings: WorldSystemSetting[];
  state: "loading" | "ready" | "failed";
  /** The key of a write in flight, if there is one. */
  saving: string | null;
  /** Why the last write was refused, in the server's words. */
  refusal: string | null;
  /**
   * What this world plays by for one setting. `undefined` until the first
   * read lands, and for a key the system does not declare — a caller decides
   * what it shows before it knows.
   */
  valueOf: (key: string) => WorldSystemSettingValue | undefined;
  /** Game Master only; the server refuses anyone else. */
  set: (key: string, value: WorldSystemSettingValue) => Promise<void>;
}

/**
 * A world's system settings, kept current for whoever is looking (spec 067).
 *
 * One hook for the settings form and for a pack's own surfaces, so both read
 * the same values and hear the same event. It reads on mount and again each
 * time the Game Master changes a setting, so a sheet open at the table
 * follows the change without a reload.
 */
export function useWorldSystemSettings(
  worldId: string,
): WorldSystemSettingsHandle {
  const [settings, setSettings] = useState<WorldSystemSetting[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "failed">("loading");
  const [saving, setSaving] = useState<string | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);

  useResetOnChange(worldId, () => {
    setSettings([]);
    setState("loading");
    setRefusal(null);
  });

  useEffect(() => {
    let active = true;
    const read = () => {
      fetchWorldSystemSettings(worldId)
        .then((next) => {
          if (active) {
            setSettings(next);
            setState("ready");
          }
        })
        .catch(() => {
          if (active) {
            setState("failed");
          }
        });
    };
    read();
    const stop = watchWorldSystemSettings(worldId, read);
    return () => {
      active = false;
      stop();
    };
  }, [worldId]);

  const valueOf = useCallback(
    (key: string) => settings.find((setting) => setting.key === key)?.value,
    [settings],
  );

  const set = useCallback(
    async (key: string, value: WorldSystemSettingValue): Promise<void> => {
      setRefusal(null);
      setSaving(key);
      try {
        const stored = await setWorldSystemSetting(worldId, key, value);
        if (stored) {
          setSettings((current) =>
            current.map((setting) => (setting.key === key ? stored : setting)),
          );
        }
      } catch (thrown) {
        setRefusal(
          thrown instanceof Error ? thrown.message : "Something went wrong.",
        );
      } finally {
        setSaving(null);
      }
    },
    [worldId],
  );

  return { settings, state, saving, refusal, valueOf, set };
}
