import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { FrontendLogger } from "./engine/utils/logger";
import App from "./App";
import "./styles/main.scss";

(window as typeof window & { logger?: typeof FrontendLogger }).logger = FrontendLogger;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>
);