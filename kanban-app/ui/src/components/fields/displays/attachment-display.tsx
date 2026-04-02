/**
 * Attachment display components — renders attachment metadata with file-type
 * icons and human-readable file sizes.
 *
 * Two variants:
 * - AttachmentDisplay: renders a single attachment metadata object
 * - AttachmentListDisplay: renders an array of attachment metadata objects
 *
 * Icons are selected from lucide-react based on MIME type and file extension.
 * No file content is loaded — this is purely metadata-driven.
 */

import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ComponentType,
} from "react";
import {
  File,
  FileImage,
  FileCode,
  FileText,
  FileVideo,
  FileAudio,
  FileSpreadsheet,
  FileArchive,
  Paperclip,
} from "lucide-react";
import {
  backendDispatch,
  CommandScopeProvider,
  CommandScopeContext,
  resolveCommand,
  dispatchCommand,
  useActiveBoardPath,
  type CommandDef,
} from "@/lib/command-scope";
import { useFileDrop, type DropCallback } from "@/lib/file-drop-context";
import { useContextMenu } from "@/lib/context-menu";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Shape of an attachment metadata object from the entity layer. */
export interface AttachmentMeta {
  id: string;
  name: string;
  size: number;
  mime_type: string;
  path: string;
}

/** Props for the AttachmentDisplay component. */
export interface AttachmentDisplayProps {
  value: unknown;
  mode: "compact" | "full";
  onCommit?: (value: unknown) => void;
}

/** Props for the AttachmentListDisplay component. */
export interface AttachmentListDisplayProps {
  value: unknown;
  mode: "compact" | "full";
  onCommit?: (value: unknown) => void;
}

