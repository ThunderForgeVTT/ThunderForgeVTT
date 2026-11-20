import { Navigate, useParams } from "react-router-dom";

/**
 * `/world/:id/staging` was the routed staging screen of spec 010. Spec 100
 * US2 folded it into the world's overview at `/world/:id`; the address stays
 * so links and bookmarks that name it still arrive.
 */
export default function WorldStagingRoutePage() {
  const { id = "" } = useParams();
  return <Navigate to={`/world/${id}`} replace />;
}
