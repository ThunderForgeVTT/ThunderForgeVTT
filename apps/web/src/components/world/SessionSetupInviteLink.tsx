import { useState } from "react";
import { generateInviteCode } from "@/api/world";
import { Button } from "@/components/ui/button/Button";
import { Input } from "@/components/ui/input";

interface SessionSetupInviteLinkProps {
  worldId: string;
}

/**
 * Spec 017 (FR-015/SC-005): surfaces the same shareable invite URL already
 * generated from the world dashboard's "Generate Join Link" control
 * (`CampaignSettingsPanel.tsx`), directly on Session Setup, so the GM
 * doesn't need to leave this page to copy/distribute it. Reuses the same
 * `generateInviteCode` mutation — no new server surface for this.
 */
export function SessionSetupInviteLink({ worldId }: SessionSetupInviteLinkProps) {
  const [isGenerating, setIsGenerating] = useState(false);
  const [inviteUrl, setInviteUrl] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const handleGenerate = async () => {
    setIsGenerating(true);
    setStatus(null);
    try {
      // Spec 027: was a private `postGraphQL` copy — one the transport
      // consolidation missed because it lived in a component rather than
      // `src/api/`. Now shares the hardened client like everything else.
      const created = await generateInviteCode(worldId, 5);
      const url = `${window.location.origin}/join/${created.inviteCode}`;
      setInviteUrl(url);
      await navigator.clipboard.writeText(url).catch(() => {});
      setStatus("Copied to clipboard.");
    } catch (err) {
      setStatus(err instanceof Error ? err.message : "Failed to generate invite link");
    } finally {
      setIsGenerating(false);
    }
  };

  return (
    <div className="grid gap-2" data-testid="session-setup-invite-link">
      <Button
        type="button"
        variant="secondary"
        size="sm"
        onClick={() => void handleGenerate()}
        disabled={isGenerating}
        data-testid="session-setup-generate-invite"
      >
        {isGenerating ? "Generating..." : "Copy invite link"}
      </Button>
      {inviteUrl ? (
        <Input readOnly value={inviteUrl} data-testid="session-setup-invite-url" />
      ) : null}
      {status ? <p className="text-xs text-muted-foreground">{status}</p> : null}
    </div>
  );
}
