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
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { FieldPlaceholderEditor } from "@/components/fields/field-placeholder";
import { BoardSelector } from "@/components/board-selector";
import appIcon from "@/assets/app-icon-32.png";
import type { OpenBoard, BoardDataResponse, Entity, EntityBag } from "@/types/kanban";
import { entityFromBag, parseBoardData } from "@/types/kanban";

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
  const [boardEntity, setBoardEntity] = useState<Entity | null>(null);
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

  /** Load board entity for the selected path so BoardSelector can display/edit the name. */
  const loadBoardEntity = useCallback(async (path: string | null) => {
    if (!path) { setBoardEntity(null); return; }
    try {
      // Temporarily switch to the selected board, fetch its data, then switch back
      const currentActive = boards.find((b) => b.is_active);
      if (currentActive?.path !== path) {
        await invoke("set_active_board", { path });
      }
      const data = await invoke<BoardDataResponse>("get_board_data");
      if (!mountedRef.current) return;
      const parsed = parseBoardData(data);
      setBoardEntity(parsed.board);
      // Restore original active board
      if (currentActive && currentActive.path !== path) {
        await invoke("set_active_board", { path: currentActive.path }).catch(() => {});
      }
    } catch {
      setBoardEntity(null);
    }
  }, [boards]);

  // Load board entity when selection changes
  useEffect(() => {
    loadBoardEntity(selectedPath);
  }, [selectedPath, loadBoardEntity]);

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
      // Board name or field changes → update local entity
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        const { entity_type, id, fields } = event.payload;
        if (entity_type === "board" && boardEntity && id === boardEntity.id) {
          if (fields) {
            setBoardEntity({ entity_type, id, fields });
          } else {
            // External change — re-fetch
            invoke<EntityBag>("get_entity", { entityType: entity_type, id })
              .then((bag) => setBoardEntity(entityFromBag(bag)))
              .catch(() => {});
          }
        }
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
  }, [boardEntity, loadBoards]);

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
    console.log("[quick-capture] hideWindow called");
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
      <div className="w-full rounded-xl border border-border bg-background overflow-hidden animate-in fade-in zoom-in-95 duration-150">
        {/* Header — draggable, shows icon and keyboard hints */}
        <div className="flex items-center gap-2 px-3 py-1.5 bg-muted/30" data-tauri-drag-region>
          <img src={appIcon} alt="" className="h-4 w-4 shrink-0" />
          <span className="text-xs font-medium text-muted-foreground/70">Quick Capture</span>
          <span className="ml-auto text-[10px] text-muted-foreground/40">
            enter to add &middot; esc to dismiss
          </span>
        </div>

        {/* Editor + Add button */}
        <div className="px-3 py-3 flex items-center gap-2">
          <div className="flex-1 min-w-0">
            <FieldPlaceholderEditor
              key={editorKey}
              value=""
              onCommit={() => {}}
              onCancel={handleCancel}
              onSubmit={handleSubmit}
              placeholder="What needs to be done?"
              onChange={setDraft}
            />
          </div>
          <Button
            size="icon"
            className="h-7 w-7 shrink-0"
            onClick={() => { if (draft.trim()) handleSubmit(draft); }}
            disabled={!draft.trim()}
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>

        {/* Divider + Board selector — always shown */}
        <div className="border-t border-border/50 px-3 py-1.5 flex items-center gap-2 bg-muted/20">
          <span className="text-[10px] text-muted-foreground/50 shrink-0">Board</span>
          <BoardSelector
            boards={boards}
            selectedPath={selectedPath}
            onSelect={setSelectedPath}
            boardEntity={boardEntity ?? undefined}
            className="flex-1 text-xs"
          />
        </div>
      </div>
    </div>
  );
}