/** Props for the AttachmentItem component. */
export interface AttachmentItemProps {
  attachment: AttachmentMeta;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Format bytes into a human-readable string (e.g. "12.1 KB"). */
export function formatFileSize(bytes: number): string {
  if (bytes < 0) return "0 B";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / Math.pow(1024, exponent);
  // Show decimals only for KB and above
  const formatted = exponent === 0 ? value.toString() : value.toFixed(1);
  return `${formatted} ${units[exponent]}`;
}

/** Extract file extension (lowercase, without dot) from a filename. */
function getExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** Code-related MIME types. */
const CODE_MIME_TYPES = new Set([
  "application/javascript",
  "application/typescript",
  "application/x-python",
  "application/x-ruby",
  "application/json",
  "application/xml",
  "application/x-sh",
  "application/x-shellscript",
]);

/** Code-related file extensions. */
const CODE_EXTENSIONS = new Set([
  "js",
  "jsx",
  "ts",
  "tsx",
  "py",
  "rs",
  "go",
  "java",
  "c",
  "cpp",
  "h",
  "hpp",
  "cs",
  "rb",
  "php",
  "swift",
  "kt",
  "scala",
  "sh",
  "bash",
  "zsh",
  "lua",
  "r",
  "json",
  "xml",
  "yaml",
  "yml",
  "toml",
]);

/** Spreadsheet file extensions. */
const SPREADSHEET_EXTENSIONS = new Set(["csv", "xls", "xlsx", "ods"]);

/** Archive file extensions. */
const ARCHIVE_EXTENSIONS = new Set([
  "zip",
  "tar",
  "gz",
  "bz2",
  "7z",
  "rar",
  "xz",
]);

/**
 * Select the appropriate lucide icon component based on MIME type and file extension.
 *
 * @param mimeType - The MIME type string (e.g. "image/png")
 * @param name - The filename, used for extension-based fallback
 * @returns A lucide-react icon component
 */
export function getFileIcon(
  mimeType: string,
  name: string,
): ComponentType<{ className?: string; size?: number }> {
  const ext = getExtension(name);

  // MIME type prefix checks
  if (mimeType.startsWith("image/")) return FileImage;
  if (mimeType.startsWith("video/")) return FileVideo;
  if (mimeType.startsWith("audio/")) return FileAudio;
  if (mimeType.startsWith("text/")) {
    // text/x-python, text/javascript, etc. are code
    if (CODE_EXTENSIONS.has(ext)) return FileCode;
    return FileText;
  }
  if (mimeType === "application/pdf") return FileText;
  if (CODE_MIME_TYPES.has(mimeType)) return FileCode;

  // Extension-based fallback
  if (CODE_EXTENSIONS.has(ext)) return FileCode;
  if (SPREADSHEET_EXTENSIONS.has(ext)) return FileSpreadsheet;
  if (ARCHIVE_EXTENSIONS.has(ext)) return FileArchive;
  if (ext === "md" || ext === "txt") return FileText;

  return File;
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/**
 * Renders a single attachment item with icon, name, and file size.
 *
 * Wraps itself in a CommandScopeProvider with attachment.open and
 * attachment.reveal commands. Both double-click and right-click resolve
 * through the command system — same pattern as entity cards.
 */
export function AttachmentItem({ attachment }: AttachmentItemProps) {
  const Icon = getFileIcon(attachment.mime_type, attachment.name);
  const boardPath = useActiveBoardPath();

  const scopeChain = useMemo(
    () => [`attachment:${attachment.path}`],
    [attachment.path],
  );

  // Register commands in the frontend scope so resolveCommand works
  // for double-click, same as useEntityCommands does for entity cards.
  // The execute callbacks dispatch to the backend with the scope chain
  // so the Rust impls can resolve the path.
  const commands = useMemo<CommandDef[]>(
    () => [
      {
        id: "attachment.open",
        name: "Open",
        contextMenu: true,
        execute: () => {
          backendDispatch({
            cmd: "attachment.open",
            scopeChain,
            ...(boardPath ? { boardPath } : {}),
          }).catch(console.error);
        },
      },
      {
        id: "attachment.reveal",
        name: "Show in Finder",
        contextMenu: true,
        execute: () => {
          backendDispatch({
            cmd: "attachment.reveal",
            scopeChain,
            ...(boardPath ? { boardPath } : {}),
          }).catch(console.error);
        },
      },
    ],
    [scopeChain, boardPath],
  );

  return (
    <CommandScopeProvider
      commands={commands}
      moniker={`attachment:${attachment.path}`}
    >
      <AttachmentItemInner
        attachment={attachment}
        scopeChain={scopeChain}
        Icon={Icon}
      />
    </CommandScopeProvider>
  );
}

/** Props for the inner attachment item that lives inside a CommandScopeProvider. */
interface AttachmentItemInnerProps {
  attachment: AttachmentMeta;
  scopeChain: string[];
  Icon: ComponentType<{ className?: string; size?: number }>;
}

/** Inner component that has access to the command scope for resolving double-click. */
function AttachmentItemInner({
  attachment,
  scopeChain,
  Icon,
}: AttachmentItemInnerProps) {
  const scope = useContext(CommandScopeContext);
  const onContextMenu = useContextMenu(scopeChain);

  const handleDoubleClick = useCallback(() => {
    const cmd = resolveCommand(scope, "attachment.open");
    if (cmd) dispatchCommand(cmd);
  }, [scope]);

  return (
    <div
      className="flex items-center gap-2 min-w-0 cursor-pointer"
      onDoubleClick={handleDoubleClick}
      onContextMenu={onContextMenu}
    >
      <Icon className="shrink-0 text-muted-foreground" size={16} />
      <span className="truncate text-sm">{attachment.name}</span>
      <span className="shrink-0 text-xs text-muted-foreground">
        {formatFileSize(attachment.size)}
      </span>
    </div>
  );
}

/** Renders a single attachment metadata object. Also acts as a drag-drop target. */
export function AttachmentDisplay({
  value,
  mode,
  onCommit,
}: AttachmentDisplayProps) {
  const attachment = value as AttachmentMeta | null | undefined;
  const hasAttachment =
    attachment && typeof attachment === "object" && "name" in attachment;
  const { isDragging, registerDropTarget, unregisterDropTarget } =
    useFileDrop();

  const onCommitRef = useRef(onCommit);
  onCommitRef.current = onCommit;
  const valueRef = useRef(value);
  valueRef.current = value;

  useEffect(() => {
    const cb: DropCallback = (paths) => {
      if (paths.length > 0) {
        onCommitRef.current?.(paths[0]);
      }
    };
    registerDropTarget(cb);
    return () => unregisterDropTarget(cb);
  }, [registerDropTarget, unregisterDropTarget]);

  if (mode === "compact" && !hasAttachment) {
    return <span className="text-muted-foreground/50">-</span>;
  }

  return (
    <div
      className={`rounded-lg border-2 border-dashed transition-colors duration-150 p-2 ${
        isDragging
          ? "border-primary/60 bg-primary/5"
          : hasAttachment
            ? "border-transparent"
            : "border-muted-foreground/20"
      }`}
    >
      {hasAttachment ? (
        <AttachmentItem attachment={attachment as AttachmentMeta} />
      ) : (
        <div className="flex flex-col items-center text-muted-foreground opacity-40 py-1">
          <Paperclip className="h-5 w-5 mb-1" />
          <p className="text-xs">Drop file here</p>
        </div>
      )}
    </div>
  );
}

/** Renders a list of attachment metadata objects. Also acts as a drag-drop target. */
export function AttachmentListDisplay({
  value,
  mode,
  onCommit,
}: AttachmentListDisplayProps) {
  const attachments = Array.isArray(value) ? (value as AttachmentMeta[]) : [];
  const { isDragging, registerDropTarget, unregisterDropTarget } =
    useFileDrop();

  const onCommitRef = useRef(onCommit);
  onCommitRef.current = onCommit;
  const valueRef = useRef(value);
  valueRef.current = value;

  const handleDrop = useCallback((paths: string[]) => {
    const current = Array.isArray(valueRef.current) ? valueRef.current : [];
    onCommitRef.current?.([...current, ...paths]);
  }, []);

  useEffect(() => {
    registerDropTarget(handleDrop);
    return () => unregisterDropTarget(handleDrop);
  }, [handleDrop, registerDropTarget, unregisterDropTarget]);

  if (mode === "compact" && attachments.length === 0) {
    return <span className="text-muted-foreground/50">-</span>;
  }

  return (
    <div
      className={`rounded-lg border-2 border-dashed transition-colors duration-150 p-2 ${
        isDragging
          ? "border-primary/60 bg-primary/5"
          : attachments.length > 0
            ? "border-transparent"
            : "border-muted-foreground/20"
      }`}
    >
      {attachments.length > 0 ? (
        <div className="flex flex-col gap-1">
          {attachments.map((att) => (
            <AttachmentItem key={att.id} attachment={att} />
          ))}
        </div>
      ) : (
        <div className="flex flex-col items-center text-muted-foreground opacity-40 py-1">
          <Paperclip className="h-5 w-5 mb-1" />
          <p className="text-xs">Drop files here</p>
        </div>
      )}
    </div>
  );
}
