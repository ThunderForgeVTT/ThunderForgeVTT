-- Spec 004: tokens can now be dragged/positioned anywhere on the canvas,
-- via the same center-origin coordinate system walls/shapes/lights already
-- use (their tables have no coordinate-range constraint at all). The
-- original `valid_coordinates CHECK (x >= 0 AND y >= 0)` predates that
-- convention and silently rejected any token placed left of or above the
-- world origin -- discovered live while writing this feature's canvas-drag
-- e2e coverage (a drag to a negative-y position failed with a check
-- constraint violation the mutation swallowed into a generic "not found or
-- not owned by you" error, per mutations_tokens.rs's update_token).
ALTER TABLE tokens DROP CONSTRAINT IF EXISTS valid_coordinates;
