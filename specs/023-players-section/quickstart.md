# Quickstart: Validating the Players Section

Prerequisites: `make dev` running, a registered account.

## 1. Every member sees the roster as characters (User Story 1)

1. Create a world, open the **Players** section from the sidebar (new entry, alongside Scenes/NPCs/Lore/Items/Abilities).
2. Confirm the world's GM/Owner appears in the list.
3. Invite a second account, have them join and claim a character via Actor Selection.
4. Reload the Players section as either member — confirm the second member now shows their claimed character's name, not just their username.
5. Invite and join a third account without claiming a character yet — confirm they still appear, clearly marked as not having claimed one (not omitted, not an error).
6. Confirm the Overview page no longer shows a player roster.

## 2. GM manages roles and removal from the same section (User Story 2)

1. As the GM, on the Players section, change the second member's role (e.g. Player → GM). Confirm it takes effect immediately in that row.
2. Remove the third member (the one with no claimed character). Confirm they disappear from the roster and lose world access.
3. Confirm the GM cannot remove themselves (control absent or rejected).
4. As the second member (now GM-role, if promoted in step 1 — otherwise as the original non-GM account), confirm no role-change or removal controls are visible anywhere on the page.
5. Confirm the world dashboard's Campaign Settings panel no longer shows role-change/remove-member controls or a player-roster list — only invite generation and the "Allow players to create their own actors" toggle remain there.

## Automated coverage expectations (for tasks phase)

- `cargo test` coverage for: the `worldMembers` query's new `claimedActor` field (populated when a claim exists, null when it doesn't), and `updateMemberRole`/`removeMember` succeeding for a world's Owner who has no `world_members` row of their own (the bundled fix, research.md §3) — a case likely not previously covered given the bug it fixes.
- Playwright e2e coverage for: the roster-with-characters view (multi-account), the GM role-change/removal flow end-to-end, a non-GM member seeing no management controls, and confirming the Campaign Settings panel's roster/role controls are gone.
