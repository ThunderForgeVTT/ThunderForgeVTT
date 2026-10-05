import { beforeEach, describe, expect, it } from "vitest";
import {
  ENGINE_STOPPED_EVENT,
  engineStopReason,
  resetEngineStoppedForTests,
  subscribeEngineStopped,
  watchEngineStopped,
} from "../engineStopped";

describe("engineStopped", () => {
  // Standing in for `window`: what matters is which events are heard.
  let page: EventTarget;

  beforeEach(() => {
    resetEngineStoppedForTests();
    page = new EventTarget();
    watchEngineStopped(page);
  });

  it("is null until something ends the engine", () => {
    expect(engineStopReason()).toBeNull();
  });

  it("hears the engine's panic, and tells whoever is listening", () => {
    let told = 0;
    subscribeEngineStopped(() => {
      told += 1;
    });
    page.dispatchEvent(new Event(ENGINE_STOPPED_EVENT));
    expect(engineStopReason()).toBe("crashed");
    expect(told).toBe(1);
  });

  it("hears the graphics context being taken away", () => {
    page.dispatchEvent(new Event("webglcontextlost"));
    expect(engineStopReason()).toBe("context-lost");
  });

  it("keeps the first reason, and says it once", () => {
    let told = 0;
    subscribeEngineStopped(() => {
      told += 1;
    });
    page.dispatchEvent(new Event(ENGINE_STOPPED_EVENT));
    page.dispatchEvent(new Event("webglcontextlost"));
    expect(engineStopReason()).toBe("crashed");
    expect(told).toBe(1);
  });

  it("is still stopped for a listener that arrives afterwards", () => {
    page.dispatchEvent(new Event(ENGINE_STOPPED_EVENT));
    const unsubscribe = subscribeEngineStopped(() => {});
    expect(engineStopReason()).toBe("crashed");
    unsubscribe();
  });

  it("no longer tells a listener that left", () => {
    let told = 0;
    subscribeEngineStopped(() => {
      told += 1;
    })();
    page.dispatchEvent(new Event(ENGINE_STOPPED_EVENT));
    expect(told).toBe(0);
  });
});
