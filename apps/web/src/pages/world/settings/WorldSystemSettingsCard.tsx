import { useState } from "react";
import type {
  WorldSystemSetting,
  WorldSystemSettingValue,
} from "@/api/worldSystemSettings";
import { Card } from "@/components/ui/card/Card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { StatusBadge } from "@/components/ui/status-badge/StatusBadge";
import { Switch } from "@/components/ui/switch";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { useWorldSystemSettings } from "@/hooks/useWorldSystemSettings";

export interface WorldSystemSettingsCardProps {
  worldId: string;
  isGm: boolean;
}

/**
 * The settings a world's game system declares, as a form (spec 067 FR-010).
 *
 * Built entirely from what `worldSystemSettings` answers, so this file names
 * no system and a pack that declares a setting gets its control here without
 * writing one. The Game Master changes a setting; everyone else reads what
 * the table plays by. Renders nothing for a system that declares none.
 */
export function WorldSystemSettingsCard({
  worldId,
  isGm,
}: WorldSystemSettingsCardProps) {
  const { settings, saving, refusal, set } = useWorldSystemSettings(worldId);

  if (settings.length === 0) {
    return null;
  }

  return (
    <Card className="grid gap-4 p-6" data-testid="world-system-settings-card">
      <div>
        <h3 className="text-lg font-semibold">How this table plays</h3>
        <p className="text-sm text-muted-foreground">
          {isGm
            ? "The choices this game system leaves to the table. A change reaches everyone at once."
            : "The choices this game system leaves to the table, as your Game Master has set them."}
        </p>
      </div>

      <ul className="grid gap-4">
        {settings.map((setting) => (
          <li
            key={setting.key}
            className="flex flex-wrap items-center justify-between gap-3"
            data-testid={`world-system-setting-row-${setting.key}`}
          >
            <div className="grid max-w-prose gap-1">
              <label
                htmlFor={controlId(setting.key)}
                className="text-sm font-medium"
              >
                {setting.label}
              </label>
              {setting.description ? (
                <p className="text-sm text-muted-foreground">
                  {setting.description}
                </p>
              ) : null}
            </div>
            <SettingControl
              setting={setting}
              disabled={!isGm || saving !== null}
              onChange={(value) => void set(setting.key, value)}
            />
          </li>
        ))}
      </ul>

      {refusal ? (
        <StatusBadge
          variant="danger"
          data-testid="world-system-settings-refusal"
        >
          {refusal}
        </StatusBadge>
      ) : null}
    </Card>
  );
}

function controlId(key: string): string {
  return `world-system-setting-${key}`;
}

function SettingControl({
  setting,
  disabled,
  onChange,
}: {
  setting: WorldSystemSetting;
  disabled: boolean;
  onChange: (value: WorldSystemSettingValue) => void;
}) {
  const id = controlId(setting.key);

  switch (setting.kind) {
    case "boolean":
      return (
        <Switch
          id={id}
          data-testid={id}
          checked={setting.value === true}
          disabled={disabled}
          onCheckedChange={onChange}
        />
      );
    case "choice": {
      const chosen = String(setting.value);
      return (
        <Select value={chosen} onValueChange={onChange} disabled={disabled}>
          <SelectTrigger id={id} data-testid={id} className="w-56">
            {/* Spelled out so the closed trigger reads the stored value. */}
            <SelectValue>
              {setting.options.find((option) => option.value === chosen)
                ?.label ?? chosen}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            {setting.options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      );
    }
    case "integer":
    case "text":
      return (
        <DraftControl
          setting={setting}
          disabled={disabled}
          onChange={onChange}
        />
      );
  }
}

/**
 * A typed value is sent when the field is left or Enter is pressed, not per
 * keystroke: every send is a change the whole table hears.
 */
function DraftControl({
  setting,
  disabled,
  onChange,
}: {
  setting: WorldSystemSetting;
  disabled: boolean;
  onChange: (value: WorldSystemSettingValue) => void;
}) {
  const id = controlId(setting.key);
  const stored = String(setting.value);
  const [draft, setDraft] = useState(stored);
  useResetOnChange(stored, () => setDraft(stored));

  const commit = () => {
    if (draft === stored) {
      return;
    }
    if (setting.kind === "text") {
      onChange(draft);
      return;
    }
    const number = Number(draft);
    if (draft.trim() === "" || !Number.isInteger(number)) {
      // Not a whole number: put back what the world plays by rather than
      // send something the server can only refuse.
      setDraft(stored);
      return;
    }
    onChange(number);
  };

  return (
    <Input
      id={id}
      data-testid={id}
      className={setting.kind === "integer" ? "w-24" : "w-56"}
      type={setting.kind === "integer" ? "number" : "text"}
      min={setting.min ?? undefined}
      max={setting.max ?? undefined}
      maxLength={setting.maxLength ?? undefined}
      value={draft}
      disabled={disabled}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          commit();
        }
      }}
    />
  );
}
