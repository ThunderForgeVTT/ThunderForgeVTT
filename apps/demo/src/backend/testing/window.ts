/**
 * The little of `window` the demo backend touches, for Vitest's Node.
 *
 * Imported first by a test, before anything that reads `window` when it
 * loads: storage that holds nothing, and events that go nowhere.
 */
Object.assign(globalThis, {
  window: {
    addEventListener() {},
    dispatchEvent() {
      return true;
    },
    localStorage: {
      getItem: () => null,
      setItem() {},
      removeItem() {},
    },
    location: {
      href: "http://demo.test/demo/",
      origin: "http://demo.test",
    },
  },
});

export {};
