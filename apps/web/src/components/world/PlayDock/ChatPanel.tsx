import { useCallback, useEffect, useRef, useState } from "react";
import { getWorldChatMessages, sendChatMessage } from "@/api/chat";
import { Button } from "@/components/ui/button/Button";
import { subscribeToWorldEvents, startPlayPanelEventSync } from "@/engine/world/sync";
import type { ChatMessageRecord } from "@/types/chat";

export interface ChatPanelProps {
  worldId: string;
  sceneId: string | null;
  currentUserId: string | null;
  isGm: boolean;
}

/**
 * World chat: persisted backscroll plus a composer, kept live by the same
 * `world_events` subscription every other Play panel uses (event code 17).
 *
 * GM-only messages are filtered server-side — this component renders
 * whatever it is given and never decides visibility itself, so there is no
 * client rule that can drift out of sync with the server's.
 */
export function ChatPanel({ worldId, sceneId, currentUserId, isGm }: ChatPanelProps) {
  const [messages, setMessages] = useState<ChatMessageRecord[] | null>(null);
  const [body, setBody] = useState("");
  const [gmOnly, setGmOnly] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(() => {
    return getWorldChatMessages(worldId)
      .then(setMessages)
      .catch((err) => setError(err instanceof Error ? err.message : "Failed to load chat"));
  }, [worldId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Live updates. The event payload carries only a message id, so this
  // refetches rather than appending from the payload — which is also what
  // keeps a GM-only message from reaching a player's client (see
  // sync/playPanels.ts).
  useEffect(() => {
    const stop = startPlayPanelEventSync(
      { onChatMessage: () => void refresh() },
      subscribeToWorldEvents(worldId),
    );
    return stop;
  }, [worldId, refresh]);

  // Pin to the newest message whenever the log grows.
  useEffect(() => {
    const el = scrollRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [messages]);

  const handleSend = async () => {
    const trimmed = body.trim();
    if (!trimmed || sending) return;
    setSending(true);
    setError(null);
    try {
      await sendChatMessage({ worldId, sceneId, body: trimmed, gmOnly: isGm ? gmOnly : false });
      setBody("");
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to send");
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="flex h-full flex-col gap-3" data-testid="chat-panel">
      <div ref={scrollRef} className="min-h-0 flex-1 space-y-3 overflow-y-auto pr-1">
        {messages === null ? (
          <p className="text-sm text-muted-foreground">Loading chat…</p>
        ) : messages.length === 0 ? (
          <p className="text-sm text-muted-foreground">No messages yet.</p>
        ) : (
          messages.map((message) => (
            <div key={message.id} data-testid="chat-message">
              <div className="flex items-baseline gap-2">
                <span
                  className={
                    message.authorUserId === currentUserId
                      ? "text-sm font-semibold text-primary"
                      : "text-sm font-semibold"
                  }
                >
                  {message.authorLabel}
                </span>
                {message.gmOnly ? (
                  <span className="rounded bg-muted px-1.5 py-0.5 text-[0.65rem] font-semibold tracking-widest text-muted-foreground uppercase">
                    GM only
                  </span>
                ) : null}
                <time className="ml-auto text-xs text-muted-foreground">
                  {new Date(message.createdAt).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </time>
              </div>
              {/* Rendered as text, never as markup — bodies are stored raw. */}
              <p className="text-sm whitespace-pre-wrap">{message.body}</p>
            </div>
          ))
        )}
      </div>

      {error ? <p className="text-sm text-destructive">{error}</p> : null}

      <div className="grid gap-2 border-t border-border pt-3">
        <textarea
          value={body}
          onChange={(event) => setBody(event.target.value)}
          onKeyDown={(event) => {
            // Enter sends, Shift+Enter makes a newline — the convention
            // every chat this sits next to already uses.
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void handleSend();
            }
          }}
          rows={2}
          placeholder="Say something…"
          aria-label="Message"
          data-testid="chat-input"
          className="resize-none rounded-lg border border-input bg-transparent px-2.5 py-2 text-sm outline-none transition-colors focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        />
        <div className="flex items-center gap-2">
          {isGm ? (
            <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <input
                type="checkbox"
                checked={gmOnly}
                onChange={(event) => setGmOnly(event.target.checked)}
                data-testid="chat-gm-only-toggle"
              />
              GM only
            </label>
          ) : null}
          <Button
            type="button"
            size="sm"
            className="ml-auto"
            disabled={sending || !body.trim()}
            onClick={() => void handleSend()}
            data-testid="chat-send-button"
          >
            {sending ? "Sending…" : "Send"}
          </Button>
        </div>
      </div>
    </div>
  );
}
