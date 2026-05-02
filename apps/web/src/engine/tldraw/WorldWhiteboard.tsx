import React, { useEffect, useRef } from "react";
import { Editor, Tldraw, createShapeId } from "tldraw";
import "tldraw/tldraw.css";

import type { WorldStore } from "../world/store";
import type { WorldToken } from "../world/types";

type WorldWhiteboardProps = {
  worldId: string;
  worldStore: WorldStore;
};

function tokenShapeId(tokenId: string) {
  return createShapeId(`token-${tokenId}`);
}

function tokenFromShape(record: any): WorldToken | null {
  if (!record || record.typeName !== "shape") {
    return null;
  }

  const id = String(record.id ?? "");
  const prefix = "shape:token-";

  if (!id.startsWith(prefix)) {
    return null;
  }

  const tokenId = id.slice(prefix.length);
  const text = record.props?.text;

  return {
    id: tokenId,
    x: Number(record.x ?? 0),
    y: Number(record.y ?? 0),
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
          ...(existing as any).props,
          text: token.label ?? token.id,
        },
      } as any,
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
      color: "blue",
      fill: "semi",
      text: token.label ?? token.id,
    },
  } as any);
}

export function WorldWhiteboard({ worldId, worldStore }: WorldWhiteboardProps) {
  const editorRef = useRef<Editor | null>(null);

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
      (entry: any) => {
        const changes = entry?.changes ?? {};
        const added = Object.values(changes.added ?? {});
        const updated = Object.values(changes.updated ?? {}).map((value: any) => value[1] ?? value);
        const removed = Object.values(changes.removed ?? {});

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

          worldStore.dispatch({ type: "remove_token", tokenId: token.id }, "tldraw");
        }
      },
      { source: "user", scope: "document" }
    );
  };

  return (
    <div style={{ width: "100%", height: "100%" }} data-world-id={worldId}>
      <Tldraw persistenceKey={`thunderforge-world-${worldId}`} onMount={handleMount} />
    </div>
  );
}