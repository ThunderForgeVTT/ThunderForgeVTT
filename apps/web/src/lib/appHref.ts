/**
 * A path in this app as a link that leaves React Router: a new tab, an
 * `<a target="_blank">`. Router links already carry the basename; these do
 * not, so in the demo (served under `/demo/`) a bare `/world/...` would open
 * the site's own world route instead of the demo's.
 */
export function appHref(path: string): string {
  return `${import.meta.env.BASE_URL.replace(/\/$/, "")}${path}`;
}
