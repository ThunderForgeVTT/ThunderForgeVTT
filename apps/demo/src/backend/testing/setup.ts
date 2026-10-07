/**
 * The little of a browser the backend touches as it loads: storage that
 * keeps nothing, and a page address. Each test file starts from the seed.
 */
Object.assign(globalThis, {
  window: {
    addEventListener() {},
    localStorage: { getItem: () => null, setItem() {}, removeItem() {} },
    location: { href: "http://demo.test/demo/", origin: "http://demo.test" },
  },
});
