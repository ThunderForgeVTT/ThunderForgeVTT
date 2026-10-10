import { describe, expect, it, vi } from "vitest";
import { installUnsavedGuard, leavesThePage } from "./useUnsavedChanges";

type Listener = (event: unknown) => void;

function fakeTarget() {
  const listeners = new Map<string, Listener>();
  return {
    listeners,
    addEventListener: (type: string, listener: (event: Event) => void) =>
      listeners.set(type, listener as Listener),
    removeEventListener: (type: string) => listeners.delete(type),
  };
}

const HERE = "https://vtt.test/admin/settings/mail";

function click(href: string | null, over: Record<string, unknown> = {}) {
  const anchor = href === null ? null : { href, target: "" };
  return {
    button: 0,
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    defaultPrevented: false,
    target: { closest: () => anchor },
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    ...over,
  };
}

describe("useUnsavedChanges", () => {
  it("a link to another page in the app leaves it; an anchor on this page does not", () => {
    expect(leavesThePage("https://vtt.test/admin/settings/legal", HERE)).toBe(
      true,
    );
    expect(
      leavesThePage("https://vtt.test/admin/settings/mail?x=1", HERE),
    ).toBe(true);
    expect(
      leavesThePage(
        "https://vtt.test/admin/settings/mail#setting-mail.host",
        HERE,
      ),
    ).toBe(false);
    // Another site is a full unload, which beforeunload already asks about.
    expect(leavesThePage("https://elsewhere.test/", HERE)).toBe(false);
  });

  it("asks before a reload or a closed tab", () => {
    const win = fakeTarget();
    const doc = fakeTarget();
    installUnsavedGuard(
      win,
      doc,
      "Leave?",
      () => true,
      () => HERE,
    );
    const event = {
      preventDefault: vi.fn(),
      returnValue: undefined as unknown,
    };
    win.listeners.get("beforeunload")!(event);
    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.returnValue).toBe("");
  });

  it("an in-app link asks first; staying stops the navigation, leaving lets it through", () => {
    const win = fakeTarget();
    const doc = fakeTarget();
    const confirm = vi.fn(() => false);
    installUnsavedGuard(win, doc, "Leave?", confirm, () => HERE);
    const stay = click("https://vtt.test/admin/settings/legal");
    doc.listeners.get("click")!(stay);
    expect(confirm).toHaveBeenCalledWith("Leave?");
    expect(stay.preventDefault).toHaveBeenCalled();
    expect(stay.stopPropagation).toHaveBeenCalled();

    confirm.mockReturnValue(true);
    const go = click("https://vtt.test/admin/settings/legal");
    doc.listeners.get("click")!(go);
    expect(go.preventDefault).not.toHaveBeenCalled();
  });

  it("leaves alone a click that is not a plain in-app navigation", () => {
    const win = fakeTarget();
    const doc = fakeTarget();
    const confirm = vi.fn(() => false);
    installUnsavedGuard(win, doc, "Leave?", confirm, () => HERE);
    for (const event of [
      click(null),
      click("https://vtt.test/x", { metaKey: true }),
      click("https://vtt.test/x", { button: 1 }),
      click("https://vtt.test/x", { defaultPrevented: true }),
      click("https://vtt.test/admin/settings/mail#setting-mail.port"),
    ]) {
      doc.listeners.get("click")!(event);
    }
    expect(confirm).not.toHaveBeenCalled();
  });

  it("removes both listeners when it is cleaned up", () => {
    const win = fakeTarget();
    const doc = fakeTarget();
    const remove = installUnsavedGuard(
      win,
      doc,
      "Leave?",
      () => true,
      () => HERE,
    );
    remove();
    expect(win.listeners.size).toBe(0);
    expect(doc.listeners.size).toBe(0);
  });
});
