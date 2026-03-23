/**
 * Quick Capture window — a minimal UI for rapidly adding tasks.
 *
 * Layout: header (icon + hints) → text input → divider → board selector.
 * Enter submits the task to the first column of the selected board;
 * Escape dismisses the window.
 *
 * Listens for Tauri entity events so board names update dynamically when
 * changed on disk or in the main app window.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { EntityIcon } from "@/components/entity-icon";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { EditorView } from "@codemirror/view";
import { getCM } from "@replit/codemirror-vim";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { TextEditor } from "@/components/fields/text-editor";
import { BoardSelector } from "@/components/board-selector";
import { useUIState } from "@/lib/ui-state-context";
import appIcon from "@/assets/app-icon-32.png";
import type { OpenBoard, BoardDataResponse, Entity } from "@/types/kanban";

const STORAGE_KEY = "quick-capture-last-board";

/** Payload for entity-field-changed Tauri event. */
interface EntityFieldChangedEvent {
  entity_type: string;
  id: string;
  fields?: Record<string, unknown>;
}

export function QuickCapture() {
  const [boards, setBoards] = useState<OpenBoard[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [draft, setDraft] = useState("");
  // Key to force-remount the editor on each window show
  const [editorKey, setEditorKey] = useState(0);
  const mountedRef = useRef(true);
  const cardRef = useRef<HTMLDivElement>(null);

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

  // Derive a minimal board entity from the selected OpenBoard for BoardSelector.
  // The board entity always has entity_type "board" and id "board".
  const selected = boards.find((b) => b.path === selectedPath);
  const boardEntity: Entity | undefined = selected
    ? { entity_type: "board", id: "board", fields: { name: selected.name } }
    : undefined;

  useEffect(() => {
    mountedRef.current = true;
    loadBoards();

    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        loadBoards();
        setDraft("");
        setEditorKey((k) => k + 1);
      }
    });

    return () => {
      mountedRef.current = false;
      unlisten.then((fn) => fn());
    };
  }, [loadBoards]);

  // -------------------------------------------------------------------------
  // Tauri entity event listeners — keep board names in sync with main app
  // -------------------------------------------------------------------------
  useEffect(() => {
    const unlisteners = [
      // Board name/field changes → reload board list (names come from list_open_boards)
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        if (event.payload.entity_type === "board") loadBoards();
      }),
      // Structural board changes (open/close/switch) → reload board list
      listen("board-changed", () => {
        loadBoards();
      }),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn) => fn());
      }
    };
  }, [loadBoards]);

  const hideWindow = useCallback(() => {
    getCurrentWindow().hide();
  }, []);

  const handleSubmit = useCallback(
    async (text: string) => {
      if (!selectedPath || !text.trim()) return;

      try {
        const active = boards.find((b) => b.is_active);

        const boardData = await invoke<BoardDataResponse>("get_board_data", {
          boardPath: selectedPath,
        });
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
          boardPath: selectedPath,
        });

        localStorage.setItem(STORAGE_KEY, selectedPath);

        // If we switched to a different board for the add, restore the previous active
        if (active && active.path !== selectedPath) {
          await invoke("dispatch_command", {
            cmd: "file.switchBoard",
            args: { path: active.path },
          }).catch(() => {});
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

  // Window-level keyboard handler for Escape (dismiss) and Enter (submit).
  // In vim mode, we distinguish insert vs normal:
  //   insert Escape → let vim handle (exits to normal)
  //   normal Escape → dismiss window
  //   normal Enter  → submit task
  const { keymap_mode: keymapMode } = useUIState();
  const handleSubmitRef = useRef(handleSubmit);
  handleSubmitRef.current = handleSubmit;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const cmEl = (e.target as HTMLElement)?.closest?.(".cm-editor");

      if (cmEl && keymapMode === "vim") {
        const view = EditorView.findFromDOM(cmEl as HTMLElement);
        if (view) {
          const cm = getCM(view);
          if (cm?.state?.vim?.insertMode) return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          getCurrentWindow().hide();
        } else if (e.key === "Enter") {
          e.preventDefault();
          e.stopPropagation();
          const text =
            EditorView.findFromDOM(cmEl as HTMLElement)?.state.doc.toString() ??
            "";
          if (text.trim()) handleSubmitRef.current(text);
        }
        return;
      }

      if (e.key === "Escape") {
        getCurrentWindow().hide();
      }
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [keymapMode]);

  // Auto-resize the Tauri window to match card content height (max 400px).
  // The outer wrapper has p-2 (8px each side), so window height = card + 16.
  useEffect(() => {
    const card = cardRef.current;
    if (!card) return;
    const win = getCurrentWindow();
    const observer = new ResizeObserver(() => {
      const cardHeight = Math.min(card.scrollHeight, 400);
      win.setSize(new LogicalSize(560, cardHeight + 16)).catch(() => {});
    });
    observer.observe(card);
    return () => observer.disconnect();
  }, []);

  if (!ready) return null;

  return (
    <div
      className="h-screen w-screen flex items-start justify-center p-2"
      style={{ background: "transparent" }}
    >
      <div
        ref={cardRef}
        className="w-full rounded-xl bg-background border border-border shadow-xl animate-in fade-in zoom-in-95 duration-150"
      >
        {/* Header — draggable, shows icon and keyboard hints */}
        <div
          className="flex items-center gap-2 px-3 py-1.5 bg-muted/30 rounded-t-xl"
          data-tauri-drag-region
        >
          <img src={appIcon} alt="" className="h-4 w-4 shrink-0" />
          <span className="text-xs font-medium text-muted-foreground/70">
            Quick Capture
          </span>
          <span className="ml-auto text-[10px] text-muted-foreground/40">
            enter to add &middot; esc to dismiss
          </span>
        </div>

        {/* Editor + Add button */}
        <div className="px-3 py-3 flex items-center gap-2">
          <div className="flex-1 min-w-0">
            <TextEditor
              key={editorKey}
              value=""
              onCommit={() => {}}
              onCancel={handleCancel}
              onSubmit={handleSubmit}
              placeholder="What needs to be done?"
              onChange={setDraft}
              popup
            />
          </div>
          <Button
            size="icon"
            className="h-7 w-7 shrink-0"
            onClick={() => {
              if (draft.trim()) handleSubmit(draft);
            }}
            disabled={!draft.trim()}
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>

        {/* Divider + Board selector — always shown */}
        <div className="border-t border-border/50 px-3 py-1.5 flex items-center gap-2 bg-muted/20">
          <EntityIcon
            entityType="board"
            className="h-3 w-3 shrink-0 text-muted-foreground/50"
          />
          <BoardSelector
            boards={boards}
            selectedPath={selectedPath}
            onSelect={setSelectedPath}
            boardEntity={boardEntity}
            className="flex-1 text-xs"
          />
        </div>
      </div>
    </div>
  );
}
