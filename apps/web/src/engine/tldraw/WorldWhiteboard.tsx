import { useEffect, useMemo, useRef, useState } from "react";
import { Editor, Tldraw, createShapeId } from "tldraw";
import "tldraw/tldraw.css";
import { Button } from "@/components/ui/button/Button";
import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { Panel } from "@/components/ui/panel/Panel";
import type { WorldStore } from "../world/store";
import type { WorldToken } from "../world/types";
import styles from "./WorldWhiteboard.module.scss";

type WorldWhiteboardProps = {
  worldId: string;
  worldStore: WorldStore;
};

type ToolbarTool = "select" | "draw" | "geo" | "eraser";

function tokenShapeId(tokenId: string) {
  return createShapeId(`token-${tokenId}`);
}

function tokenFromShape(record: unknown): WorldToken | null {
  if (
    !record ||
    typeof record !== "object" ||
    (record as { typeName?: string }).typeName !== "shape"
  ) {
    return null;
  }

  const shape = record as {
    id?: string;
    x?: number;
    y?: number;
    props?: { text?: string };
  };
  const id = String(shape.id ?? "");
  const prefix = "shape:token-";

  if (!id.startsWith(prefix)) {
    return null;
  }

  const tokenId = id.slice(prefix.length);
  const text = shape.props?.text;

  return {
    id: tokenId,
    x: Number(shape.x ?? 0),
    y: Number(shape.y ?? 0),
    z: 0,
    label: typeof text === "string" ? text : tokenId,
  };
}

function upsertTokenShape(editor: Editor, token: WorldToken) {
  const id = tokenShapeId(token.id);
  const existing = editor.getShape(id);

  if (existing) {
    editor.updateShapes([
      {
        id,
        type: existing.type,
        x: token.x,
        y: token.y,
        props: {
          ...(existing as { props?: Record<string, unknown> }).props,
          text: token.label ?? token.id,
        },
      },
    ]);
    return;
  }

  editor.createShape({
    id,
    type: "geo",
    x: token.x,
    y: token.y,
    props: {
      w: 120,
      h: 70,
      geo: "rectangle",
      color: "violet",
      fill: "semi",
      text: token.label ?? token.id,
      dash: "draw",
    },
  });
}

export function WorldWhiteboard({ worldId, worldStore }: WorldWhiteboardProps) {
  const editorRef = useRef<Editor | null>(null);
  const [activeTool, setActiveTool] = useState<ToolbarTool>("select");
  const [tokens, setTokens] = useState(() =>
    Object.values(worldStore.getState().tokens),
  );

  useEffect(
    () =>
      worldStore.subscribe((event) => {
        setTokens(Object.values(event.state.tokens));
      }),
    [worldStore],
  );

  useEffect(() => {
    const editor = editorRef.current;

    if (!editor) {
      return;
    }

    const unsubscribe = worldStore.subscribe((event) => {
      if (event.source === "tldraw") {
        return;
      }

      editor.store.mergeRemoteChanges(() => {
        if (event.command.type === "upsert_token") {
          upsertTokenShape(editor, event.command.token);
        }

        if (event.command.type === "remove_token") {
          editor.deleteShapes([tokenShapeId(event.command.tokenId)]);
        }
      });
    });

    return unsubscribe;
  }, [worldStore]);

  const handleMount = (editor: Editor) => {
    editorRef.current = editor;

    const state = worldStore.getState();
    editor.store.mergeRemoteChanges(() => {
      for (const token of Object.values(state.tokens)) {
        upsertTokenShape(editor, token);
      }
    });

    editor.store.listen(
      (entry: unknown) => {
        const changes =
          (entry as { changes?: Record<string, unknown> })?.changes ?? {};
        const added = Object.values(
          (changes as { added?: Record<string, unknown> }).added ?? {},
        );
        const updated = Object.values(
          (changes as { updated?: Record<string, [unknown, unknown]> })
            .updated ?? {},
        ).map((value) => value[1] ?? value);
        const removed = Object.values(
          (changes as { removed?: Record<string, unknown> }).removed ?? {},
        );

        for (const record of [...added, ...updated]) {
          const token = tokenFromShape(record);

          if (!token) {
            continue;
          }

          worldStore.dispatch({ type: "upsert_token", token }, "tldraw");
        }

        for (const record of removed) {
          const token = tokenFromShape(record);

          if (!token) {
            continue;
          }

          worldStore.dispatch(
            { type: "remove_token", tokenId: token.id },
            "tldraw",
          );
        }
      },
      { source: "user", scope: "document" },
    );
  };

  const minimapTokens = useMemo(() => {
    const maxX = Math.max(500, ...tokens.map((token) => token.x + 40));
    const maxY = Math.max(400, ...tokens.map((token) => token.y + 40));

    return tokens.map((token) => ({
      ...token,
      left: `${(token.x / maxX) * 100}%`,
      top: `${(token.y / maxY) * 100}%`,
    }));
  }, [tokens]);

  const setTool = (tool: ToolbarTool) => {
    editorRef.current?.setCurrentTool(tool);
    setActiveTool(tool);
  };

  return (
    <div className={styles.wrapper} data-world-id={worldId}>
      <div className={styles.toolbar}>
        <Button
          variant={activeTool === "select" ? "primary" : "secondary"}
          size="sm"
          icon="wand"
          onClick={() => setTool("select")}
        >
          Wand
        </Button>
        <Button
          variant={activeTool === "draw" ? "primary" : "secondary"}
          size="sm"
          icon="quill"
          onClick={() => setTool("draw")}
        >
          Quill
        </Button>
        <Button
          variant={activeTool === "geo" ? "primary" : "secondary"}
          size="sm"
          icon="rune"
          onClick={() => setTool("geo")}
        >
          Chisel
        </Button>
        <Button
          variant={activeTool === "eraser" ? "primary" : "secondary"}
          size="sm"
          icon="torch"
          onClick={() => setTool("eraser")}
        >
          Torch
        </Button>
      </div>

      <div className={styles.boardFrame}>
        <div className={styles.gridOverlay} aria-hidden="true" />
        <Tldraw
          hideUi
          persistenceKey={`thunderforge-world-${worldId}`}
          onMount={handleMount}
        />
      </div>

      <Panel variant="parchment" className={styles.minimapPanel}>
        <div className={styles.minimapHeader}>
          <div>
            <p className={styles.panelEyebrow}>Scene editor</p>
            <h3>Parchment minimap</h3>
          </div>
          <FantasyIcon name="map" size={18} tone="gold" />
        </div>
        <div className={styles.minimap}>
          {minimapTokens.map((token) => (
            <span
              key={token.id}
              className={styles.minimapToken}
              style={{ left: token.left, top: token.top }}
              title={token.label ?? token.id}
            />
          ))}
        </div>
      </Panel>
    </div>
  );
}
