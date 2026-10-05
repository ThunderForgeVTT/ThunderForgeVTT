import { Navigate, useLocation } from "react-router-dom";
import App from "@/App";
import { DemoNotice } from "./DemoNotice";
import { DEMO_WORLD_ID } from "./seed/world";

/**
 * The demo is the web app, opened on one world.
 *
 * Every route below is the real client's own. The one thing decided here is
 * where a visitor lands: on the demo world's dashboard (FR-004), not on a
 * list of worlds with one entry in it.
 */
export function DemoApp() {
  const { pathname } = useLocation();
  return (
    <>
      {pathname === "/" ? (
        <Navigate to={`/world/${DEMO_WORLD_ID}`} replace />
      ) : (
        <App />
      )}
      <DemoNotice />
    </>
  );
}
