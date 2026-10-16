# Quickstart: proving Smooth World Navigation

## Checks

```sh
cd apps/web
node_modules/.bin/tsc -b --noEmit
node_modules/.bin/eslint src/hooks src/pages/world
node_modules/.bin/vitest run src/hooks
```

## Slice

```sh
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:worlds
```

`world-navigation.spec.ts` must show:

1. GM: overview loads, then Players, Scenes, Compendium, System Settings
   and back to Overview, with no full-screen loader after the first page
   (SC-001), and the GM-only sidebar group present on each page's first
   paint (SC-002).
2. Player: the GM-only group never appears on the same walk (SC-002).
3. A player whose claim the GM releases is sent to choose a character on
   returning to the overview (SC-003).
4. Sign out, sign in as another user in the same tab: none of the first
   user's world name, role or pause shows (SC-004).

Neighbours in the slice (actor-claim, players-section, play-pause,
gm-staging-page) stay green.

## Demo

```sh
cd apps/demo && pnpm run e2e e2e/demo-pages.spec.ts e2e/demo-board.spec.ts
pnpm run build   # plain build for the phone preview
```

## Owner

Phone check on the demo preview: page changes inside the world feel
smooth (SC-007). Nothing ships before that greenlight.
