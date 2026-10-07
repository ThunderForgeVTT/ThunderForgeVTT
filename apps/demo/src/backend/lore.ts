/**
 * Lore as the server keeps it: Markdown rendered to sanitised HTML, `[[links]]`
 * resolved against the world, and slugs that stay unique.
 *
 * The server renders with comrak (GFM tables, task lists, strikethrough and
 * autolinks; raw HTML omitted) and sanitises with ammonia, which puts
 * `rel="noopener noreferrer"` on every link. `marked` in GFM mode is the same
 * dialect, so the demo renders the same Markdown into the same tags; the
 * renderer below only takes away what the server's sanitiser would.
 *
 * Nothing here knows who is looking except through `gm`, so `actors.ts` can
 * ask which entries link to an actor without the two importing each other.
 */
import { Marked, type Tokens } from "marked";
import type { DemoState, Row } from "./state";

const SAFE_HREF = /^(https?:|mailto:|\/|#|\.)/i;

/**
 * Where the app is served. The server's links start at `/world/...`; the demo
 * lives under its own base, and a link that dropped it would leave the demo
 * for a page this browser has no server for.
 */
const APP_BASE = (import.meta.env?.BASE_URL ?? "/").replace(/\/$/, "");

const escapeHtml = (text: string) =>
  text.replace(
    /[&<>"]/g,
    (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!,
  );

const unescapeHtml = (text: string) =>
  text
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, "&");

const markdown = new Marked({
  gfm: true,
  breaks: false,
  renderer: {
    // comrak with `unsafe` off omits raw HTML, and ammonia drops the comment
    // it leaves behind: the tags go, and the text around them stays.
    html: () => "",
    link(this: { parser: { parseInline(t: Tokens.Generic[]): string } }, token) {
      const text = this.parser.parseInline(token.tokens);
      if (!SAFE_HREF.test(token.href)) return text;
      const title = token.title ? ` title="${escapeHtml(token.title)}"` : "";
      return `<a href="${escapeHtml(token.href)}"${title} rel="noopener noreferrer">${text}</a>`;
    },
    image: ({ text }) => escapeHtml(text),
  },
});

const WIKI_LINK = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;

export type LinkKind = "LORE_ENTRY" | "ACTOR";

export interface LinkTarget {
  kind: LinkKind;
  id: string;
  title: string;
  href: string;
}

const byAge = (a: Row, b: Row) =>
  String(a.createdAt).localeCompare(String(b.createdAt));

/** What a `[[name]]` names, by the server's order: lore first, then actors. */
export function resolveLink(
  state: DemoState,
  name: string,
  gm: boolean,
): LinkTarget | null {
  const wanted = name.trim().toLowerCase();
  const worldId = state.world.id;
  const entry = [...state.lore]
    .sort(byAge)
    .find((e) => String(e.title).toLowerCase() === wanted);
  if (entry) {
    return {
      kind: "LORE_ENTRY",
      id: entry.id as string,
      title: entry.title as string,
      href: `/world/${worldId}/lore/${entry.slug}/view`,
    };
  }
  const actor = [...state.actors]
    .sort(byAge)
    .find(
      (a) =>
        String(a.label).toLowerCase() === wanted &&
        (gm || !a.isNpc || a.visibleToPlayers === true),
    );
  if (actor) {
    return {
      kind: "ACTOR",
      id: actor.id as string,
      title: actor.label as string,
      href: `/world/${worldId}/actor/${actor.id}/view`,
    };
  }
  return null;
}

/** The targets a body's resolved links point at. */
export function linksIn(
  state: DemoState,
  content: string,
  gm: boolean,
): LinkTarget[] {
  return [...content.matchAll(WIKI_LINK)]
    .map((match) => resolveLink(state, match[1], gm))
    .filter((target): target is LinkTarget => target !== null);
}

/** Every entry whose body has a resolved link to `id`. */
export function entriesLinkingTo(
  state: DemoState,
  kind: LinkKind,
  id: string,
  gm: boolean,
): Row[] {
  return state.lore.filter((entry) =>
    linksIn(state, String(entry.content ?? ""), gm).some(
      (target) => target.kind === kind && target.id === id,
    ),
  );
}

/** Markdown to the HTML the server would send this reader. */
export function renderLore(
  state: DemoState,
  content: string,
  gm: boolean,
): string {
  const html = markdown.parse(content, { async: false }) as string;
  // Links in code are text the author meant literally.
  return html
    .split(/(<code[^>]*>[\s\S]*?<\/code>)/)
    .map((part, index) =>
      index % 2 === 1
        ? part
        : part.replace(WIKI_LINK, (_, target: string, label?: string) => {
            const name = unescapeHtml(target).trim();
            const shown = escapeHtml(unescapeHtml(label ?? target).trim());
            const found = resolveLink(state, name, gm);
            return found
              ? `<a class="lore-link" href="${APP_BASE}${found.href}">${shown}</a>`
              : `<span class="lore-link-broken" title="Unresolved link">${shown}</span>`;
          }),
    )
    .join("");
}

/** The server's slug: ASCII, lower case, hyphens, unique in the world. */
export function slugFor(state: DemoState, title: string, self?: string): string {
  const base =
    title
      .normalize("NFKD")
      .replace(/[̀-ͯ]/g, "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "entry";
  const taken = new Set(
    state.lore.filter((e) => e.id !== self).map((e) => e.slug),
  );
  if (!taken.has(base)) return base;
  let n = 2;
  while (taken.has(`${base}-${n}`)) n += 1;
  return `${base}-${n}`;
}
