import { useEffect, useState, useRef } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";

export function useDragDrop(onDrop: (paths: string[]) => void) {
  const [dragOver, setDragOver] = useState(false);
  const onDropRef = useRef(onDrop);
  onDropRef.current = onDrop;

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const { type } = event.payload;
        if (type === "enter") {
          setDragOver(true);
        } else if (type === "leave") {
          setDragOver(false);
        } else if (type === "drop") {
          setDragOver(false);
          const csvPaths = event.payload.paths.filter((p) =>
            p.toLowerCase().endsWith(".csv")
          );
          if (csvPaths.length > 0) {
            onDropRef.current(csvPaths);
          }
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, []);

  return { dragOver };
}
