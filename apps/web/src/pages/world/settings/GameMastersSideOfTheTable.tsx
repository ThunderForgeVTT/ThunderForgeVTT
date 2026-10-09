import { useState } from "react";
import { Link } from "react-router-dom";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { Container } from "@/components/ui/container/Container";
import { IN_DEMO } from "@/lib/demoBuild";

/**
 * Hotlinked, not committed: the owner's pick, served by Tenor. The web app
 * sends no Content-Security-Policy, so nothing has to allow the host.
 */
export const GM_SIDE_GIF_URL =
  "https://media.tenor.com/ejjuR2cYxvoAAAAM/wait-nahhh.gif";

export interface GameMastersSideOfTheTableProps {
  worldId: string;
}

/**
 * What a Player sees on the world's settings page, in place of the settings.
 *
 * The owner's decision: nothing on that page is a Player's to change, so a
 * Player who types the address in is told so, with a laugh, and sent back to
 * the world. What they may *read* from it lives on the Overview, under
 * "About this table" (`AboutThisTable`).
 *
 * The GIF is decoration over a sentence that stands on its own: if Tenor is
 * unreachable the image is dropped and the sentence and the way back remain.
 * The demo never asks for it — its page may load nothing from another site
 * (spec 074 SC-003), and its one person is the world's Owner anyway.
 */
export function GameMastersSideOfTheTable({
  worldId,
}: GameMastersSideOfTheTableProps) {
  const [gifFailed, setGifFailed] = useState(false);

  return (
    <Container
      className="grid w-full max-w-xl py-8 sm:py-10"
      data-testid="settings-not-for-players"
    >
      <Card className="grid justify-items-center gap-4 p-6 text-center">
        {IN_DEMO || gifFailed ? null : (
          <img
            src={GM_SIDE_GIF_URL}
            alt="Wait… nahhh."
            className="max-h-64 w-auto rounded-lg"
            data-testid="settings-not-for-players-gif"
            onError={() => setGifFailed(true)}
          />
        )}
        <h1 className="text-xl font-semibold">
          Nice try — this is the Game Master&apos;s side of the table.
        </h1>
        <p className="max-w-prose text-sm text-muted-foreground">
          The rules your table plays by, and their licence, are on the
          world&apos;s Overview.
        </p>
        <Button asChild icon="arrow-left" data-testid="settings-back-to-world">
          <Link to={`/world/${worldId}/staging`}>Back to the world</Link>
        </Button>
      </Card>
    </Container>
  );
}
