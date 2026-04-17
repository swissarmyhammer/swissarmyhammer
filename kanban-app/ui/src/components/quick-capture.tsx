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
import { useDispatchCommand } from "@/lib/command-scope";
import { EditorView } from "@codemirror/view";
import { getCM } from "@replit/codemirror-vim";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { TextEditor } from "@/components/fields/text-editor";
import { BoardSelector } from "@/components/board-selector";
import { useUIState } from "@/lib/ui-state-context";
import appIcon from "@/assets/app-icon-32.png";
import type { OpenBoard, BoardDataResponse, Entity } from "@/types/kanban";
import { entityFromBag, getNum } from "@/types/kanban";

const STORAGE_KEY = "quick-capture-last-board";

/** Fixed window width for the quick-capture Tauri window, in logical pixels. */
const WINDOW_WIDTH_PX = 560;
/** Upper bound on card height before the card scrolls instead of growing the window. */
const MAX_CARD_HEIGHT_PX = 400;
/** Vertical chrome around the card (matches the outer `p-2` padding = 8px × 2). */
const WINDOW_VERTICAL_PADDING_PX = 16;

/** Payload for entity-field-changed Tauri event. */
interface EntityFieldChangedEvent {
  entity_type: string;
  id: string;
  fields?: Record<string, unknown>;
}

/** Derives a minimal "board" entity for BoardSelector from the selected OpenBoard. */
function deriveBoardEntity(
  boards: OpenBoard[],
  selectedPath: string | null,
): Entity | undefined {
  const selected = boards.find((b) => b.path === selectedPath);
  if (!selected) return undefined;
  return {
    entity_type: "board",
    id: "board",
    moniker: "board:board",
    fields: { name: selected.name },
  };
}

/** Loads the board list and chooses the initial selected path. */
function useBoardList() {
  const [boards, setBoards] = useState<OpenBoard[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
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
    return () => {
      mountedRef.current = false;
    };
  }, []);

  return { boards, selectedPath, setSelectedPath, ready, loadBoards };
}

/** Subscribes to Tauri entity and board events to keep the board list fresh. */
function useBoardEventListeners(loadBoards: () => Promise<void>) {
  useEffect(() => {
    const unlisteners = [
      // Only reload when a board entity changes, ignoring task/column changes.
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        if (event.payload.entity_type === "board") loadBoards();
      }),
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
}

/** Reloads the board list and resets the editor when the window regains focus. */
function useWindowFocusReset(
  loadBoards: () => Promise<void>,
  resetDraft: () => void,
) {
  useEffect(() => {
    loadBoards();
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        loadBoards();
        resetDraft();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadBoards, resetDraft]);
}

/** Handles Escape/Enter inside a CM6 editor in vim mode. Insert mode passes through. */
function handleVimCmKey(
  e: KeyboardEvent,
  cmEl: Element,
  submit: (text: string) => void,
  dismiss: () => void,
) {
  const view = EditorView.findFromDOM(cmEl as HTMLElement);
  if (view && getCM(view)?.state?.vim?.insertMode) return;
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    dismiss();
    return;
  }
  if (e.key !== "Enter") return;
  e.preventDefault();
  e.stopPropagation();
  const text = view?.state.doc.toString() ?? "";
  if (text.trim()) submit(text);
}

/**
 * Window-level keyboard handler for Escape (dismiss) and Enter (submit).
 *
 * In vim mode: insert Escape → vim handles; normal Escape → dismiss;
 * normal Enter → submit task.
 */
function useQuickCaptureKeybinds(
  onSubmit: (text: string) => void,
  onDismiss: () => void,
) {
  const { keymap_mode: keymapMode } = useUIState();
  const submitRef = useRef(onSubmit);
  submitRef.current = onSubmit;
  const dismissRef = useRef(onDismiss);
  dismissRef.current = onDismiss;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const cmEl = (e.target as HTMLElement)?.closest?.(".cm-editor");
      if (cmEl && keymapMode === "vim") {
        handleVimCmKey(e, cmEl, submitRef.current, dismissRef.current);
        return;
      }
      if (e.key === "Escape") dismissRef.current();
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [keymapMode]);
}

/** Auto-resizes the Tauri window to match card content height (max 400px). */
function useAutoResizeWindow(cardRef: React.RefObject<HTMLDivElement | null>) {
  useEffect(() => {
    const card = cardRef.current;
    if (!card) return;
    const win = getCurrentWindow();
    const observer = new ResizeObserver(() => {
      const cardHeight = Math.min(card.scrollHeight, MAX_CARD_HEIGHT_PX);
      const windowHeight = cardHeight + WINDOW_VERTICAL_PADDING_PX;
      win
        .setSize(new LogicalSize(WINDOW_WIDTH_PX, windowHeight))
        .catch(() => {});
    });
    observer.observe(card);
    return () => observer.disconnect();
  }, [cardRef]);
}

