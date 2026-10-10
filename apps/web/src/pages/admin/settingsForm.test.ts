import { describe, expect, it } from "vitest";
import {
  applyResults,
  baselineOf,
  canSave,
  clear,
  dirtyKeys,
  discard,
  draftOf,
  edit,
  emptyForm,
  rebase,
  savedNotice,
  sendValue,
  type Baseline,
} from "./settingsForm";
import type { ResolvedSetting } from "@/api/instanceSettings";

const ORDER = [
  "mail.enabled",
  "mail.host",
  "mail.port",
  "mail.password",
  "mail.from_address",
];

function baseline(): Baseline {
  return {
    "mail.enabled": { value: "false", secret: false, fixed: false },
    "mail.host": { value: "smtp.example.test", secret: false, fixed: false },
    "mail.port": { value: "587", secret: false, fixed: true },
    "mail.password": { value: null, secret: true, fixed: false, set: false },
    "mail.from_address": { value: null, secret: false, fixed: false },
  };
}

describe("settingsForm", () => {
  it("reads a baseline from the server's settings: secrets and fixed keys", () => {
    const setting = (over: Partial<ResolvedSetting>): ResolvedSetting =>
      ({
        key: "k",
        kind: "STRING",
        enumOptions: [],
        value: null,
        secretState: null,
        source: "DEFAULT",
        fixedBy: null,
        editable: true,
        requirement: "OPTIONAL",
        capability: null,
        whatToSet: "",
        group: "Mail",
        undecryptable: false,
        ...over,
      }) as ResolvedSetting;
    const read = baselineOf([
      setting({ key: "mail.host", value: "h" }),
      setting({ key: "mail.password", secretState: "SET" }),
      setting({ key: "mail.port", value: "25", editable: false }),
    ]);
    expect(read["mail.host"]).toEqual({
      value: "h",
      secret: false,
      fixed: false,
    });
    expect(read["mail.password"]).toEqual({
      value: null,
      secret: true,
      fixed: false,
      set: true,
    });
    expect(read["mail.port"].fixed).toBe(true);
  });

  it("is clean as loaded, and Save is off", () => {
    const state = emptyForm(baseline());
    expect(dirtyKeys(state, ORDER)).toEqual([]);
    expect(canSave(state)).toBe(false);
    expect(draftOf(state, "mail.host")).toBe("smtp.example.test");
    expect(draftOf(state, "mail.password")).toBe("");
  });

  it("compares after trimming, as the server does, and a change back is clean", () => {
    let state = edit(
      emptyForm(baseline()),
      "mail.host",
      "  smtp.example.test ",
    );
    expect(dirtyKeys(state, ORDER)).toEqual([]);
    state = edit(state, "mail.host", "mail.example.test");
    expect(dirtyKeys(state, ORDER)).toEqual(["mail.host"]);
    expect(sendValue(state, "mail.host")).toBe("mail.example.test");
    state = edit(state, "mail.host", "smtp.example.test");
    expect(dirtyKeys(state, ORDER)).toEqual([]);
  });

  it("keeps the draft as typed, and sends it trimmed", () => {
    const state = edit(
      emptyForm(baseline()),
      "mail.from_address",
      " a@b.test ",
    );
    expect(draftOf(state, "mail.from_address")).toBe(" a@b.test ");
    expect(sendValue(state, "mail.from_address")).toBe("a@b.test");
  });

  it("a secret is dirty only when typed into, and clearing it sends null", () => {
    let state = edit(emptyForm(baseline()), "mail.password", "");
    expect(dirtyKeys(state, ORDER)).toEqual([]);
    state = edit(state, "mail.password", " hunter2 ");
    expect(dirtyKeys(state, ORDER)).toEqual(["mail.password"]);
    expect(sendValue(state, "mail.password")).toBe(" hunter2 ");

    const set = baseline();
    set["mail.password"] = {
      value: null,
      secret: true,
      fixed: false,
      set: true,
    };
    const cleared = clear(emptyForm(set), "mail.password");
    expect(dirtyKeys(cleared, ORDER)).toEqual(["mail.password"]);
    expect(sendValue(cleared, "mail.password")).toBeNull();
    // Clearing what is not set is no change.
    expect(
      dirtyKeys(clear(emptyForm(baseline()), "mail.password"), ORDER),
    ).toEqual([]);
    expect(
      dirtyKeys(clear(emptyForm(baseline()), "mail.from_address"), ORDER),
    ).toEqual([]);
  });

  it("a key fixed by the environment is never editable or dirty", () => {
    let state = edit(emptyForm(baseline()), "mail.port", "2525");
    state = clear(state, "mail.port");
    expect(dirtyKeys(state, ORDER)).toEqual([]);
    expect(draftOf(state, "mail.port")).toBe("587");
  });

  it("lists dirty keys in form order, with mail.enabled last", () => {
    let state = emptyForm(baseline());
    state = edit(state, "mail.enabled", "true");
    state = edit(state, "mail.from_address", "a@b.test");
    state = edit(state, "mail.host", "x");
    expect(dirtyKeys(state, ORDER)).toEqual([
      "mail.host",
      "mail.from_address",
      "mail.enabled",
    ]);
  });

  it("a client error holds Save off until it is fixed", () => {
    const validate = (value: string) =>
      value.includes("@") ? null : "An address needs an @.";
    let state = edit(
      emptyForm(baseline()),
      "mail.from_address",
      "nope",
      validate,
    );
    expect(state.errors["mail.from_address"]).toBe("An address needs an @.");
    expect(canSave(state)).toBe(false);
    state = edit(state, "mail.from_address", "a@b.test", validate);
    expect(state.errors["mail.from_address"]).toBeUndefined();
    expect(canSave(state)).toBe(true);
  });

  it("discard drops every change and error", () => {
    let state = edit(emptyForm(baseline()), "mail.host", "x");
    state = edit(state, "mail.from_address", "bad", () => "bad");
    state = discard(state);
    expect(dirtyKeys(state, ORDER)).toEqual([]);
    expect(state.errors).toEqual({});
  });

  it("partial results: saved keys become the baseline, a refused key stays dirty with its error", () => {
    let state = emptyForm(baseline());
    state = edit(state, "mail.host", "a.test");
    state = edit(state, "mail.from_address", "a@b.test");
    state = edit(state, "mail.password", "pw");
    const results = [
      { key: "mail.host", saved: true as const },
      { key: "mail.from_address", saved: false as const, error: "Refused." },
      { key: "mail.password", saved: true as const },
    ];
    state = applyResults(state, results);
    expect(dirtyKeys(state, ORDER)).toEqual(["mail.from_address"]);
    expect(state.refused["mail.from_address"]).toBe("Refused.");
    expect(state.baseline["mail.host"].value).toBe("a.test");
    expect(state.baseline["mail.password"].set).toBe(true);
    expect(draftOf(state, "mail.password")).toBe("");
    expect(savedNotice(results)).toBe("2 of 3 saved");
    // A refusal does not hold Save off; editing the key clears it.
    expect(canSave(state)).toBe(true);
    state = edit(state, "mail.from_address", "c@d.test");
    expect(state.refused["mail.from_address"]).toBeUndefined();
  });

  it("rebase keeps only the keys still dirty against the fresh settings", () => {
    let state = emptyForm(baseline());
    state = edit(state, "mail.host", "new.test");
    state = edit(state, "mail.from_address", "a@b.test");
    const fresh = baseline();
    fresh["mail.host"] = { value: "new.test", secret: false, fixed: false };
    fresh["mail.from_address"] = {
      value: "a@b.test",
      secret: false,
      fixed: true,
    };
    state = edit(state, "mail.enabled", "true");
    state = rebase(state, fresh);
    // The host now matches; the address became fixed; enabled still differs.
    expect(dirtyKeys(state, ORDER)).toEqual(["mail.enabled"]);
    expect(state.baseline).toBe(fresh);
  });
});
