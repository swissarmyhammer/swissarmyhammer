/**
 * Quick Capture window — a minimal UI for rapidly adding tasks.
 *
 * Layout: header (icon + hints) → text input → divider → board selector.
 * Enter submits the task to the first column of the selected board;
 * Escape dismisses the window.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FieldPlaceholderEditor } from "@/components/fields/field-placeholder";
import { BoardSelector } from "@/components/board-selector";
import appIcon from "@/assets/app-icon-32.png";
import type { OpenBoard, BoardDataResponse } from "@/types/kanban";

const STORAGE_KEY = "quick-capture-last-board";

export function QuickCapture() {
  const [boards, setBoards] = useState<OpenBoard[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  // Key to force-remount the editor on each window show
  const [editorKey, setEditorKey] = useState(0);
  const mountedRef = useRef(true);

  const loadBoards = useCallback(async () => {
    try {
      const result = await invoke<OpenBoard[]>("list_open_boards");
      if (!mountedRef.current) return;
      setBoards(result);

      const stored = localStorage.getItem(STORAGE_KEY);
      const match = result.find((b) => b.path === stored);
      const active = result.find((b) => b.is_active);
      setSelectedPath(match?.path ?? active?.path ?? result[0]?.path ?? null);
      setReady(true);
    } catch {
      setReady(true);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    loadBoards();

    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        loadBoards();
        setEditorKey((k) => k + 1);
      }
    });

    return () => {
      mountedRef.current = false;
      unlisten.then((fn) => fn());
    };
  }, [loadBoards]);

  // Window-level Escape fallback for when CM6 doesn't have focus
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !(e.target as HTMLElement)?.closest?.(".cm-editor")) {
        getCurrentWindow().hide();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const hideWindow = useCallback(() => {
    getCurrentWindow().hide();
  }, []);

  const handleSubmit = useCallback(
    async (text: string) => {
      if (!selectedPath || !text.trim()) return;

      try {
        const active = boards.find((b) => b.is_active);
        if (active?.path !== selectedPath) {
          await invoke("set_active_board", { path: selectedPath });
        }

        const boardData = await invoke<BoardDataResponse>("get_board_data");
        const columns = [...boardData.columns].sort((a, b) => {
          const orderA = typeof a.order === "number" ? a.order : 0;
          const orderB = typeof b.order === "number" ? b.order : 0;
          return orderA - orderB;
        });
        const firstColumnId = columns[0]?.id;
        if (!firstColumnId) return;

        await invoke("dispatch_command", {
          cmd: "task.add",
          args: { column: firstColumnId, title: text.trim() },
        });

        localStorage.setItem(STORAGE_KEY, selectedPath);

        if (active && active.path !== selectedPath) {
          await invoke("set_active_board", { path: active.path }).catch(() => {});
        }
      } catch (err) {
        console.error("Quick capture failed:", err);
      }

      hideWindow();
    },
    [selectedPath, boards, hideWindow],
  );

  const handleCancel = useCallback(() => {
    hideWindow();
  }, [hideWindow]);

  if (!ready) return null;

  return (
    <div className="h-screen w-screen flex items-start justify-center p-2" style={{ background: "transparent" }}>
      <div className="w-full rounded-xl border border-border bg-background/95 backdrop-blur-xl shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        {/* Header — draggable, shows icon and keyboard hints */}
        <div className="flex items-center gap-2 px-3 py-1.5 bg-muted/30" data-tauri-drag-region>
          <img src={appIcon} alt="" className="h-4 w-4 shrink-0" />
          <span className="text-xs font-medium text-muted-foreground/70">Quick Capture</span>
          <span className="ml-auto text-[10px] text-muted-foreground/40">
            enter to add &middot; esc to dismiss
          </span>
        </div>

        {/* Editor */}
        <div className="px-3 py-2">
          <FieldPlaceholderEditor
            key={editorKey}
            value=""
            onCommit={() => {}}
            onCancel={handleCancel}
            onSubmit={handleSubmit}
            placeholder="What needs to be done?"
          />
        </div>

        {/* Divider + Board selector — always shown */}
        <div className="border-t border-border/50 px-3 py-1.5 flex items-center gap-2 bg-muted/20">
          <span className="text-[10px] text-muted-foreground/50 shrink-0">Board</span>
          <BoardSelector
            boards={boards}
            selectedPath={selectedPath}
            onSelect={setSelectedPath}
            variant="compact"
            className="flex-1 text-muted-foreground"
          />
        </div>
      </div>
    </div>
  );
}
