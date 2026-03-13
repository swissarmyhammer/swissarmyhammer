/**
 * Listens for Tauri "init-progress" events and displays them as toast
 * notifications via sonner. Renders nothing visually — mount once near
 * the app root alongside the `<Toaster />` component.
 */

import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

/** Shape of the tagged-enum payload emitted by `TauriReporter`. */
interface InitEvent {
  kind: string;
  data: {
    message?: string;
    verb?: string;
    component?: string;
    reason?: string;
    elapsed_ms?: number;
  };
}

/**
 * Subscribes to the Tauri `init-progress` event channel and maps each
 * `InitEvent` variant to an appropriate toast level.
 */
export function InitProgressListener() {
  useEffect(() => {
    const unlisten = listen<InitEvent>("init-progress", (event) => {
      const { kind, data } = event.payload;
      switch (kind) {
        case "Header":
          toast.info(data.message);
          break;
        case "Action":
          toast.success(`${data.verb}: ${data.message}`);
          break;
        case "Warning":
          toast.warning(data.message);
          break;
        case "Error":
          toast.error(data.message);
          break;
        case "Finished":
          toast.success(
            `${data.message} (${((data.elapsed_ms ?? 0) / 1000).toFixed(1)}s)`,
          );
          break;
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return null;
}
