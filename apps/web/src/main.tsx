import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { HelmetProvider } from "react-helmet-async";
import App from "./App";
import { FeedbackLauncher } from "./components/feedback/FeedbackLauncher";
import { AuthProvider } from "./hooks/useAuth";
import { ThemeProvider } from "./hooks/useTheme";
import { registerAssetCache } from "./serviceWorker";
import { startLogCapture } from "./services/feedbackLogBuffer";
import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <HelmetProvider>
    <ThemeProvider>
      <AuthProvider>
        <BrowserRouter>
          <App />
          {/*
            Spec 037 FR-001: the feedback control has to be reachable from
            *any* screen, and this is the only place above every route —
            `/world/:id/play`, `/join/:code` and `/status` are all rendered
            outside `MainLayout`, and those are the screens a person is most
            likely to be on when something goes wrong. A sibling of `<App />`
            rather than a wrapper, so it survives App's own error states.

            This is the first app-root overlay host in this codebase.
          */}
          <FeedbackLauncher />
        </BrowserRouter>
      </AuthProvider>
    </ThemeProvider>
  </HelmetProvider>,
);

/*
  Spec 037 FR-007: start capturing browser logs before anything else can throw.
  Placed after render for the same reason the asset cache is — nothing here
  competes with first paint — but before the cache registration, because a
  failure *in* that registration is exactly the kind of line a bug report
  wants. The buffer is bounded, in memory, and never written down; see
  `services/feedbackLogBuffer.ts`.
*/
startLogCapture();

// Caches scene backgrounds, the largest thing this app repeatedly downloads.
// Registered after render so it never competes with first paint.
registerAssetCache();
