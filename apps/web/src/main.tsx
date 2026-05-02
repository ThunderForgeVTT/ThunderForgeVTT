import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { set } from "lodash";
import { FrontendLogger } from "./engine/utils/logger";
import App from "./App";
import "./styles/main.scss";

set(window, "logger", FrontendLogger);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>
);