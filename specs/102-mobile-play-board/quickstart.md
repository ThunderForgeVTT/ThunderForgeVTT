# Quickstart: proving the phone board

## At the desk

```bash
pnpm -F @thunderforge/web test -- phoneRows      # who gets which buttons
pnpm -F @thunderforge/web check                  # tsc + lint
pnpm e2e:phone:standalone                        # demo, both views, no server
pnpm e2e:phone:integration                       # app, real roles and grants
pnpm e2e:which --diff                            # then run the neighbours it names
```

Expected: the overlap scan and map-share checks in
[contracts/phone-board-ui.md](contracts/phone-board-ui.md) pass for GM and
player; the neighbours pass unchanged at desktop size.

## On the owner's phone

1. Build the demo with a release engine (`ENGINE_PROFILE=release`; a dev wasm
   is 337 MB and the phone runs out of memory), then
   `pnpm -F @thunderforge/demo build` and serve it with `vite preview`.
2. Give it https: `cloudflared tunnel --url http://localhost:5298
   --http-host-header localhost`. Plain `http://<LAN IP>` crashes the board
   (`crypto.randomUUID` needs a secure context; out of scope here).
3. Open `<tunnel>/demo/world/d0000000-0000-4000-0002-000000000001/play` on
   the phone (or push it: `adb shell am start -a android.intent.action.VIEW
   -d <url>`).
4. Check, as GM then as player (switch from the demo notice):
   - only the map and five tiles with nothing open;
   - every tool and panel reachable; the armed tool's chip;
   - a sheet leaves the map live above it; expand, swipe down, back gesture;
   - chat with the keyboard open;
   - rotate to landscape and back with a sheet open;
   - full screen on and off.
5. For the installed app: open the web app (not the demo) over https in
   Chrome, install it from the menu, open it from the home screen, reach a
   world's board.

Screenshots for the record: `adb exec-out screencap -p > <file>.png`.
