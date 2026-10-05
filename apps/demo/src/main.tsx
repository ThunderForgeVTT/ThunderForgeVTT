// First, and before anything of the app's: after this line the page cannot
// reach a server (spec 074; see `guard/install.ts`).
import "./guard/install";

import ReactDOM from "react-dom/client";
import { HelmetProvider } from "react-helmet-async";
import { BrowserRouter } from "react-router-dom";
import { AuthProvider } from "@/hooks/useAuth";
import { ThemeProvider } from "@/hooks/useTheme";
import { DemoApp } from "./DemoApp";
import "./demo.css";

// The web app's own tree, without the three things that exist to talk to an
// instance: the feedback launcher, the log capture behind it, and the service
// worker that caches an instance's assets.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <HelmetProvider>
    <ThemeProvider>
      <AuthProvider>
        <BrowserRouter basename={import.meta.env.BASE_URL.replace(/\/$/, "")}>
          <DemoApp />
        </BrowserRouter>
      </AuthProvider>
    </ThemeProvider>
  </HelmetProvider>,
);
