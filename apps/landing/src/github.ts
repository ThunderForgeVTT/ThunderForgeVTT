// Who starred the repository and who built it, read through the homepage's
// own cached proxy (nginx.conf.template). Never a list we typed: on failure a
// part is simply missing.

export type Person = { login: string; avatar: string; url: string };
export type Sky = {
  contributors: Person[] | null;
  stargazers: Person[] | null;
};

type GitHubUser = { login: string; avatar_url: string; html_url: string; type?: string };

const PAGE = 100;
const MAX_PAGES = 20;

function person(u: GitHubUser): Person {
  return { login: u.login, avatar: u.avatar_url, url: u.html_url };
}

async function list(path: string): Promise<GitHubUser[]> {
  const r = await fetch(path);
  if (!r.ok) throw new Error(`${path}: ${r.status}`);
  const body: unknown = await r.json();
  if (!Array.isArray(body)) throw new Error(`${path}: not a list`);
  return body as GitHubUser[];
}

async function contributors(): Promise<Person[]> {
  const all = await list("/gh/contributors");
  return all.filter((u) => u.type !== "Bot" && !u.login.endsWith("[bot]")).map(person);
}

/** Oldest first, the order GitHub keeps them in. */
async function stargazers(): Promise<Person[]> {
  const all: Person[] = [];
  for (let page = 1; page <= MAX_PAGES; page++) {
    const batch = await list(`/gh/stargazers?page=${page}`);
    all.push(...batch.map(person));
    if (batch.length < PAGE) break;
  }
  return all;
}

let pending: Promise<Sky> | null = null;

export function fetchSky(): Promise<Sky> {
  pending ??= Promise.all([
    contributors().catch(() => null),
    stargazers().catch(() => null),
  ]).then(([c, s]) => ({ contributors: c, stargazers: s }));
  return pending;
}

/** A GitHub avatar at a given pixel size. */
export function avatarAt(avatar: string, px: number): string {
  const u = new URL(avatar);
  u.searchParams.set("s", String(Math.round(px)));
  return u.toString();
}