/** Builds the submit handler that adds a task to the first column of the selected board. */
function useQuickCaptureSubmit(
  boards: OpenBoard[],
  selectedPath: string | null,
  onDone: () => void,
) {
  const dispatchTaskAdd = useDispatchCommand("task.add");
  const dispatchSwitchBoard = useDispatchCommand("file.switchBoard");

  return useCallback(
    async (text: string) => {
      if (!selectedPath || !text.trim()) return;
      try {
        const active = boards.find((b) => b.is_active);
        const boardData = await invoke<BoardDataResponse>("get_board_data", {
          boardPath: selectedPath,
        });
        const columns = boardData.columns
          .map(entityFromBag)
          .sort((a, b) => getNum(a, "order") - getNum(b, "order"));
        const firstColumnId = columns[0]?.id;
        if (!firstColumnId) return;

        await dispatchTaskAdd({
          args: { column: firstColumnId, title: text.trim() },
        });
        localStorage.setItem(STORAGE_KEY, selectedPath);

        // Restore the previous active board if we switched for the add.
        if (active && active.path !== selectedPath) {
          await dispatchSwitchBoard({ args: { path: active.path } }).catch(
            () => {},
          );
        }
      } catch (err) {
        console.error("Quick capture failed:", err);
      }
      onDone();
    },
    [selectedPath, boards, onDone, dispatchTaskAdd, dispatchSwitchBoard],
  );
}

/** Header row — draggable, shows icon and keyboard hints. */
function QuickCaptureHeader() {
  return (
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
  );
}

/** Editor + Add button row. */
function QuickCaptureEditor({
  editorKey,
  draft,
  setDraft,
  onSubmit,
}: {
  editorKey: number;
  draft: string;
  setDraft: (text: string) => void;
  onSubmit: (text: string) => void;
}) {
  return (
    <div className="px-3 py-3 flex items-center gap-2">
      <div className="flex-1 min-w-0">
        <TextEditor
          key={editorKey}
          value=""
          placeholder="What needs to be done?"
          onChange={setDraft}
        />
      </div>
      <Button
        size="icon"
        className="h-7 w-7 shrink-0"
        onClick={() => {
          if (draft.trim()) onSubmit(draft);
        }}
        disabled={!draft.trim()}
      >
        <Plus className="h-4 w-4" />
      </Button>
    </div>
  );
}

/** Bottom divider row with the board selector. */
function QuickCaptureBoardRow({
  boards,
  selectedPath,
  setSelectedPath,
  boardEntity,
}: {
  boards: OpenBoard[];
  selectedPath: string | null;
  setSelectedPath: (path: string | null) => void;
  boardEntity: Entity | undefined;
}) {
  return (
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
  );
}

/** Quick-capture window: pick a board, type a title, Cmd+Enter to create. */
export function QuickCapture() {
  const dispatchDismiss = useDispatchCommand("app.dismiss");
  const { boards, selectedPath, setSelectedPath, ready, loadBoards } =
    useBoardList();
  const [draft, setDraft] = useState("");
  // Key to force-remount the editor on each window show.
  const [editorKey, setEditorKey] = useState(0);
  const cardRef = useRef<HTMLDivElement>(null);
  const boardEntity = deriveBoardEntity(boards, selectedPath);

  const hideWindow = useCallback(() => {
    dispatchDismiss().catch(console.error);
  }, [dispatchDismiss]);

  const handleSubmit = useQuickCaptureSubmit(boards, selectedPath, hideWindow);

  const resetDraft = useCallback(() => {
    setDraft("");
    setEditorKey((k) => k + 1);
  }, []);

  useWindowFocusReset(loadBoards, resetDraft);
  useBoardEventListeners(loadBoards);
  useQuickCaptureKeybinds(handleSubmit, hideWindow);
  useAutoResizeWindow(cardRef);

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
        <QuickCaptureHeader />
        <QuickCaptureEditor
          editorKey={editorKey}
          draft={draft}
          setDraft={setDraft}
          onSubmit={handleSubmit}
        />
        <QuickCaptureBoardRow
          boards={boards}
          selectedPath={selectedPath}
          setSelectedPath={setSelectedPath}
          boardEntity={boardEntity}
        />
      </div>
    </div>
  );
}
