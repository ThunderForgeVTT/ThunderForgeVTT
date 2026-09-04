/**
 * `clocks` — Genie's session loop again, reached from the play dock.
 *
 * Deliberately the *same component object* as `world-staging.tsx`, not a
 * copy and not a variant. The Doom Clock and Puzzle Clocks are the session
 * loop; a GM sets them up while staging and comes back to them mid-session
 * from the dock, and there is one panel because there is one session.
 *
 * That two slots share a component is a case the slot vocabulary has to
 * allow rather than a duplication to collapse — `systemPanels.test.ts`
 * asserts the two keys resolve to the identical reference, so a future
 * refactor that quietly forks them fails a test instead of drifting.
 */
export { default } from "./world-staging";
