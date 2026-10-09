# Quickstart: Bringing a Character In

## Real game

1. **Set up.**
   - Run `make dev` with `THUNDERFORGE_FEATURE_SHEET_IMPORT=true`. That line
     is in `.env.example` until T097 turns the flag on by default.
   - Generate the fixtures with `cargo test -p thunderforge-system-dnd5e-sheet`.
     The PDFs land in `packs/systems/dnd5e/sheet/tests/fixtures/out/`.
   - Sign in as a GM and create a 5e world. Invite a second account, and
     promote a third to Trusted Player.
   - Create a 5e character that is available for claim. The player claims
     it, and the claim grants Editor.
2. **Bring it in.**
   - The player opens the Players screen and chooses **Bring in a sheet** on
     their hero's row. **Bring in a sheet** on the actor screen does the
     same.
   - Pick `fighter3-wizard2.pdf`. The review appears within a couple of
     seconds, before anything is uploaded.
3. **Review.**
   - Every field shows Read, Uncertain or Unread, with the text it came
     from. Changed fields show "will overwrite", with the old value.
   - Filter to **Uncertain**. Correct one value.
   - The class line reads Fighter 3 / Wizard 2, at level 5.
   - A feat the world lacks is listed as **new, will wait for the GM**.
4. **Accept.**
   - The sheet carries both classes, the scores, hit points, hit-dice pools,
     proficiencies, coins, defences and linked spells.
   - The feat shows **awaiting the GM**.
   - On the play field, the feat is absent from the character's actions.
5. **Decline.** Run steps 2 and 3 again and choose **Decline**. Nothing
   changes, and no file is uploaded.
6. **The GM decides.**
   - On the world compendium, open **Brought by players**. The player's
     name heads their list.
   - **Adopt** the feat. It appears in the world compendium, marked
     Uploaded, and the character's sheet shows it as the world's.
   - **Decline** a spell. The player's sheet shows **declined by the GM**,
     and the spell is gone from play.
7. **Refused and reported.**
   - Force a roll from the declined spell, for example from a browser tab
     that was open before the decline. It is refused with the sentence
     about adoption, and **no report** reaches the GM, because that client
     was stale.
   - Refresh and force it again. It is refused again, and the GM's chat
     shows a factual report.
8. **Trusted Player.**
   - The Trusted Player opens **Brought by players** and chooses **Adopt
     all** for the player.
   - They see no **Roll back** button and cannot download the file.
9. **Again, changed.**
   - Lower the character's current HP in play. Import `fighter3-wizard2-l6.pdf`,
     the level-6 variant.
   - The review shows only what differs, and current HP is listed as kept.
   - Accept. The level rises, and current HP is what the table set.
10. **Roll back.**
    - The GM opens **Import history** on the actor and rolls back to the
      first import. The sheet returns, and current HP is kept.
    - The history reads import, import, rollback.
    - The player can download their file. The other player cannot.
11. **Refusals.** Try an encrypted PDF, a 30-page book and a Roll for Shoes
    actor's sheet on a 5e actor. Each is refused with its reason, and
    nothing is written.

## Proof

```sh
cargo test -p thunderforge-pdf -p thunderforge-sheet-import \
  -p thunderforge-system-dnd5e-sheet -p thunderforge-system-dnd5e \
  -p thunderforge-system-roll-for-shoes-sheet
make test-rust ARGS="-p thunderforge-server"     # needs the thunderforge-canvas-assets bucket
pnpm -F @thunderforge/web test
make lint && pnpm verify
pnpm e2e:sheet-import:standalone
THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT=1 pnpm e2e:sheet-import -- --workers=1   # external stack only
pnpm e2e:which --diff     # prints FULL SUITE for the migrations; run the slices it names instead
pnpm playtest             # last
```

Never run `node ./scripts/e2e-parallel.mjs` on its own as a gate.

## Measuring the real corpus (by hand, nothing committed)

```sh
THUNDERFORGE_SHEET_CORPUS=~/sheet-corpus \
  cargo run -p thunderforge-system-dnd5e-sheet --example measure_corpus
```

The output is per-field counts of read, uncertain and unread. It contains no
values. Record the counts in research R17 (T093).
