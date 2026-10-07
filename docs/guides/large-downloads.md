# Large downloads on a poor connection

The board's engine and a scene's maps can be large. Your browser fetches
them in several parts at once, and a part that is cut off — a train going
into a tunnel, a phone moving between networks — is picked up again from
the byte where it stopped. Nothing you already received is downloaded twice,
and the progress bar never jumps back.

If a map or the engine changes on the server while you are fetching it, the
download starts over on the new version, so you never end up with half of
each.

## Switching it off

An administrator can turn this off for the whole instance: in the instance's
settings, under **Features**, switch off **feature.download_in_parts**, or
set `THUNDERFORGE_FEATURE_DOWNLOAD_IN_PARTS=false` in its environment.

With it off, every file is one plain request again. That works everywhere,
but a dropped connection starts a large download over from the beginning.
Turn it off only if something between your players and the server — a proxy
or a cache — mishandles requests for part of a file.
