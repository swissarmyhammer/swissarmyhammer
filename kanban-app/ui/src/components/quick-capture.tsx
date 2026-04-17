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

/** True when the CodeMirror editor at `cmEl` is currently in vim insert mode. */
function isVimInsertMode(cmEl: Element): boolean {
  const view = EditorView.findFromDOM(cmEl as HTMLElement);
  if (!view) return false;
  const cm = getCM(view);
  return !!cm?.state?.vim?.insertMode;
}

/** Read the current editor text (raw, untrimmed) from a CodeMirror element. */
function readEditorText(cmEl: Element): string {
  return EditorView.findFromDOM(cmEl as HTMLElement)?.state.doc.toString() ?? "";
}

/**
 * Handle vim-normal-mode key events inside the editor.
 *
 * Returns true if the event was handled (caller should stop propagation).
 * Escape dismisses; Enter submits the trimmed text. Text is read directly
 * from CodeMirror and validated via `trim()` before being forwarded to
 * `onSubmit`, matching the prior inline implementation.
 */
function handleVimNormalKey(
  e: KeyboardEvent,
  cmEl: Element,
  hideWindow: () => void,
  onSubmit: (text: string) => void,
): boolean {
  if (e.key === "Escape") {
    e.preventDefault();
    e.stopPropagation();
    hideWindow();
    return true;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    e.stopPropagation();
    const text = readEditorText(cmEl);
    if (text.trim()) onSubmit(text);
    return true;
  }
  return false;
}

/**
 * Window-level keydown handler for Escape (dismiss) and Enter (submit).
 *
 * Vim mode distinguishes insert vs normal:
 * - insert Escape → let vim handle (exits to normal)
 * - normal Escape → dismiss window
 * - normal Enter  → submit task (text read from CodeMirror, trim-validated)
 *
 * Non-vim: Escape dismisses; Enter is handled by the editor itself.
 */
function useQuickCaptureKeyboard(
  keymapMode: string | undefined,
  hideWindow: () => void,
  handleSubmit: (text: string) => void,
) {
  const hideWindowRef = useRef(hideWindow);
  hideWindowRef.current = hideWindow;
  const handleSubmitRef = useRef(handleSubmit);
  handleSubmitRef.current = handleSubmit;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const cmEl = (e.target as HTMLElement)?.closest?.(".cm-editor");
      if (cmEl && keymapMode === "vim") {
        if (isVimInsertMode(cmEl)) return;
        handleVimNormalKey(e, cmEl, hideWindowRef.current, (text) =>
          handleSubmitRef.current(text),
        );
        return;
      }
      if (e.key === "Escape") hideWindowRef.current();
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [keymapMode]);
}

/** Payload for entity-field-changed Tauri event. */
interface EntityFieldChangedEvent {
  entity_type: string;
  id: string;
  fields?: Record<string, unknown>;
}

/**
 * Load and maintain the open-boards list; auto-refresh on window focus and
 * Tauri entity/board events. Returns the board list + selection state and
 * the "ready" flag used by the loading gate.
 *
 * `onShow` fires when the window regains focus — used by the caller to reset
 * draft text and force-remount the editor.
 */
/** Subscribe to focus + Tauri entity events that should trigger a reload. */
function useBoardReloadTriggers(loadBoards: () => void, onShow: () => void) {
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        loadBoards();
        onShow();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadBoards, onShow]);

  useEffect(() => {
    // "board" here is an entity-type filter, not a field name — we only
    // reload the board list when a board entity changes.
    const unlisteners = [
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        if (event.payload.entity_type === "board") loadBoards();
      }),
      listen("board-changed", () => loadBoards()),
    ];
    return () => {
      for (const p of unlisteners) p.then((fn) => fn());
    };
  }, [loadBoards]);
}

function useQuickCaptureBoards(onShow: () => void) {
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
    loadBoards();
    return () => {
      mountedRef.current = false;
    };
  }, [loadBoards]);

  useBoardReloadTriggers(loadBoards, onShow);

  return { boards, selectedPath, setSelectedPath, ready };
}

/**
 * Submit the draft as a task on the selected board's first column.
 *
 * Validates text at the boundary via `trim()` before forwarding to
 * `entity.add:task`. If the selected board is not currently active, the
 * prior active board is restored after the write so the user stays on the
 * board they were working on.
 */
function useQuickCaptureSubmit(
  selectedPath: string | null,
  boards: OpenBoard[],
  hideWindow: () => void,
) {
  const dispatchEntityAddTask = useDispatchCommand("entity.add:task");
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
        await dispatchEntityAddTask({
          args: { column: firstColumnId, title: text.trim() },
        });
        localStorage.setItem(STORAGE_KEY, selectedPath);
        if (active && active.path !== selectedPath) {
          await dispatchSwitchBoard({ args: { path: active.path } }).catch(
            () => {},
          );
        }
      } catch (err) {
        console.error("Quick capture failed:", err);
      }
      hideWindow();
    },
    [selectedPath, boards, hideWindow, dispatchEntityAddTask, dispatchSwitchBoard],
  );
}

