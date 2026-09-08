import { Field } from "@/components/ui/field/Field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  requirementLabel,
  settingLabel,
  type RequiredSetting,
} from "@/services/instanceSetup";

/**
 * One registry declaration, drawn from its `kind`.
 *
 * Spec 040 FR-012 reaching the interface: a new declaration renders here
 * because its kind is one of these, not because somebody added a `<Field>` for
 * it. An unrecognised kind falls through to text rather than throwing — a
 * server newer than this bundle is a thing that happens on a rolling deploy,
 * and a blank screen would be a worse answer than a text box.
 *
 * # The error testid
 *
 * `Field` already emits `data-testid="field-error"`, but every field in the
 * wizard would then be indistinguishable from every other one. The error is
 * rendered here instead, as `setup-field-error` carrying `data-field`, so an
 * e2e can assert *which* field was refused. `Field`'s `error` prop is
 * deliberately unused for that reason; passing both drew the message twice.
 */
export interface SettingFieldProps {
  setting: RequiredSetting;
  value: string;
  error?: string;
  onChange: (key: string, value: string) => void;
}

const INPUT_CLASSES =
  "flex h-10 w-full rounded-lg border border-input bg-transparent px-2.5 py-2 text-base transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 md:text-sm";

export function SettingField({
  setting,
  value,
  error,
  onChange,
}: SettingFieldProps) {
  const id = `setup-setting-${setting.key}`;
  const testId = id;
  const label = settingLabel(setting.key);
  // A setting the instance has already stored, being shown again on a resumed
  // pass. The status endpoint reports that it is set without sending the
  // value, so the box is empty and the hint has to say what an empty box
  // means here (FR-006).
  const alreadyStored = setting.satisfied && !value;
  const hint = alreadyStored
    ? `This is already set on this instance. Leave it blank to keep it. ${setting.what_to_set ?? ""}`.trim()
    : (setting.what_to_set ?? undefined);

  const control = () => {
    if (setting.kind === "PROSE") {
      return (
        <Textarea
          data-testid={testId}
          id={id}
          name={setting.key}
          rows={4}
          value={value}
          onChange={(event) => onChange(setting.key, event.target.value)}
        />
      );
    }

    if (setting.kind === "ENUM") {
      const options = setting.options ?? [];
      return (
        <select
          data-testid={testId}
          id={id}
          name={setting.key}
          className={INPUT_CLASSES}
          value={value}
          onChange={(event) => onChange(setting.key, event.target.value)}
        >
          <option value="">Not set</option>
          {options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      );
    }

    if (setting.kind === "BOOL") {
      return (
        <label className="flex items-center gap-2 text-sm">
          <input
            data-testid={testId}
            id={id}
            name={setting.key}
            type="checkbox"
            className="size-4"
            checked={value === "true"}
            onChange={(event) =>
              onChange(setting.key, event.target.checked ? "true" : "false")
            }
          />
          <span className="text-muted-foreground">Enabled</span>
        </label>
      );
    }

    return (
      <Input
        data-testid={testId}
        id={id}
        name={setting.key}
        type={
          setting.secret
            ? "password"
            : setting.kind === "EMAIL"
              ? "email"
              : setting.kind === "PORT"
                ? "number"
                : "text"
        }
        autoComplete={setting.secret ? "new-password" : "off"}
        value={value}
        onChange={(event) => onChange(setting.key, event.target.value)}
      />
    );
  };

  return (
    <div className="grid gap-1" data-setting-key={setting.key}>
      <Field
        label={label}
        htmlFor={id}
        accent={requirementLabel(setting)}
        hint={error ? undefined : hint}
      >
        {control()}
      </Field>
      {error ? (
        <span
          data-testid="setup-field-error"
          data-field={setting.key}
          className="text-sm text-destructive"
        >
          {error}
        </span>
      ) : null}
    </div>
  );
}
