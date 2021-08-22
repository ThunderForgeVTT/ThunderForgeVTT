import { set } from "lodash";
import { FrontendLogger, LoaderLogger } from "./engine/utils/logger";

import("./styles/main.scss").then(() => {
  LoaderLogger.debug("Loaded Styles...");
});

window.loadPIXI = function () {};

window.logger = FrontendLogger;

import("./engine")
  .then(async (engine) => set(window, "loadPIXI", () => engine.load()))
  .then(() =>
    import("../pkg")
      .then((client) => {
        client.main();
        LoaderLogger.debug("Loaded Client...");
      })
      .then(() => {
        window.loadPIXI();
      })
  );

if ("serviceWorker" in navigator) {
  window.addEventListener("load", () => {
    navigator.serviceWorker
      .register("/service-worker.js")
      .then((registration) => {
        LoaderLogger.info("SW registered: ", registration);
      })
      .catch((registrationError) => {
        LoaderLogger.info("SW registration failed: ", registrationError);
      });
  });
}