/** Derive a minimal `board` entity from the selected OpenBoard for BoardSelector. */
function deriveBoardEntity(selected: OpenBoard | undefined): Entity | undefined {
  if (!selected) return undefined;
  return {
    entity_type: "board",
    id: "board",
    moniker: "board:board",
    fields: { name: selected.name },
  };
}

/** Auto-resize the Tauri window to match card content height (max 400px). */
function useWindowAutoResize(cardRef: React.RefObject<HTMLDivElement | null>) {
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
  }, [cardRef]);
}

interface QuickCaptureCardProps {
  cardRef: React.RefObject<HTMLDivElement | null>;
  editorKey: number;
  draft: string;
  setDraft: (v: string) => void;
  onSubmit: (text: string) => void;
  onCancel: () => void;
  boards: OpenBoard[];
  selectedPath: string | null;
  onSelectBoard: (path: string | null) => void;
  boardEntity: Entity | undefined;
}

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

interface QuickCaptureEditorProps {
  editorKey: number;
  draft: string;
  setDraft: (v: string) => void;
  onSubmit: (text: string) => void;
  onCancel: () => void;
}

function QuickCaptureEditor(p: QuickCaptureEditorProps) {
  return (
    <div className="px-3 py-3 flex items-center gap-2">
      <div className="flex-1 min-w-0">
        <TextEditor
          key={p.editorKey}
          value=""
          onCommit={() => {}}
          onCancel={p.onCancel}
          onSubmit={p.onSubmit}
          placeholder="What needs to be done?"
          onChange={p.setDraft}
        />
      </div>
      <Button
        size="icon"
        className="h-7 w-7 shrink-0"
        onClick={() => {
          if (p.draft.trim()) p.onSubmit(p.draft);
        }}
        disabled={!p.draft.trim()}
      >
        <Plus className="h-4 w-4" />
      </Button>
    </div>
  );
}

interface QuickCaptureBoardBarProps {
  boards: OpenBoard[];
  selectedPath: string | null;
  onSelectBoard: (path: string | null) => void;
  boardEntity: Entity | undefined;
}

function QuickCaptureBoardBar(p: QuickCaptureBoardBarProps) {
  return (
    <div className="border-t border-border/50 px-3 py-1.5 flex items-center gap-2 bg-muted/20">
      <EntityIcon
        entityType="board"
        className="h-3 w-3 shrink-0 text-muted-foreground/50"
      />
      <BoardSelector
        boards={p.boards}
        selectedPath={p.selectedPath}
        onSelect={p.onSelectBoard}
        boardEntity={p.boardEntity}
        className="flex-1 text-xs"
      />
    </div>
  );
}

function QuickCaptureCard(props: QuickCaptureCardProps) {
  return (
    <div
      ref={props.cardRef}
      className="w-full rounded-xl bg-background border border-border shadow-xl animate-in fade-in zoom-in-95 duration-150"
    >
      <QuickCaptureHeader />
      <QuickCaptureEditor
        editorKey={props.editorKey}
        draft={props.draft}
        setDraft={props.setDraft}
        onSubmit={props.onSubmit}
        onCancel={props.onCancel}
      />
      <QuickCaptureBoardBar
        boards={props.boards}
        selectedPath={props.selectedPath}
        onSelectBoard={props.onSelectBoard}
        boardEntity={props.boardEntity}
      />
    </div>
  );
}

export function QuickCapture() {
  const dispatchDismiss = useDispatchCommand("app.dismiss");
  const [draft, setDraft] = useState("");
  // Key to force-remount the editor on each window show
  const [editorKey, setEditorKey] = useState(0);
  const cardRef = useRef<HTMLDivElement>(null);

  const onShow = useCallback(() => {
    setDraft("");
    setEditorKey((k) => k + 1);
  }, []);

  const { boards, selectedPath, setSelectedPath, ready } =
    useQuickCaptureBoards(onShow);

  const hideWindow = useCallback(() => {
    dispatchDismiss().catch(console.error);
  }, [dispatchDismiss]);

  const handleSubmit = useQuickCaptureSubmit(selectedPath, boards, hideWindow);

  const { keymap_mode: keymapMode } = useUIState();
  useQuickCaptureKeyboard(keymapMode, hideWindow, handleSubmit);
  useWindowAutoResize(cardRef);

  if (!ready) return null;

  const boardEntity = deriveBoardEntity(
    boards.find((b) => b.path === selectedPath),
  );

  return (
    <div
      className="h-screen w-screen flex items-start justify-center p-2"
      style={{ background: "transparent" }}
    >
      <QuickCaptureCard
        cardRef={cardRef}
        editorKey={editorKey}
        draft={draft}
        setDraft={setDraft}
        onSubmit={handleSubmit}
        onCancel={hideWindow}
        boards={boards}
        selectedPath={selectedPath}
        onSelectBoard={setSelectedPath}
        boardEntity={boardEntity}
      />
    </div>
  );
}
